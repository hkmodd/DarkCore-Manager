use crate::state::AppState;
use crate::utils::lua_parser::{parse_content, ScriptData};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use tauri::State;
use zip::ZipArchive;

#[derive(serde::Serialize)]
pub struct ImportMetadata {
    pub script_data: Option<ScriptData>,
    pub manifest_count: usize,
    pub depot_count: usize,
    pub file_path: String,
}

#[tauri::command]
pub async fn scan_zip_for_import(path: String) -> Result<ImportMetadata, String> {
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut script_data: Option<ScriptData> = None;
    let mut manifest_count = 0;

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let name = file.name().to_string();

            // Check for Scripts
            if name.ends_with(".lua") || name.ends_with(".txt") {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(data) = parse_content(&content) {
                        // If we already have data, maybe merge or just keep first?
                        // Legacy keeps the last one or first valid one.
                        if script_data.is_none() {
                            script_data = Some(data);
                        }
                    }
                }
            } else if name.ends_with(".manifest") {
                manifest_count += 1;
            }
        }
    }

    let depot_count = script_data.as_ref().map(|d| d.depots.len()).unwrap_or(0);

    Ok(ImportMetadata {
        script_data,
        manifest_count,
        depot_count,
        file_path: path,
    })
}

#[tauri::command]
pub async fn import_zip_action(
    state: State<'_, AppState>,
    path: String,
    method: String, // "steam" or "direct"
) -> Result<String, String> {
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    
    // re-scan to get AppID (inefficient but safe)
    // Actually the caller should pass AppID? 
    // No, for "direct" we invoke scan again or just trust the zip content.
    // Let's re-scan quickly to find the script and manifests.
    
    let mut script_data: Option<ScriptData> = None;
    let mut manifests = HashMap::new(); // depot_id -> bytes

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let name = file.name().to_string();
            if name.ends_with(".lua") || name.ends_with(".txt") {
                if script_data.is_none() {
                    let mut content = String::new();
                    if file.read_to_string(&mut content).is_ok() {
                        if let Ok(data) = parse_content(&content) {
                            script_data = Some(data);
                        }
                    }
                }
            } else if name.ends_with(".manifest") {
                // Parse depot_id from filename {depot_id}_{gid}.manifest
                let basename = std::path::Path::new(&name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&name);
                if let Some(depot_id_str) = basename.split('_').next() {
                    if let Ok(depot_id) = depot_id_str.parse::<u32>() {
                        let mut bytes = Vec::new();
                        if file.read_to_end(&mut bytes).is_ok() {
                            manifests.insert(depot_id, bytes);
                        }
                    }
                }
            }
        }
    }

    let data = script_data.ok_or("No valid script found in ZIP")?;
    let app_id = data.app_id.ok_or("Script does not match any AppID")?;
    
    // 1. Add to AppList
    let dlcs: Vec<String> = data.dlcs.iter().map(|d| d.app_id.to_string()).collect();
    let mut ids_to_add = vec![app_id.to_string()];
    ids_to_add.extend(dlcs);
    
    let ids_count = ids_to_add.len();
    {
        let app_list = state.app_list.lock().unwrap();
        app_list.add_games_to_list(ids_to_add).map_err(|e| e.to_string())?;

        // CRITICAL: Save Relationships for Tree View
        if !data.dlcs.is_empty() {
             let mut rels = app_list.load_relationships();
             for dlc in &data.dlcs {
                 rels.insert(dlc.app_id.to_string(), app_id.to_string());
             }
             app_list.save_relationships(&rels);
        }
    }

    // 2. Inject Keys (CRITICAL PARITY FIX)
    if !data.depot_keys.is_empty() {
        let config = state.config_manager.config.lock().map_err(|e| e.to_string())?;
        let steam_path = PathBuf::from(&config.steam_path);
        // Helper to convert HashMap<u32, String> to what vdf expects
        // Actually vdf::inject_keys_into_config expects exactly that.
        crate::utils::vdf::inject_keys_into_config(&steam_path, &data.depot_keys)
            .map_err(|e| format!("Key Injection Failed: {}", e))?;
    }
    
    if method == "steam" {
        // Trigger Steam Install
        crate::commands::install::trigger_steam_install(app_id.to_string())?;
        return Ok(format!("Added {} games to AppList, Injected Keys, and triggered Steam install for {}", ids_count, app_id));
    }
    
    // Direct Action: Extract manifests
    let config = state.config_manager.config.lock().map_err(|e| e.to_string())?;
    let steam_path = PathBuf::from(&config.steam_path);
    let depot_cache = steam_path.join("depotcache");
    
    if !depot_cache.exists() {
        std::fs::create_dir_all(&depot_cache).map_err(|e| e.to_string())?;
    }
    
    drop(config); 
    
    // Re-open archive for extraction
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut count = 0;

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let name = file.name().to_string();
            if name.ends_with(".manifest") {
                 if let Some(filename) = std::path::Path::new(&name).file_name() {
                     let dest_path = depot_cache.join(filename);
                     let mut out = File::create(dest_path).map_err(|e| e.to_string())?;
                     std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
                     count += 1;
                 }
            }
        }
    }

    Ok(format!("Imported successfully. Added {} IDs to AppList. Injected Keys. Extracted {} manifests.", ids_count, count))
}
