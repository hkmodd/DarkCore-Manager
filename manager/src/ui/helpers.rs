//! UI Helper functions.

/// Cleans and tokenizes text for search purposes.
/// Converts to lowercase, keeps only alphanumerics and spaces, then splits by whitespace.
#[allow(dead_code)] // Used by install_modal, may be useful elsewhere
pub fn clean_tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub fn download_manifests_wudrm(
    appid: &str,
    steam_root: &str,
    log: &dyn Fn(String),
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use crate::api::ApiClient;

    let runtime = tokio::runtime::Runtime::new()?;
    // Anonymous client for public SteamCMD API
    let client = ApiClient::new("".to_string());
    let downloader = crate::manifest_downloader::ManifestDownloader::new();
    let depot_cache_dir = std::path::Path::new(steam_root).join("depotcache");

    if !depot_cache_dir.exists() {
        std::fs::create_dir_all(&depot_cache_dir)?;
    }

    log(format!(
        "Wudrm: Connecting to SteamCMD for AppID {}...",
        appid
    ));

    // We need 'get_app_info' to return public info with GIDs
    let info = runtime.block_on(client.get_app_info(appid))?;

    let vault = crate::vault::VaultManager::new(".");

    let mut valid_manifests = 0;
    // Download manifest for EACH depot that has a GID
    for (depot_id, depot_curr) in info.depots {
        if let Some(gid) = depot_curr.gid {
            let expected_name = format!("{}_{}.manifest", depot_id, gid);
            let expected_path = depot_cache_dir.join(&expected_name);

            // 1. Check if exists (Restored from Vault or previous run)
            if expected_path.exists() {
                log(format!(
                    "   - Skipping Wudrm (Found local): {}",
                    expected_name
                ));
                valid_manifests += 1;
                // Ensure it is in Vault too (Sync)
                let _ = vault.store_manifest(appid, &expected_path);
                continue;
            }

            log(format!(
                "   - Downloading Manifest: Depot {} | GID: {}",
                depot_id, gid
            ));
            match runtime.block_on(downloader.download_manifest(&depot_id, &gid, &depot_cache_dir))
            {
                Ok(path) => {
                    log(format!("      ✅ Success! Saved to {:?}", path));
                    valid_manifests += 1;
                    // 2. Save to Vault immediately
                    let _ = vault.store_manifest(appid, &path);
                }
                Err(e) => {
                    log(format!("      ❌ Failed to download {}: {}", depot_id, e));
                }
            }
        }
    }

    Ok(valid_manifests)
}

pub fn setup_greenluma_config(gl_path: &str, enable_stealth: bool) -> std::io::Result<()> {
    let path = std::path::Path::new(gl_path);
    if !path.exists() {
        return Ok(());
    }

    // GreenLuma uses these empty files as flags for Stealth Mode and NoQuestion
    let files = ["NoQuestion.bin"];
    for f in files.iter() {
        let p = path.join(f);
        if !p.exists() {
            let _ = std::fs::write(&p, "");
        }
    }

    // Stealth Mode Toggle
    let stealth_bin = path.join("StealthMode.bin");
    if enable_stealth {
        if !stealth_bin.exists() {
            let _ = std::fs::write(&stealth_bin, "");
        }
    } else if stealth_bin.exists() {
        let _ = std::fs::remove_file(&stealth_bin);
    }

    // NOTE: Removed GreenLuma_2025_x64.ini creation
    Ok(())
}

pub fn relaunch_steam_protocol(app: &crate::ui::state::DarkCoreApp) {
    let steam_path = app.config.steam_path.clone();
    let log_arc = app.system_log.clone();

    std::thread::spawn(move || {
        let log = move |msg: String| {
            if let Ok(mut logs) = log_arc.lock() {
                crate::ui::state::push_log(&mut logs, msg);
            }
        };

        log("⚠ STEAM PURGE PROTOCOL INITIATED...".to_string());

        // 1. Kill Steam
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "steam.exe"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(2500));

        // 2. Launch Steam Normal
        let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
        if steam_exe.exists() {
            log("🔄 Relaunching Steam (Normal Mode)...".to_string());
            match open::that(steam_exe) {
                Ok(_) => log("✅ Steam Relaunched.".to_string()),
                Err(e) => log(format!("❌ Launch Failed: {}", e)),
            }
        } else {
            log("❌ steam.exe not found.".to_string());
        }
    });
}

pub fn is_probable_dlc(name: &str) -> bool {
    let lower = name.to_lowercase();
    let keywords = [
        "dlc",
        "pack",
        "soundtrack",
        " ost",
        "artbook",
        "upgrade",
        "season pass",
        "expansion",
        "ticket",
        "skin",
        "costume",
        "bonus",
        "content",
        "kit",
        "bundle",
        "edition",
    ];
    for kw in keywords {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

#[allow(dead_code)] // Fallback utility, auto-scan logic moved inline
pub fn detect_auto_install_path(
    game_name: &str,
    libraries: &[std::path::PathBuf],
) -> (Option<String>, Option<std::path::PathBuf>, String) {
    // Returns: (DirName, LibraryPath, ConfidenceLevel)
    let target_tokens = clean_tokenize(game_name);
    let target_clean = game_name
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "");

    let mut best_match: Option<String> = None;
    let mut best_lib: Option<std::path::PathBuf> = None;
    let mut best_score = 0;

    for lib in libraries {
        let common = lib.join("steamapps").join("common");
        if let Ok(entries) = std::fs::read_dir(common) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(folder_name) = path.file_name().and_then(|s| s.to_str()) {
                        // Skip Utility Folders
                        if folder_name.eq_ignore_ascii_case("common")
                            || folder_name.eq_ignore_ascii_case("Steamworks Shared")
                        {
                            continue;
                        }

                        let folder_clean = folder_name
                            .to_lowercase()
                            .replace(|c: char| !c.is_alphanumeric(), "");

                        // 1. Exact Match (Sanitized)
                        if folder_clean == target_clean {
                            return (
                                Some(folder_name.to_string()),
                                Some(lib.clone()),
                                "EXACT".to_string(),
                            );
                        }

                        // 2. Token Overlap
                        let folder_tokens = clean_tokenize(folder_name);
                        let mut overlap = 0;
                        for t in &target_tokens {
                            if folder_tokens.contains(t) {
                                overlap += 1;
                            }
                        }

                        let score = if !target_tokens.is_empty() {
                            (overlap * 100) / target_tokens.len()
                        } else {
                            0
                        };

                        if score > best_score && score > 60 {
                            best_score = score;
                            best_match = Some(folder_name.to_string());
                            best_lib = Some(lib.clone());
                        }
                    }
                }
            }
        }
    }

    if let Some(dir) = best_match {
        (Some(dir), best_lib, format!("FUZZY_{}%", best_score))
    } else {
        (None, None, "NONE".to_string())
    }
}

pub fn remove_games_by_id(
    app: &crate::ui::state::DarkCoreApp,
    mut ids: Vec<String>,
    full_wipe: bool,
) {
    // AUTO-DETECT CHILDREN (Fix for Hidden Orphans)
    {
        let mut children_to_add = Vec::new();
        if let Ok(map) = app.relationships.lock() {
            for target_id in &ids {
                // Find all children where parent == target_id
                for (child, parent) in map.iter() {
                    if parent == target_id && !ids.contains(child) {
                        children_to_add.push(child.clone());
                    }
                }
            }
        }
        if !children_to_add.is_empty() {
            if let Ok(mut logs) = app.system_log.lock() {
                crate::ui::state::push_log(
                    &mut logs,
                    format!(
                        "♻ Linked Deletion: Found {} attached DLCs/Depots.",
                        children_to_add.len()
                    ),
                );
            }
            ids.extend(children_to_add);
        }
    }

    let gl_path = app.config.gl_path.clone();
    let steam_path = app.config.steam_path.clone();
    let al_path = std::path::Path::new(&gl_path).join("AppList");

    // 1. Remove from AppList
    if let Ok(paths) = glob::glob(&al_path.join("*.txt").to_string_lossy()) {
        for path in paths.flatten() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if ids.contains(&content.trim().to_string()) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    // 2. Full Wipe
    if full_wipe {
        let libraries = crate::game_path::GamePathFinder::get_library_folders(&steam_path);
        let vault = crate::vault::VaultManager::new(".");

        for id in &ids {
            // [FIX] Avoid Ghost Folders: Only backup Parent AppIDs
            // Skip children (Depots/DLCs) because the Parent backup handles the main ACF and all mounted manifests.
            let is_child = if let Ok(map) = app.relationships.lock() {
                map.contains_key(id)
            } else {
                false
            };

            let mut backed_up = false;

            // Only backup if NOT a child (Base Game)
            if !is_child {
                for lib in &libraries {
                    if let Ok(c) = vault.backup_manifests(&lib.to_string_lossy(), id) {
                        if c > 0 {
                            if let Ok(mut logs) = app.system_log.lock() {
                                crate::ui::state::push_log(
                                    &mut logs,
                                    format!(
                                        "Vault: Secured {} manifests for {} from {:?}.",
                                        c, id, lib
                                    ),
                                );
                            }
                            backed_up = true;
                            break;
                        }
                    }
                }
                if !backed_up {
                    let _ = vault.backup_manifests(&steam_path, id);
                }
            } // End if !is_child

            let mut locations = libraries.clone();
            locations.push(std::path::Path::new(&steam_path).to_path_buf());

            for lib in &locations {
                let acf = lib
                    .join("steamapps")
                    .join(format!("appmanifest_{}.acf", id));
                if acf.exists() {
                    if let Ok(content) = std::fs::read_to_string(&acf) {
                        let mut install_dir = String::new();
                        for line in content.lines() {
                            if line.to_lowercase().contains("installdir") {
                                let parts: Vec<&str> = line.split('"').collect();
                                if parts.len() >= 4 {
                                    install_dir = parts[3].to_string();
                                }
                            }
                        }

                        if !install_dir.is_empty() {
                            let content_path =
                                lib.join("steamapps").join("common").join(&install_dir);
                            if content_path.exists() {
                                if let Ok(mut logs) = app.system_log.lock() {
                                    crate::ui::state::push_log(
                                        &mut logs,
                                        format!("Deleting Game Files: {:?}", content_path),
                                    );
                                }
                                let _ = std::fs::remove_dir_all(&content_path);
                            }
                        }
                    }

                    let _ = std::fs::remove_file(acf);
                }
            }
        }
    }

    // 3. Remove from config.vdf
    if let Err(e) = crate::vdf_injector::remove_vdf_keys(&steam_path, &ids) {
        if let Ok(mut logs) = app.system_log.lock() {
            crate::ui::state::push_log(&mut logs, format!("VDF Cleanup Warning: {}", e));
        }
    }

    // 4. Update Relationships
    if let Ok(mut map) = app.relationships.lock() {
        let initial_len = map.len();
        map.retain(|k, _| !ids.contains(k));
        if map.len() != initial_len {
            crate::app_list::save_relationships(".", &map);
        }
    }

    // 5. Reorder
    if let Ok(mut logs) = app.system_log.lock() {
        crate::ui::state::push_log(&mut logs, "Reordering AppList...".to_string());
    }
    let cache_guard = app.game_cache.lock().ok();
    let cache_ref = cache_guard.as_deref();

    if let Err(e) = crate::app_list::nuke_reorder(&gl_path, &steam_path, None, cache_ref) {
        if let Ok(mut logs) = app.system_log.lock() {
            crate::ui::state::push_log(&mut logs, format!("Reorder Warning: {}", e));
        }
    }

    if let Ok(mut logs) = app.system_log.lock() {
        crate::ui::state::push_log(
            &mut logs,
            format!("Deleted {} items. Full Wipe: {}", ids.len(), full_wipe),
        );
    }
}
