use crate::ui::state::DarkCoreApp;
use crate::ui::state::{PendingInstall, push_log};
use crate::api::ApiClient;


use std::path::PathBuf;
use crate::vault::VaultManager;

pub fn install_game(
    app: &mut DarkCoreApp,
    appid: String,
    name: String,
    target_library: Option<PathBuf>,
    install_dir_name: Option<String>,
) {
    // Check if we have Manifestor data available to pass
    let hierarchy = if let Ok(data) = app.manifestor_data.lock() {
        data.clone()
    } else {
        None
    };

    finalize_installation(
        app,
        appid,
        name,
        target_library,
        install_dir_name,
        app.manifestor_selections.clone(),
        None,
        hierarchy,
    );
}

pub fn finalize_installation(
    app: &mut DarkCoreApp,
    appid: String,
    name: String,
    target_library: Option<PathBuf>,
    install_dir_name: Option<String>,
    selected_dlcs: Vec<String>,
    cached_zip: Option<Vec<u8>>,
    hierarchy: Option<crate::api::GameHierarchy>,
) {
    // PHASE 2 INTERCEPTOR: Save state and ask user
    app.pending_install = Some(PendingInstall {
        appid,
        name,
        target_library,
        install_dir_name,
        selected_dlcs,
        cached_zip,
        hierarchy,
    });
    app.download_method_modal_open = true;
}

pub fn legacy_install_game(
    app: &mut DarkCoreApp,
    appid: String,
    name: String,
    target_library: Option<PathBuf>,
    install_dir_name: Option<String>,
) {
    let client_opt = app.api_client.clone();
    let _log_arc = app.system_log.clone();
    
    // CAPTURE CONTEXT FOR ASYNC
    let appid_c = appid.clone();
    
    // Prepare Scanner State
    let scan_res = app.dlc_scan_result.clone();
    
    // RESET ZIP CACHE
    if let Ok(mut zip) = app.dlc_scan_result_zip.lock() {
        *zip = None;
    }

    // [Scanner State Reset]
    let scan_zip_res = app.dlc_scan_result_zip.clone();
    if let Ok(mut s) = scan_res.lock() { *s = None; }
    if let Ok(mut z) = scan_zip_res.lock() { *z = None; }
    app.is_scanning_dlcs = true;
    
    // Store candidate info for the UI to pick up after scan
    app.dlc_picker_candidate = Some((appid.clone(), name.clone()));
    app.dlc_picker_pending_library = target_library.clone();
    app.dlc_picker_pending_install_dir = install_dir_name.clone();
    
    // Log
    let log_arc = app.system_log.clone();
    if let Ok(mut l) = log_arc.lock() {
        l.push(format!("Checking DLCs for {}...", name));
    }

    std::thread::spawn(move || {
        if let Some(client) = client_opt {
             if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                  
                  // 1. Fetch ALL known DLCs from Steam (to show what is missing)
                  let steam_dlcs_res = rt.block_on(client.get_dlc_list(&appid_c));
                  
                  // 2. Fetch Available Content from Morrenus
                  match rt.block_on(client.download_manifest(&appid_c)) {
                      Ok(lua_bytes) => {
                          // Cache bytes
                          if let Ok(mut z) = scan_zip_res.lock() { *z = Some(lua_bytes.to_vec()); }

                          let lua_content = String::from_utf8_lossy(&lua_bytes).to_string();
                          let (morrenus_ids, keys) = crate::vdf_injector::parse_lua_for_keys(&lua_content);
                          let depot_count = keys.len();
                          
                          // Smart Merge: Steam List vs Morrenus List
                          let mut final_items: Vec<(String, String, bool, bool)> = Vec::new();
                          let mut seen_ids = std::collections::HashSet::new();
                          
                          // A. Process Steam DLCs (The "Official" List)
                          if let Ok(steam_dlcs) = steam_dlcs_res {
                              if let Ok(mut l) = log_arc.lock() {
                                  l.push(format!("Steam: Found {} known DLCs.", steam_dlcs.len()));
                              }
                              
                              for dlc_id in steam_dlcs {
                                   let is_available = morrenus_ids.contains(&dlc_id);
                                   let name = format!("DLC {}", dlc_id); // Name update later?
                                   // Auto-select ONLY if available
                                   final_items.push((dlc_id.clone(), name, is_available, is_available));
                                   seen_ids.insert(dlc_id);
                              }
                          }
                          
                          // B. Process Morrenus-only items (Hidden DLCs/Depots?)
                          for id in morrenus_ids {
                              if id != appid_c && !seen_ids.contains(&id) {
                                  let name = extract_dlc_name_from_lua(&lua_content, &id)
                                      .unwrap_or_else(|| format!("Bonus Content {}", id));
                                  final_items.push((id, name, true, true));
                              }
                          }
                          
                          // Sort by ID
                          final_items.sort_by(|a, b| a.0.cmp(&b.0));
                          
                          if let Ok(mut res) = scan_res.lock() {
                              *res = Some((final_items, depot_count));
                          }
                      },
                      Err(e) => {
                           if let Ok(mut l) = log_arc.lock() {
                               l.push(format!("Morrenus Error: {}. Falling back to clean Steam list.", e));
                           }
                           // Fallback: Just Steam List (All marked unavailable/unsafe?) or just empty?
                           // Safest: Empty, can't install without Morrenus.
                           if let Ok(mut res) = scan_res.lock() { *res = Some((Vec::new(), 0)); }
                      }
                  }
             }
        } else {
            // No Client
            if let Ok(mut res) = scan_res.lock() { *res = Some((Vec::new(), 0)); }
        }
    });
}

pub fn spawn_direct_install(
    app: &DarkCoreApp,
    appid: String,
    name: String,
    target_library: Option<PathBuf>,
    install_dir_name: Option<String>,
    selected_dlcs: Vec<String>,
    cached_zip: Option<Vec<u8>>,
    hierarchy: Option<crate::api::GameHierarchy>,
) {
    let log_arc = app.system_log.clone();
    let api_key = app.config.api_key.clone();
    let download_state = app.download_state.clone();
    let steam_path = app.config.steam_path.clone();
    let gl_path = app.config.gl_path.clone(); // Needed for AppList

    
    // Reset State
    if let Ok(mut s) = download_state.lock() {
        s.reset();
        s.status = crate::direct_download::state::DownloadStatus::Initializing;
        s.active_game_id = Some(appid.clone());
        s.start_time = Some(std::time::Instant::now());
        s.target_dir = target_library.clone().unwrap_or(PathBuf::from(&steam_path)).join("steamapps/common").join(install_dir_name.clone().unwrap_or(name.clone()));
    }

    std::thread::spawn(move || {
        let log = move |msg: String| {
            if let Ok(mut logs) = log_arc.lock() {
                println!("[DIRECT] {}", msg);
                push_log(&mut logs, format!("[DIRECT] {}", msg));
            }
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
             // 1. Auth (Anon)
             log("Authenticating with Steam...".to_string());
             use crate::direct_download::downloader::DirectDownloader;
             
             // We need to fetch the ZIP first (Manifests + Keys)
             let client = crate::api::ApiClient::new(api_key.clone());
             
             let zip_bytes = if let Some(b) = cached_zip {
                 log("Using cached Morrenus ZIP.".to_string());
                 b
             } else {
                 log("Downloading Morrenus ZIP...".to_string());
                 match client.download_manifest(&appid).await {
                     Ok(b) => b.to_vec(),
                     Err(e) => {
                         log(format!("Failed to download ZIP: {}", e));
                         if let Ok(mut s) = download_state.lock() { s.status = crate::direct_download::state::DownloadStatus::Error(e.to_string()); }
                         return;
                     }
                 }
             };
             
             // 2. Parse Lua
             log("Parsing Script Data...".to_string());
             // We need to extract lua from zip
             let mut lua_content = String::new();
             let mut manifest_bytes_map: std::collections::HashMap<u64, Vec<u8>> = std::collections::HashMap::new();
             
             if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                 for i in 0..archive.len() {
                     if let Ok(mut f) = archive.by_index(i) {
                         if f.name().ends_with(".lua") {
                             use std::io::Read;
                             let _ = f.read_to_string(&mut lua_content);
                         } else if f.name().ends_with(".manifest") {
                              // Try to parse filename as ID
                             let fname = f.name();
                             let stem = std::path::Path::new(fname).file_stem().unwrap().to_string_lossy();
                             
                             // Try direct parse first
                             let maybe_mid = stem.parse::<u64>()
                                 .or_else(|_| {
                                     // Try {DepotID}_{ManifestID} format
                                     if let Some(idx) = stem.rfind('_') {
                                         let generated_mid = &stem[idx+1..];
                                         generated_mid.parse::<u64>()
                                     } else {
                                         Err(std::num::ParseIntError::clone(&stem.parse::<u64>().unwrap_err())) 
                                     }
                                 });

                             if let Ok(mid) = maybe_mid {
                                  let mut buf = Vec::new();
                                  use std::io::Read;
                                  let _ = f.read_to_end(&mut buf);
                                  manifest_bytes_map.insert(mid, buf);
                                  log(format!("Loaded Manifest {} from ZIP", mid));
                             }
                         }
                     }
                 }
             }
             
             if lua_content.is_empty() {
                  lua_content = String::from_utf8_lossy(&zip_bytes).to_string();
             }
             
             let script_data = match crate::direct_download::lua_parser::parse_content(&lua_content) {
                 Ok(d) => d,
                 Err(e) => {
                     log(format!("Lua Parse Error: {}", e));
                     return;
                 }
             };
             
             // 3. Connect to Steam (Get CDN Tokens)
             let _steam_conn = match DirectDownloader::authenticate_anonymous().await {
                 Ok(c) => Some(c),
                 Err(e) => {
                     log(format!("Steam Auth Warning (Non-Fatal): {}", e));
                     None
                 }
             };
             
             // 4. Download Loop
             let downloader = std::sync::Arc::new(DirectDownloader::new());
             
             // Track installed depots for ACF generation
             let mut installed_depots_info: Vec<(String, u64, String)> = Vec::new();

             let mut depots_to_process = Vec::new();
             
             // Use Hierarchy if available
             if let Some(h) = hierarchy {
                  let mut allowed_depots = std::collections::HashSet::new();
                  for d in h.base_depots { allowed_depots.insert(d.depot_id); }
                  for dlc in h.dlcs {
                      if selected_dlcs.contains(&dlc.app_id) {
                          for d in dlc.depots { allowed_depots.insert(d.depot_id); }
                      }
                  }
                  
                  for depot in script_data.depots {
                      if allowed_depots.contains(&depot.depot_id.to_string()) {
                          depots_to_process.push(depot);
                      }
                  }
             } else {
                  log("Warning: No Hierarchy. Downloading ALL depots found in script.".to_string());
                  depots_to_process = script_data.depots;
             }
             
                for depot in depots_to_process {
                    if let Some(mid) = depot.manifest_id {
                        log(format!("Processing Depot {}...", depot.depot_id));
                        
                        // FIX: Safe decoding of depot key
                        let key_bytes: [u8; 32] = match hex::decode(&depot.depot_key) {
                            Ok(bytes) => match bytes.try_into() {
                                Ok(arr) => arr,
                                Err(_) => {
                                    log(format!("Invalid key length for depot {}", depot.depot_id));
                                    continue;
                                }
                            },
                            Err(e) => {
                                log(format!("Hex decode error for depot {}: {}", depot.depot_id, e));
                                continue;
                            }
                        };
                        
                        let token = "".to_string(); 
                        let base_url = "http://lancache.steamcontent.com".to_string(); 
                        
                        let manifest = if let Some(bytes) = manifest_bytes_map.get(&mid) {
                            // This unwrap is safe if we trust our own internal bytes, but could be handled too
                            match downloader.load_manifest_from_bytes(bytes) {
                                Ok(m) => m,
                                Err(e) => {
                                    log(format!("Failed to load cached manifest {}: {}", mid, e));
                                    continue;
                                }
                            }
                        } else {
                            // Use decoded key_bytes
                            match downloader.fetch_manifest(depot.depot_id, mid, &key_bytes, &base_url, &token).await {
                                Ok(m) => m,
                                Err(e) => {
                                    log(format!("Failed to fetch manifest {}: {}", mid, e));
                                    continue;
                                }
                            }
                        };
                        
                        let jobs = DirectDownloader::generate_jobs(&manifest);
                        let total_bytes: u64 = jobs.iter().map(|j| j.uncompressed_length as u64).sum();
                        
                        // key_bytes is already available here, no need to re-decode
                        let target_path = download_state.lock().unwrap().target_dir.clone();
                     
                     if let Ok(mut s) = download_state.lock() {
                         s.status = crate::direct_download::state::DownloadStatus::Downloading { 
                             files_total: jobs.len(), files_done: 0, bytes_total: total_bytes, bytes_downloaded: 0, speed_mbps: 0.0 
                         };
                         s.last_update = std::time::Instant::now();
                         s.last_bytes_snapshot = 0;
                     }
                     
                     if let Err(e) = downloader.clone().start_download_pool(
                         jobs, key_bytes, base_url.clone(), target_path, download_state.clone(), appid.clone(), depot.depot_id, token.clone()
                     ).await {
                         log(format!("Download Failed: {}", e));
                         return;
                     }
                     
                     // Track for ACF
                     installed_depots_info.push((depot.depot_id.to_string(), total_bytes, mid.to_string()));
                 }
             }
             
             // v1.7.2: SAVE TO VAULT FOR FUTURE REINSTALLATIONS
             log("Saving to Vault for future use...".to_string());
             let vault = crate::vault::VaultManager::new(".");
             
             // Save ZIP for complete backup
             if let Err(e) = vault.store_zip(&appid, &zip_bytes) {
                 log(format!("⚠️ Could not save ZIP to Vault: {}", e));
             } else {
                 log(format!("✅ Saved ZIP to Vault ({} bytes)", zip_bytes.len()));
             }
             
             // Save individual manifests for Steam Install compatibility
             // Note: We use GIDs from the manifest filenames in the ZIP instead of script_data.depots
             //       because depots was already consumed in the download loop
             let mut saved_manifests = 0;
             for (mid, bytes) in &manifest_bytes_map {
                 // Extract depot_id from ZIP filename pattern via stored data
                 // The ZIP has format: {depot_id}_{manifest_id}.manifest
                 // We can infer depot_id from manifest bytes or just save with mid as identifier
                 // For compatibility, we'll extract from the original zip
                 if let Ok(gids) = crate::api::ApiClient::extract_gids_from_zip(&zip_bytes) {
                     for (depot_id, gid) in &gids {
                         if gid.parse::<u64>().ok() == Some(*mid) {
                             if vault.store_manifest_bytes(&appid, depot_id.parse().unwrap_or(0), *mid, bytes).is_ok() {
                                 saved_manifests += 1;
                             }
                             break;
                         }
                     }
                 }
             }
             if saved_manifests > 0 {
                 log(format!("✅ Saved {} manifest files to Vault", saved_manifests));
             }
             
             // v1.7.3: FINALIZATION (ACF + APPLIST + REGISTRY)
             // 1. Generate Full ACF
             let final_install_dir = install_dir_name.clone().unwrap_or(name.clone());
             let acf_filename = format!("appmanifest_{}.acf", appid);
             let target_dir_path = download_state.lock().unwrap().target_dir.clone();
             // target_dir is game root (e.g. steamapps/common/Game), acf goes in steamapps
             if let Some(common_dir) = target_dir_path.parent() { // steamapps/common
                 if let Some(steamapps) = common_dir.parent() { // steamapps
                     let acf_path = steamapps.join(&acf_filename);
                     log(format!("Generating ACF at: {:?}", acf_path));
                     if let Err(e) = crate::steam::manifest::generate_full_acf(&acf_path, &appid, &name, &installed_depots_info) {
                         log(format!("❌ Error generating ACF: {}", e));
                     } else {
                         log("✅ Generated AppManifest.acf (Steam will see game as Installed)".to_string());
                     }
                     
                     // 2. Register Source
                     let mut registry = crate::registry::InstallRegistry::load();
                     registry.register(
                         appid.clone(), 
                         name.clone(), 
                         crate::registry::InstallSource::DirectDownload, 
                         final_install_dir
                     );
                     log("✅ Registered installation source: DirectDownload".to_string());
                 }
             }

             // 3. Update AppList (Critical for 'Launch' button)
             // 3. Update AppList (Critical for 'Launch' button)
             // Extract IDs from LUA
             let (applist_ids, keys) = crate::vdf_injector::parse_lua_for_keys(&lua_content);
             
             // FILTER: Only include Base APPID + Selected DLCs
             let final_applist_ids: Vec<String> = applist_ids.into_iter()
                 .filter(|id| *id == appid || selected_dlcs.contains(id))
                 .collect();

             log(format!("Patching AppList with {} IDs (Filtered from {})...", final_applist_ids.len(), keys.len()));
             
             match crate::app_list::add_games_to_list(&gl_path, final_applist_ids) {
                 Ok(_) => log("✅ AppList patched successfully.".to_string()),
                 Err(e) => log(format!("❌ Error patching AppList: {}", e)),
             }
             
             // 4. Inject Keys (Optional but good practice)
             if !keys.is_empty() {
                 match crate::vdf_injector::inject_vdf(steam_path.as_str(), &keys) {
                     Ok(_) => log("✅ Depot Keys injected.".to_string()),
                     Err(e) => log(format!("⚠️ Key Injection Warning: {}", e)),
                 }
             }

             log("Download & Install Complete. Ready to Launch!".to_string());
             if let Ok(mut s) = download_state.lock() { s.status = crate::direct_download::state::DownloadStatus::Completed; }
        });
    });
}

pub fn spawn_steam_install(
    app: &DarkCoreApp,
    appid: String,
    name: String,
    target_library: Option<PathBuf>,
    install_dir_name: Option<String>,
    selected_dlcs: Vec<String>,
    cached_zip: Option<Vec<u8>>,
    hierarchy: Option<crate::api::GameHierarchy>,
) {
    let log_arc = app.system_log.clone();
    let steam_path = app.config.steam_path.clone();
    let gl_path = app.config.gl_path.clone();
    let api_key = app.config.api_key.clone();
    let enable_stealth = app.config.enable_stealth_mode;
    let status_queue = app.status_update_queue.clone();
    
    let status_queue_closure = status_queue.clone();
    let update_status = move |msg: String| {
        if let Ok(mut lock) = status_queue_closure.lock() {
            *lock = Some(msg);
        }
    };

    std::thread::spawn(move || {
        let log = move |msg: String| {
            if let Ok(mut logs) = log_arc.lock() {
                println!("[LOG] {}", msg);
                push_log(&mut logs, msg);
            }
        };
        
        // Re-initialize client inside thread
        let client = ApiClient::new(api_key.clone());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        
        // FETCH DEPOT INFO via SteamCMD
        let mut _steamcmd_info = None;
        match runtime.block_on(client.get_app_info(&appid)) {
            Ok(info) => {
                log(format!("SteamCMD: Fetched info. Found {} depots.", info.depots.len()));
                _steamcmd_info = Some(info);
            },
            Err(e) => {
                log(format!("Warning: Could not fetch SteamCMD info: {}", e));
            }
        }

        log(format!("START: Protocol for {}", name));
        update_status(format!("Installing {}", name));

        // STEP 0.5: SETUP GREENLUMA CONFIG
        if let Err(e) = crate::ui::helpers::setup_greenluma_config(&gl_path, enable_stealth) {
             log(format!("Warning: Could not setup GreenLuma config: {}", e));
        } else if enable_stealth {
            log("GreenLuma configured (Stealth Mode: ON).".to_string());
        } else {
            log("GreenLuma configured (Stealth Mode: OFF).".to_string());
        }


        // STEP 1: Kill Steam
        log("STEP 1: Killing Steam Process...".to_string());
        let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "steam.exe"]).output();
        std::thread::sleep(std::time::Duration::from_millis(2000));

        let steam_root = steam_path.clone(); 
        let library_path = if let Some(target) = target_library {
            log(format!("Using selected library: {:?}", target));
            target.to_string_lossy().to_string()
        } else {
            steam_path.clone()
        };

        log(format!("Steam Root (Config): {}", steam_root));
        log(format!("Library Path (Game): {}", library_path));

        // STEP 1.5: GHOST INSTALLATION -> GENERATE ACF
        let acf_filename = format!("appmanifest_{}.acf", appid);
        let acf_path = std::path::Path::new(&library_path).join("steamapps").join(&acf_filename);
        
        // Conflict Cleanup
        let all_libs = crate::game_path::GamePathFinder::get_library_folders(&steam_root);
        for lib in all_libs {
             let lib_str = lib.to_string_lossy().to_string();
             if lib_str != library_path {
                 let conflict = lib.join("steamapps").join(&acf_filename);
                 if conflict.exists() {
                     log(format!("Removing conflicting manifest at: {:?}", conflict));
                     let _ = std::fs::remove_file(conflict);
                 }
             }
        }

        // VAULT RESTORE CHECK (with Version Verification)
        let vault = VaultManager::new(".");
        let mut skip_ghost = false;
        let mut skip_morrenus = false;
        let final_install_dir = install_dir_name.clone().unwrap_or(name.clone());

        // v1.7.2: VAULT VERSION VERIFICATION
        // Check if vault manifests are up-to-date before restoring
        let mut vault_is_valid = false;
        if vault.has_manifests(&appid) {
            log("Vault: Checking manifest versions...".to_string());
            
            // Use SteamCMD API (FREE) to get current GIDs
            match runtime.block_on(client.get_public_gids(&appid)) {
                Ok(current_gids) => {
                    let (is_valid, outdated_depots) = vault.verify_manifests(&appid, &current_gids);
                    
                    if is_valid {
                        log("✅ Vault manifests are up-to-date!".to_string());
                        vault_is_valid = true;
                    } else {
                        log(format!("⚠️ Vault outdated! {} depots need update.", outdated_depots.len()));
                        // Invalidate outdated depots only
                        if let Err(e) = vault.invalidate_depots(&appid, &outdated_depots) {
                            log(format!("Warning: Could not invalidate depots: {}", e));
                        }
                        vault_is_valid = false;
                    }
                },
                Err(e) => {
                    // If we can't check, assume vault is okay (fail-safe)
                    log(format!("⚠️ Could not verify vault version ({}). Using cached data.", e));
                    vault_is_valid = true;
                }
            }
        }

        // Only restore from vault if verified
        if vault_is_valid {
            if let Ok((restored_acf, count)) = vault.restore_manifests(&library_path, &appid) {
                if count > 0 { 
                    log(format!("Vault: Restored {} verified depot manifests. SKIPPING MORRENUS (Token Saved). 🛡️", count)); 
                    skip_morrenus = true;
                }
                if restored_acf {
                    log("Vault: Restored AppManifest.acf. Skipping Ghost Generation. 🛡️".to_string());
                    skip_ghost = true;
                }
            }
        }

        if !skip_ghost {
            if acf_path.exists() {
                log(format!("Removing old ACF: {:?}", acf_path));
                let _ = std::fs::remove_file(&acf_path);
            }

            log(format!("Generating Ghost ACF (SMD-Style) at: {:?}", acf_path));
            if let Err(e) = crate::steam::manifest::generate_smd_style_acf(&acf_path, &appid, &final_install_dir) {
                log(format!("Error writing ACF: {}", e));
            } else {
                 log("Ghost ACF generated (SMD-Style). Steam will see game as 'Update Required'.".to_string());
            }
        }

        // STEP 2: MORRENUS MANIFEST DOWNLOAD
        #[allow(unused_assignments)]
        let mut applist_ids = Vec::new(); // IDs to inject
        let mut keys: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let depot_cache = std::path::Path::new(&steam_root).join("depotcache");
        if !depot_cache.exists() { let _ = std::fs::create_dir_all(&depot_cache); }
        
        if skip_morrenus {
            log("STEP 2: SKIPPED - Using Vault manifests. ðŸ›¡ï¸ ".to_string());
            applist_ids.push(appid.clone());
        } else {
            log("STEP 2: Fetching game data from Morrenus...".to_string());
            update_status(format!("Downloading manifests for {}", name));
            
            // USE CACHED ZIP IF AVAILABLE, ELSE DOWNLOAD
            let zip_bytes = if let Some(b) = cached_zip {
                log("Using cached Morrenus ZIP.".to_string());
                b
            } else {
                 match runtime.block_on(client.download_manifest(&appid)) {
                     Ok(b) => b.to_vec(),
                     Err(e) => {
                         log(format!("Download Error: {}", e));
                         update_status("Error downloading manifests".to_string());
                         return;
                     }
                 }
            };
            
            let mut lua_content = String::new();
            if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                for i in 0..archive.len() {
                    if let Ok(mut f) = archive.by_index(i) {
                         if f.name().ends_with(".lua") {
                             use std::io::Read;
                             let _ = f.read_to_string(&mut lua_content);
                         } else if f.name().contains("depotcache") {
                              // Extract depot manifest
                              let out_path = depot_cache.join(f.name());
                              if let Some(parent) = out_path.parent() { let _ = std::fs::create_dir_all(parent); }
                              
                              let mut buf = Vec::new();
                              use std::io::Read; 
                              let _ = f.read_to_end(&mut buf);
                              let _ = std::fs::write(&out_path, &buf);
                              log(format!("Extracted: {}", f.name()));
                         }
                    }
                }
            }
            if lua_content.is_empty() {
                lua_content = String::from_utf8_lossy(&zip_bytes).to_string();
            }
            
            // Parse LUA for AppList IDs
            let (ids, parsed_keys) = crate::vdf_injector::parse_lua_for_keys(&lua_content);
            applist_ids = ids;
            keys = parsed_keys;
            log(format!("Parsed {} AppIDs and {} Keys from LUA.", applist_ids.len(), keys.len()));
        }

        // STEP 3: UPDATE APPLIST.JSON
        log("STEP 3: Patching GreenLuma AppList...".to_string());
        update_status("Patching AppList...".to_string());
        
        let mut final_ids = Vec::new();
        
        if let Some(h) = &hierarchy {
             log("Using GameHierarchy for Mandatory Depot Resolution...".to_string());
             final_ids = resolve_mandatory_depots(h, &selected_dlcs);
             log(format!("Resolved {} mandatory IDs (Base + DLCs + Depots).", final_ids.len()));
        } else {
             // Fallback: Use simple AppID + Selected DLCs logic (Legacy)
             final_ids.push(appid.clone());
             
             if !selected_dlcs.is_empty() {
                  log(format!("Using {} user-selected DLCs (Fallback Mode).", selected_dlcs.len()));
                  for id in &selected_dlcs {
                      if !final_ids.contains(id) {
                          final_ids.push(id.clone());
                      }
                  }
             } else {
                 // Try to use applist_ids from LUA if no selection (e.g. First Install default)
                 // But strictly speaking, if selected_dlcs is passed, it should be respected.
                 // If selected_dlcs is empty, it might mean "Just Base Game" OR "No Selection made".
                 // In UI, selected_dlcs is usually populated?
                 // We will trust selected_dlcs if present.
                 if !applist_ids.is_empty() && selected_dlcs.is_empty() {
                      // Maybe Auto-Select All from Lua if nothing selected?
                      // No, adhere to safety: Base Game Only.
                      log("No DLCs selected (Base Game Only).".to_string());
                 }
             }
        }

        match crate::app_list::add_games_to_list(&gl_path, final_ids.clone()) {
            Ok(_) => log("AppList patched successfully.".to_string()),
            Err(e) => log(format!("Error patching AppList: {}", e)),
        }
        
        // STEP 4: INJECT KEYS
        if !keys.is_empty() {
            log(format!("STEP 4: Injecting {} Depot Keys...", keys.len()));
            match crate::vdf_injector::inject_vdf(&steam_root, &keys) {
                Ok(_) => log("Keys injected into config.vdf.".to_string()),
                Err(e) => log(format!("Key Injection Error: {}", e)),
            }
        }

        // STEP 5: RELAUNCH STEAM
        log("STEP 5: Relaunching Steam...".to_string());
        update_status("Relaunching Steam...".to_string());
        
        let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
        if steam_exe.exists() {
            if let Err(e) = open::that(steam_exe) {
                log(format!("Failed to launch Steam: {}", e));
            } else {
                 log("Steam relaunched. Download should begin shortly.".to_string());
            }
        } else {
            log("Steame.exe not found.".to_string());
        }
        
        update_status("Ready".to_string());
        
        // Force Refresh Library UI
        if let Ok(mut lock) = status_queue.lock() {
            *lock = Some("REFRESH_LIB".to_string());
        }
        // Register Source
        let final_install_dir_reg = install_dir_name.unwrap_or(name.clone());
        let mut registry = crate::registry::InstallRegistry::load();
        registry.register(
            appid.clone(), 
            name.clone(), 
            crate::registry::InstallSource::SteamCMD, 
            final_install_dir_reg
        );
        log("✅ Registered installation source: SteamCMD".to_string());

    });
}

pub fn install_game_family_godmode(app: &mut DarkCoreApp, appid: String) {
   if !app.config.family_godmode_ids.contains(&appid) {
       app.config.family_godmode_ids.push(appid.clone());
       let _ = crate::config::save_config(&app.config);
   }

   let gl_path = app.config.gl_path.clone();
   let include_dlcs = app.include_dlcs;
   let client_opt = app.api_client.clone(); 
   let log_arc = app.system_log.clone();
   let status_queue = app.status_update_queue.clone();

   std::thread::spawn(move || {
       let log = move |msg: String| {
           if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, msg); }
       };
       
       log(format!("Family Godmode: Initializing for {}...", appid));

       let mut ids = vec![appid.clone()];

       if include_dlcs {
           let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
           log("Fetching DLCs...".to_string());
           
           let dlcs_result = if let Some(client) = client_opt {
                rt.block_on(client.get_dlc_list(&appid))
           } else {
                rt.block_on(async {
                    let client = reqwest::Client::new();
                    let url = format!("https://store.steampowered.com/api/appdetails?appids={}&filters=dlc", appid);
                    if let Ok(resp) = client.get(&url).send().await {
                        if let Ok(root) = resp.json::<serde_json::Value>().await {
                            let mut dlc_ids = Vec::new();
                            if let Some(app_data) = root.get(&appid) {
                                if let Some(data) = app_data.get("data") {
                                    if let Some(dlc_array) = data.get("dlc").and_then(|v| v.as_array()) {
                                        for item in dlc_array {
                                            if let Some(id) = item.as_u64() { dlc_ids.push(id.to_string()); }
                                        }
                                    }
                                }
                            }
                            return Ok(dlc_ids);
                        }
                    }
                    Ok(vec![])
                })
           };

           match dlcs_result {
                Ok(dlcs) => {
                    log(format!("Found {} DLCs to unlock.", dlcs.len()));
                    ids.extend(dlcs);
                },
                Err(e) => log(format!("DLC Fetch Warning: {}", e)),
           }
       }

       match crate::app_list::add_games_to_list(&gl_path, ids) {
           Ok(_) => {
               log("âœ… Family Shared Godmode Active.".to_string());
               if let Ok(mut q) = status_queue.lock() {
                   *q = Some("REFRESH_LIB".to_string());
               }
           },
           Err(e) => log(format!("â Œ Error writing AppList: {}", e)),
       }
   });
}

pub fn disable_family_godmode(app: &mut DarkCoreApp, appid: String) {
     if let Some(pos) = app.config.family_godmode_ids.iter().position(|x| *x == appid) {
         app.config.family_godmode_ids.remove(pos);
         let _ = crate::config::save_config(&app.config);
     }

     let gl_path = app.config.gl_path.clone();
     let client_opt = app.api_client.clone();
     let log_arc = app.system_log.clone();
     let status_queue = app.status_update_queue.clone();

     std::thread::spawn(move || {
         let log = move |msg: String| {
             if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, msg); }
         };
         
         log(format!("Disabling Family Godmode for {}...", appid));
         
         let mut ids_to_remove = vec![appid.clone()];
         let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

         let dlcs_result = if let Some(client) = client_opt {
              rt.block_on(client.get_dlc_list(&appid))
         } else {
             rt.block_on(async {
                 let client = reqwest::Client::new();
                 let url = format!("https://store.steampowered.com/api/appdetails?appids={}&filters=dlc", appid);
                 if let Ok(resp) = client.get(&url).send().await {
                       if let Ok(root) = resp.json::<serde_json::Value>().await {
                           let mut dlc_ids = Vec::new();
                           if let Some(app_data) = root.get(&appid) {
                               if let Some(data) = app_data.get("data") {
                                   if let Some(dlc_array) = data.get("dlc").and_then(|v| v.as_array()) {
                                       for item in dlc_array {
                                           if let Some(id) = item.as_u64() { dlc_ids.push(id.to_string()); }
                                       }
                                   }
                               }
                           }
                           return Ok(dlc_ids);
                       }
                 }
                 Ok(vec![])
             })
         };
         
         if let Ok(dlcs) = dlcs_result {
             ids_to_remove.extend(dlcs);
         }
         
         match crate::app_list::remove_games_from_list(&gl_path, ids_to_remove) {
              Ok(_) => {
                   log("âœ… Family Godmode Disabled.".to_string());
                   if let Ok(mut q) = status_queue.lock() {
                       *q = Some("REFRESH_LIB".to_string());
                   }
              },
              Err(e) => log(format!("Error removing from AppList: {}", e)),
         }
     });
}

fn extract_dlc_name_from_lua(lua_content: &str, dlc_id: &str) -> Option<String> {
    for line in lua_content.lines() {
        if line.contains(dlc_id) && line.contains("--") {
            let parts: Vec<&str> = line.split("--").collect();
            if parts.len() > 1 {
                return Some(parts[1].trim().to_string());
            }
        }
    }
    None
}

fn resolve_mandatory_depots(
    hierarchy: &crate::api::GameHierarchy,
    selected_dlcs: &[String],
) -> Vec<String> {
    let mut ids = Vec::new();
    
    // 1. Root AppID
    ids.push(hierarchy.root_id.clone());
    
    // 2. Base Depots
    for depot in &hierarchy.base_depots {
        ids.push(depot.depot_id.clone());
    }
    
    // 3. Selected DLCs and THEIR Depots
    for dlc in &hierarchy.dlcs {
        if selected_dlcs.contains(&dlc.app_id) {
            ids.push(dlc.app_id.clone());
            for depot in &dlc.depots {
                ids.push(depot.depot_id.clone());
            }
        }
    }
    
    ids.sort();
    ids.dedup();
    
    ids
}
