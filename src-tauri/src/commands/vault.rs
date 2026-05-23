use crate::state::AppState;
use crate::vault::VaultGame;
use tauri::State;

#[tauri::command]
pub fn get_vault_games(state: State<'_, AppState>) -> Vec<VaultGame> {
    let vault = state.vault.lock().unwrap();
    vault.list_games()
}

#[tauri::command]
pub fn delete_vault_game(appid: String, state: State<'_, AppState>) -> Result<(), String> {
    let vault = state.vault.lock().unwrap();
    let dir = vault.get_storage_dir(&appid);

    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Game not found in vault".to_string())
    }
}
