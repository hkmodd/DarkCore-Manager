//! Application state types and constants.
//!
//! This module contains supporting types, constants, and helper functions
//! that are used by the main `DarkCoreApp` in `ui_old.rs`.
//!
//! ## Migration Note
//! This is the first module being extracted from the monolithic `ui_old.rs`.
//! The main `DarkCoreApp` struct will be migrated in a later phase.

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::api::{ApiClient, SearchResult, UserStats};
use crate::app_list::GameProfile;
use crate::app_list::RelationshipMap;
use crate::config::AppConfig;
use crate::direct_download::state::DownloadState;
use crate::goldberg::GoldbergGenerator;
use crate::profiles::ProfileManager;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum number of IDs allowed in GreenLuma AppList.
/// GreenLuma has a hard limit of ~145, we use 130 for safety margin.
pub const APPLIST_LIMIT: usize = 130;

lazy_static::lazy_static! {
    /// Shared Async Runtime for background tasks.
    /// Prevents creating a new runtime for every thread (Performance Fix).
    pub static ref ASYNC_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create shared Tokio runtime");
}

/// Maximum number of log entries to keep in memory.
/// Prevents unbounded memory growth during long sessions.
pub const MAX_LOG_ENTRIES: usize = 100;

/// Maximum number of covers to keep in GPU cache.
/// Prevents unbounded VRAM usage.
pub const MAX_COVER_CACHE_SIZE: usize = 100;

/// Maximum number of Matrix trails for the About animation.
/// NOTE: 450 is the original artistic value - DO NOT reduce!
pub const MAX_MATRIX_TRAILS: usize = 450;

// ============================================================================
// TYPE ALIASES
// ============================================================================

/// Type alias for cover queue items: (AppID, Width, Height, Pixels)
pub type CoverQueueItem = (String, u32, u32, Vec<u8>);

// ============================================================================
// SUPPORTING TYPES
// ============================================================================

/// Pending installation data passed between install flow stages.
#[derive(Clone)]
pub struct PendingInstall {
    pub appid: String,
    pub name: String,
    pub target_library: Option<std::path::PathBuf>,
    pub install_dir_name: Option<String>,
    pub selected_dlcs: Vec<String>,
    pub cached_zip: Option<Vec<u8>>,
    pub hierarchy: Option<crate::api::GameHierarchy>,
}

/// Matrix rain effect trail for the easter egg animation.
#[derive(Clone)]
pub struct MatrixTrail {
    pub x: f32,
    pub head_y: f32,
    pub speed: f32,
    pub len: usize,
    pub chars: Vec<char>,
    pub layer: u8, // 0=Back (Slow/Small), 1=Mid, 2=Front (Fast/Large)
}

/// Holds data extracted from a user-imported Morrenus ZIP file.
pub struct ImportedZipData {
    pub script_data: crate::direct_download::lua_parser::ScriptData,
    pub manifest_bytes: HashMap<u32, Vec<u8>>,
    pub source_path: std::path::PathBuf,
}

// ============================================================================
// MAIN APP STRUCT
// ============================================================================

pub struct DarkCoreApp {
    pub(crate) config: AppConfig,
    pub(crate) active_tab: usize,

    // UI Variables
    pub(crate) search_query: String,
    pub(crate) last_searched_query: String,
    pub(crate) last_input_time: Option<Instant>,
    pub(crate) search_results: Arc<Mutex<Vec<SearchResult>>>,
    pub(crate) active_games: Arc<Mutex<Vec<GameProfile>>>, // Restored
    pub(crate) game_cache: Arc<Mutex<HashMap<String, String>>>,
    pub(crate) update_cache: Arc<Mutex<HashMap<String, bool>>>,
    pub(crate) relationships: Arc<Mutex<RelationshipMap>>, // New Relationship Map

    // Legacy: Steamless DRM tab fields (now integrated into Library)
    #[allow(dead_code)]
    pub(crate) target_exe: String,

    // Options
    pub(crate) include_dlcs: bool,

    // Async/Status
    pub(crate) status_msg: String, // Keep for header/footer quick status
    pub(crate) system_log: Arc<Mutex<Vec<String>>>,

    // Covers
    pub(crate) cover_cache:
        Arc<Mutex<std::collections::HashMap<String, Option<egui::TextureHandle>>>>,
    // Queue for loaded images
    pub(crate) cover_queue: Arc<Mutex<Vec<CoverQueueItem>>>,

    pub(crate) api_client: Option<ApiClient>,

    // Profiles
    pub(crate) profile_manager: ProfileManager,
    pub(crate) profile_name_input: String,
    pub(crate) active_profile_name: String,

    // Thread Communication
    pub(crate) status_update_queue: Arc<Mutex<Option<String>>>,

    // Matrix Easter Egg
    pub(crate) matrix_trails: Vec<MatrixTrail>,

    // API Key Glitch State
    pub(crate) api_key_glitch_update: Instant,
    pub(crate) api_key_glitch_cache: String,

    // Feedback State
    pub(crate) config_saved_at: Option<Instant>,
    pub(crate) api_refresh_timer: Option<Instant>, // Automation

    // UI State
    pub(crate) delete_modal_open: bool,
    pub(crate) delete_candidate_id: Option<String>,
    pub(crate) delete_candidate_name: Option<String>,
    pub(crate) delete_associated_dlcs: Vec<String>,
    pub(crate) is_scanning_dlcs: bool,
    // pub(crate) is_scanning_dlcs: bool, // Duplicate removed
    pub(crate) dlc_scan_result: Arc<Mutex<Option<(Vec<(String, String, bool, bool)>, usize)>>>, // (ID, Name, Selected, Available)
    pub(crate) dlc_scan_result_zip: Arc<Mutex<Option<Vec<u8>>>>, // Transfer ZIP bytes from thread
    pub(crate) delete_scan_result: Arc<Mutex<Option<Vec<String>>>>,

    // Install Modal
    pub(crate) install_modal_open: bool,
    pub(crate) install_candidate: Option<(String, String)>, // (AppID, Name)
    #[allow(dead_code)] // Reserved for F2P differentiation
    pub(crate) install_candidate_is_free: bool, // Tracks F2P status
    pub(crate) detected_libraries: Vec<std::path::PathBuf>,
    pub(crate) selected_library_index: usize,
    pub(crate) install_dir_input: String, // Manual override for Folder Name

    pub(crate) show_free_content: bool,

    pub(crate) create_profile_modal_open: bool,
    pub(crate) create_profile_save_default: bool,

    // NEW:
    pub(crate) delete_profile_modal_open: bool,

    // Family Shared vs Download choice modal
    pub(crate) family_or_download_modal_open: bool,

    pub(crate) logo_texture: Option<egui::TextureHandle>,
    pub(crate) logo_data: Option<egui::ColorImage>,
    pub(crate) tab_changed_at: Instant,

    // Audio Init
    pub(crate) _audio_stream: Option<rodio::OutputStream>,
    pub(crate) _audio_stream_handle: Option<rodio::OutputStreamHandle>,
    pub(crate) audio_sink: Option<rodio::Sink>,
    pub(crate) volume: f32,

    pub(crate) user_stats: Arc<Mutex<Option<UserStats>>>,
    pub(crate) api_last_error: Arc<Mutex<Option<String>>>,
    pub(crate) is_validating_api: Arc<Mutex<bool>>,
    pub(crate) request_api_refresh: Arc<Mutex<bool>>, // Signal from threads to main loop

    // DLC Picker
    pub(crate) dlc_picker_open: bool,
    pub(crate) dlc_picker_candidate: Option<(String, String)>,
    pub(crate) dlc_picker_items: Vec<(String, String, bool, bool)>, // (ID, Name, Selected, Available)
    pub(crate) dlc_picker_depot_count: usize,
    pub(crate) dlc_picker_search: String,
    pub(crate) dlc_picker_pending_library: Option<std::path::PathBuf>,
    pub(crate) dlc_picker_pending_install_dir: Option<String>,
    pub(crate) dlc_picker_cached_bytes: Option<Vec<u8>>,

    pub(crate) updates_downloading: Arc<Mutex<HashSet<String>>>,

    pub(crate) goldberg: GoldbergGenerator,
    pub(crate) goldberg_modal_open: bool,
    pub(crate) goldberg_candidate_id: Option<String>,
    pub(crate) goldberg_user_input: String,
    pub(crate) goldberg_steamid_input: String,
    pub(crate) goldberg_use_64bit: bool,

    // Watcher
    pub(crate) watcher_pending_updates: Arc<Mutex<HashMap<String, (String, u64, u64)>>>,
    pub(crate) watcher_updating: Arc<Mutex<HashSet<String>>>,

    // OTA Update System
    pub(crate) update_available: Arc<Mutex<Option<String>>>,
    pub(crate) is_updating: Arc<Mutex<bool>>,

    // Import ZIP Feature (Phase 3A)
    pub(crate) import_zip_data: Option<ImportedZipData>,
    pub(crate) import_modal_open: bool,

    // Manifestor
    pub(crate) manifestor_open: bool,
    pub(crate) manifestor_data: Arc<Mutex<Option<crate::api::GameHierarchy>>>,
    pub(crate) manifestor_candidate_id: Option<String>,
    pub(crate) manifestor_candidate_name: String,
    pub(crate) manifestor_target_library: Option<std::path::PathBuf>,
    #[allow(dead_code)] // Reserved for custom install name feature
    pub(crate) manifestor_install_name: String,
    pub(crate) manifestor_selections: Vec<String>, // IDs of selected depots

    pub(crate) download_state: Arc<Mutex<DownloadState>>,
    pub(crate) download_method_modal_open: bool,
    pub(crate) pending_install: Option<PendingInstall>,

    // FIX 8: Library search/filter
    pub(crate) library_search_query: String,

    // FIX 4: Track if install modal has auto-scanned
    pub(crate) install_modal_auto_scanned: bool,

    // Hover Card Details (Phase 17)
    pub(crate) hover_start_time: Option<(String, Instant)>,
    pub(crate) hover_details_cache: Arc<Mutex<HashMap<String, crate::api::GameDetails>>>,
    pub(crate) hover_loading: Arc<Mutex<HashSet<String>>>,
    pub(crate) show_detail_popup: Option<String>,

    // Premium Hover Animation (Phase 18)
    pub(crate) card_hover_scale: HashMap<String, f32>,
    pub(crate) card_rects: HashMap<String, (f32, f32, f32, f32)>,
    pub(crate) popup_fade_alpha: f32,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to push a log entry with FIFO rotation.
/// Use this instead of direct `logs.push()` to prevent memory leaks.
///
/// # Arguments
/// * `logs` - Mutable reference to the log vector
/// * `msg` - Message to add to the log
pub fn push_log(logs: &mut Vec<String>, msg: String) {
    logs.push(msg);
    while logs.len() > MAX_LOG_ENTRIES {
        logs.remove(0);
    }
}

impl DarkCoreApp {
    pub(crate) fn log<S: Into<String>>(&self, msg: S) {
        let msg = msg.into();
        if let Ok(mut logs) = self.system_log.lock() {
            push_log(&mut logs, msg);
        }
    }
}
