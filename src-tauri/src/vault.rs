use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VaultGame {
    pub app_id: String,
    pub name: String,
    pub size_gb: String, // Calculated from manifests if possible, or placeholder
    pub timestamp: i64,  // Modified time
}

pub struct VaultManager {
    base_path: PathBuf,
}

impl VaultManager {
    pub fn new(_app_handle: &tauri::AppHandle) -> Self {
        // Use app_local_data_dir or just relative "Vault" as in legacy?
        Self::new_local()
    }

    pub fn new_local() -> Self {
        // Use absolute path relative to exe, NOT CWD (which Tauri may change)
        let path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("Vault")))
            .unwrap_or_else(|| Path::new("Vault").to_path_buf());
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        Self { base_path: path }
    }

    pub fn get_storage_dir(&self, appid: &str) -> PathBuf {
        self.base_path.join(appid)
    }

    pub fn get_base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn ensure_structure(&self, appid: &str) -> std::io::Result<()> {
        let path = self.get_storage_dir(appid);
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        Ok(())
    }

    pub fn list_games(&self) -> Vec<VaultGame> {
        let mut games = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.base_path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let appid = entry.file_name().to_string_lossy().to_string();
                        // Vault Policy: Check for {appid}.lua existence to confirm it's a valid vault entry
                        let lua_path = entry.path().join(format!("{}.lua", appid));

                        // We check if it has valuable data (Lua or Manifests)
                        if lua_path.exists() || self.has_manifests(&appid) {
                            // Try to resolve name from a simple heuristic or placeholder
                            let name = format!("AppID {}", appid);

                            // Try to check if we have a cached name from game_names_cache (would need to read external file,
                            // but for now we keep it simple to satisfy the struct requirements)

                            let mut timestamp = 0;
                            if let Ok(meta) = entry.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    timestamp = modified
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs()
                                        as i64;
                                }
                            }

                            games.push(VaultGame {
                                app_id: appid,
                                name,
                                size_gb: "0.0 GB".to_string(), // Recalculate if needed
                                timestamp,
                            });
                        }
                    }
                }
            }
        }
        games
    }

    /// Stores a single manifest file into the Vault (used by Watcher/WUDRM)
    pub fn store_manifest(&self, appid: &str, source_path: &Path) -> std::io::Result<()> {
        let vault_dir = self.get_storage_dir(appid);
        if !vault_dir.exists() {
            fs::create_dir_all(&vault_dir)?;
        }

        if let Some(fname) = source_path.file_name() {
            let dest = vault_dir.join(fname);
            fs::copy(source_path, dest)?;
        }
        Ok(())
    }

    // =========================================================================
    // v1.7.2 PORTED FUNCTIONS — Lua, Version Verification, Invalidation, ZIP
    // =========================================================================

    fn get_lua_path(&self, appid: &str) -> PathBuf {
        self.base_path.join(appid).join(format!("{}.lua", appid))
    }

    pub fn get_zip_path(&self, appid: &str) -> PathBuf {
        self.base_path.join(appid).join("data.zip")
    }

    /// Check if Lua script exists in Vault for this appid
    pub fn exists(&self, appid: &str) -> bool {
        self.get_lua_path(appid).exists()
    }

    /// Check if Vault data exists AND is fresher than max_age_days
    /// Returns true if file exists and is less than max_age_days old
    pub fn is_fresh(&self, appid: &str, max_age_days: u64) -> bool {
        self.check_freshness(self.get_lua_path(appid), max_age_days)
    }

    pub fn is_zip_fresh(&self, appid: &str, max_age_days: u64) -> bool {
        self.check_freshness(self.get_zip_path(appid), max_age_days)
    }

    fn check_freshness(&self, path: PathBuf, max_age_days: u64) -> bool {
        if !path.exists() {
            return false;
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                return age.as_secs() < max_age_days * 86400;
            }
        }
        true
    }

    /// Save raw ZIP bytes to Vault/{appid}/data.zip (Level 2 Cache)
    pub fn save_zip(&self, appid: &str, data: &[u8]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        fs::write(self.get_zip_path(appid), data)
    }

    /// Retrieve cached raw ZIP from Vault
    pub fn get_zip(&self, appid: &str) -> std::io::Result<Vec<u8>> {
        fs::read(self.get_zip_path(appid))
    }

    /// Save Lua script bytes to Vault/{appid}/{appid}.lua
    pub fn save_lua(&self, appid: &str, data: &[u8]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        fs::write(self.get_lua_path(appid), data)
    }

    /// Retrieve cached Lua script from Vault
    pub fn get_lua(&self, appid: &str) -> std::io::Result<Vec<u8>> {
        fs::read(self.get_lua_path(appid))
    }

    /// Check if vault has manifest data for an appid (folder exists)
    pub fn has_manifests(&self, appid: &str) -> bool {
        let storage_dir = self.base_path.join(appid);
        storage_dir.exists() && storage_dir.is_dir()
    }

    /// Verify if vault manifests are up-to-date compared to expected GIDs.
    /// Returns (is_valid, outdated_depot_ids)
    pub fn verify_manifests(
        &self,
        appid: &str,
        expected_gids: &std::collections::HashMap<String, String>,
    ) -> (bool, Vec<String>) {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            return (false, expected_gids.keys().cloned().collect());
        }

        let mut outdated = Vec::new();
        for (depot_id, expected_gid) in expected_gids {
            let expected_filename = format!("{}_{}.manifest", depot_id, expected_gid);
            let expected_path = storage_dir.join(&expected_filename);
            if !expected_path.exists() {
                outdated.push(depot_id.clone());
            }
        }

        let is_valid = outdated.is_empty();
        (is_valid, outdated)
    }

    /// Delete ALL vault data for an appid (full invalidation)
    pub fn invalidate_app(&self, appid: &str) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if storage_dir.exists() {
            fs::remove_dir_all(&storage_dir)?;
        }
        Ok(())
    }

    /// Delete only manifests for specific depot IDs (partial invalidation)
    pub fn invalidate_depots(&self, appid: &str, depot_ids: &[String]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            return Ok(());
        }

        if let Ok(entries) = fs::read_dir(&storage_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(fname) = path.file_name() {
                    let fname_str = fname.to_string_lossy();
                    if fname_str.ends_with(".manifest") {
                        for depot_id in depot_ids {
                            if fname_str.starts_with(&format!("{}_", depot_id)) {
                                let _ = fs::remove_file(&path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Store raw manifest bytes directly to vault (for Direct Download)
    pub fn store_manifest_bytes(
        &self,
        appid: &str,
        depot_id: u32,
        manifest_gid: u64,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        fs::write(storage_dir.join(filename), bytes)
    }

    /// Mark a download as failed by creating a timestamped marker file.
    /// Used for Circuit Breaker pattern to prevent token waste.
    pub fn mark_failure(&self, appid: &str) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        let failure_path = storage_dir.join(".download_failed");
        fs::write(failure_path, "") // Timestamp comes from metadata
    }

    /// Clear failure marker on success
    pub fn clear_failure(&self, appid: &str) {
        let path = self.base_path.join(appid).join(".download_failed");
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    /// Check if a download failed recently (Circuit Breaker)
    pub fn is_failed_recently(&self, appid: &str, cooldown_minutes: u64) -> bool {
        let path = self.base_path.join(appid).join(".download_failed");
        if !path.exists() {
            return false;
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                return age.as_secs() < cooldown_minutes * 60;
            }
        }
        false // If we can't check age, ignore marker to be safe
    }
}
