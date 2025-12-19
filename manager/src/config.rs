use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub steam_path: String,
    pub gl_path: String,
    pub steamless_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            steam_path: find_default_steam().unwrap_or_default(),
            gl_path: String::new(),
            steamless_path: String::new(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    // Try to use the directory of the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join("darkcore_config.json");
        }
    }
    PathBuf::from("darkcore_config.json")
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&content) {
                return cfg;
            }
        }
    }
    AppConfig::default()
}

pub fn save_config(config: &AppConfig) -> Result<(), std::io::Error> {
    let path = get_config_path();
    let content = serde_json::to_string_pretty(config)?;
    let mut file = fs::File::create(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

fn find_default_steam() -> Option<String> {
    let paths = [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"];
    for p in paths {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}
