use crate::utils::vdf::{VdfParser, VdfValue};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct VdfInjector {
    steam_path: PathBuf,
}

impl VdfInjector {
    pub fn new(steam_path: &Path) -> Self {
        Self {
            steam_path: steam_path.to_path_buf(),
        }
    }

    pub fn inject_vdf(
        &self,
        vdf_keys: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cfg_path = self.steam_path.join("config").join("config.vdf");
        if !cfg_path.exists() {
            return Ok(());
        }

        // Backup
        let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

        let content_bytes = fs::read(&cfg_path)?;
        let content = String::from_utf8_lossy(&content_bytes).to_string();

        let mut root = match VdfParser::parse(&content) {
            Some(r) => r,
            None => return Err("Failed to parse config.vdf".into()),
        };

        // Traverse to "depots". config.vdf structure:
        // "InstallConfigStore" -> "Software" -> "Valve" -> "Steam" -> "depots"
        let base = if root.has_key("InstallConfigStore") {
            root.get_mut("InstallConfigStore").unwrap()
        } else {
            return Err("Invalid config.vdf structure (missing InstallConfigStore)".into());
        };

        if let Some(steam_node) = base.ensure_path(&["Software", "Valve", "Steam"]) {
            if let Some(depots) = steam_node.ensure_path(&["depots"]) {
                for (appid, key) in vdf_keys {
                    // Logic: Check if AppID block exists
                    // If yes, update DecryptionKey. If no, create it.
                    if !depots.has_key(appid) {
                        let mut new_obj = Vec::new();
                        new_obj.push(("DecryptionKey".to_string(), VdfValue::Str(key.clone())));
                        depots.insert_or_update(appid.clone(), VdfValue::Obj(new_obj));
                    } else {
                        if let Some(app_node) = depots.get_mut(appid) {
                            if let VdfValue::Obj(fields) = app_node {
                                let mut found = false;
                                for (k, v) in fields.iter_mut() {
                                    if k.eq_ignore_ascii_case("DecryptionKey") {
                                        *v = VdfValue::Str(key.clone());
                                        found = true;
                                        break;
                                    }
                                }
                                if !found {
                                    fields.push((
                                        "DecryptionKey".to_string(),
                                        VdfValue::Str(key.clone()),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        let new_content = VdfParser::serialize(&root);
        fs::write(cfg_path, new_content)?;

        Ok(())
    }

    pub fn set_paths(&mut self, steam_path: &Path) {
        self.steam_path = steam_path.to_path_buf();
    }

    /// Remove decryption keys for the given depot IDs from config.vdf.
    /// Used during FULL WIPE delete to clean up VDF entries.
    pub fn remove_vdf_keys(
        &self,
        depot_ids: &[String],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let cfg_path = self.steam_path.join("config").join("config.vdf");
        if !cfg_path.exists() {
            return Ok(0);
        }

        // Backup
        let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

        let content_bytes = fs::read(&cfg_path)?;
        let content = String::from_utf8_lossy(&content_bytes).to_string();

        let mut root = match VdfParser::parse(&content) {
            Some(r) => r,
            None => return Err("Failed to parse config.vdf".into()),
        };

        let mut removed = 0;

        let base = if root.has_key("InstallConfigStore") {
            root.get_mut("InstallConfigStore").unwrap()
        } else {
            return Ok(0);
        };

        if let Some(steam_node) = base.ensure_path(&["Software", "Valve", "Steam"]) {
            if let Some(depots) = steam_node.get_mut("depots") {
                if let VdfValue::Obj(entries) = depots {
                    let before = entries.len();
                    entries.retain(|(key, _)| !depot_ids.contains(key));
                    removed = before - entries.len();
                }
            }
        }

        if removed > 0 {
            let new_content = VdfParser::serialize(&root);
            fs::write(cfg_path, new_content)?;
        }

        Ok(removed)
    }
}
