use crate::config::get_config_path;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn load_game_cache() -> HashMap<String, String> {
    let mut path = get_config_path();
    path.set_file_name("game_names_cache.json");

    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str(&content) {
                return cache;
            }
        }
    }
    HashMap::new()
}

pub fn save_game_cache(cache: &HashMap<String, String>) -> Result<(), std::io::Error> {
    let mut path = get_config_path();
    path.set_file_name("game_names_cache.json");

    let content = serde_json::to_string_pretty(cache)?;
    fs::write(path, content)?;
    Ok(())
}
