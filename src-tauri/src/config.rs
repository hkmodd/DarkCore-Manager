use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub steam_path: String,
    pub gl_path: String,
    pub steamless_path: String,
    pub api_key: String,
    pub enable_stealth_mode: bool,
    pub active_profile: String,

    #[serde(default)]
    pub family_godmode_ids: Vec<String>,

    #[serde(default = "default_language")]
    pub target_language: String,
}

fn default_language() -> String {
    "italian".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            steam_path: find_default_steam().unwrap_or_default(),
            gl_path: String::new(),
            steamless_path: String::new(),
            api_key: String::new(),
            enable_stealth_mode: true,
            active_profile: "Default".to_string(),
            family_godmode_ids: Vec::new(),
            target_language: "italian".to_string(),
        }
    }
}

pub struct ConfigManager {
    pub config: Mutex<AppConfig>,
    pub file_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let path = get_config_path();
        let config = if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                serde_json::from_str(&content).unwrap_or_default()
            } else {
                AppConfig::default()
            }
        } else {
            AppConfig::default()
        };

        Self {
            config: Mutex::new(config),
            file_path: path,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let config = self.config.lock().unwrap();
        let content = serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, content).map_err(|e| e.to_string())
    }

    pub fn get(&self) -> AppConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn update(&self, new_config: AppConfig) -> Result<(), String> {
        {
            let mut cfg = self.config.lock().unwrap();
            *cfg = new_config;
        }
        self.save()
    }
}

pub fn get_config_path() -> PathBuf {
    // Portable: Next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join("darkcore_config.json");
        }
    }
    PathBuf::from("darkcore_config.json")
}

fn find_default_steam() -> Option<String> {
    // 1. Try Registry (Windows)
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hklm.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                // Steam uses forward slashes in registry sometimes, fix them
                let path = path.replace("/", "\\");
                if Path::new(&path).exists() {
                    return Some(path);
                }
            }
        }
    }

    // 2. Fallback to Common Paths
    let paths = [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"E:\Steam",
    ];
    for p in paths {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}
