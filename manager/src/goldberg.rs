#![allow(dead_code)] // Reserved: generate_dlc_config for future Goldberg DLC support
use libloading::{Library, Symbol};
// use log::{error, info, warn};
use std::fs;

use std::path::{Path, PathBuf};
use serde_json::json;
use crate::api::ApiClient;

#[derive(Clone)]
pub struct GoldbergGenerator {
    pub dll_source_path: PathBuf,
}

impl GoldbergGenerator {
    pub fn new(app_data_path: &Path) -> Self {
        let dll_path = app_data_path.join("core_data");
        Self {
            dll_source_path: dll_path,
        }
    }

    /// Deploy Goldberg Emulator to the game directory
    pub fn deploy(&self, game_path: &Path, app_id: u32, use_64bit: bool) -> Result<(), String> {
        println!("Deploying Goldberg for AppID: {}", app_id);

        let target_dll_name = if use_64bit {
            "steam_api64.dll"
        } else {
            "steam_api.dll"
        };
        let source_dll = self.dll_source_path.join(target_dll_name);

        if !source_dll.exists() {
            return Err(format!("Source DLL not found: {:?}", source_dll));
        }

        let target_path = game_path.join(target_dll_name);

        // Backup original if it exists and is NOT Goldberg (basic check size or signature? logic needed)
        // For now, simpler backup:
        let backup_path = game_path.join(format!("{}.darkcore_backup", target_dll_name));
        if target_path.exists() && !backup_path.exists() {
            fs::copy(&target_path, &backup_path)
                .map_err(|e| format!("Failed to backup original DLL: {}", e))?;
        }

        // Copy Goldberg DLL
        fs::copy(&source_dll, &target_path)
            .map_err(|e| format!("Failed to copy Goldberg DLL: {}", e))?;

        // Create steam_settings folder
        let settings_dir = game_path.join("steam_settings");
        if !settings_dir.exists() {
            fs::create_dir(&settings_dir).map_err(|e| e.to_string())?;
        }

        // Write steam_appid.txt
        let appid_file = settings_dir.join("steam_appid.txt");
        fs::write(&appid_file, app_id.to_string())
            .map_err(|e| format!("Failed to write steam_appid.txt: {}", e))?;

        Ok(())
    }

    /// Generate a valid Ticket using the Emulator's internal logic
    /// This binds to the Goldberg DLL, initializes a fake Steam context, requests a ticket,
    /// and writes it to `steam_settings/configs.user.ini`.
    /// Generate a valid Ticket using the Emulator's internal logic
    /// This binds to the Goldberg DLL, initializes a fake Steam context, requests a ticket,
    /// and writes it to `steam_settings/configs.user.ini`.
    pub fn generate_ticket(&self, app_id: u32, game_path: &Path) -> Result<(), String> {
        println!("Starting Native Ticket Generation for AppID: {}", app_id);

        let is_64bit = true; // TODO: Detect from game EXE, defaulting to 64-bit for now as most modern games are
                             // We use the DLLs in our core_data, not the deployed ones (to avoid locking game files if we were to run this while game is running, though rare)
        let dll_name = if is_64bit {
            "steam_api64.dll"
        } else {
            "steam_api.dll"
        };
        let dll_path = self.dll_source_path.join(dll_name);

        if !dll_path.exists() {
            return Err(format!("DLL not found for generation: {:?}", dll_path));
        }

        // Set Environment Variable for Goldberg to know which AppID to initialize
        std::env::set_var("SteamAppId", app_id.to_string());
        // Also set SteamGameId just in case
        std::env::set_var("SteamGameId", app_id.to_string());

        let ticket_hex = unsafe {
            // Load the library
            let lib = Library::new(&dll_path)
                .map_err(|e| format!("Failed to load Goldberg DLL: {}", e))?;

            // Bind Symbols
            let steam_init: Symbol<extern "C" fn() -> bool> =
                lib.get(b"SteamAPI_Init").map_err(|e| e.to_string())?;
            let steam_shutdown: Symbol<extern "C" fn()> =
                lib.get(b"SteamAPI_Shutdown").map_err(|e| e.to_string())?;

            // Try v023, then v022, etc. (Goldberg usually exports latest)
            let get_user: Symbol<extern "C" fn() -> *mut std::ffi::c_void> = lib
                .get(b"SteamAPI_SteamUser_v023")
                .or_else(|_| lib.get(b"SteamAPI_SteamUser_v022"))
                .or_else(|_| lib.get(b"SteamAPI_SteamUser_v021")) // Fallback
                .map_err(|e| format!("Failed to find SteamAPI_SteamUser export: {}", e))?;

            let request_ticket: Symbol<
                extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, i32) -> u64,
            > = lib
                .get(b"SteamAPI_ISteamUser_RequestEncryptedAppTicket")
                .map_err(|e| e.to_string())?;

            let get_ticket: Symbol<
                extern "C" fn(*mut std::ffi::c_void, *mut u8, i32, *mut u32) -> bool,
            > = lib
                .get(b"SteamAPI_ISteamUser_GetEncryptedAppTicket")
                .map_err(|e| e.to_string())?;

            // Initialize
            if !steam_init() {
                return Err("SteamAPI_Init failed".to_string());
            }

            let user_ptr = get_user();
            if user_ptr.is_null() {
                steam_shutdown();
                return Err("Failed to get ISteamUser interface".to_string());
            }

            // Request Ticket (Goldberg synchronous-ish generation)
            // We pass NULL for data to include
            let _handle = request_ticket(user_ptr, std::ptr::null_mut(), 0);

            // Fetch the generated ticket
            let mut buffer = vec![0u8; 2048];
            let mut out_len = 0u32;

            let result = if get_ticket(user_ptr, buffer.as_mut_ptr(), 2048, &mut out_len) {
                // Resize buffer to actual length
                buffer.truncate(out_len as usize);
                Some(hex::encode(&buffer))
            } else {
                None
            };

            // Shutdown
            steam_shutdown();
            result
        };

        // Unset env vars
        std::env::remove_var("SteamAppId");
        std::env::remove_var("SteamGameId");

        if let Some(hex_ticket) = ticket_hex {
            println!("Generated valid ticket: {} chars", hex_ticket.len());

            // Write to configs.user.ini
            let settings_dir = game_path.join("steam_settings");
            if !settings_dir.exists() {
                fs::create_dir_all(&settings_dir).map_err(|e| e.to_string())?;
            }

            let config_file = settings_dir.join("configs.user.ini");
            let mut content = String::new();

            // Preserve existing content if possible
            if config_file.exists() {
                content = fs::read_to_string(&config_file).unwrap_or_default();
            }

            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.trim().starts_with("customEncryptedAppTicket=") {
                    *line = format!("customEncryptedAppTicket={}", hex_ticket);
                    found = true;
                    break;
                }
            }

            if !found {
                lines.push(format!("customEncryptedAppTicket={}", hex_ticket));
            }

            fs::write(&config_file, lines.join("\n")).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Failed to GetEncryptedAppTicket via SteamAPI".to_string())
        }
    }

    /// Download Achievements and Icons for Goldberg
    pub async fn download_achievements(
        &self, 
        appid: &str, 
        client: &ApiClient, 
        game_path: &Path
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let schema = client.get_schema_for_game(appid).await?;
        
        let settings_dir = game_path.join("steam_settings");
        let images_dir = settings_dir.join("images"); // Goldberg ignored images folder? No, usually in images or adjacent?
        // Goldberg docs: "By default, the emulator will look for achievement images in the folder steam_settings/images"
        if !images_dir.exists() {
             std::fs::create_dir_all(&images_dir)?;
        }

        let mut json_achievements = Vec::new();
        let mut download_count = 0;

        if let Some(stats) = schema.game.available_game_stats {
            if let Some(achievements) = stats.achievements {
                for ach in achievements {
                    let safe_name = ach.name.clone();
                    
                    // JSON Object for Goldberg
                    let mut obj = json!({
                        "name": ach.name,
                        "displayName": ach.display_name.clone().unwrap_or(ach.name.clone()),
                        "description": ach.description.clone().unwrap_or_default(),
                        "hidden": ach.hidden.unwrap_or(0)
                    });

                    // Download Icons
                    if let Some(url) = &ach.icon {
                        let fname = format!("{}", safe_name); // Goldberg format: name only, looks for .jpg
                        let path = images_dir.join(format!("{}.jpg", fname));
                        
                        if !path.exists() {
                            if let Ok(bytes) = client.download_file(url).await {
                                let _ = std::fs::write(&path, &bytes);
                                download_count += 1;
                            }
                        }
                        obj["icon"] = json!(fname);
                    }

                    if let Some(url) = &ach.icon_gray {
                         let fname = format!("{}_gray", safe_name);
                         let path = images_dir.join(format!("{}.jpg", fname));
                         
                         if !path.exists() {
                            if let Ok(bytes) = client.download_file(url).await {
                                let _ = std::fs::write(&path, &bytes);
                            }
                         }
                         obj["iconGray"] = json!(fname);
                    }

                    json_achievements.push(obj);
                }
            }
        }

        // Write achievements.json
        let json_path = settings_dir.join("achievements.json");
        let json_str = serde_json::to_string_pretty(&json_achievements)?;
        std::fs::write(json_path, json_str)?;

        Ok(format!("Downloaded {} achievements and {} icons.", json_achievements.len(), download_count))
    }

    /// Generate configs.app.ini with DLCs
    pub async fn generate_dlc_config(
        &self, 
        appid: &str, 
        client: &ApiClient, 
        game_path: &Path
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch info from SteamCMD
        let info = client.get_app_info(appid).await?;
        
        let settings_dir = game_path.join("steam_settings");
        if !settings_dir.exists() { std::fs::create_dir_all(&settings_dir)?; }
        
        let config_path = settings_dir.join("configs.app.ini");
        
        // Prepare content
        let mut content = String::from("[app::dlcs]\nunlock_all=0\n");
        
        let count = info.dlcs.len();
        if count == 0 {
             return Ok("No DLCs found.".to_string());
        }

        for dlc_id in &info.dlcs {
             content.push_str(&format!("{} = DLC\n", dlc_id));
        }

        std::fs::write(&config_path, content)?;

        Ok(format!("Refreshed DLC config with {} items.", count))
    }
}
