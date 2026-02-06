use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
// use std::sync::{Arc, Mutex}; // Unused

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InstallSource {
    SteamCMD,       // Installed via Steam Protocol (greenluma)
    DirectDownload, // Installed via Direct Download (Morrenus)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    pub appid: String,
    pub name: String,
    pub source: InstallSource,
    pub install_dir: String, // Relative to steamapps/common
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallRegistry {
    pub deployed_games: HashMap<String, InstallRecord>,
}

impl InstallRegistry {
    fn get_path() -> PathBuf {
        Path::new("core_data").join("registry.json")
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(reg) = serde_json::from_str::<InstallRegistry>(&content) {
                    return reg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_path();
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn register(
        &mut self,
        appid: String,
        name: String,
        source: InstallSource,
        install_dir: String,
    ) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let record = InstallRecord {
            appid: appid.clone(),
            name,
            source,
            install_dir,
            timestamp,
        };

        self.deployed_games.insert(appid, record);
        let _ = self.save();
    }

    #[allow(dead_code)]
    pub fn get_source(&self, appid: &str) -> Option<InstallSource> {
        self.deployed_games.get(appid).map(|r| r.source.clone())
    }
}
