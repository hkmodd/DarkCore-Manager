use crate::app_list::GameProfile;
use crate::profiles::Profile;
use crate::state::AppState;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tauri::{State, Manager, Emitter};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteResult {
    pub backed_up: usize,
    pub children_removed: Vec<String>,
    pub vdf_keys_removed: usize,
    pub files_deleted: bool,
}

#[tauri::command]
pub async fn get_active_games(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<Vec<GameProfile>, String> {
    let (profiles, unknowns) = {
        let app_list = state.app_list.lock().unwrap();
        let cache = state.name_cache.lock().unwrap();
        let family_ids = state.config_manager.config.lock().unwrap().family_godmode_ids.clone();
        let pending = state.watcher_pending.lock().unwrap();
        let profiles = app_list.refresh_active_games_list(&cache, &family_ids, &pending);
        
        let unknowns: Vec<String> = profiles
            .iter()
            .filter(|p| {
                p.name == "Unknown" || 
                p.name.starts_with("Unknown App") ||
                p.name.ends_with("(Content)") ||
                p.name.starts_with("AppID ") ||
                p.name.starts_with("Depot (")
            })
            .map(|p| p.app_id.clone())
            .collect();
            
        (profiles, unknowns)
    };

    if !unknowns.is_empty() {
        let handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let state = handle.state::<AppState>();
            let client = {
                let guard = state.api_client.lock().unwrap();
                if let Some(c) = &*guard {
                    c.clone()
                } else {
                    // Create minimal public client for public store info
                    crate::api::ApiClient::new("".to_string())
                }
            };
// ... (rest of async block unchanged)
            let mut updates = HashMap::new();
            for appid in unknowns {
                // SteamCMD API: Works for ALL types (games, DLCs, depots)
                // Unlike Steam Store API which fails for depots/DLCs without store pages
                if let Ok(name) = client.get_app_name(&appid).await {
                    if !name.is_empty() {
                        updates.insert(appid.clone(), name);
                    }
                }
                // Small delay to avoid rate limiting SteamCMD API
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            }

            if !updates.is_empty() {
                {
                    let mut cache = state.name_cache.lock().unwrap();
                    for (k, v) in &updates {
                        cache.insert(k.clone(), v.clone());
                    }
                    let _ = crate::cache::save_game_cache(&cache);
                }
                // Emit event to refresh UI
                let _ = handle.emit("library-update", ());
            }
        });
    }

    Ok(profiles)
}

// ...

#[tauri::command]
pub fn scan_delete_children(
    state: State<'_, AppState>,
    parent_id: String,
) -> Vec<(String, String)> {
    let app_list = state.app_list.lock().unwrap();
    let cache = state.name_cache.lock().unwrap();
    let profiles = app_list.refresh_active_games_list(&cache, &[], &HashMap::new());

    let mut children = Vec::new();

    for profile in &profiles {
        if let Some(pid) = &profile.parent_id {
            if pid == &parent_id {
                children.push((profile.app_id.clone(), profile.name.clone()));
            }
        }
    }

    children
}

#[tauri::command]
pub async fn repair_library_relationships(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    // 1. Identify "Root" candidates (Synchronous Block)
    // We strictly limit the scope of the lock to this block
    let root_candidates = {
        let app_list = state.app_list.lock().unwrap();
        let cache = state.name_cache.lock().unwrap();
        // refresh_active_games_list requires a &HashMap for pending updates, just pass empty
        // We don't need real pending updates for this logic
        let profiles = app_list.refresh_active_games_list(&cache, &[], &HashMap::new());
        
        let mut candidates = Vec::new();
        for p in &profiles {
            // Only consider installed/injected games that don't already have a parent
            if p.parent_id.is_some() { continue; }
            if p.is_installed || p.app_id.chars().all(char::is_numeric) {
                candidates.push(p.app_id.clone());
            }
        }
        candidates.sort();
        candidates.dedup();
        candidates
    }; // app_list and cache locks are DROPPED here. 
       // 'root_candidates' is a Vec<String>, which is Send.

    if root_candidates.is_empty() {
        return Ok("No candidate games found to repair.".to_string());
    }

    // 2. Client Extraction (Synchronous Block)
    let client = {
        let guard = state.api_client.lock().unwrap();
        if let Some(c) = &*guard {
            c.clone()
        } else {
             return Err("API Client not initialized (Missing Key).".to_string());
        }
    }; // Lock dropped. Client is Send.

    // 3. Async Scanning Phase (No Mutexes held)
    let mut relationships_updates = HashMap::new();
    let mut processed_count = 0;

    for appid in root_candidates {
        // Emit non-blocking event
        let _ = app_handle.emit("library-update", ());

        if let Ok(hierarchy) = client.fetch_full_hierarchy(&appid, "english").await {
             let parent = hierarchy.root_id.clone();
             
             for depot in hierarchy.base_depots {
                 if depot.depot_id != parent {
                     relationships_updates.insert(depot.depot_id, parent.clone());
                 }
             }

             for dlc in hierarchy.dlcs {
                 relationships_updates.insert(dlc.app_id.clone(), parent.clone());
                 for depot in dlc.depots {
                     relationships_updates.insert(depot.depot_id, parent.clone());
                 }
             }
             processed_count += 1;
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // 4. Save Changes (Synchronous Block)
    if !relationships_updates.is_empty() {
        let app_list = state.app_list.lock().unwrap();
        let mut current_rels = app_list.load_relationships();
        let old_len = current_rels.len();
        
        for (child, parent) in relationships_updates {
            current_rels.insert(child, parent);
        }
        
        app_list.save_relationships(&current_rels);
        let new_len = current_rels.len();
        
        Ok(format!("Repaired relationships for {} games. Added {} links.", processed_count, new_len - old_len))
    } else {
        Ok(format!("Scanned {} games, no new relationships found.", processed_count))
    }
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    state.profile_manager.lock().unwrap().delete_profile(&name)
}

#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, name: String, app_ids: Vec<String>) -> Result<(), String> {
    let profile = Profile { name: name.clone(), app_ids };
    state.profile_manager.lock().unwrap().save_profile(&profile)
}

#[tauri::command]
pub fn load_profile(state: State<'_, AppState>, name: String) -> Result<Profile, String> {
    state.profile_manager.lock().unwrap().load_profile(&name)
}

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Vec<String> {
    state.profile_manager.lock().unwrap().list_profiles()
}

#[tauri::command]
pub fn create_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let profile = Profile { name, app_ids: vec![] };
    state.profile_manager.lock().unwrap().save_profile(&profile)
}

#[tauri::command]
pub fn reorder_list(state: State<'_, AppState>) -> Result<(), String> {
    let app_list = state.app_list.lock().unwrap();
    let cache = state.name_cache.lock().unwrap();
    app_list.nuke_reorder(None, Some(&cache)).map_err(|e: std::io::Error| e.to_string())
}

#[tauri::command]
pub fn remove_game_from_applist(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let app_list = state.app_list.lock().unwrap();
    app_list.remove_games_from_list(vec![app_id]).map_err(|e: std::io::Error| e.to_string())
}

#[tauri::command]
pub fn add_games_to_list(state: State<'_, AppState>, app_ids: Vec<String>) -> Result<(), String> {
    let app_list = state.app_list.lock().unwrap();
    app_list.add_games_to_list(app_ids).map_err(|e: std::io::Error| e.to_string())
}

#[tauri::command]
pub fn update_name_cache(state: State<'_, AppState>, app_id: String, name: String) {
    let mut cache = state.name_cache.lock().unwrap();
    cache.insert(app_id, name);
    let _ = crate::cache::save_game_cache(&cache);
}

#[tauri::command]
pub fn inject_vdf_keys(state: State<'_, AppState>) -> Result<String, String> {
    let injector = state.vdf_injector.lock().unwrap();
    
    // Scan GreenLuma directory for .lua files to recover keys
    let gl_path = state.config_manager.config.lock().unwrap().gl_path.clone();
    let gl_dir = std::path::Path::new(&gl_path);
    
    if !gl_dir.exists() {
        return Err("GreenLuma directory not found".to_string());
    }
    
    let mut all_keys = std::collections::HashMap::new();
    let mut scanned_files = 0;
    
    if let Ok(entries) = std::fs::read_dir(gl_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                 if let Ok(content) = std::fs::read_to_string(&path) {
                     let (_, keys) = crate::vdf_injector::parse_lua_for_keys(&content);
                     all_keys.extend(keys);
                     scanned_files += 1;
                 }
            }
        }
    }
    
    if all_keys.is_empty() {
        return Ok(format!("Scanned {} scripts, no keys found.", scanned_files));
    }
    
    match injector.inject_vdf(&all_keys) {
        Ok(count) => Ok(format!("Injected {} keys from {} scripts.", count, scanned_files)),
        Err(e) => Err(format!("Injection failed: {}", e))
    }
}

#[tauri::command]
pub fn import_legacy_applist(state: State<'_, AppState>) -> Result<usize, String> {
     let steam_path = state.config_manager.config.lock().unwrap().steam_path.clone();
     let legacy_path = std::path::Path::new(&steam_path).join("AppList");
     if !legacy_path.exists() {
         return Err("Legacy AppList folder not found".to_string());
     }
     
     let mut ids_to_add = Vec::new();
     
     if let Ok(entries) = std::fs::read_dir(legacy_path) {
         for entry in entries.flatten() {
             if let Some(name) = entry.file_name().to_str() {
                 if name.ends_with(".txt") && name.chars().any(char::is_numeric) {
                     let appid = name.trim_end_matches(".txt").to_string();
                     ids_to_add.push(appid);
                 }
             }
         }
     }
     
     let count = ids_to_add.len();
     if count > 0 {
        let app_list = state.app_list.lock().unwrap();
        app_list.add_games_to_list(ids_to_add).map_err(|e: std::io::Error| e.to_string())?;
     }
     
     Ok(count)
}

#[tauri::command]
pub async fn full_delete_game(state: State<'_, AppState>, app_id: String) -> Result<DeleteResult, String> {
    let mut result = DeleteResult {
        backed_up: 0,
        children_removed: vec![],
        vdf_keys_removed: 0,
        files_deleted: false
    };

    let config = state.config_manager.config.lock().unwrap().clone();
    let steam_path = config.steam_path.clone();
    
    // 1. Identify Target & Children
    let (to_remove, types_map) = {
        let app_list = state.app_list.lock().unwrap();
        let rels = app_list.load_relationships();
        let types = app_list.load_types();
        
        let mut targets = vec![app_id.clone()];
        
        // Find children
        for (child, parent) in &rels {
            if parent == &app_id {
                targets.push(child.clone());
            }
        }
        
        // Find associated depots from types (if any are not explicitly children but related?)
        // Mostly children covers it if healing worked.
        
        targets.sort();
        targets.dedup();
        (targets, types)
    };
    
    // 2. VAULT BACKUP
    let vault = crate::vault::VaultManager::new_local();
    let _ = vault.ensure_structure(&app_id); // Initialize vault folder

    // A. Backup ACF
    // Use find_manifest_path to locate the ACF, then derive library path from it
    if let Some(acf_path) = crate::game_path::GamePathFinder::find_manifest_path(&steam_path, &app_id) {
        if acf_path.exists() {
            // Copy to Vault
            let vault_acf = vault.get_storage_dir(&app_id).join(format!("appmanifest_{}.acf", app_id));
            if std::fs::copy(&acf_path, &vault_acf).is_ok() {
                result.backed_up += 1;
            }
        }
    }

    // B. Backup Manifests (DepotCache)
    let depot_cache = std::path::Path::new(&steam_path).join("depotcache");
    if depot_cache.exists() {
        for target in &to_remove {
            // Is this target a depot?
            let _is_depot = types_map.get(target).map(|t| t == "depot").unwrap_or(false);
            
            // Or just try to find manifests for ALL targets (Game, DLC, Depot)
            // Pattern: {ID}_*.manifest
            let pattern = format!("{}_", target);
            if let Ok(entries) = std::fs::read_dir(&depot_cache) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(&pattern) && name.ends_with(".manifest") {
                         let src = entry.path();
                         if let Ok(_) = vault.store_manifest(&app_id, &src) { // Store under MAIN AppID
                             result.backed_up += 1;
                         }
                    }
                }
            }
        }
    }

    // 3. Remove from AppList
    if let Ok(app_list) = state.app_list.lock() {
        if let Err(e) = app_list.remove_games_from_list(to_remove.clone()) {
            return Err(format!("AppList Removal Error: {}", e));
        }
        
        // 4. Cleanup Relationships & Types
        let mut rels = app_list.load_relationships();
        let mut types = app_list.load_types();
        let mut changed = false;
        
        for id in &to_remove {
            if rels.remove(id).is_some() { changed = true; }
            if types.remove(id).is_some() { changed = true; }
        }
        
        if changed {
            app_list.save_relationships(&rels);
            app_list.save_types(&types);
        }
    }
    
    result.children_removed = to_remove;
    // Status update logic? emit?
    
    Ok(result)
}

#[tauri::command]
pub fn check_legacy_exists(state: State<'_, AppState>) -> bool {
    let steam_path = state.config_manager.config.lock().unwrap().steam_path.clone();
    let legacy_dir = std::path::Path::new(&steam_path).join("AppList");
    legacy_dir.exists()
}
