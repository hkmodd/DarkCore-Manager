use tauri::State;
use crate::state::AppState;
use std::collections::HashMap;

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Vec<String> {
    let logs = state.system_log.lock().unwrap();
    logs.clone()
}

#[tauri::command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn run_steamless(exe_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config_manager.config.lock().unwrap();
    let steamless_path = &config.steamless_path;

    if steamless_path.is_empty() {
        return Err("Steamless path not configured in settings.".to_string());
    }

    crate::steamless::run_steamless(&exe_path, steamless_path)
}

#[tauri::command]
pub fn generate_goldberg(appid: String, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config_manager.config.lock().unwrap();
    let steam_path = &config.steam_path;
    
    // Find game path
    let game_path = crate::game_path::GamePathFinder::find_game_path(steam_path, &appid)
        .ok_or("Could not locate game installation. Ensure it is installed in Steam.")?;
        
    let appid_u32 = appid.parse::<u32>().map_err(|_| "Invalid AppID")?;
    
    // Instantiate Generator (assumes core_data next to executable)
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let app_root = exe_path.parent().unwrap_or(std::path::Path::new("."));
    
    let generator = crate::goldberg::GoldbergGenerator::new(app_root);
    
    generator.deploy(&game_path, appid_u32, true).map_err(|e| e.to_string())?;
    
    // Set Language
    let language = &config.target_language;
    if !language.is_empty() {
        generator.set_language(&game_path, language).map_err(|e| e.to_string())?;
    }

    generator.generate_ticket(appid_u32, &game_path).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn check_updates(_state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    // Wrapper for watcher check
    Ok(HashMap::new()) // Placeholder
}

#[tauri::command]
pub async fn get_api_stats(state: State<'_, AppState>) -> Result<crate::api::UserStats, String> {
    let client = {
        let guard = state.api_client.lock().unwrap();
        guard.clone()
    };

    if let Some(client) = client {
        client.get_user_stats().await.map_err(|e| e.to_string())
    } else {
        Err("API Client not initialized (No Key)".to_string())
    }
}

#[tauri::command]
pub async fn launch_greenluma_stealth(state: State<'_, AppState>) -> Result<(), String> {
    let (steam_path, gl_path, enable_stealth) = {
        let config = state.config_manager.config.lock().unwrap();
        (config.steam_path.clone(), config.gl_path.clone(), config.enable_stealth_mode)
    };

    // 1. Kill Steam & Injectors (User Requirement)
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "steam.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "steamwebhelper.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "DLLInjector.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "spawner.exe"])
        .output();
    
    // Wait for shutdown
    std::thread::sleep(std::time::Duration::from_millis(3000));

    // 2. Setup Config
    let gl_path_buf = std::path::PathBuf::from(&gl_path);
    if let Err(e) = crate::utils::gl_config::setup_greenluma_config(&gl_path_buf, enable_stealth) {
        println!("Warning: Failed to setup GreenLuma config: {}", e);
    }

    // Log
    {
        let mut log = state.system_log.lock().unwrap();
        log.push("[SUCCESS] Initiating Stealth Start (Kill -> Inject)...".to_string());
    }

    // 3. Launch
    let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
    let gl_dll = gl_path_buf.join("GreenLuma_2025_x64.dll");

    crate::injector::launch_injected(
        steam_exe.to_string_lossy().to_string(),
        gl_dll.to_string_lossy().to_string(),
        Some("-inhibitbootstrap".to_string())
    )
}

#[tauri::command]
pub async fn relaunch_steam(state: State<'_, AppState>) -> Result<(), String> {
    let steam_path = {
        let config = state.config_manager.config.lock().unwrap();
        config.steam_path.clone()
    };

    // 1. Kill Steam & Injectors
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "steam.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "steamwebhelper.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "DLLInjector.exe"])
        .output();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "spawner.exe"])
        .output();
    
    // Wait for shutdown
    std::thread::sleep(std::time::Duration::from_millis(3000));

    // 2. CHECK FOR PROXY DLLS (The likely cause of persistent injection)
    // GreenLuma often installs as user32.dll, version.dll, etc. in the Steam folder.
    let steam_dir = std::path::Path::new(&steam_path);
    let proxy_dlls = ["user32.dll", "version.dll", "winmm.dll", "winhttp.dll", "d3d9.dll"];
    let mut found_proxies = Vec::new();

    for dll in proxy_dlls {
        if steam_dir.join(dll).exists() {
            found_proxies.push(dll);
        }
    }

    if !found_proxies.is_empty() {
        {
            let mut log = state.system_log.lock().unwrap();
            log.push(format!("[WARNING] Proxy DLLs detected in Steam folder: {:?}. These force GreenLuma injection.", found_proxies));
            log.push("[ERROR] clean steam aborted. User must remove local DLLs manually to play clean.".to_string());
        }
        return Err(format!("Clean Launch Impossible: GreenLuma proxy DLLs found in Steam folder ({:?}). Please remove them or use Stealth Mode.", found_proxies));
    }

    // Log
    {
        let mut log = state.system_log.lock().unwrap();
        log.push("[SUCCESS] Restarting Clean Steam (No Injection)...".to_string());
    }

    // 3. Launch Clean
    let steam_exe = steam_dir.join("steam.exe");
    
    std::process::Command::new(steam_exe)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}
