pub mod api;
pub mod app_list;
pub mod cache;
pub mod commands;
pub mod config;
pub mod direct_download;
pub mod downloader;
pub mod game_path;
pub mod goldberg;
pub mod injector;
pub mod profiles;
pub mod registry;
pub mod state;
// commands module defined in mod.rs

pub mod services;
pub mod steamless;
pub mod updater;
pub mod utils;
pub mod vault;
pub mod vdf_injector;
pub mod watcher;

use crate::state::AppState;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize Config
            let config_manager = Arc::new(config::ConfigManager::new());
            let config = config_manager.get();

            // Initialize Managers
            let vault_manager = vault::VaultManager::new(app.handle());
            let downloader = Arc::new(downloader::ManifestDownloader::new());
            let app_list_manager = app_list::AppListManager::new(
                Path::new(&config.gl_path),
                Path::new(&config.steam_path),
            );
            let vdf_injector = vdf_injector::VdfInjector::new(Path::new(&config.steam_path));
            let watcher = Arc::new(crate::watcher::Watcher::new(15)); // Check every 15 mins
            let profile_manager = profiles::ProfileManager::new();

            // Initialize API Client if key exists
            let api_client = if !config.api_key.is_empty() {
                Some(api::ApiClient::new(config.api_key.clone()))
            } else {
                None
            };

            let direct_downloader =
                Arc::new(crate::services::downloader::download_engine::DirectDownloader::new());
            let download_state = Arc::new(tokio::sync::Mutex::new(
                crate::services::downloader::download_engine::DownloadState::new(),
            ));

            app.manage(AppState {
                config_manager,
                watcher,
                api_client: Mutex::new(api_client),
                system_log: Mutex::new(Vec::new()),
                name_cache: Mutex::new(crate::cache::load_game_cache()),
                profile_manager: Mutex::new(profile_manager),
                active_profile: Mutex::new(String::new()),
                vault: Mutex::new(vault_manager),
                downloader,
                app_list: Mutex::new(app_list_manager),
                vdf_injector: Mutex::new(vdf_injector),
                watcher_pending: Mutex::new(HashMap::new()),
                direct_downloader,
                download_state,
            });
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Inherited/Inlined Commands (Keep these or move them?)
            injector::launch_injected,
            injector::is_greenluma_injected,
            injector::is_process_running,
            // Search & API
            commands::search::search_games,
            commands::search::get_game_details,
            commands::search::get_cover_url,
            // Library & Profiles
            commands::library::get_active_games,
            commands::library::get_profiles,
            commands::library::load_profile,
            commands::library::save_profile,
            commands::library::create_profile,
            commands::library::delete_profile,
            commands::library::reorder_list,
            commands::library::remove_game_from_applist,
            commands::library::add_games_to_list,
            commands::library::update_name_cache,
            commands::library::inject_vdf_keys,
            commands::library::import_legacy_applist,
            commands::library::scan_delete_children,
            commands::library::full_delete_game,
            commands::library::check_legacy_exists,
            commands::library::repair_library_relationships,
            // Install & Downloads
            commands::install::get_app_info,
            commands::install::fetch_hierarchy,
            commands::install::get_library_folders,
            commands::install::detect_install_path,
            commands::install::download_manifest,
            commands::install::parse_lua_script,
            commands::install::trigger_steam_install,
            commands::install::install_godmode,
            commands::install::resolve_install_ids,
            // Steam Install (v1.7.2 Full Pipeline)
            commands::steam_install::steam_protocol_install,
            commands::steam_install::scan_dlcs,
            commands::steam_install::get_applist_count,
            commands::steam_install::save_relationships,
            commands::steam_install::disable_family_godmode,
            // Settings
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::validate_path,
            commands::settings::validate_api_key,
            // System & Utilities
            commands::system::get_logs,
            commands::system::get_version,
            commands::system::get_api_stats,
            commands::system::run_steamless,
            commands::system::generate_goldberg,
            commands::system::relaunch_steam,
            commands::system::launch_greenluma_stealth,
            // Watcher
            commands::watcher::check_updates_cmd,
            commands::watcher::update_game_manifests,
            commands::watcher::run_startup_scan,
            commands::watcher::scan_for_updates_async,
            // Vault
            commands::vault::get_vault_games,
            commands::vault::delete_vault_game,
            // Integrity
            commands::integrity::verify_integrity,
            // Import
            commands::import::scan_zip_for_import,
            commands::import::import_zip_action,
            // Direct Download
            commands::direct_download::start_direct_download,
            commands::direct_download::get_download_status,
            commands::direct_download::pause_download,
            commands::direct_download::resume_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
