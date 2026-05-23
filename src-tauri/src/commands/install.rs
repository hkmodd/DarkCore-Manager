use tauri::State;
use crate::state::AppState;
use crate::api::{GameHierarchy, SteamCmdInfo};

#[tauri::command]
pub async fn get_app_info(appid: String, state: State<'_, AppState>) -> Result<SteamCmdInfo, String> {
    let client = {
        let guard = state.api_client.lock().unwrap();
        guard.clone()
    };
    
    if let Some(client) = client {
        client.get_app_info(&appid).await.map_err(|e| e.to_string())
    } else {
        Err("API Client not initialized".to_string())
    }
}

#[tauri::command]
pub async fn fetch_hierarchy(appid: String, state: State<'_, AppState>) -> Result<GameHierarchy, String> {
    let (client, target_language) = {
        let guard = state.api_client.lock().unwrap();
        let config = state.config_manager.config.lock().unwrap();
        (guard.clone(), config.target_language.clone())
    };

    if let Some(client) = client {
        client.fetch_full_hierarchy(&appid, &target_language).await.map_err(|e| e.to_string())
    } else {
        Err("API Client not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_library_folders(state: State<'_, AppState>) -> Vec<String> {
    let libs = crate::game_path::GamePathFinder::get_library_folders(&state.config_manager.config.lock().unwrap().steam_path);
    // Convert PathBuf to String
    libs.into_iter().map(|p| p.to_string_lossy().to_string()).collect()
}

#[tauri::command]
pub fn detect_install_path(_appid: String, name: String, library: String) -> String {
    // Basic implementation: library/steamapps/common/Name
    let lib_path = std::path::Path::new(&library);
    let common = lib_path.join("steamapps").join("common").join(name);
    common.to_string_lossy().to_string()
}

#[tauri::command]
pub async fn download_manifest(
    state: State<'_, AppState>,
    depot_id: String,
    manifest_gid: String,
    output_path: String,
) -> Result<String, String> {
    let target_path = if output_path == "." || output_path.is_empty() {
        let config = state.config_manager.config.lock().unwrap();
        std::path::PathBuf::from(&config.steam_path).join("depotcache")
    } else {
        std::path::PathBuf::from(&output_path)
    };

    // Ensure directory exists
    if !target_path.exists() {
        let _ = std::fs::create_dir_all(&target_path);
    }

    state.downloader.download_manifest(&depot_id, &manifest_gid, &target_path)
        .await
        .map_err(|e| e.to_string())
        .map(|p| p.path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn parse_lua_script(path: String) -> Result<crate::utils::lua_parser::ScriptData, String> {
    let p = std::path::Path::new(&path);
    crate::utils::lua_parser::parse_file(p).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_godmode(
    state: State<'_, AppState>,
    appid: String,
    include_dlcs: bool,
) -> Result<(), String> {
    
    // 1. Persist to Config
    {
        let mut config_mgr = state.config_manager.config.lock().unwrap();
        if !config_mgr.family_godmode_ids.contains(&appid) {
            config_mgr.family_godmode_ids.push(appid.clone());
            // Drop lock to save
        }
    }
    state.config_manager.save().map_err(|e| e.to_string())?;

    // 2. Build ID List (Start with AppID)
    let mut ids = vec![appid.clone()];

    // 3. Fetch DLCs if requested
    if include_dlcs {
        let client = {
            let guard = state.api_client.lock().unwrap();
            guard.clone()
        };
        
        // Use client if available, or fetch from Store API directly
        if let Some(client) = client {
             if let Ok(dlcs) = client.get_dlc_list(&appid).await {
                 ids.extend(dlcs);
             }
        } 
        // Fallback: Store API via reqwest would go here if client is missing, 
        // but for now we assume client exists or we skip DLCs to be safe.
        // Legacy code strictly used client or direct reqwest.
    }

    // 4. Update AppList
    let gl_path = {
        let config = state.config_manager.config.lock().unwrap();
        std::path::PathBuf::from(&config.gl_path)
    };

    let am = crate::app_list::AppListManager::new(&gl_path, &std::path::Path::new("")); // steam_path irrelevant for write
    am.add_games_to_list(ids).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn resolve_install_ids(
    state: State<'_, AppState>,
    appid: String,
    selected_dlcs: Vec<String>,
) -> Result<Vec<String>, String> {
    // 1. Fetch Hierarchy (Source of Truth)
    let client = {
        let guard = state.api_client.lock().unwrap();
        guard.clone()
    };

    let hierarchy = if let Some(client) = client {
        client.fetch_full_hierarchy(&appid, "english").await.map_err(|e| e.to_string())?
    } else {
        return Err("API Client not initialized".to_string());
    };

    // 2. Legacy Logic Implementation: resolve_mandatory_depots
    let mut ids = Vec::new();

    // 2.1 Always include Root AppID
    ids.push(hierarchy.root_id.clone());

    // 2.2 Include Base Depots (Exclude 228980/228989)
    for depot in hierarchy.base_depots {
        if depot.depot_id != "228980" && depot.depot_id != "228989" {
            ids.push(depot.depot_id);
        }
    }

    // 2.3 Include Selected DLCs AND their Depots
    for dlc in hierarchy.dlcs {
        if selected_dlcs.contains(&dlc.app_id) {
            ids.push(dlc.app_id.clone());
            for depot in dlc.depots {
                if depot.depot_id != "228980" && depot.depot_id != "228989" {
                    ids.push(depot.depot_id);
                }
            }
        }
    }

    // 2.4 Sort and Dedup
    ids.sort();
    ids.dedup();

    Ok(ids)
}

#[tauri::command]
pub fn trigger_steam_install(appid: String) -> Result<(), String> {
    let url = format!("steam://install/{}", appid);
    open::that(&url).map_err(|e| e.to_string())
}
