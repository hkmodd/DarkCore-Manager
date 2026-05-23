use tauri::State;
use crate::state::AppState;
use crate::api::{SearchResult, GameDetails};

#[tauri::command]
pub async fn search_games(query: String, state: State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let client = {
        let guard = state.api_client.lock().unwrap();
        guard.clone()
    };

    if let Some(client) = client {
        client.search(&query).await.map_err(|e| {
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("401") || err_str.contains("403") || err_str.contains("unauthorized") || err_str.contains("forbidden") {
                "INVALID_KEY: Your API key is invalid or expired. Check Settings.".to_string()
            } else if err_str.contains("429") || err_str.contains("rate") || err_str.contains("too many") {
                "RATE_LIMIT: Rate limit exceeded. Please wait a moment.".to_string()
            } else if err_str.contains("timeout") || err_str.contains("connect") || err_str.contains("dns") || err_str.contains("network") {
                "NETWORK_ERROR: Could not reach the server. Check your connection.".to_string()
            } else {
                format!("SEARCH_ERROR: {}", e)
            }
        })
    } else {
        Err("NOT_CONFIGURED: No API key set. Go to Settings to configure your key.".to_string())
    }
}

#[tauri::command]
pub async fn get_game_details(appid: String, state: State<'_, AppState>) -> Result<GameDetails, String> {
    let client = {
        let guard = state.api_client.lock().unwrap();
        guard.clone()
    };

    if let Some(client) = client {
        client.get_game_details(&appid).await.map_err(|e| e.to_string())
    } else {
        Err("API Client not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_cover_url(appid: String) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg", appid)
}
