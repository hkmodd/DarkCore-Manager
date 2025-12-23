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

    #[serde(default = "default_profile")]
    pub last_active_profile: String,

    #[serde(default = "default_vec")]
    pub family_godmode_ids: Vec<String>,

    #[serde(default = "default_true")]
    pub enable_stealth_mode: bool,
}

fn default_profile() -> String {
    "Default".to_string()
}

fn default_vec() -> Vec<String> {
    Vec::new()
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            steam_path: find_default_steam().unwrap_or_default(),
            gl_path: String::new(),
            steamless_path: String::new(),
            last_active_profile: "Default".to_string(),
            family_godmode_ids: Vec::new(),
            enable_stealth_mode: true,
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
    // 1. Try Registry (Most Reliable)
    let keys = [
        ("SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"), // x64 OS
        ("SOFTWARE\\Valve\\Steam", "InstallPath"),              // x86 OS
    ];

    for (key_path, value_name) in keys.iter() {
        if let Ok(hk_lm) = std::process::Command::new("reg")
            .args(&["query", &format!("HKLM\\{}", key_path), "/v", value_name])
            .output()
        {
            let out = String::from_utf8_lossy(&hk_lm.stdout);
            // Reg query output format: ... REG_SZ    C:\Program Files (x86)\Steam
            if let Some(pos) = out.find("REG_SZ") {
                let path_part = out[pos + 6..].trim();
                let path_str = path_part.to_string();
                if Path::new(&path_str).exists() {
                    return Some(path_str);
                }
            }
        }
    }

    // 2. Fallback to Common Paths
    let paths = [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"];
    for p in paths {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}
