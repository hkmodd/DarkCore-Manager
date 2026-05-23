use tauri::State;
use crate::state::AppState;
use crate::config::AppConfig;
use std::path::Path;
use crate::api::UserStats;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config_manager.get()
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    state.config_manager.update(config.clone())?;
    
    // Update Managers
    let mut app_list = state.app_list.lock().unwrap();
    let mut vdf = state.vdf_injector.lock().unwrap();
    
    app_list.set_paths(Path::new(&config.gl_path), Path::new(&config.steam_path));
    vdf.set_paths(Path::new(&config.steam_path));
    
    // Update API Client if key changed
    let mut client_guard = state.api_client.lock().unwrap();
    if !config.api_key.is_empty() {
         *client_guard = Some(crate::api::ApiClient::new(config.api_key.clone()));
    }

    Ok(())
}

#[tauri::command]
pub fn validate_path(path: String) -> bool {
    !path.is_empty() && Path::new(&path).exists()
}

#[tauri::command]
pub async fn validate_api_key(key: String, _state: State<'_, AppState>) -> Result<UserStats, String> {
    // Create temporary client to validate
    let client = crate::api::ApiClient::new(key);
    client.get_user_stats().await.map_err(|e| e.to_string())
}
