#![allow(dead_code)] // Reserved: Watcher UI integration pending
//! Auto-Update Watcher Module
//! Monitors installed games for updates via Steam's public API
//! and triggers manifest downloads when new versions are detected.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::app_list::{GameProfile, AppListManager, RelationshipMap};
use crate::downloader::ManifestDownloader;

/// Cached state for tracking game versions
#[derive(Debug, Clone, Default)]
pub struct WatcherState {
    /// Map of AppID -> last known BuildID
    pub known_builds: HashMap<String, u64>,
    /// Whether the watcher is currently running
    pub running: bool,
    /// Last check timestamp
    pub last_check: Option<std::time::Instant>,
    /// Pending updates (AppID, old_build, new_build)
    pub pending_updates: Vec<(String, u64, u64)>,
}

/// Result of an update check
#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub app_id: String,
    pub name: String,
    pub old_build: Option<u64>,
    pub new_build: u64,
    pub needs_update: bool,
}

/// The Watcher service that runs in background
pub struct Watcher {
    state: Arc<RwLock<WatcherState>>,
    check_interval_mins: u64,
}

impl Watcher {
    pub fn new(check_interval_mins: u64) -> Self {
        Self {
            state: Arc::new(RwLock::new(WatcherState::default())),
            check_interval_mins,
        }
    }

    /// On Startup: Scan ALL library folders for games in "Update Required" state (StateFlags 6, 1026, 1042, 1062)
    /// and Flag them for User Update (Do NOT auto-execute).
    pub async fn startup_scan(
        &self,
        _api_key: String,
        steam_path: String,
        _downloader: Arc<ManifestDownloader>,
        managed_app_ids: &std::collections::HashSet<String>,
    ) {
        println!("[Watcher] Starting startup scan for broken games...");
        let lib_folders = crate::utils::vdf::get_all_library_folders(&steam_path); // Use shared util

        for library in lib_folders {
            let steamapps = library.join("steamapps");
            if let Ok(entries) = std::fs::read_dir(&steamapps) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                         if name.starts_with("appmanifest_") && name.ends_with(".acf") {
                             // Parse ACF
                             if let Ok(content) = std::fs::read_to_string(&path) {
                                 // Robust Key-Value Parse
                                 let mut appid = String::new();
                                 let mut state_flags = 0u32;
                                 
                                 // We use the VdfParser for robustness if available, or robust regex-less search
                                 if let Some(pos) = content.find("\"appid\"") {
                                     let remainder = &content[pos..];
                                     if let Some(start_quote) = remainder.find('\"') {
                                         if let Some(end_label) = remainder[start_quote+1..].find('\"') {
                                              let val_part = &remainder[start_quote+1+end_label+1..];
                                              if let Some(v_start) = val_part.find('\"') {
                                                  if let Some(v_end) = val_part[v_start+1..].find('\"') {
                                                      appid = val_part[v_start+1 .. v_start+1+v_end].to_string();
                                                  }
                                              }
                                         }
                                     }
                                 }
                                 
                                 if let Some(pos) = content.find("\"StateFlags\"") {
                                     let remainder = &content[pos..];
                                     if let Some(start_quote) = remainder.find('\"') {
                                         if let Some(end_label) = remainder[start_quote+1..].find('\"') {
                                              let val_part = &remainder[start_quote+1+end_label+1..];
                                              if let Some(v_start) = val_part.find('\"') {
                                                  if let Some(v_end) = val_part[v_start+1..].find('\"') {
                                                      let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                      if let Ok(val) = num_str.parse::<u32>() {
                                                          state_flags = val;
                                                      }
                                                  }
                                              }
                                         }
                                     }
                                 }

                                 // Correction Protocol: 6, 1026, 1042, 1062
                                 let needs_update = state_flags == 6 || state_flags == 1026 || state_flags == 1042 || state_flags == 1062 || (state_flags & 2) != 0;

                                 if needs_update {
                                     // FILTER: Only process if it's a managed game!
                                     if !managed_app_ids.contains(&appid) {
                                         continue;
                                     }

                                     println!("[Watcher] Found broken managed game: AppID {} (StateFlags: {})", appid, state_flags);
                                     
                                     // Populate State (Do NOT Fix)
                                     let mut s = self.state.write().await;
                                     // Add to pending updates if not exists
                                     if !s.pending_updates.iter().any(|(id,_,_)| id == &appid) {
                                         s.pending_updates.push((appid.clone(), 0, 0)); // BuildIDs unknown here
                                     }
                                 }
                             }
                         }
                    }
                }
            }
        }
        println!("[Watcher] Startup scan complete.");
    }

    /// Get a clone of the state Arc for sharing
    pub fn state_handle(&self) -> Arc<RwLock<WatcherState>> {
        Arc::clone(&self.state)
    }

    /// Start the background update loop (synchronous version that spawns a thread)
    pub fn start_update_loop_sync(
        &self,
        api_key: String,
        gl_path: String,
        steam_path: String,
        game_cache: Arc<std::sync::Mutex<HashMap<String, String>>>,
        _relationships: Arc<std::sync::Mutex<RelationshipMap>>,
        log_tx: std::sync::mpsc::Sender<String>,
    ) -> std::thread::JoinHandle<()> {
        let state = Arc::clone(&self.state);
        let interval_mins = self.check_interval_mins;

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };

            // Mark as running
            rt.block_on(async {
                let mut s = state.write().await;
                s.running = true;
            });

            let _ = log_tx.send("[Watcher] Update loop started".to_string());
            
            // Helper to get active games
            let get_active_games = || -> Vec<GameProfile> {
                let cache_snapshot = game_cache.lock().unwrap().clone();
                let mgr = AppListManager::new(
                    std::path::Path::new(&gl_path),
                    std::path::Path::new(&steam_path)
                );
                mgr.refresh_active_games_list(&cache_snapshot, &[], &HashMap::new())
            };

            loop {
                // Sleep for interval
                std::thread::sleep(Duration::from_secs(interval_mins * 60));

                // Check if we should still be running
                let should_run = rt.block_on(async {
                    let s = state.read().await;
                    s.running
                });
                
                if !should_run {
                    break;
                }

                let _ = log_tx.send("[Watcher] Checking for updates...".to_string());

                let active_games = get_active_games();

                // Check each game for updates
                let updates = rt.block_on(async {
                    check_games_for_updates_internal(
                        &api_key,
                        &active_games,
                        &state,
                        std::path::Path::new(&steam_path),
                    ).await
                });

                if !updates.is_empty() {
                    let update_count = updates.iter().filter(|u| u.needs_update).count();
                    if update_count > 0 {
                        let _ = log_tx.send(format!(
                            "[Watcher] Found {} games with updates!",
                            update_count
                        ));

                        // Store pending updates
                        rt.block_on(async {
                            let mut s = state.write().await;
                            s.pending_updates = updates.iter()
                                .filter(|u| u.needs_update)
                                .map(|u| (
                                    u.app_id.clone(),
                                    u.old_build.unwrap_or(0),
                                    u.new_build,
                                ))
                                .collect();
                        });

                        // Log each update
                        for update in &updates {
                            if update.needs_update {
                                let _ = log_tx.send(format!(
                                    "[Watcher] UPDATE: {} ({}) - Build {} -> {}",
                                    update.name,
                                    update.app_id,
                                    update.old_build.unwrap_or(0),
                                    update.new_build
                                ));
                            }
                        }
                    } else {
                        let _ = log_tx.send("[Watcher] All games are up to date.".to_string());
                    }
                } else {
                    let _ = log_tx.send("[Watcher] No games to check.".to_string());
                }

                // Update last check time
                rt.block_on(async {
                    let mut s = state.write().await;
                    s.last_check = Some(std::time::Instant::now());
                });
            }

            let _ = log_tx.send("[Watcher] Update loop stopped".to_string());
        })
    }

    /// Stop the watcher loop
    pub fn stop_sync(&self) {
        // Use a blocking runtime to set the flag
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async {
                let mut s = self.state.write().await;
                s.running = false;
            });
        }
    }
}

/// Check a list of games for available updates (internal async helper)
async fn check_games_for_updates_internal(
    api_key: &str,
    games: &[GameProfile],
    state: &Arc<RwLock<WatcherState>>,
    steam_path: &std::path::Path,
) -> Vec<UpdateCheckResult> {
    let mut results = Vec::new();

    // Create a fresh client for this check
    let client = crate::api::ApiClient::new(api_key.to_string());

    // Collect unique parent AppIDs (skip depot-only entries)
    let mut checked_ids = std::collections::HashSet::new();

    for game in games {
        // Skip if this looks like a depot
        if game.name.starts_with("Depot (") || game.name.contains("(Content)") {
            continue;
        }

        // Skip if already checked
        if checked_ids.contains(&game.app_id) {
            continue;
        }
        checked_ids.insert(game.app_id.clone());

        // Fetch current build info from Steam (handle error locally)
        let info_result = client.get_app_info(&game.app_id).await;
        
        if let Ok(info) = info_result {
            if let Some(remote_build) = info.buildid {
                let mut local_build: u64 = 0;
                let mut state_flags: u32 = 0;
                let all_libraries = crate::utils::vdf::get_all_library_folders(steam_path.to_str().unwrap_or("")); // Shared helper
                let mut acf_found = false;

                for lib in &all_libraries {
                    let acf_path = lib.join("steamapps").join(format!("appmanifest_{}.acf", game.app_id));
                    if acf_path.exists() {
                        acf_found = true;
                        if let Ok(content) = std::fs::read_to_string(&acf_path) {
                            // Robust Parse Logic
                             if let Some(pos) = content.find("\"buildid\"") {
                                let remainder = &content[pos..];
                                if let Some(start_quote) = remainder.find('\"') {
                                    if let Some(end_label) = remainder[start_quote+1..].find('\"') {
                                         let val_part = &remainder[start_quote+1+end_label+1..];
                                         if let Some(v_start) = val_part.find('\"') {
                                             if let Some(v_end) = val_part[v_start+1..].find('\"') {
                                                 let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                 if let Ok(b) = num_str.parse::<u64>() {
                                                     local_build = b;
                                                 }
                                             }
                                         }
                                    }
                                }
                            }

                            if let Some(pos) = content.find("\"StateFlags\"") {
                                let remainder = &content[pos..];
                                if let Some(start_quote) = remainder.find('\"') {
                                    if let Some(end_label) = remainder[start_quote+1..].find('\"') {
                                         let val_part = &remainder[start_quote+1+end_label+1..];
                                         if let Some(v_start) = val_part.find('\"') {
                                             if let Some(v_end) = val_part[v_start+1..].find('\"') {
                                                 let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                 if let Ok(val) = num_str.parse::<u32>() {
                                                     state_flags = val;
                                                 }
                                             }
                                         }
                                    }
                                }
                            }
                        }
                        break;
                    }
                }

                // CHECK: StateFlags Priority
                let state_requires_update = state_flags == 6 || state_flags == 1026 || state_flags == 1042 || state_flags == 1062 || (state_flags & 2) != 0;

                let mut needs_update = if state_requires_update {
                    true
                } else if local_build > 0 && remote_build > 0 {
                    remote_build > local_build
                } else {
                    false
                };

                // Fallback: Depotcache check
                if !needs_update && (!acf_found || local_build == 0) {
                     let depot_cache = steam_path.join("depotcache");
                     if depot_cache.exists() {
                         for (depot_id, depot_info) in &info.depots {
                             if let Some(gid) = &depot_info.gid {
                                 let expected = format!("{}_{}.manifest", depot_id, gid);
                                 let expected_path = depot_cache.join(&expected);
                                 
                                 if !expected_path.exists() {
                                     let pattern = format!("{}_", depot_id);
                                     if let Ok(entries) = std::fs::read_dir(&depot_cache) {
                                         for entry in entries.flatten() {
                                             let name = entry.file_name().to_string_lossy().to_string();
                                             // Match old but not current
                                             if name.starts_with(&pattern) && name.ends_with(".manifest") && name != expected {
                                                 needs_update = true;
                                                 break;
                                             }
                                         }
                                     }
                                 }
                             }
                             if needs_update { break; }
                         }
                     }
                }

                if needs_update {
                    results.push(UpdateCheckResult {
                        app_id: game.app_id.clone(),
                        name: game.name.clone(),
                        old_build: Some(local_build),
                        new_build: remote_build,
                        needs_update,
                    });
                } 

                // Update known build to avoid re-checking satisfied state
                {
                    let mut s = state.write().await;
                    s.known_builds.insert(game.app_id.clone(), remote_build);
                }
            }
        }
        // Small delay
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    results
}

/// Trigger manifest download with WUDRM logic (Vault, Validation, Cleanup)
pub async fn trigger_update_download(
    api_key: &str,
    downloader: &ManifestDownloader,
    app_id: &str,
    steam_path: &std::path::Path,
    _target_language: &str,
) -> Result<usize, String> {
    
    // Standard Steam Protobuf Magic: 0x71F617D0 (Little Endian)
    const STEAM_MANIFEST_MAGIC: u32 = 0x71F617D0;

    // 1. Get Depot List
    let client = crate::api::ApiClient::new(api_key.to_string());
    // We need a runtime for the vault? No, vault is sync mostly.
    // However, Watcher is async.
    
    let info = client.get_app_info(app_id).await.map_err(|e| e.to_string())?;

    let mut valid_manifests = 0;
    
    // Ensure depotcache exists
    let depot_cache = steam_path.join("depotcache");
    if !depot_cache.exists() {
        let _ = std::fs::create_dir_all(&depot_cache);
    }

    // Initialize Vault Manager (path relative like in VaultManager::new)
    let vault = crate::vault::VaultManager::new_local();

    for (depot_id, depot_info) in info.depots {
        if let Some(gid) = depot_info.gid {
             let filename = format!("{}_{}.manifest", depot_id, gid);
             let expected_path = depot_cache.join(&filename);
             
             // 2. SMART SKIP: Check if already exists AND is valid
             if expected_path.exists() {
                 let mut is_valid = false;
                 if let Ok(meta) = std::fs::metadata(&expected_path) {
                     if meta.len() > 50 {
                         if let Ok(mut file) = std::fs::File::open(&expected_path) {
                             let mut buf = [0u8; 4];
                             use std::io::Read;
                             if file.read_exact(&mut buf).is_ok() {
                                 if u32::from_le_bytes(buf) == STEAM_MANIFEST_MAGIC {
                                     is_valid = true;
                                 }
                             }
                         }
                     }
                 }

                 if is_valid {
                     valid_manifests += 1;
                     // Ensure backed up
                     let _ = vault.store_manifest(app_id, &expected_path);
                     continue;
                 } else {
                     // Corrupt, remove
                     let _ = std::fs::remove_file(&expected_path);
                 }
             }

             // 3. CLEANUP: Remove OLD manifests for this depot
             if let Ok(entries) = std::fs::read_dir(&depot_cache) {
                 for entry in entries.flatten() {
                     let path = entry.path();
                     if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                         if fname.starts_with(&format!("{}_", depot_id)) && fname.ends_with(".manifest") {
                             if fname != filename {
                                 // Outdated
                                 let _ = std::fs::remove_file(path);
                             }
                         }
                     }
                 }
             }

             // 4. VAULT RESTORE
             let vault_dir = vault.get_storage_dir(app_id);
             let vault_manifest = vault_dir.join(&filename);
             if vault_manifest.exists() {
                 // Validate Vault Copy
                 let mut is_vault_valid = false;
                 if let Ok(mut file) = std::fs::File::open(&vault_manifest) {
                     let mut buf = [0u8; 4];
                     use std::io::Read;
                     if file.read_exact(&mut buf).is_ok() {
                         if u32::from_le_bytes(buf) == STEAM_MANIFEST_MAGIC {
                             is_vault_valid = true;
                         }
                     }
                 }

                 if is_vault_valid {
                     if std::fs::copy(&vault_manifest, &expected_path).is_ok() {
                         valid_manifests += 1;
                         continue;
                     }
                 }
             }

             // 5. DOWNLOAD
             match downloader.download_manifest(&depot_id, &gid, &depot_cache).await {
                 Ok(result) => {
                     // 6. VERIFY NEW FILE
                     let mut verification_passed = false;
                     if let Ok(mut f) = std::fs::File::open(&result.path) {
                        let mut buf = [0u8; 4];
                        use std::io::Read;
                        if f.read_exact(&mut buf).is_ok() {
                            if u32::from_le_bytes(buf) == STEAM_MANIFEST_MAGIC {
                                verification_passed = true;
                            }
                        }
                     }

                     if verification_passed {
                         valid_manifests += 1;
                         let _ = vault.store_manifest(app_id, &result.path);
                     } else {
                         let _ = std::fs::remove_file(&result.path);
                     }
                 },
                 Err(e) => println!("Failed to download manifest for {}: {}", depot_id, e),
             }
        }
    }
    
    Ok(valid_manifests)
}
