//! Steam Protocol Install Command — Full 5-Step Pipeline
//! 
//! Ported from v1.7.2 install_logic.rs::spawn_steam_install (lines 573-980)
//! This is the CORE of DarkCore Manager — the complete installation pipeline.

use crate::state::AppState;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub step: String,
    pub message: String,
    pub progress: f32, // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub applist_ids_count: usize,
    pub keys_injected: usize,
}

// =============================================================================
// HELPER: Parse Lua for Keys (v1.7.2 port from vdf_injector.rs)
// =============================================================================

/// Parse LUA file for AppIDs and Depot Keys
/// Returns (applist_ids, depot_keys)
fn parse_lua_for_keys(lua_content: &str) -> (Vec<String>, HashMap<String, String>) {
    let mut applist_ids = Vec::new();
    let mut keys = HashMap::new();

    // 3-arg: addappid(depot_id, flag, "key")
    let re_3arg =
        Regex::new(r#"addappid\s*\(\s*(\d+)\s*,\s*\d+\s*,\s*["']([a-fA-F0-9]{64})["']"#).unwrap();

    // 2-arg: addappid(depot_id, "key")
    let re_2arg =
        Regex::new(r#"addappid\s*\(\s*(\d+)\s*,\s*["']([a-fA-F0-9]{64})["']"#).unwrap();

    // ID-only: addappid(ID)
    let re_id_only = Regex::new(r#"addappid\s*\(\s*(\d+)\s*\)"#).unwrap();

    for line in lua_content.lines() {
        let trimmed = line.trim();

        // Skip Lua comments
        if trimmed.starts_with("--") {
            continue;
        }

        // 3-arg (with key)
        if let Some(cap) = re_3arg.captures(trimmed) {
            if let (Some(id_m), Some(key_m)) = (cap.get(1), cap.get(2)) {
                let id = id_m.as_str().to_string();
                let key = key_m.as_str().to_string();
                keys.insert(id.clone(), key);
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }
                continue;
            }
        }

        // 2-arg (fallback with key)
        if let Some(cap) = re_2arg.captures(trimmed) {
            if let (Some(id_m), Some(key_m)) = (cap.get(1), cap.get(2)) {
                let id = id_m.as_str().to_string();
                let key = key_m.as_str().to_string();
                if !keys.contains_key(&id) {
                    keys.insert(id.clone(), key);
                }
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }
                continue;
            }
        }

        // ID-only (AppID / DLC, no key)
        if let Some(cap) = re_id_only.captures(trimmed) {
            if let Some(id_m) = cap.get(1) {
                let id = id_m.as_str().to_string();
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }
            }
        }
    }

    (applist_ids, keys)
}

// =============================================================================
// MAIN COMMAND: steam_protocol_install
// =============================================================================

/// Full 5-step Steam Protocol Install — ported from v1.7.2 spawn_steam_install
///
/// Steps:
/// 0.5: Setup GreenLuma config
/// 1:   Kill Steam + Conflict ACF cleanup + Ghost ACF generation
/// 2:   Morrenus ZIP download/extract OR Vault restore (smart skip)
/// 3:   AppList patching with mandatory depot resolution + relationship recording
/// 4:   VDF key injection
/// 5:   Relaunch Steam with GreenLuma injection
#[tauri::command]
pub async fn steam_protocol_install(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    appid: String,
    name: String,
    library_path: String,
    install_dir: Option<String>,
    selected_dlcs: Vec<String>,
) -> Result<InstallResult, String> {
    let config = state.config_manager.config.lock().unwrap().clone();
    let steam_path = config.steam_path.clone();
    let gl_path = config.gl_path.clone();
    let enable_stealth = config.enable_stealth_mode;

    let emit = |step: &str, msg: &str, progress: f32| {
        let _ = app_handle.emit("install-progress", InstallProgress {
            step: step.to_string(),
            message: msg.to_string(),
            progress,
        });
    };

    // Log helper
    let log = |msg: &str| {
        if let Ok(mut logs) = state.system_log.lock() {
            logs.push(msg.to_string());
            println!("[INSTALL] {}", msg);
        }
    };

    log(&format!("START: Protocol for {} (AppID: {})", name, appid));
    emit("init", &format!("Installing {}", name), 0.0);

    // =========================================================================
    // STEP 0.5: SETUP GREENLUMA CONFIG
    // =========================================================================
    let gl_path_buf = Path::new(&gl_path).to_path_buf();
    if let Err(e) = crate::utils::gl_config::setup_greenluma_config(&gl_path_buf, enable_stealth) {
        log(&format!("Warning: Could not setup GreenLuma config: {}", e));
    } else {
        log(&format!("GreenLuma configured (Stealth: {}).", if enable_stealth { "ON" } else { "OFF" }));
    }

    // =========================================================================
    // STEP 1: KILL STEAM + CONFLICT ACF CLEANUP + GHOST ACF GENERATION
    // =========================================================================
    emit("step1", "Killing Steam Process...", 0.05);
    log("STEP 1: Killing Steam Process...");
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "steam.exe"])
        .output();
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let steam_root = steam_path.clone();
    let effective_library = if library_path.is_empty() {
        steam_path.clone()
    } else {
        library_path.clone()
    };

    log(&format!("Steam Root: {}", steam_root));
    log(&format!("Library Path: {}", effective_library));

    // Conflict ACF cleanup
    let acf_filename = format!("appmanifest_{}.acf", appid);
    let acf_path = Path::new(&effective_library)
        .join("steamapps")
        .join(&acf_filename);

    let all_libs = crate::game_path::GamePathFinder::get_library_folders(&steam_root);
    for lib in &all_libs {
        let lib_str = lib.to_string_lossy().to_string();
        if lib_str != effective_library {
            let conflict = lib.join("steamapps").join(&acf_filename);
            if conflict.exists() {
                log(&format!("Removing conflicting manifest at: {:?}", conflict));
                let _ = std::fs::remove_file(conflict);
            }
        }
    }

    // Vault Restore Check (with Version Verification) — v1.7.2 lines 668-725
    let vault = crate::vault::VaultManager::new_local();
    let skip_ghost = false;
    let mut skip_morrenus = false;
    let final_install_dir = install_dir.clone().unwrap_or_else(|| name.clone());

    let client = crate::api::ApiClient::new(config.api_key.clone());

    let mut vault_is_valid = false;
    if vault.has_manifests(&appid) {
        log("Vault: Checking manifest versions...");
        emit("step1", "Verifying Vault manifests...", 0.10);

        match client.get_public_gids(&appid).await {
            Ok(current_gids) => {
                let (is_valid, outdated_depots) = vault.verify_manifests(&appid, &current_gids);
                if is_valid {
                    log("✅ Vault manifests are up-to-date!");
                    vault_is_valid = true;
                } else {
                    log(&format!("⚠️ Vault outdated! {} depots need update.", outdated_depots.len()));
                    if let Err(e) = vault.invalidate_depots(&appid, &outdated_depots) {
                        log(&format!("Warning: Could not invalidate depots: {}", e));
                    }
                }
            }
            Err(e) => {
                log(&format!("⚠️ Could not verify vault version ({}). Using cached data.", e));
                vault_is_valid = true; // fail-safe
            }
        }
    }

    if vault_is_valid {
        let has_keys = vault.exists(&appid);
        // CORRECTION PROTOCOL: DO NOT RESTORE ACFs via restore_game().
        // Only check if we have the data to skip the download.
        if has_keys {
             log(&format!("Vault: Manifests found. SKIPPING MORRENUS (Token Saved). 🛡️"));
             skip_morrenus = true;
             // skip_ghost remains false, we ALWAYS generate a fresh ghost ACF.
        } else {
             log("Vault: Manifests verified but KEYS MISSING? Forcing Morrenus Download.");
        }
    }

    if !skip_ghost {
        if acf_path.exists() {
            log(&format!("Removing old ACF: {:?}", acf_path));
            let _ = std::fs::remove_file(&acf_path);
        }

        emit("step1", "Generating Ghost ACF...", 0.15);
        log(&format!("Generating Ghost ACF at: {:?}", acf_path));
        let empty_depots = std::collections::HashMap::new();
        if let Err(e) = crate::utils::generate_ghost_acf(&acf_path, &appid, &final_install_dir, &name, &empty_depots, 0, 0) {
            log(&format!("Error writing ACF: {}", e));
        } else {
            log("Ghost ACF generated. Steam will see game as 'Update Required'.");
        }
    }

    // =========================================================================
    // STEP 2: MORRENUS MANIFEST DOWNLOAD (OR VAULT SKIP)
    // =========================================================================
    let mut keys: HashMap<String, String> = HashMap::new();
    let depot_cache = Path::new(&steam_root).join("depotcache");
    if !depot_cache.exists() {
        let _ = std::fs::create_dir_all(&depot_cache);
    }

    let mut lua_content = String::new();

    if skip_morrenus {
        emit("step2", "Using Vault manifests (0 tokens) 🛡️", 0.30);
        log("STEP 2: SKIPPED - Using Vault manifests. 🛡️");

        // Load keys from Vault Lua
        if let Ok(lua_bytes) = vault.get_lua(&appid) {
            lua_content = String::from_utf8_lossy(&lua_bytes).to_string();
            let (_ids, parsed_keys) = parse_lua_for_keys(&lua_content);
            keys = parsed_keys;
            log(&format!("Vault: Restored {} decryption keys from backup Lua.", keys.len()));
        } else {
            log("Warning: Vault has manifests but NO Lua script. Keys might be missing!");
        }
    } else {
        emit("step2", "Downloading manifests from Morrenus...", 0.20);
        log("STEP 2: Fetching game data from Morrenus...");

        let zip_bytes = match client.download_manifest(&appid).await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                log(&format!("Download Error: {}", e));
                return Err(format!("Morrenus download failed: {}", e));
            }
        };

        emit("step2", "Extracting manifests...", 0.30);

        // Extract ZIP contents
        if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
            for i in 0..archive.len() {
                if let Ok(mut f) = archive.by_index(i) {
                    if f.name().ends_with(".lua") {
                        let _ = f.read_to_string(&mut lua_content);
                    } else if f.name().ends_with(".manifest") {
                        // FLATTEN to depotcache root (strip ZIP internal folders)
                        let fname_owned = Path::new(f.name())
                            .file_name()
                            .map(|n| n.to_owned());
                        if let Some(fname) = fname_owned {
                            let out_path = depot_cache.join(&fname);
                            let mut buf = Vec::new();
                            let _ = f.read_to_end(&mut buf);
                            
                            // RAW WRITE: Do not trim payload. Trust the ZIP content.
                            // Payload Trimming was likely corrupting valid manifests.
                            let _ = std::fs::write(&out_path, &buf);
                            log(&format!("Extracted Manifest: {:?}", fname));
                        }
                    }
                }
            }
        }

        if lua_content.is_empty() {
            lua_content = String::from_utf8_lossy(&zip_bytes).to_string();
        }

        // Parse Lua for keys
        let (ids, parsed_keys) = parse_lua_for_keys(&lua_content);
        keys = parsed_keys;
        log(&format!("Parsed {} AppIDs and {} Keys from LUA.", ids.len(), keys.len()));

        // VAULT BACKUP: Save downloaded data for future 0-token reinstalls
        emit("step2", "Backing up to Vault...", 0.35);
        log("Backing up Morrenus data to Vault...");

        // 1. Save Lua
        if let Err(e) = vault.save_lua(&appid, lua_content.as_bytes()) {
            log(&format!("Warning: Could not save Lua to Vault: {}", e));
        } else {
            log("✅ Lua script backed up to Vault.");
        }

        // 2. Save Manifests
        if let Ok(gids) = crate::api::ApiClient::extract_gids_from_zip(&zip_bytes) {
            for (depot_id, gid) in gids {
                if let Ok(mut archive_v) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes))
                {
                    let target_name = format!("{}_{}.manifest", depot_id, gid);
                    if let Ok(mut f) = archive_v.by_name(&target_name) {
                        let mut buf = Vec::new();
                        if f.read_to_end(&mut buf).is_ok() {
                             // RAW SAVE: Do not trim payload.
                            let _ = vault.store_manifest_bytes(
                                &appid,
                                depot_id.parse().unwrap_or(0),
                                gid.parse().unwrap_or(0),
                                &buf,
                            );
                        }
                    }
                }
            }
            log("✅ Depot manifests backed up to Vault.");
        }
    }

    // =========================================================================
    // STEP 3: UPDATE APPLIST
    // =========================================================================
    emit("step3", "Patching GreenLuma AppList...", 0.50);
    log("STEP 3: Patching GreenLuma AppList...");

    let mut final_ids = Vec::new();

    // === MANDATORY: Base Game AppID + Derived Depot ID ===
    // Rule: Depot ID = AppID with last digit replaced by 1
    // Examples: 322170 → 322171, 1091500 → 1091501, 730 → 731
    final_ids.push(appid.clone());
    let derived_depot = {
        let mut chars: Vec<char> = appid.chars().collect();
        if let Some(last) = chars.last_mut() {
            *last = '1';
        }
        chars.into_iter().collect::<String>()
    };
    if derived_depot != appid {
        final_ids.push(derived_depot.clone());
        log(&format!("Derived DepotID: {} → {}", appid, derived_depot));
    }

    // Use hierarchy if available (preferred — v1.7.2 lines 861-914)
    let target_language = config.target_language.clone();
    let hierarchy = client
        .fetch_full_hierarchy(&appid, &target_language)
        .await
        .ok();

    if let Some(ref h) = hierarchy {
        log("Using GameHierarchy for Mandatory Depot Resolution...");

        // resolve_mandatory_depots — v1.7.2 lines 1127-1160
        final_ids.push(h.root_id.clone());
        for depot in &h.base_depots {
            if depot.depot_id != "228980" && depot.depot_id != "228989" {
                final_ids.push(depot.depot_id.clone());
            }
        }
        for dlc in &h.dlcs {
            if selected_dlcs.contains(&dlc.app_id) {
                final_ids.push(dlc.app_id.clone());
                for depot in &dlc.depots {
                    if depot.depot_id != "228980" && depot.depot_id != "228989" {
                        final_ids.push(depot.depot_id.clone());
                    }
                }
            }
        }

        // Force add ANY selected DLCs not found in hierarchy (e.g. from Lua only)
        for sel_id in &selected_dlcs {
            if !final_ids.contains(sel_id) {
                log(&format!("Adding Non-Hierarchy DLC: {}", sel_id));
                final_ids.push(sel_id.clone());
            }
        }
        final_ids.sort();
        final_ids.dedup();
        log(&format!(
            "Resolved {} mandatory IDs (Base + DLCs + Depots).",
            final_ids.len()
        ));

        // Record Relationships & Types — v1.7.2 lines 866-876 + v2.0 Strict Types
        if let Ok(app_list_mgr) = state.app_list.lock() {
            let mut relationships = app_list_mgr.load_relationships();
            let mut types = app_list_mgr.load_types();
            
            let parent = h.root_id.clone();
            
            // 1. Relationships
            for id in &final_ids {
                if *id != parent {
                    relationships.insert(id.clone(), parent.clone());
                }
            }

            // 2. Types (Strict Scanner Support)
            // 2. Types (Strict Scanner Support)
            // Base Depots
            for depot in &h.base_depots {
                let type_str = if depot.is_dlc_depot { "depot_dlc" } else { "depot_base" };
                types.insert(depot.depot_id.clone(), type_str.to_string());
            }
            // DLCs (and their depots) - Only if in final_ids (selected)
            for dlc in &h.dlcs {
                 if final_ids.contains(&dlc.app_id) {
                     types.insert(dlc.app_id.clone(), "dlc".to_string());
                     for depot in &dlc.depots {
                         types.insert(depot.depot_id.clone(), "depot_dlc".to_string());
                     }
                 }
            }

            app_list_mgr.save_relationships(&relationships);
            app_list_mgr.save_types(&types);
            log("Relationships & Types saved.");
        }
    } else {
        // Fallback: Smart Merge from Lua — v1.7.2 lines 877-913
        log("No hierarchy available, using Smart Merge from Lua...");

        // appid already added above with derived depot
        for dlc in &selected_dlcs {
            if !final_ids.contains(dlc) {
                final_ids.push(dlc.clone());
            }
        }

        // Parse Lua for depots (SharedDepots only)
        if let Ok(script_data) =
            crate::utils::lua_parser::parse_content(&lua_content)
        {
            for depot in script_data.depots {
                if depot.category
                    == crate::utils::lua_parser::DepotCategory::SharedDepot
                {
                    let did = depot.depot_id.to_string();
                    if !final_ids.contains(&did) {
                        log(&format!("Included Shared Depot (Redist): {}", did));
                        final_ids.push(did);
                    }
                }
            }
        }

        log(&format!(
            "Smart Patching: {} IDs (Base + {} DLCs + Shared).",
            final_ids.len(),
            selected_dlcs.len()
        ));
    }

    // Add to AppList
    if let Ok(app_list_mgr) = state.app_list.lock() {
        match app_list_mgr.add_games_to_list(final_ids.clone()) {
            Ok(_) => log("AppList patched successfully."),
            Err(e) => log(&format!("Error patching AppList: {}", e)),
        }
    }

    // =========================================================================
    // STEP 4: INJECT VDF KEYS
    // =========================================================================
    let keys_count = keys.len();
    if !keys.is_empty() {
        emit("step4", &format!("Injecting {} Depot Keys...", keys_count), 0.70);
        log(&format!("STEP 4: Injecting {} Depot Keys...", keys_count));

        if let Ok(vdf) = state.vdf_injector.lock() {
            match vdf.inject_vdf(&keys) {
                Ok(count) => log(&format!("{} keys injected into config.vdf.", count)),
                Err(e) => log(&format!("Key Injection Error: {}", e)),
            }
        }
    } else {
        emit("step4", "No keys to inject.", 0.70);
        log("STEP 4: No depot keys to inject (Skipped).");
    }

    // =========================================================================
    // STEP 5: RELAUNCH STEAM VIA GREENLUMA
    // =========================================================================
    emit("step5", "Relaunching Steam (GreenLuma)...", 0.85);
    log("STEP 5: Relaunching Steam (GreenLuma Injection)...");

    let steam_exe = Path::new(&steam_path).join("steam.exe");
    let dll_name = "GreenLuma_2025_x64.dll";
    let dll_path = Path::new(&gl_path).join(dll_name);

    if steam_exe.exists() && dll_path.exists() {
        // Double-tap kill
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "steam.exe"])
            .output();
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        // Re-apply config
        let _ = crate::utils::gl_config::setup_greenluma_config(&gl_path_buf, enable_stealth);

        match crate::injector::launch_injected(
            steam_exe.to_str().unwrap_or("").to_string(),
            dll_path.to_str().unwrap_or("").to_string(),
            Some("-inhibitbootstrap".to_string()),
        ) {
            Ok(_) => log("✅ Steam Launched with GreenLuma."),
            Err(e) => log(&format!("❌ Launch Failed: {}", e)),
        }
    } else {
        if !steam_exe.exists() {
            log("❌ steam.exe not found.");
        }
        if !dll_path.exists() {
            log(&format!("❌ Error: Missing {}", dll_name));
        }
    }

    // =========================================================================
    // DONE
    // =========================================================================
    emit("done", "Installation complete!", 1.0);
    log("✅ Installation complete!");

    // Register source
    let final_install_dir_reg = install_dir.unwrap_or_else(|| name.clone());
    let mut registry = crate::registry::InstallRegistry::load();
    registry.register(
        appid.clone(),
        name.clone(),
        crate::registry::InstallSource::SteamCMD,
        final_install_dir_reg,
    );
    log("✅ Registered installation source: SteamCMD");

    Ok(InstallResult {
        success: true,
        message: format!("{} installed successfully", name),
        applist_ids_count: final_ids.len(),
        keys_injected: keys_count,
    })
}

/// Scan DLCs for an AppID — Returns list of available DLCs for the picker
/// Uses Vault-first strategy: check cached Lua before hitting API
#[tauri::command]
pub async fn scan_dlcs(
    state: State<'_, AppState>,
    appid: String,
) -> Result<Vec<DlcItem>, String> {
    let config = state.config_manager.config.lock().unwrap().clone();
    let client = crate::api::ApiClient::new(config.api_key.clone());
    let target_lang = config.target_language.clone();

    // Container for unique DLCs (id -> Item)
    let mut dlc_map: HashMap<String, DlcItem> = HashMap::new();

    // =========================================================================
    // 1. VAULT (PRIMARY SOURCE) - Local Crack Definitions
    // =========================================================================
    // =========================================================================
    // 1. VAULT (PRIMARY SOURCE) - Local Crack Definitions
    // =========================================================================
    let vault = crate::vault::VaultManager::new_local();
    
    // Log vault path for debugging
    eprintln!("[VAULT] Base path: {:?}, checking appid: {}", vault.get_base_path(), appid);
    
    // AUTO-DOWNLOAD: Only fetch from Morrenus if vault is MISSING or OLDER than 30 days
    if !vault.is_fresh(&appid, 30) {
        let reason = if vault.exists(&appid) { "stale (>30 days)" } else { "missing" };
        
        // LEVEL 2 CACHE: Check if we have a fresh raw ZIP even if Lua is missing
        let zip_result = if vault.is_zip_fresh(&appid, 30) {
             if let Ok(mut logs) = state.system_log.lock() {
                logs.push(format!("Vault: Level 2 Cache HIT (ZIP) for {}. Skipping download.", appid));
            }
            vault.get_zip(&appid).map_err(|e| format!("Failed to read cached ZIP: {}", e))
        } else {
            // ZIP missing/stale -> Download from Morrenus
            if let Ok(mut logs) = state.system_log.lock() {
                logs.push(format!("Vault: Data {} for {}. Fetching from Morrenus...", reason, appid));
            }
            match client.download_manifest(&appid).await {
                Ok(bytes) => {
                    // SAVE RAW ZIP (Level 2 Cache) - CRITICAL FOR TOKEN SAVING
                    if let Err(e) = vault.save_zip(&appid, &bytes) {
                        eprintln!("Vault: Failed to save ZIP cache: {}", e);
                    } else {
                         if let Ok(mut logs) = state.system_log.lock() {
                            logs.push(format!("Vault: Saved Level 2 Cache (ZIP) for {}", appid));
                        }
                    }
                    Ok(bytes.to_vec())
                },
                Err(e) => Err(e.to_string())
            }
        };

        // Process either Cached ZIP or Downloaded ZIP
        match zip_result {
            Ok(zip_bytes) => {
                // Extract and Save to Vault
                if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                    // 1. Find and Save Lua (use read_to_end for BOM/binary safety)
                    let mut found_lua_bytes: Vec<u8> = Vec::new();
                    for i in 0..archive.len() {
                        if let Ok(mut f) = archive.by_index(i) {
                            if f.name().ends_with(".lua") {
                                let _ = f.read_to_end(&mut found_lua_bytes);
                                break; 
                            }
                        }
                    }
                    if !found_lua_bytes.is_empty() {
                        match vault.save_lua(&appid, &found_lua_bytes) {
                            Ok(_) => {
                                if let Ok(mut logs) = state.system_log.lock() {
                                    logs.push(format!("Vault: Saved Lua ({} bytes) for {}", found_lua_bytes.len(), appid));
                                }
                            }
                            Err(e) => {
                                if let Ok(mut logs) = state.system_log.lock() {
                                    logs.push(format!("Vault: FAILED to save Lua for {}: {}", appid, e));
                                }
                            }
                        }
                    } else {
                        if let Ok(mut logs) = state.system_log.lock() {
                            logs.push(format!("Vault: No .lua found in ZIP for {}", appid));
                        }
                    }
                }
                
                // Helper to save manifests correctly
                if let Ok(gids) = crate::api::ApiClient::extract_gids_from_zip(&zip_bytes) {
                    for (depot_id, gid) in &gids {
                        let target_name = format!("{}_{}.manifest", depot_id, gid);
                        if let Ok(mut archive_v) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                            if let Ok(mut f) = archive_v.by_name(&target_name) {
                                let mut buf = Vec::new();
                                if f.read_to_end(&mut buf).is_ok() {
                                    // APPLIED FIX: Payload Trimming
                                    if buf.len() >= 8 {
                                        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                                        if magic == 0x71F617D0 {
                                            let payload_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
                                            if payload_len + 8 <= buf.len() {
                                                buf.truncate(payload_len + 8);
                                            }
                                        }
                                    }
                                    
                                    let _ = vault.store_manifest_bytes(
                                        &appid,
                                        depot_id.parse().unwrap_or(0),
                                        gid.parse().unwrap_or(0),
                                        &buf,
                                    );
                                }
                            }
                        }
                    }
                    if let Ok(mut logs) = state.system_log.lock() {
                        logs.push(format!("Vault: Saved {} manifests for {}", gids.len(), appid));
                    }
                }
            }
            Err(e) => {
                if let Ok(mut logs) = state.system_log.lock() {
                    logs.push(format!("Vault: Morrenus download FAILED for {}: {}", appid, e));
                }
            }
        }
    }

    // Now proceed with standard Vault check (it should be populated now)
    let has_lua = vault.exists(&appid);
    
    if has_lua {
        if let Ok(mut logs) = state.system_log.lock() {
            logs.push(format!("Vault: Reading cached data for {} (no API token used)", appid));
        }
    }
    
    if let Ok(lua_bytes) = vault.get_lua(&appid) {
        let content = String::from_utf8_lossy(&lua_bytes);
        // Use the robust parser from utils
        if let Ok(script_data) = crate::utils::lua_parser::parse_content(&content) {
            if !script_data.dlcs.is_empty() {
                // Primary: Use parsed DLCs
                for dlc in &script_data.dlcs {
                    let id_str = dlc.app_id.to_string();
                    dlc_map.insert(id_str.clone(), DlcItem {
                        app_id: id_str,
                        name: dlc.name.clone(),
                        depots_count: 0,
                        available: true,
                    });
                }
            } 
            // REMOVED FALLBACK: Do NOT guess depots are DLCs. 
            // If Lua says no DLCs, then trust it (and rely on API backup below).
        }
    }

    // =========================================================================
    // 2. API (SECONDARY SOURCE) - Enrichment & Verification
    // =========================================================================
    let hierarchy_result = client
        .fetch_full_hierarchy(&appid, &target_lang)
        .await;

    // We also need available depots from Morrenus for API-only DLCs
    let available_depots_result = client.get_available_depots().await;
    let available_depots = available_depots_result.unwrap_or_default();

    if let Ok(hierarchy) = hierarchy_result {
        for dlc in hierarchy.dlcs {
            let id_str = dlc.app_id.to_string();
            let is_in_lua = dlc_map.contains_key(&id_str);

            if is_in_lua {
                // ENRICHMENT: If name is generic/missing in Lua, use Official Name
                if let Some(item) = dlc_map.get_mut(&id_str) {
                     if item.name.to_lowercase().contains("dlc") && item.name.chars().any(|c| c.is_numeric()) && !dlc.name.is_empty() {
                         item.name = dlc.name.clone();
                     }
                     item.depots_count = dlc.depots.len();
                }
            } else {
                // ADD NEW: API found a DLC not in Lua
                // Check availability via Keys
                let mut is_available_online = false;
                if !dlc.depots.is_empty() {
                     for depot in &dlc.depots {
                         if available_depots.contains(&depot.depot_id) {
                             is_available_online = true;
                             break;
                         }
                     }
                }

                dlc_map.insert(id_str, DlcItem {
                    app_id: dlc.app_id,
                    name: dlc.name,
                    depots_count: dlc.depots.len(),
                    available: is_available_online, // Only available if keys exist
                });
            }
        }
    }

    // Convert map to vec and sort
    let mut dlc_items: Vec<DlcItem> = dlc_map.into_values().collect();
    dlc_items.sort_by(|a, b| {
        // Human sort by ID usually keeps chronological order
        a.app_id.parse::<u64>().unwrap_or(0).cmp(&b.app_id.parse::<u64>().unwrap_or(0))
    });

    // 4. Get current AppList count for slot calculation
    let current_count = if let Ok(app_list) = state.app_list.lock() {
        let cache = state.name_cache.lock().unwrap_or_else(|e| e.into_inner());
        app_list.refresh_active_games_list(&cache, &[], &HashMap::new()).len()
    } else {
        0
    };

    // Log for debugging
    if let Ok(mut logs) = state.system_log.lock() {
        logs.push(format!(
            "DLC Scan: Found {} items (Lua+API). Vault: {}. AppList: {}",
            dlc_items.len(), has_lua, current_count
        ));
    }

    Ok(dlc_items)
}

#[derive(Debug, Clone, Serialize)]
pub struct DlcItem {
    pub app_id: String,
    pub name: String,
    pub depots_count: usize,
    pub available: bool,
}

/// Get current AppList entry count (for DLC picker slot calculation)
#[tauri::command]
pub fn get_applist_count(state: State<'_, AppState>) -> usize {
    if let Ok(app_list) = state.app_list.lock() {
        let cache = state.name_cache.lock().unwrap_or_else(|e| e.into_inner());
        app_list.refresh_active_games_list(&cache, &[], &HashMap::new()).len()
    } else {
        0
    }
}

/// Save relationships map (exposed to frontend)
#[tauri::command]
pub fn save_relationships(
    state: State<'_, AppState>,
    relationships: HashMap<String, String>,
) -> Result<(), String> {
    if let Ok(app_list) = state.app_list.lock() {
        app_list.save_relationships(&relationships);
        Ok(())
    } else {
        Err("Failed to lock app_list".to_string())
    }
}

/// Disable Family Godmode — v1.7.2 port (lines 1053-1113)
#[tauri::command]
pub async fn disable_family_godmode(
    state: State<'_, AppState>,
    appid: String,
) -> Result<(), String> {
    let config = state.config_manager.config.lock().unwrap().clone();
    let gl_path = config.gl_path.clone();

    // Remove from config
    {
        let mut cfg = state.config_manager.config.lock().unwrap();
        if let Some(pos) = cfg.family_godmode_ids.iter().position(|x| *x == appid) {
            cfg.family_godmode_ids.remove(pos);
        }
    }

    let mut ids_to_remove = vec![appid.clone()];

    // Fetch DLCs to remove as well
    let client = crate::api::ApiClient::new(config.api_key.clone());
    if let Ok(dlcs) = client.get_dlc_list(&appid).await {
        ids_to_remove.extend(dlcs);
    }

    // Remove from AppList
    if let Ok(_app_list) = state.app_list.lock() {
        let gl_path_buf = Path::new(&gl_path).to_path_buf();
        let mgr = crate::app_list::AppListManager::new(&gl_path_buf, Path::new(&config.steam_path));
        mgr.remove_games_from_list(ids_to_remove)
            .map_err(|e| format!("Error removing from AppList: {}", e))?;
    }

    if let Ok(mut logs) = state.system_log.lock() {
        logs.push(format!("✅ Family Godmode Disabled for {}", appid));
    }

    Ok(())
}
