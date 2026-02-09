use crate::ui::state::DarkCoreApp;
use crate::api::ApiClient;
use crate::cache::save_game_cache;
use crate::ui::state::push_log;
// use crate::vault::VaultManager;
// use crate::manifest_downloader::ManifestDownloader;
// use std::collections::HashMap;
// use zip::ZipArchive;
use std::path::Path;

pub fn resolve_unknown_games(app: &mut DarkCoreApp) {
    // Hybrid System: Even without key, we can resolve names via Steam Store API.
    let active_games = app.active_games.clone();
    let game_cache = app.game_cache.clone();
    let client_key = app.config.api_key.clone();
    let steam_path = app.config.steam_path.clone();
    let status_queue = app.status_update_queue.clone();
    let relationships = app.relationships.clone(); // Capture relationships

    app.status_msg = "Resolving unknown games & DLCs...".to_string();

    std::thread::spawn(move || {
        let mut ids_to_resolve = Vec::new();

        // Identify unknowns OR orphans (possible unlinked DLCs)
        {
            if let Ok(games) = active_games.lock() {
                for g in games.iter() {
                    // Check if needs Name Resolution OR Relationship Check
                    if g.name == "Unknown" || g.name.starts_with("Depot of") || g.parent_id.is_none() {
                        ids_to_resolve.push(g.app_id.clone());
                    }
                }
            }
        }

        // Use Shared Runtime (Optimization)
        crate::ui::state::ASYNC_RUNTIME.block_on(async {
            let mut handles = Vec::new();
            let shared_client = ApiClient::new(client_key.clone());

            for id in ids_to_resolve {
                let client = shared_client.clone();
                let game_cache = game_cache.clone();
                let rel_map = relationships.clone();
                let id_clone = id.clone();
                let steam_path_ref = steam_path.clone();

                handles.push(tokio::spawn(async move {
                    let mut found_name = None;

                    // 0. Hardcoded Common Redists
                    match id_clone.as_str() {
                         "228980" => found_name = Some("Steamworks Common Redistributables".to_string()),
                         "228981" | "228982" | "228983" | "228984" | "228985" | 
                         "228986" | "228987" | "228988" | "228989" | "228990" => {
                             found_name = Some(format!("Steamworks Redist ({})", id_clone));
                         },
                         "366850" => found_name = Some("Old World".to_string()),
                         "408630" => found_name = Some("Europa Universalis IV".to_string()),
                         _ => {}
                    }

                    // 1. Try Morrenus Search first
                    if found_name.is_none() {
                        if let Ok(results) = client.search(&id_clone).await {
                            use crate::api::val_to_string;
                            let matched = results.iter().find(|r| {
                                let rid = val_to_string(&r.game_id);
                                let rid2 = val_to_string(&r.app_id);
                                rid == id_clone || rid2 == id_clone
                            });

                            if let Some(res) = matched {
                                let name = res
                                    .game_name
                                    .as_deref()
                                    .or(res.name.as_deref())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                if name != "Unknown" {
                                    found_name = Some(name);
                                }
                            }
                        }
                    }

                    // 2. Fallback: Steam Store API & HTML Scraper
                    if found_name.is_none() {
                        let url = format!(
                            "https://store.steampowered.com/api/appdetails?appids={}",
                            id_clone
                        );
                        let mut api_success = false;

                        if let Ok(resp) = reqwest::get(&url).await {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(data) =
                                    json.get(&id_clone).and_then(|v| v.get("data"))
                                {
                                    if let Some(name_val) = data.get("name") {
                                        if let Some(n) = name_val.as_str() {
                                            found_name = Some(n.to_string());
                                            api_success = true;
                                        }
                                    }
                                }
                            }
                        }

                        // 2b. HTML Title Scraper (Nuclear Option)
                        if !api_success {
                            let page_url = format!("https://store.steampowered.com/app/{}", id_clone);
                            if let Ok(resp) = reqwest::get(&page_url).await {
                                if let Ok(text) = resp.text().await {
                                     if let Some(start) = text.find("<title>") {
                                         if let Some(end) = text[start..].find(" on Steam</title>") {
                                             let raw = &text[start + 7 .. start + end];
                                             let cleaned = raw.trim()
                                                .replace("&amp;", "&")
                                                .replace("&apos;", "'")
                                                .replace("&#39;", "'");
                                             
                                             if !cleaned.is_empty() {
                                                 found_name = Some(cleaned);
                                             }
                                         }
                                     }
                                }
                            }
                        }
                    }

                    // 3. Fallback: Local Config VDF (Depot Check)
                    if found_name.is_none() {
                        if let Some(parent_id) = crate::game_path::GamePathFinder::find_parent_for_depot(&steam_path_ref, &id_clone) {
                            // Try to get parent name from cache
                            let parent_name = {
                                if let Ok(c) = game_cache.lock() {
                                    c.get(&parent_id).cloned()
                                } else {
                                    None
                                }
                            };
                            
                            if let Some(p_name) = parent_name {
                                found_name = Some(format!("{} [Depot]", p_name));
                            } else {
                                found_name = Some(format!("Depot of AppID {}", parent_id));
                            }
                        }
                    }

                    // 4. Fallback: Deep Manifest Scan (User Mounted Depots)
                    if found_name.is_none() {
                        if let Some(parent_id) = crate::game_path::GamePathFinder::find_parent_by_scanning_manifests(&steam_path_ref, &id_clone) {
                            let parent_name = {
                                if let Ok(c) = game_cache.lock() {
                                    c.get(&parent_id).cloned()
                                } else {
                                    None
                                }
                            };
                            
                            if let Some(p_name) = parent_name {
                                found_name = Some(format!("{} [Depot]", p_name));
                            } else {
                                found_name = Some(format!("Depot of AppID {}", parent_id));
                            }
                        }
                    }

                    // 5. DLC Auto-Link (Store API)
                    // This fixes "Standalone DLC" issues by finding the fullgame ID
                    if let Ok(Some(parent_id)) = client.get_details_parent(&id_clone).await {
                         if let Ok(mut map) = rel_map.lock() {
                             // Only link if not already linked (or orphan)
                             if !map.contains_key(&id_clone) {
                                 map.insert(id_clone.clone(), parent_id.clone());
                                 crate::app_list::save_relationships(".", &map);
                                 
                                 // If we found a parent, try to make the name nicer if it's still generic
                                 if found_name.is_none() || found_name.as_deref() == Some("Unknown") {
                                      found_name = Some(format!("DLC (Parent: {})", parent_id));
                                 }
                             }
                         }
                    }

                    // 3. Save if found
                    if let Some(name) = found_name {
                        if let Ok(mut cache) = game_cache.lock() {
                            cache.insert(id_clone.clone(), name.clone());
                            let _ = save_game_cache(&cache);
                        }
                    }
                }));
            }

            for h in handles {
                let _ = h.await;
            }
        });
        
        if let Ok(mut guard) = status_queue.lock() {
            *guard = Some("Resolution Complete.".to_string());
        }
    });
}

// NUOVO: Smart Update con Vault-first (Option C)
async fn smart_update_manifests(
    appid: &str,
    steam_path: &str,
    _api_client: &ApiClient,
    log: impl Fn(String) + Send + Sync + 'static,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Unified Logic: Use the shared helper which includes:
    // 1. Magic Byte Validation
    // 2. Vault Restoration
    // 3. Smart Skipping
    // 4. Robust Download
    match crate::ui::helpers::download_manifests_wudrm(appid, steam_path, &log) {
        Ok(count) => {
            if count > 0 {
                log(format!("✅ Successfully processed {} manifests.", count));
                Ok(())
            } else {
                log("⚠️ No valid manifests processed (Count 0).".to_string());
                Ok(())
            }
        }
        Err(e) => {
             log(format!("❌ Update failed: {}", e));
             Err(e)
        }
    }
}

pub fn check_updates_for_ids(app: &DarkCoreApp, ids: Vec<String>) {
    if ids.is_empty() { return; }
    let client_opt = app.api_client.clone();
    let cache_arc = app.update_cache.clone();
    let steam_path = app.config.steam_path.clone();
    let log_arc = app.system_log.clone();
    let updates_dl = app.updates_downloading.clone();

    std::thread::spawn(move || {
        let client = if let Some(c) = client_opt { c } else { return; };
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build() 
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        
        let mut handles = Vec::new();
        
        for appid in ids {
            let client = client.clone();
            let cache = cache_arc.clone();
            let sp = steam_path.clone();
            let log_clone = log_arc.clone();
            let updates_clone = updates_dl.clone();
            
            handles.push(rt.spawn(async move {
                // 1. Get Local BuildID & StateFlags
                let acf_path = Path::new(&sp).join("steamapps")
                    .join(format!("appmanifest_{}.acf", appid));
                
                let mut local_buildid = 0u64;
                let mut state_flags = 0u32;

                if acf_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&acf_path) {
                        // Parse StateFlags
                        if let Some(pos) = content.find("\"StateFlags\"") {
                             let remainder = &content[pos..];
                             if let Some(start_quote) = remainder.find("\"") {
                                 if let Some(end_label) = remainder[start_quote+1..].find("\"") {
                                      let val_part = &remainder[start_quote+1+end_label+1..];
                                      if let Some(v_start) = val_part.find("\"") {
                                          if let Some(v_end) = val_part[v_start+1..].find("\"") {
                                              let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                              state_flags = num_str.parse().unwrap_or(0);
                                          }
                                      }
                                 }
                             }
                        }
                        // Parse buildid
                        if let Some(pos) = content.find("\"buildid\"") {
                            let remainder = &content[pos..];
                            if let Some(start_quote) = remainder.find("\"") {
                                if let Some(end_label) = remainder[start_quote+1..].find("\"") {
                                     let val_part = &remainder[start_quote+1+end_label+1..];
                                     if let Some(v_start) = val_part.find("\"") {
                                         if let Some(v_end) = val_part[v_start+1..].find("\"") {
                                             let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                             local_buildid = num_str.parse().unwrap_or(0);
                                         }
                                     }
                                }
                            }
                        }
                    }
                }

                // 2. Always set UI cache to false (PLAY button, never UPDATE)
                if let Ok(mut c) = cache.lock() {
                    c.insert(appid.clone(), false);
                }

                // 3. If installed, check for buildid mismatch and trigger WUDRM
                let is_installed = (state_flags & 4) != 0;
                if is_installed && local_buildid > 0 {
                    // Get remote buildid from SteamCMD (FREE)
                    if let Ok(info) = client.get_app_info(&appid).await {
                        if let Some(remote_bid) = info.buildid {
                            if remote_bid > local_buildid {
                                // Update available!
                                if let Ok(mut logs) = log_clone.lock() {
                                    push_log(&mut logs, format!("ðŸ”„ Update detected for AppID {}. Refreshing manifests...", appid));
                                }
                                
                                // Mark as downloading
                                if let Ok(mut dl) = updates_clone.lock() {
                                    dl.insert(appid.clone());
                                }
                                
                                // Trigger Smart Update (Async)
                                let sp_ref = sp.clone();
                                let aid = appid.clone();
                                let log_ref = log_clone.clone();
                                
                                // Direct async await
                                if let Err(e) = smart_update_manifests(&aid, &sp_ref, &client, move |msg| {
                                     if let Ok(mut logs) = log_ref.lock() {
                                         push_log(&mut logs, msg);
                                     }
                                }).await {
                                     if let Ok(mut logs) = log_clone.lock() {
                                         push_log(&mut logs, format!("Update failed: {}", e));
                                     }
                                }
                                
                                // Remove from downloading set
                                if let Ok(mut dl) = updates_clone.lock() {
                                    dl.remove(&aid);
                                }
                            }
                        }
                    }
                }
            }));
        }
        
        rt.block_on(async {
            for h in handles { let _ = h.await; }
        });
    });
}

impl DarkCoreApp {
    /// Start background update check for all installed games
    pub(crate) fn start_watcher_check(&self) {
        let api_key = self.config.api_key.clone();
        let gl_path = self.config.gl_path.clone();
        let steam_path = self.config.steam_path.clone();
        let pending_arc = self.watcher_pending_updates.clone();
        let game_cache = self.game_cache.clone();
        let relationships = self.relationships.clone();
        let log_arc = self.system_log.clone();
        
        if api_key.is_empty() {
            // Log manually or via method if accessible
            // self.log cannot be called if it's not defined in this impl block
            // Use push_log directly
            if let Ok(mut logs) = log_arc.lock() {
                 push_log(&mut logs, "[Watcher] Skipped: No API key configured.".to_string());
            }
            return;
        }
        
        if let Ok(mut logs) = log_arc.lock() {
             push_log(&mut logs, "[Watcher] Starting update check...".to_string());
        }
        
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };
            
            // Get active games
            let (cache_snapshot, rels_snapshot) = {
                let cache = game_cache.lock().unwrap();
                let rels = relationships.lock().unwrap();
                (cache.clone(), rels.clone())
            };
            
            let games = crate::app_list::refresh_active_games_list(
                &gl_path,
                &steam_path,
                &cache_snapshot,
                &rels_snapshot,
            );
            
            if games.is_empty() {
                if let Ok(mut logs) = log_arc.lock() {
                    push_log(&mut logs, "[Watcher] No games to check.".to_string());
                }
                return;
            }
            
            let client = crate::api::ApiClient::new(api_key);
            let mut found_updates = 0;
            
            // Check each game
            // Check each game
            rt.block_on(async {
                for game in &games {
                    // Skip depots
                    if game.name.starts_with("Depot (") || game.name.contains("(Content)") {
                        continue;
                    }
                    
                    // Get remote build info
                    if let Ok(info) = client.get_app_info(&game.app_id).await {
                        let remote_build = info.buildid.unwrap_or(0);
                        
                        // Strategy 1: Compare BuildID from ACF (traditional games)
                        let mut local_build: u64 = 0;
                        
                        // Try ALL library folders, not just the main steam path
                        let all_libraries = crate::game_path::GamePathFinder::get_library_folders(&steam_path);
                        let mut acf_found = false;
                        
                        for lib in &all_libraries {
                            let acf_path = lib
                                .join("steamapps")
                                .join(format!("appmanifest_{}.acf", game.app_id));
                            
                            if acf_path.exists() {
                                acf_found = true;
                                if let Ok(content) = std::fs::read_to_string(&acf_path) {
                                    for line in content.lines() {
                                        if line.contains("\"buildid\"") {
                                            if let Some(start) = line.rfind('"') {
                                                if let Some(prev) = line[..start].rfind('"') {
                                                    if let Ok(b) = line[prev+1..start].parse::<u64>() {
                                                        local_build = b;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                break; // Found ACF, stop searching
                            }
                        }
                        
                        // Strategy 2: If no ACF or local_build=0, check depotcache manifests
                        // GreenLuma games often don't have ACF files.
                        // Compare: do we have manifests matching the CURRENT GIDs?
                        let mut manifests_outdated = false;
                        
                        if !acf_found || local_build == 0 {
                            let depot_cache = std::path::Path::new(&steam_path).join("depotcache");
                            if depot_cache.exists() {
                                for (depot_id, depot_info) in &info.depots {
                                    if let Some(gid) = &depot_info.gid {
                                        let expected = format!("{}_{}.manifest", depot_id, gid);
                                        let expected_path = depot_cache.join(&expected);
                                        
                                        if !expected_path.exists() {
                                            // We DON'T have the current manifest → needs update
                                            // But only flag if we have ANY manifest for this depot
                                            // (i.e., we had a previous version)
                                            let pattern = format!("{}_", depot_id);
                                            if let Ok(entries) = std::fs::read_dir(&depot_cache) {
                                                for entry in entries.flatten() {
                                                    let name = entry.file_name().to_string_lossy().to_string();
                                                    if name.starts_with(&pattern) && name.ends_with(".manifest") {
                                                        // Has OLD manifest but not current → outdated
                                                        manifests_outdated = true;
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Determine if update needed
                        let needs_update = if local_build > 0 && remote_build > 0 {
                            // Traditional: BuildID comparison
                            remote_build > local_build
                        } else {
                            // GreenLuma fallback: manifest GID comparison
                            manifests_outdated
                        };
                        
                        if needs_update {
                            found_updates += 1;
                            if let Ok(mut p) = pending_arc.lock() {
                                p.insert(
                                    game.app_id.clone(),
                                    (game.name.clone(), local_build, remote_build)
                                );
                            }
                        }
                    }
                    
                    // Rate limit
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            });
            
            if let Ok(mut logs) = log_arc.lock() {
                if found_updates > 0 {
                    push_log(&mut logs, format!("[Watcher] Found {} games with updates available!", found_updates));
                } else {
                    push_log(&mut logs, "[Watcher] All games are up to date.".to_string());
                }
            }
        });
    }
}

impl DarkCoreApp {
    pub(crate) fn resolve_unknown_games(&mut self) {
        resolve_unknown_games(self);
    }

    #[allow(dead_code)] // Reserved for selective update functionality
    pub(crate) fn check_updates_for_ids(&self, ids: Vec<String>) {
        check_updates_for_ids(self, ids);
    }
}
