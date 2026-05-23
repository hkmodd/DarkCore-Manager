use crate::state::AppState;
use tauri::State;

#[derive(serde::Serialize)]
pub struct IntegrityReport {
    pub app_id: String,
    pub status: String, // "OK", "CORRUPT", "MISSING_MANIFESTS", "ERROR"
    pub missing_depots: Vec<String>,
    pub corrupt_manifests: Vec<String>,
    pub timestamp: u64,
}

#[tauri::command]
pub async fn verify_integrity(
    state: State<'_, AppState>,
    appid: String,
) -> Result<IntegrityReport, String> {
    // 1. Fetch Remote Info (Source of Truth)
    let client = {
        let guard = state.api_client.lock().unwrap();
        if let Some(c) = &*guard {
            c.clone()
        } else {
            return Err("API Client not initialized".to_string());
        }
    };

    // We fetch remote info to know what manifests SHOULD be there (current GIDs)
    let info = client.get_app_info(&appid).await.map_err(|e| e.to_string())?;

    // 2. Scan Local Manifests
    let steam_path = {
        let config = state.config_manager.config.lock().unwrap();
        std::path::PathBuf::from(&config.steam_path)
    };
    
    let depot_cache = steam_path.join("depotcache");
    
    let mut missing = Vec::new();
    let mut corrupt = Vec::new();
    
    const STEAM_MANIFEST_MAGIC: u32 = 0x71F617D0;

    for (depot_id, depot_info) in info.depots {
        if let Some(gid) = depot_info.gid {
            let filename = format!("{}_{}.manifest", depot_id, gid);
            let path = depot_cache.join(&filename);
            
            if !path.exists() {
                // Check if maybe we have a different version? 
                // Integrity check implies we want the LATEST.
                missing.push(format!("{} (latest)", depot_id));
            } else {
                // Check VALIDITY
                 let mut is_valid = false;
                 if let Ok(meta) = std::fs::metadata(&path) {
                     if meta.len() > 50 {
                         if let Ok(mut file) = std::fs::File::open(&path) {
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
                 
                 if !is_valid {
                     corrupt.push(depot_id.clone());
                 }
            }
        }
    }
    
    let status = if !missing.is_empty() {
        "MISSING_MANIFESTS".to_string()
    } else if !corrupt.is_empty() {
        "CORRUPT".to_string()
    } else {
        "OK".to_string()
    };

    Ok(IntegrityReport {
        app_id: appid,
        status,
        missing_depots: missing,
        corrupt_manifests: corrupt,
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
    })
}
