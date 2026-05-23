use crate::api::ApiClient;
use crate::app_list::AppListManager;
use crate::config::ConfigManager;
use crate::downloader::ManifestDownloader;
use crate::profiles::ProfileManager;
use crate::vault::VaultManager;
use crate::vdf_injector::VdfInjector;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
// Note: download_state uses tokio::sync::Mutex

pub struct AppState {
    pub config_manager: Arc<ConfigManager>, // Wraps config
    pub watcher: Arc<crate::watcher::Watcher>,
    pub api_client: Mutex<Option<ApiClient>>,
    pub system_log: Mutex<Vec<String>>,
    pub name_cache: Mutex<HashMap<String, String>>, // AppID -> Name
    pub profile_manager: Mutex<ProfileManager>,
    pub active_profile: Mutex<String>,
    pub vault: Mutex<VaultManager>,
    pub downloader: Arc<ManifestDownloader>,
    pub app_list: Mutex<AppListManager>,
    pub vdf_injector: Mutex<VdfInjector>,
    // Map<AppID, (VersionName/Gid, Size, Time)>
    pub watcher_pending: Mutex<HashMap<String, (String, u64, u64)>>,

    // Direct Download Engine
    pub direct_downloader: Arc<crate::services::downloader::download_engine::DirectDownloader>,
    pub download_state:
        Arc<tokio::sync::Mutex<crate::services::downloader::download_engine::DownloadState>>,
}
