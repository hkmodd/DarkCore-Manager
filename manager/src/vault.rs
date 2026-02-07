#![allow(dead_code)] // Reserved for future Vault/Backup feature
use std::fs;
use std::path::{Path, PathBuf};

pub struct VaultManager {
    base_path: PathBuf,
}

impl VaultManager {
    pub fn new(base_dir: &str) -> Self {
        let path = Path::new(base_dir).join("Vault");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        Self { base_path: path }
    }

    fn get_path(&self, appid: &str) -> PathBuf {
        // Fix: Store Lua INSIDE the AppID folder to avoid loose ghost files
        // Vault/{AppID}/{AppID}.lua
        self.base_path.join(appid).join(format!("{}.lua", appid))
    }

    pub fn exists(&self, appid: &str) -> bool {
        self.get_path(appid).exists()
    }

    pub fn save_lua(&self, appid: &str, data: &[u8]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        fs::write(self.get_path(appid), data)
    }

    pub fn get_lua(&self, appid: &str) -> std::io::Result<Vec<u8>> {
        fs::read(self.get_path(appid))
    }

    /// Backs up the AppManifest (ACF) and identified Depot Manifests to the Vault.
    /// Vital for Offline Manual Fixes.
    pub fn backup_manifests(&self, steam_path: &str, appid: &str) -> std::io::Result<usize> {
        let mut count = 0;
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }

        // 1. Find and Copy Main ACF
        let acf_name = format!("appmanifest_{}.acf", appid);
        let steam_apps = Path::new(steam_path).join("steamapps");
        let acf_path = steam_apps.join(&acf_name);

        let mut mounted_depots = Vec::new();

        if acf_path.exists() {
            if let Ok(content) = fs::read_to_string(&acf_path) {
                // Parse MountedDepots via Regex
                // Pattern: "MountedDepots"\s*\{([^\}]+)\}
                let re_block = regex::Regex::new(r#""MountedDepots"\s*\{([^}]+)\}"#).unwrap();
                if let Some(caps) = re_block.captures(&content) {
                    if let Some(block) = caps.get(1) {
                        let re_val = regex::Regex::new(r#""(\d+)""#).unwrap();
                        for cap in re_val.captures_iter(block.as_str()) {
                            if let Some(id) = cap.get(1) {
                                mounted_depots.push(id.as_str().to_string());
                            }
                        }
                    }
                }

                // Copy ACF
                fs::copy(&acf_path, storage_dir.join(&acf_name))?;
                count += 1;
            }
        }

        // 2. DepotCache Manifests
        let depot_cache = Path::new(steam_path).join("depotcache");
        if depot_cache.exists() {
            for depot_id in mounted_depots {
                let pattern = format!("{}*.manifest", depot_id); // e.g. 12345*.manifest
                let glob_pat = depot_cache.join(&pattern);

                if let Ok(paths) = glob::glob(&glob_pat.to_string_lossy()) {
                    for path in paths.flatten() {
                        if let Some(fname) = path.file_name() {
                            let dest = storage_dir.join(fname);
                            if fs::copy(&path, dest).is_ok() {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    pub fn store_manifest(&self, appid: &str, source: &Path) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        if let Some(fname) = source.file_name() {
            fs::copy(source, storage_dir.join(fname))?;
        }
        Ok(())
    }

    /// Restores AppManifest and Depot Manifests from the Vault to Steam.
    /// Returns: (restored_acf, restored_depots_count)
    pub fn restore_manifests(
        &self,
        steam_path: &str,
        appid: &str,
    ) -> std::io::Result<(bool, usize)> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            return Ok((false, 0));
        }

        let mut restored_acf = false;
        let mut restored_count = 0;

        // 1. Restore ACF
        let acf_name = format!("appmanifest_{}.acf", appid);
        let vault_acf = storage_dir.join(&acf_name);
        // We restore ACF to the MAIN steamapps for simplicity.
        // Logic could be improved to restore to original library if tracked, but default is SteamPath.
        let steam_apps = Path::new(steam_path).join("steamapps");
        let target_acf = steam_apps.join(&acf_name);

        if vault_acf.exists() {
            // Only restore if target doesn't exist? Or overwrite?
            // "Repair" implies overwrite corruption.
            // "Install" implies target doesn't exist.
            // Overwriting is generally safer for "Restore".
            fs::copy(&vault_acf, &target_acf)?;
            restored_acf = true;
        }

        // 2. Restore Depot Manifests
        let depot_cache = Path::new(steam_path).join("depotcache");
        if !depot_cache.exists() {
            let _ = fs::create_dir_all(&depot_cache);
        }

        // Glob all .manifest files in vault
        let pattern = storage_dir.join("*.manifest");
        if let Ok(paths) = glob::glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Some(fname) = path.file_name() {
                    let dest = depot_cache.join(fname);
                    if !dest.exists() {
                        // Avoid redundant writes if already there
                        if fs::copy(&path, dest).is_ok() {
                            restored_count += 1;
                        }
                    }
                }
            }
        }

        Ok((restored_acf, restored_count))
    }

    // =========================================================================
    // VAULT AUDIT v1.7.2 - VERSION VERIFICATION & INVALIDATION
    // =========================================================================

    /// Get the vault storage directory for an appid
    pub fn get_storage_dir(&self, appid: &str) -> PathBuf {
        self.base_path.join(appid)
    }

    /// Check if vault has data for an appid (manifest folder exists)
    pub fn has_manifests(&self, appid: &str) -> bool {
        let storage_dir = self.base_path.join(appid);
        storage_dir.exists() && storage_dir.is_dir()
    }

    /// Verifies if the manifests in Vault are up-to-date compared to expected GIDs.
    /// Returns (is_valid, outdated_depot_ids)
    ///
    /// expected_gids: HashMap<depot_id, expected_manifest_gid>
    pub fn verify_manifests(
        &self,
        appid: &str,
        expected_gids: &std::collections::HashMap<String, String>,
    ) -> (bool, Vec<String>) {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            // No vault data = all outdated
            return (false, expected_gids.keys().cloned().collect());
        }

        let mut outdated = Vec::new();

        for (depot_id, expected_gid) in expected_gids {
            let expected_filename = format!("{}_{}.manifest", depot_id, expected_gid);
            let expected_path = storage_dir.join(&expected_filename);

            if !expected_path.exists() {
                // Manifest doesn't exist or has different GID
                outdated.push(depot_id.clone());
            }
        }

        let is_valid = outdated.is_empty();
        (is_valid, outdated)
    }

    /// Deletes ALL vault data for an appid (full invalidation)
    pub fn invalidate_app(&self, appid: &str) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if storage_dir.exists() {
            fs::remove_dir_all(&storage_dir)?;
        }
        // NOTE: .lua lives INSIDE storage_dir (Vault/{appid}/{appid}.lua)
        // so remove_dir_all already handles it. No separate delete needed.
        Ok(())
    }

    /// Deletes only manifests for specific depot IDs (partial invalidation)
    pub fn invalidate_depots(&self, appid: &str, depot_ids: &[String]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            return Ok(());
        }

        let pattern = storage_dir.join("*.manifest");
        if let Ok(paths) = glob::glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Some(fname) = path.file_name() {
                    let fname_str = fname.to_string_lossy();
                    // Filename format: {depot_id}_{gid}.manifest
                    for depot_id in depot_ids {
                        if fname_str.starts_with(&format!("{}_", depot_id)) {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Stores raw manifest bytes directly to vault (for Direct Download integration)
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

    /// Stores a ZIP file in the vault for future use
    pub fn store_zip(&self, appid: &str, bytes: &[u8]) -> std::io::Result<()> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        let zip_path = storage_dir.join(format!("{}.zip", appid));
        fs::write(zip_path, bytes)
    }

    /// Gets stored ZIP bytes if available
    pub fn get_zip(&self, appid: &str) -> Option<Vec<u8>> {
        let zip_path = self.base_path.join(appid).join(format!("{}.zip", appid));
        if zip_path.exists() {
            fs::read(&zip_path).ok()
        } else {
            None
        }
    }
}
