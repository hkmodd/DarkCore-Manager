use tauri::{AppHandle, Emitter, State, Manager}; 
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

// External Crates
use hex;
use zip;
use serde_json;

use crate::services::downloader::download_engine::{DirectDownloader, DownloadState, DownloadStatus};
use crate::services::downloader::manifest_parser::ManifestParser;
// use crate::services::steam_api::SteamApiClient; // DEPRECATED: Use crate::api::ApiClient
use crate::vdf_injector::{parse_lua_for_keys, VdfInjector};
use crate::state::AppState;
use crate::vault::VaultManager;
// Import for dynamic CDN resolution
use crate::downloader::ManifestDownloader;

#[tauri::command]
pub async fn start_direct_download(
    app: AppHandle,
    app_id: String,
    game_name: String,
    library_path: String,
    user_selected_ids: Vec<String>, // Changed from _depot_ids: Vec<u32>
    state: State<'_, AppState>,
) -> Result<String, String> {
    
    // Helper to log synchronously before thread spawn
    {
        if let Ok(mut logs) = state.system_log.lock() {
            logs.push(format!("[DirectDownload] START: {} (AppID: {}) Lib: {}", game_name, app_id, library_path));
            if !user_selected_ids.is_empty() {
                 logs.push(format!("[DirectDownload] User selected {} specific DLCs/Items.", user_selected_ids.len()));
            }
        }
    }

    // 1. Check/Set State
    {
        let mut ds = state.download_state.lock().await;
        if let DownloadStatus::Downloading { .. } = ds.status {
            return Err("Download already in progress".to_string());
        }
        ds.status = DownloadStatus::Initializing;
        ds.active_game_id = Some(app_id.clone());
    }

    let app_handle = app.clone();
    let download_state_arc = state.download_state.clone();
    let direct_downloader = state.direct_downloader.clone();
    
    // Retrieve Config properly
    let (steam_path_str, api_key, target_language) = {
        let config = state.config_manager.get();
        (config.steam_path, config.api_key, config.target_language)
    };

    tokio::spawn(async move {
        // Re-acquire state inside the thread for logging/access
        let state = app_handle.state::<AppState>();
        
        // Log Helper
        let log = |msg: &str| {
            if let Ok(mut logs) = state.system_log.lock() {
                logs.push(msg.to_string());
            }
            println!("[DirectDownload] {}", msg);
        };

        log(&format!("Initializing background download for {}", game_name));

        let steam_path = PathBuf::from(&steam_path_str);
        let depot_cache = steam_path.join("depotcache");
        if !depot_cache.exists() {
            let _ = std::fs::create_dir_all(&depot_cache);
        }
        
        // Determine Target Library & Paths
        let effective_library = if library_path.is_empty() {
            steam_path.clone()
        } else {
            PathBuf::from(&library_path)
        };
        
        // Create Sanitize Name for directory
        let install_dir_name: String = game_name.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_' || *c == '.')
            .collect::<String>().trim().to_string();

        let acf_path = effective_library.join("steamapps").join(format!("appmanifest_{}.acf", app_id));
        // We will generate ACF later when we have the confirmed depot list.

        // A. Vault Check
        let vault = VaultManager::new_local();
        let mut skip_morrenus = false;
        
        // Logic: if vault exists, we might skip download. 
        // But what if user wants to install NEW DLCs? 
        // For now, if Vault exists, we assume it has what we need OR we try to restore what we can.
        // Explicitly: The Vault stores MANIFESTS, not game files. 
        // So restores allow us to skip the "Morrenus Zip" download, but we still do "Direct Download" of content.
        
        if vault.has_manifests(&app_id) && vault.exists(&app_id) { 
             emit_progress(&app_handle, "init", "Vault: Found cached data. Verified.", 0.02).await;
             log("Vault: Local data found. Skipping Morrenus download.");
             skip_morrenus = true;
        }

        let mut lua_content = String::new();
        // Map: DepotID -> (GID, Bytes)
        let mut manifest_map: HashMap<u32, (u64, Vec<u8>)> = HashMap::new(); 

        if skip_morrenus {
            emit_progress(&app_handle, "downloading", "Restoring from Vault (0 Tokens)...", 0.05).await;
            
            // 1. Restore Lua
            match vault.get_lua(&app_id) {
                Ok(bytes) => {
                    lua_content = String::from_utf8_lossy(&bytes).to_string();
                },
                Err(e) => {
                    log(&format!("Vault Lua Error: {}. Fallback to Morrenus.", e));
                    skip_morrenus = false; 
                }
            }
            
            // 2. Load Manifests from Vault to Memory Map
            if skip_morrenus {
                let vault_game_dir = vault.get_storage_dir(&app_id);
                if let Ok(entries) = std::fs::read_dir(&vault_game_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(ext) = path.extension() {
                            if ext == "manifest" {
                                if let Ok(bytes) = std::fs::read(&path) {
                                     if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                         let parts: Vec<&str> = stem.split('_').collect();
                                         if parts.len() >= 2 {
                                             let depot_id = parts[0].parse::<u32>().unwrap_or(0);
                                             let gid = parts[1].parse::<u64>().unwrap_or(0);
                                             if depot_id > 0 && gid > 0 {
                                                 manifest_map.insert(depot_id, (gid, bytes));
                                             }
                                         }
                                     }
                                }
                            }
                        }
                    }
                }
                // FIX: Do NOT call restore_game here. Manifests are restored manually below.
                // FIX: Do NOT call restore_game here. It blindly restores ACF to default steam_path (C:),
                // causing duplicate ACFs if user selected a custom library (D:).
                // Manifests are restored manually below from `manifest_map` to `depotcache`.
                // ACF is generated manually below to `effective_library`.
            }
        }

        if !skip_morrenus {
            // A. Download Morrenus ZIP
            emit_progress(&app_handle, "downloading", "Fetching Repository Data...", 0.05).await;
            log("Downloading manifests from Morrenus...");
            
            let api_client = crate::api::ApiClient::new(api_key.clone());
            let zip_bytes = match api_client.download_manifest(&app_id).await {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    let err = format!("Failed to download from Morrenus: {}", e);
                    log(&format!("❌ {}", err));
                    emit_error(&app_handle, &err, &download_state_arc).await;
                    return;
                }
            };
    
            emit_progress(&app_handle, "downloading", "Processing Repository Data...", 0.1).await;
    
            // B. Extract ZIP
            let mut archive = match zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                Ok(a) => a,
                Err(e) => {
                    emit_error(&app_handle, &format!("Invalid ZIP archive: {}", e), &download_state_arc).await;
                    return;
                }
            };

            for i in 0..archive.len() {
                if let Ok(mut f) = archive.by_index(i) {
                    if f.name().ends_with(".lua") {
                        use std::io::Read;
                        let _ = f.read_to_string(&mut lua_content);
                        let _ = vault.save_lua(&app_id, lua_content.as_bytes());
                    } else if f.name().ends_with(".manifest") {
                        let fname = f.name();
                        let clean_name = Path::new(fname).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        
                        if !clean_name.is_empty() {
                             let out_path = depot_cache.join(&clean_name);
                             let mut bytes = Vec::new();
                             use std::io::Read;
                             if f.read_to_end(&mut bytes).is_ok() {
                                 let _ = std::fs::write(&out_path, &bytes);
                                 let stem = Path::new(&clean_name).file_stem().unwrap().to_string_lossy().to_string();
                                 let parts: Vec<&str> = stem.split('_').collect();
                                 if parts.len() >= 2 {
                                     let depot_id = parts[0].parse::<u32>().unwrap_or(0);
                                     let gid = parts[1].parse::<u64>().unwrap_or(0);
                                     if depot_id > 0 && gid > 0 {
                                          let _ = vault.store_manifest_bytes(&app_id, depot_id, gid, &bytes);
                                          manifest_map.insert(depot_id, (gid, bytes));
                                     }
                                 }
                             }
                        }
                    }
                }
            }
    
            if lua_content.is_empty() {
                lua_content = String::from_utf8_lossy(&zip_bytes).to_string();
            }
        }
        
        // C. Parse Keys
        emit_progress(&app_handle, "downloading", "Parsing Decryption Keys...", 0.15).await;
        let (_, keys) = parse_lua_for_keys(&lua_content);
        log(&format!("Parsed {} keys from Lua.", keys.len()));

        // =====================================================================
        // CRITICAL FIX: UPDATE APPLIST (Strict Filtering)
        // =====================================================================
        emit_progress(&app_handle, "downloading", "Patching AppList...", 0.18).await;
        log("Patching GreenLuma AppList...");

        let mut final_ids = Vec::new();
        final_ids.push(app_id.clone());
        // Derived Depot
        let derived_depot = {
            let mut chars: Vec<char> = app_id.chars().collect();
            if let Some(last) = chars.last_mut() { *last = '1'; }
            chars.into_iter().collect::<String>()
        };
        if derived_depot != app_id {
            final_ids.push(derived_depot);
        }

        // Fetch Hierarchy for DLCs/Depots (Simulate steam_install logic)
        let api_client = crate::api::ApiClient::new(api_key.clone());
        let hierarchy = api_client.fetch_full_hierarchy(&app_id, &target_language).await.ok();

        // ─── FILTERING LOGIC ─────────────────────────────────────────────────
        let mut allowed_depots = std::collections::HashSet::new();
        let use_filtering = !user_selected_ids.is_empty();

        if let Some(h) = &hierarchy {
            log("Using GameHierarchy for AppList patching...");
            final_ids.push(h.root_id.clone());
            
            // ALWAYS allow Base Game Depots
            for depot in &h.base_depots {
                if depot.depot_id != "228980" && depot.depot_id != "228989" {
                    final_ids.push(depot.depot_id.clone());
                    allowed_depots.insert(depot.depot_id.clone());
                }
            }
            
            for dlc in &h.dlcs {
                // If user selected specific DLCs, check if this DLC is in the list
                let is_selected = if use_filtering {
                    user_selected_ids.contains(&dlc.app_id)
                } else {
                    true // Default: Select All
                };

                if is_selected {
                     final_ids.push(dlc.app_id.clone());
                     for depot in &dlc.depots {
                        if depot.depot_id != "228980" && depot.depot_id != "228989" {
                            final_ids.push(depot.depot_id.clone());
                            allowed_depots.insert(depot.depot_id.clone());
                        }
                     }
                }
            }
            
            // Save Relationships (Critical for Library Grouping/Scanning)
            if let Ok(app_list_mgr) = state.app_list.lock() {
                let mut relationships = app_list_mgr.load_relationships();
                let mut types = app_list_mgr.load_types();
                
                let parent = h.root_id.clone();
                for id in &final_ids {
                    if *id != parent {
                        relationships.insert(id.clone(), parent.clone());
                    }
                }

                // Update Types (Depots vs DLCs)
                for dlc in &h.dlcs {
                    types.insert(dlc.app_id.clone(), "dlc".to_string());
                }
                for depot in &h.base_depots {
                    let type_str = if depot.is_dlc_depot { "depot_dlc" } else { "depot_base" };
                    types.insert(depot.depot_id.clone(), type_str.to_string());
                }
                for dlc in &h.dlcs {
                     for depot in &dlc.depots {
                         types.insert(depot.depot_id.clone(), "depot_dlc".to_string());
                     }
                }

                app_list_mgr.save_relationships(&relationships);
                app_list_mgr.save_types(&types);
                log("✅ Relationships and Types saved.");
            }
        } else {
            log("Fallback: Adding IDs from Lua keys (No Hierarchy available)...");
        }
        
        // ─────────────────────────────────────────────────────────────────────
        // FORCE ADD ALL KEYS: Even if hierarchy exists, we MUST ensure every depot 
        // we have a key for is in the AppList. Users might be installing a specific 
        // depot that isn't in the hierarchy or is mislabeled.
        // ─────────────────────────────────────────────────────────────────────
        // ─────────────────────────────────────────────────────────────────────
        // FORCE ADD ALL KEYS & MANIFESTS: 
        // 1. Add all parsed keys (Depot IDs)
        // 2. Add all found manifests (Depot IDs from ZIP)
        // ─────────────────────────────────────────────────────────────────────
        for (k, _) in &keys {
             if !final_ids.contains(k) { final_ids.push(k.clone()); }
        }
        for (depot_id, _) in &manifest_map {
             let depot_str = depot_id.to_string();
             if !final_ids.contains(&depot_str) {
                 final_ids.push(depot_str);
             }
        }
        // ─────────────────────────────────────────────────────────────────────

        final_ids.sort();
        final_ids.dedup();
        
        if let Ok(app_list_mgr) = state.app_list.lock() {
            if let Err(e) = app_list_mgr.add_games_to_list(final_ids.clone()) {
                log(&format!("❌ AppList Patch Error: {}", e));
            } else {
                log(&format!("✅ AppList patched with {} IDs.", final_ids.len()));
            }
        }
        // =====================================================================

        // Inject Keys into config.vdf
        let injector = VdfInjector::new(&steam_path);
        if let Err(e) = injector.inject_vdf(&keys) {
             log(&format!("Key Injection Warning: {}", e));
        } else {
            log("✅ Depot Keys injected.");
        }

        // D. Authenticate Steam
        emit_progress(&app_handle, "downloading", "Authenticating with Steam CDN...", 0.2).await;
        log("Authenticating anonymous Steam session...");
        
        let _connection = match DirectDownloader::authenticate_anonymous().await {
            Ok(c) => c,
            Err(e) => {
                 let err = format!("Steam Auth Failed: {}", e);
                 log(&format!("❌ {}", err));
                 emit_error(&app_handle, &err, &download_state_arc).await;
                 return;
            }
        };

        // E. Prepare Download Jobs
        emit_progress(&app_handle, "downloading", "Resolving Steam CDN...", 0.22).await;
        // let base_url = "http://lancache.steamcontent.com"; // OLD: Hardcoded LanCache
        
        // NEW: Dynamic Resolution via Valve API
        let manifest_downloader = ManifestDownloader::new();
        let cdn_host = match manifest_downloader.get_cdn_host().await {
            Ok(host) => {
                log(&format!("Resolved Steam CDN: {}", host));
                host
            },
            Err(e) => {
                log(&format!("CDN Resolution Failed: {}. Fallback to LanCache.", e));
                "lancache.steamcontent.com".to_string()
            }
        };
        // Use HTTPS for secure download from Valve CDN
        let base_url = format!("https://{}", cdn_host);

        let cdn_token = ""; 
        
        let mut total_download_size = 0u64;
        let mut target_depots = Vec::new();

        for (depot_id_str, key_hex) in &keys {
            if let Ok(depot_id) = depot_id_str.parse::<u32>() {
                 // FILTER: Start logic
                 // If using filtering, only download explicitly allowed depots.
                 // But wait, if hierarchy failed, allowed_depots is empty. Use fallback.
                 let should_download = if use_filtering && !allowed_depots.is_empty() {
                     allowed_depots.contains(depot_id_str)
                 } else {
                     true 
                 };

                 if should_download {
                     if let Some((gid, bytes)) = manifest_map.get(&depot_id) {
                         if let Ok(key_bytes_vec) = hex::decode(key_hex) {
                             if key_bytes_vec.len() == 32 {
                                 let mut key_arr = [0u8; 32];
                                 key_arr.copy_from_slice(&key_bytes_vec);
                                 
                                 if let Ok(parsed_manifest) = ManifestParser::parse(bytes) {
                                     let jobs = DirectDownloader::generate_jobs(&parsed_manifest);
                                     let depot_sum: u64 = jobs.iter().map(|j| j.uncompressed_length as u64).sum();
                                     total_download_size += depot_sum;
                                     target_depots.push((depot_id, *gid, key_arr, jobs));
                                 }
                             }
                         }
                     }
                 }
            }
        }
        
        if target_depots.is_empty() {
            let err = "No matching depots (Keys vs Manifests) found for selection.";
            log(&format!("❌ {}", err));
            emit_error(&app_handle, err, &download_state_arc).await;
            return;
        }

        // F. Start Download
        emit_progress(&app_handle, "downloading", &format!("Downloading {} Depots...", target_depots.len()), 0.25).await;
        log(&format!("Starting download of {} files ({} bytes)...", target_depots.iter().map(|t| t.3.len()).sum::<usize>(), total_download_size));
        
        // Reset Status
        {
             let mut ds = download_state_arc.lock().await;
             ds.status = DownloadStatus::Downloading { 
                 files: target_depots.iter().map(|t| t.3.len()).sum(), 
                 bytes_downloaded: 0, 
                 total_bytes: total_download_size, 
                 speed: 0,
                 start_time: std::time::Instant::now(),
             };
        }

        let target_dir = effective_library.join("steamapps/common").join(&install_dir_name); 
        log(&format!("Target Directory: {:?}", target_dir));

        // Generate ACF with Correct Mounted Depots & BuildID
        // 1. Fetch BuildID from SteamCMD (if not already fetched for hierarchy)
        let build_id = match api_client.get_app_info(&app_id).await {
            Ok(info) => info.buildid.unwrap_or(0),
            Err(e) => {
                log(&format!("⚠️ Could not fetch BuildID: {}. Defaulting to 1 (Risk of 'Update Paused').", e));
                1 // Better than 0
            }
        };
        log(&format!("Using BuildID: {}", build_id));

        // 2. Prepare Depot Info Map
        let mut acf_depots = HashMap::new();
        for (depot_id, gid, _, jobs) in &target_depots {
            let size: u64 = jobs.iter().map(|j| j.uncompressed_length as u64).sum();
            acf_depots.insert(*depot_id, crate::utils::AcfDepotInfo {
                 gid: *gid,
                 size,
            });
        }
        
        log(&format!("Generating Final ACF: BuildID={}, Size={}, Depots={}", build_id, total_download_size, acf_depots.len()));
        if let Err(e) = crate::utils::generate_ghost_acf(
            &acf_path, 
            &app_id, 
            &install_dir_name, 
            &game_name, 
            &acf_depots, 
            build_id, 
            total_download_size
        ) {
             let err_msg = format!("ACF Creation Failed: {}", e);
             log(&format!("❌ {}", err_msg));
        }

        // ===================================
        // CRITICAL FIX: Ensure Manifests in depotcache
        // ===================================
        // Even if we skip Morrenus download (Vault hit), Steam needs these files in depotcache to verify ownership.
        for (depot_id, gid, _, _) in &target_depots {
             let manifest_filename = format!("{}_{}.manifest", depot_id, gid);
             let dest_path = depot_cache.join(&manifest_filename);
             
             if !dest_path.exists() {
                 if let Some((_, bytes)) = manifest_map.get(depot_id) {
                     log(&format!("Restoring manifest {} to depotcache...", manifest_filename));
                     let _ = std::fs::write(&dest_path, bytes);
                 }
             }
        }

        for (depot_id, _, key, jobs) in target_depots {
             log(&format!("Downloading Depot {}...", depot_id));
             let _ = direct_downloader.clone().start_download_pool(
                 jobs, 
                 key, 
                 base_url.to_string(), 
                 target_dir.clone(), 
                 download_state_arc.clone(), 
                 depot_id, 
                 cdn_token.to_string()
             ).await;
        }

        emit_progress(&app_handle, "complete", "Download Complete!", 1.0).await;
        log("✅ Download Complete!");
        
        {
            let mut ds = download_state_arc.lock().await;
            ds.status = DownloadStatus::Completed;
        }
    });

    Ok("Download Initiated via Morrenus ZIP".into())
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>) -> Result<String, String> {
    let mut ds = state.download_state.lock().await;
    match &ds.status {
        DownloadStatus::Downloading { files, bytes_downloaded, total_bytes, .. } => {
            let progress_val = (*bytes_downloaded as f64 / *total_bytes as f64).min(1.0).max(0.0) * 100.0;
            ds.status = DownloadStatus::Paused {
                files: *files,
                bytes_downloaded: *bytes_downloaded,
                total_bytes: *total_bytes,
                progress_val,
            };
            Ok("Paused".into())
        },
        _ => Err("Cannot pause: not downloading".into())
    }
}

#[tauri::command]
pub async fn resume_download(state: State<'_, AppState>) -> Result<String, String> {
    let mut ds = state.download_state.lock().await;
    match &ds.status {
        DownloadStatus::Paused { files, bytes_downloaded, total_bytes, .. } => {
            ds.status = DownloadStatus::Downloading {
                files: *files,
                bytes_downloaded: *bytes_downloaded,
                total_bytes: *total_bytes,
                speed: 0,
                start_time: std::time::Instant::now(), // Reset timer for speed calc
            };
            Ok("Resumed".into())
        },
        _ => Err("Cannot resume: not paused".into())
    }
}

#[tauri::command]
pub async fn get_download_status(state: State<'_, AppState>) -> Result<DownloadStatusViewModel, String> {
    let ds = state.download_state.lock().await;
    
    let (status_str, progress_str, speed_str, progress_val) = match &ds.status {
        DownloadStatus::Idle => ("Idle".to_string(), "".to_string(), "0 B/s".to_string(), 0.0),
        DownloadStatus::Initializing => ("Initializing".to_string(), "0%".to_string(), "0 B/s".to_string(), 0.0),
        DownloadStatus::Downloading { files, bytes_downloaded, total_bytes, start_time, .. } => {
             let elapsed = start_time.elapsed().as_secs_f64();
             let speed_bps = if elapsed > 0.5 { *bytes_downloaded as f64 / elapsed } else { 0.0 };
             
             let speed_fmt = if speed_bps > 1_048_576.0 {
                 format!("{:.1} MB/s", speed_bps / 1_048_576.0)
             } else {
                 format!("{:.1} KB/s", speed_bps / 1024.0)
             };

             let downloaded_mb = *bytes_downloaded as f64 / 1_048_576.0;
             let total_mb = *total_bytes as f64 / 1_048_576.0;
             let progress_txt = format!("{:.1} MB / {:.1} MB", downloaded_mb, total_mb);
             
             let pct = (*bytes_downloaded as f64 / *total_bytes as f64).min(1.0).max(0.0) * 100.0;

             (format!("Downloading {} files", files), progress_txt, speed_fmt, pct)
        },
        DownloadStatus::Paused { files: _, bytes_downloaded, total_bytes, progress_val } => {
             let downloaded_mb = *bytes_downloaded as f64 / 1_048_576.0;
             let total_mb = *total_bytes as f64 / 1_048_576.0;
             let progress_txt = format!("{:.1} MB / {:.1} MB", downloaded_mb, total_mb);
             ("Paused".to_string(), progress_txt, "0 B/s".to_string(), *progress_val)
        },
        DownloadStatus::Completed => ("Completed".to_string(), "100%".to_string(), "0 B/s".to_string(), 100.0),
        DownloadStatus::Error(e) => (format!("Error: {}", e), "Failed".to_string(), "0 B/s".to_string(), 0.0),
    };

    Ok(DownloadStatusViewModel {
        status: status_str,
        game_id: ds.active_game_id.clone(),
        speed: speed_str,
        progress: progress_str,
        progress_val: progress_val,
    })
}

// Helpers
async fn emit_progress(app: &AppHandle, step: &str, msg: &str, progress: f64) {
    let _ = app.emit("install-progress", serde_json::json!({
        "step": step,
        "message": msg,
        "progress": progress
    }));
}

async fn emit_error(app: &AppHandle, msg: &str, state: &Arc<Mutex<DownloadState>>) {
    let _ = app.emit("install-progress", serde_json::json!({
        "step": "error",
        "message": msg,
        "progress": 0.0
    }));
    
    let mut ds = state.lock().await;
    ds.status = DownloadStatus::Error(msg.to_string());
}

#[derive(serde::Serialize)]
pub struct DownloadStatusViewModel {
    status: String,
    game_id: Option<String>,
    speed: String,
    progress: String,
    progress_val: f64,
}
