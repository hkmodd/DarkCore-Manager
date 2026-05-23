use tauri::{State, Manager, Emitter};
use crate::state::AppState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::process::Command;

#[tauri::command]
pub async fn run_startup_scan(_state: State<'_, AppState>) -> Result<(), String> {
    // Startup scan logic moved to generic checker or kept simple
    // For now, let's just trigger the generic check logic via an emit or similar if needed.
    // The Watcher service usually handles this.
    Ok(())
}

#[tauri::command]
pub async fn check_updates_cmd(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let games = {
        let cache = state.name_cache.lock().unwrap();
        state.app_list.lock().unwrap().refresh_active_games_list(&cache, &[], &std::collections::HashMap::new())
    };
    let steam_path = state.config_manager.get().steam_path.clone();
    
    // Clear pending
    {
        let mut pending_lock = state.watcher_pending.lock().unwrap();
        pending_lock.clear();
    }

    let mut updates_found = HashMap::new();
    let mut checked_ids = std::collections::HashSet::new();

    // Correction Protocol: Scanner Logic
    // Scan ACFs for StateFlags: 6, 1026, 1042, 1062
    // Also check standard BuildID mismatch
    
    let library_folders = crate::utils::vdf::get_all_library_folders(&steam_path);

    for game in &games {
        if checked_ids.contains(&game.app_id) { continue; }
        checked_ids.insert(game.app_id.clone());

        // Skip Depots/DLCs from top-level check (we check parent ACF)
        if game.parent_id.is_some() { continue; }

        let mut acf_path = PathBuf::new();

        let mut found = false;

        // Find ACF
        for lib in &library_folders {
             let candidate = lib.join("steamapps").join(format!("appmanifest_{}.acf", game.app_id));
             if candidate.exists() {
                 acf_path = candidate;
                 found = true;
                 break;
             }
        }

        let mut needs_update = false;
        let mut reason = String::new();
        if found {
             if let Ok(content) = std::fs::read_to_string(&acf_path) {
                 if let Some(parsed) = crate::utils::vdf::VdfParser::parse(&content) {
                     if let Some(app_node) = parsed.find_key("AppState") {
                         // Check StateFlags
                         if let Some(f_val) = app_node.find_key("StateFlags") {
                             if let Some(s) = f_val.get_str() {
                                 if let Ok(flags) = s.parse::<u32>() {
                                     // Check specific flags
                                     // 6 (Installed + UpdateRequired)
                                     // 1026 (Update Required + ?)
                                     // 1042, 1062
                                     if flags == 6 || flags == 1026 || flags == 1042 || flags == 1062 || (flags & 2) != 0 {
                                         needs_update = true;
                                         reason = format!("StateFlags: {}", flags);
                                     }
                                 }
                             }
                         }
                     }
                 }
             }
        }

        // If not flagged by StateFlags, check BuildID if we have API access
        if !needs_update {
             // Optional: Classic BuildID check
             // For Correction Protocol, StateFlags is priority.
        }

        if needs_update {
             updates_found.insert(game.app_id.clone(), reason.clone());
             let mut pending = state.watcher_pending.lock().unwrap();
             pending.insert(game.app_id.clone(), (reason, 0, 0));
        }
    }

    Ok(updates_found)
}

/// Correction Protocol: Update Execution Workflow
/// 1. Kill Steam
/// 2. Download Encrypted Blob
/// 3. Decrypt
/// 4. Write Manifest
/// 5. Mutate ACF
#[tauri::command]
pub async fn update_game_manifests(
    app_id: String,
    state: State<'_, AppState>
) -> Result<String, String> {
    let steam_path = state.config_manager.get().steam_path.clone();
    let api_client_opt = state.api_client.lock().unwrap().clone();
    
    if api_client_opt.is_none() {
        return Err("API unreachable".to_string());
    }
    let api = api_client_opt.unwrap();

    // 1. Kill Steam
    // Terminazione brutale
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill").args(&["/F", "/IM", "steam.exe"]).output();
        // Wait a bit
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    
    // 2. Prepare for Download
    let depot_cache = Path::new(&steam_path).join("depotcache");
    if !depot_cache.exists() { let _ = std::fs::create_dir_all(&depot_cache); }
    
    let gids = api.get_public_gids(&app_id).await.map_err(|e| e.to_string())?;
    
    // Get Keys from Vault (Lua) or Config
    // We need keys to decrypt!
    let vault = crate::vault::VaultManager::new_local();
    
    // Helper to get ALL keys (Base + DLCs)
    let mut keys_map = HashMap::new();
    if let Ok(lua_bytes) = vault.get_lua(&app_id) {
        let content = String::from_utf8_lossy(&lua_bytes);
        let (_, keys) = crate::vdf_injector::parse_lua_for_keys(&content);
        keys_map = keys;
    }
    
    let http = reqwest::Client::new();
    let mut success_count = 0;
    
    // Process Depots
    for (depot_id, gid) in &gids {
        // Find key
        let key_hex = keys_map.get(depot_id).cloned().unwrap_or_default();
        if key_hex.is_empty() {
            println!("Skipping depot {} - No Key Found", depot_id);
            continue;
        }
        
        // Hex decode key
        let key_bytes = match hex::decode(&key_hex) {
            Ok(b) => {
                if b.len() != 32 { continue; }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            },
            Err(_) => continue,
        };

        // 3. Download Encrypted Manifest
        // Using a hardcoded reliable steam CDN for the correction protocol
        let url = format!("http://valve.vo.llnwd.net/steampowered/depot/{}/manifest/{}/5", depot_id, gid);
        
        // 4. Download
        let resp = http.get(&url).send().await;
        if let Ok(res) = resp {
             if res.status().is_success() {
                 if let Ok(encrypted_blob) = res.bytes().await {
                     // 5. Decrypt
                     match crate::utils::crypto::decrypt_and_decompress(&encrypted_blob, &key_bytes) {
                         Ok(decrypted) => {
                             // Write to depotcache
                             // Check Magic
                             if decrypted.len() > 4 {
                                 // Little Endian Magic: D0 17 F6 71
                                 if decrypted[0] == 0xD0 && decrypted[1] == 0x17 && decrypted[2] == 0xF6 && decrypted[3] == 0x71 {
                                     let filename = format!("{}_{}.manifest", depot_id, gid);
                                     let _ = std::fs::write(depot_cache.join(&filename), &decrypted);
                                     success_count += 1;
                                 }
                             }
                         },
                         Err(e) => println!("Decrypt failed for {}: {}", depot_id, e),
                     }
                 }
             }
        }
    }
    
    // 6. Mutate ACF
    // Scan all ACFs, find the one with this AppID
    // Parse it, find "InstalledDepots", update "manifest" ID for matching Depots
    // Force StateFlags to "4"
    
    let library_folders = crate::utils::vdf::get_all_library_folders(&steam_path);
    for lib in &library_folders {
        let acf_path = lib.join("steamapps").join(format!("appmanifest_{}.acf", app_id));
        if acf_path.exists() {
             let raw = std::fs::read_to_string(&acf_path).unwrap_or_default();
             // Simple String Replacement for safety?
             // Or VDF Parser? Parser is safer.
             if let Some(mut root) = crate::utils::vdf::VdfParser::parse(&raw) {
                 if let Some(app_state) = root.get_mut("AppState") {
                     // Update StateFlags
                     if let Some(flags) = app_state.get_mut("StateFlags") {
                         if let crate::utils::vdf::VdfValue::Str(s) = flags {
                             *s = "4".to_string();
                         }
                     }
                     
                     // Update InstalledDepots
                     if let Some(installed) = app_state.get_mut("InstalledDepots") {
                         if let crate::utils::vdf::VdfValue::Obj(depots_entries) = installed {
                             for (did, gid) in &gids {
                                 // Find entry in vector
                                 if let Some((_, entry_val)) = depots_entries.iter_mut().find(|(k, _)| k == did) {
                                     if let Some(manifest_val) = entry_val.get_mut("manifest") {
                                         if let crate::utils::vdf::VdfValue::Str(s) = manifest_val {
                                             *s = gid.clone();
                                         }
                                     }
                                 }
                             }
                         }
                     }
                 }
                 // Write back
                 let new_content = crate::utils::vdf::VdfParser::serialize(&root);
                 let _ = std::fs::write(acf_path, new_content);
             }
             break;
        }
    }

    Ok(format!("Correction Complete. Updated {} manifests.", success_count))
}

#[tauri::command]
pub async fn scan_for_updates_async(app_handle: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        
        let games = {
            let cache = state.name_cache.lock().unwrap();
            state.app_list.lock().unwrap().refresh_active_games_list(&cache, &[], &std::collections::HashMap::new())
        };
        let steam_path = state.config_manager.get().steam_path.clone();
        
        // Clone client to use in thread
        let api_client = {
            let guard = state.api_client.lock().unwrap();
            guard.clone()
        };

        if let Some(api) = api_client {
            // Clear pending updates first
            {
                let mut pending_lock = state.watcher_pending.lock().unwrap();
                pending_lock.clear();
            }

            let mut checked_ids = std::collections::HashSet::new();
            let mut found_any = false;

            for game in &games {
                // Skip depots
                if game.name.starts_with("Depot (") || game.name.contains("(Content)") {
                    continue;
                }
                // Skip children (DLCs checked via parent)
                if game.parent_id.is_some() { continue; }
                // Skip duplicates
                if checked_ids.contains(&game.app_id) { continue; }
                checked_ids.insert(game.app_id.clone());

// ─── HEALER LOGIC v3: Fix Missing Relationships & Types ────────────────
                let mut data_changed = false;
                
                // Only heal Root games (parents)
                if game.parent_id.is_none() {
                     let needs_healing = {
                         let app_list = state.app_list.lock().unwrap();
                         let rels = app_list.load_relationships();
                         let types = app_list.load_types();
                         
                         // 1. Does it have any known children?
                         let has_children = rels.values().any(|p| p == &game.app_id);
                         
                         // 2. Do known children have types?
                         let missing_types = rels.iter()
                             .filter(|(_, p)| p == &&game.app_id)
                             .any(|(c, _)| !types.contains_key(c));

                         // Heuristic: If no children known OR children exist but lack types -> Heal
                         !has_children || missing_types
                     };

                     if needs_healing {
                         println!("[Healer] Fetching hierarchy for {} ({})", game.name, game.app_id);
                         if let Ok(hierarchy) = api.fetch_full_hierarchy(&game.app_id, "english").await {
                             // Update Data
                             let app_list = state.app_list.lock().unwrap();
                             let mut rels = app_list.load_relationships();
                             let mut types = app_list.load_types();
                             
                             // Add Base Game Type
                             types.insert(game.app_id.clone(), "game".to_string());
                             
                             // Add DLCs
                             for dlc in &hierarchy.dlcs {
                                 rels.insert(dlc.app_id.clone(), game.app_id.clone());
                                 types.insert(dlc.app_id.clone(), "dlc".to_string());
                             }
                             
                             // Add Depots (Both Base & DLC depots ideally, but hierarchy puts them under respective nodes)
                             for depot in hierarchy.base_depots {
                                 rels.insert(depot.depot_id.clone(), game.app_id.clone());
                                 // Distinguish Base vs DLC depots
                                 let type_str = if depot.is_dlc_depot { "depot_dlc" } else { "depot_base" };
                                 types.insert(depot.depot_id.clone(), type_str.to_string());
                             }
                             
                             // Add DLC Depots
                             for dlc in &hierarchy.dlcs {
                                 for depot in &dlc.depots {
                                     rels.insert(depot.depot_id.clone(), game.app_id.clone());
                                     types.insert(depot.depot_id.clone(), "depot_dlc".to_string());
                                 }
                             }
                             
                             // Save
                             app_list.save_relationships(&rels);
                             app_list.save_types(&types);
                             data_changed = true;
                             println!("[Healer] Healed {}! Saved {} types.", game.name, types.len());
                         }
                     }
                }

                if data_changed {
                     let _ = app_handle.emit("library-update", ());
                }
                // ──────────────────────────────────────────────────────────────────────

                // Fetch remote build info from SteamCMD API (FREE)
                let info = match api.get_app_info(&game.app_id).await {
                    Ok(i) => i,
                    Err(e) => {
                        println!("[Watcher] Failed to get info for {}: {}", game.app_id, e);
                        continue;
                    }
                };
                let remote_build = info.buildid.unwrap_or(0);

                let mut local_build: u64 = 0;
                let all_libraries = crate::game_path::GamePathFinder::get_library_folders(&steam_path);
                let mut acf_found = false;

                for lib in &all_libraries {
                    let acf_path = lib
                        .join("steamapps")
                        .join(format!("appmanifest_{}.acf", game.app_id));

                    if acf_path.exists() {
                        acf_found = true;
                        if let Ok(content) = std::fs::read_to_string(&acf_path) {
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
                        }
                        break;
                    }
                }

                let mut needs_update = if local_build > 0 && remote_build > 0 {
                    remote_build > local_build
                } else {
                    false
                };

                if !needs_update && (!acf_found || local_build == 0) {
                    let depot_cache = Path::new(&steam_path).join("depotcache");
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
                                            if name.starts_with(&pattern) && name.ends_with(".manifest") {
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
                    let status_msg = if local_build > 0 {
                        format!("Build {} → {}", local_build, remote_build)
                    } else {
                        "Manifest outdated".to_string()
                    };

                    {
                        let mut pending_lock = state.watcher_pending.lock().unwrap();
                        pending_lock.insert(game.app_id.clone(), (status_msg, local_build, remote_build));
                    }
                    found_any = true;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            if found_any {
                 let _ = app_handle.emit("library-update", ());
            }
        }
    });

    Ok(())
}




