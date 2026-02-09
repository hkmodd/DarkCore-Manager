#![allow(dead_code)] // Reserved: Watcher UI integration pending
//! Auto-Update Watcher Module
//! Monitors installed games for updates via Steam's public API
//! and triggers manifest downloads when new versions are detected.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::app_list::{GameProfile, refresh_active_games_list, RelationshipMap};
use crate::manifest_downloader::ManifestDownloader;

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
        relationships: Arc<std::sync::Mutex<RelationshipMap>>,
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

                // Get current active games
                let (cache_snapshot, rels_snapshot) = {
                    let cache = game_cache.lock().unwrap();
                    let rels = relationships.lock().unwrap();
                    (cache.clone(), rels.clone())
                };

                let active_games = refresh_active_games_list(
                    &gl_path,
                    &steam_path,
                    &cache_snapshot,
                    &rels_snapshot,
                );

                // Check each game for updates
                let updates = rt.block_on(async {
                    check_games_for_updates_internal(
                        &api_key,
                        &active_games,
                        &state,
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
                let known_build = {
                    let s = state.read().await;
                    s.known_builds.get(&game.app_id).copied()
                };

                let needs_update = match known_build {
                    Some(local) => remote_build > local,
                    None => false, // First time seeing this game
                };

                results.push(UpdateCheckResult {
                    app_id: game.app_id.clone(),
                    name: game.name.clone(),
                    old_build: known_build,
                    new_build: remote_build,
                    needs_update,
                });

                // Update known build
                {
                    let mut s = state.write().await;
                    s.known_builds.insert(game.app_id.clone(), remote_build);
                }
            }
        }
        // Silently skip errors (rate limiting, network, etc.)

        // Small delay to avoid rate limiting
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    results
}

/// Trigger manifest download for a specific update.
/// 
/// INTELLIGENT UPDATE FLOW:
/// 1. Fetch current depot GIDs from SteamCMD (free API)
/// 2. Filter depots by target language
/// 3. For each depot: check if depotcache already has the CORRECT GID
/// 4. If outdated GID exists → delete old manifest, download new one
/// 5. If correct GID exists → skip (already up to date)
/// 6. If missing → download fresh
/// 7. Save all new manifests to Vault for future offline use
pub async fn trigger_update_download(
    _api_key: &str,
    _downloader: &ManifestDownloader,
    app_id: &str,
    steam_path: &std::path::Path,
    _target_language: &str,
) -> Result<usize, String> {
    let app_id_owned = app_id.to_string();
    let steam_path_owned = steam_path.to_string_lossy().to_string();
    
    // UNIFIED LOGIC: Delegate to the robust helper (v1.7.3+)
    let log_fn = |s: String| println!("[ManualUpdate] {}", s);
    
    match crate::ui::helpers::download_manifests_wudrm(&app_id_owned, &steam_path_owned, &log_fn) {
        Ok(count) => Ok(count),
        Err(e) => Err(e.to_string()),
    }
}
