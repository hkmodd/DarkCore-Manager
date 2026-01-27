use crate::api::{ApiClient, SearchResult};
use crate::app_list::{
    add_games_to_list, nuke_reorder, refresh_active_games_list, GameProfile,
};
use crate::cache::{load_game_cache, save_game_cache};
use crate::config::{load_config, save_config, AppConfig};
use crate::profiles::{Profile, ProfileManager};
use crate::steamless;
use crate::vdf_injector::inject_vdf;
use crate::vault::VaultManager;
use eframe::egui;
use rodio::Source;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use std::time::{Duration, Instant};

/// Maximum number of IDs allowed in GreenLuma AppList.
/// GreenLuma has a hard limit of ~145, we use 130 for safety margin.
pub const APPLIST_LIMIT: usize = 130;

/// Maximum number of log entries to keep in memory.
/// Prevents unbounded memory growth during long sessions.
const MAX_LOG_ENTRIES: usize = 100;

/// Helper function to push a log entry with FIFO rotation.
/// Use this instead of direct logs.push() to prevent memory leaks.
fn push_log(logs: &mut Vec<String>, msg: String) {
    logs.push(msg);
    while logs.len() > MAX_LOG_ENTRIES {
        logs.remove(0);
    }
}

/// Type alias for cover queue items: (AppID, Width, Height, Pixels)
type CoverQueueItem = (String, u32, u32, Vec<u8>);

#[derive(Clone)]
struct MatrixTrail {
    x: f32,
    head_y: f32,
    speed: f32,
    len: usize,
    chars: Vec<char>,
    layer: u8, // 0=Back (Slow/Small), 1=Mid, 2=Front (Fast/Large)
}

pub struct DarkCoreApp {
    config: AppConfig,
    active_tab: usize,

    // UI Variables
    search_query: String,
    last_searched_query: String,
    last_input_time: Option<Instant>,
    search_results: Arc<Mutex<Vec<SearchResult>>>,
    active_games: Arc<Mutex<Vec<GameProfile>>>, // Restored
    game_cache: Arc<Mutex<HashMap<String, String>>>,
    update_cache: Arc<Mutex<HashMap<String, bool>>>,
    relationships: Arc<Mutex<crate::app_list::RelationshipMap>>, // New Relationship Map

    // Legacy: Steamless DRM tab fields (now integrated into Library)
    #[allow(dead_code)]
    target_exe: String,

    // Options
    include_dlcs: bool,

    // Async/Status
    status_msg: String, // Keep for header/footer quick status
    system_log: Arc<Mutex<Vec<String>>>,

    // Covers
    cover_cache: Arc<Mutex<std::collections::HashMap<String, Option<egui::TextureHandle>>>>,
    // Queue for loaded images
    cover_queue: Arc<Mutex<Vec<CoverQueueItem>>>,

    api_client: Option<ApiClient>,

    // Profiles
    profile_manager: ProfileManager,
    profile_name_input: String,
    active_profile_name: String,

    // Thread Communication
    status_update_queue: Arc<Mutex<Option<String>>>,
    
    // Matrix Easter Egg
    matrix_trails: Vec<MatrixTrail>,

    // API Key Glitch State
    api_key_glitch_update: Instant,
    api_key_glitch_cache: String,

    // Feedback State
    config_saved_at: Option<Instant>,
    api_refresh_timer: Option<Instant>, // Automation

    // UI State
    delete_modal_open: bool,
    delete_candidate_id: Option<String>,
    delete_candidate_name: Option<String>,
    delete_associated_dlcs: Vec<String>,
    is_scanning_dlcs: bool,
    dlc_scan_result: Arc<Mutex<Option<(Vec<(String, String, bool)>, usize)>>>,
    dlc_scan_result_zip: Arc<Mutex<Option<Vec<u8>>>>, // NEW: Transfer ZIP bytes from thread
    delete_scan_result: Arc<Mutex<Option<Vec<String>>>>,
    
    // Install Modal
    install_modal_open: bool,
    install_candidate: Option<(String, String)>, // (AppID, Name)
    install_candidate_is_free: bool, // NEW: Tracks F2P status
    detected_libraries: Vec<std::path::PathBuf>,
    selected_library_index: usize,
    install_dir_input: String, // NEW: Manual override for Folder Name
    
    // Filters
    show_free_content: bool, // NEW: Toggle F2P visibility

    // New Profile Modal
    create_profile_modal_open: bool,
    create_profile_save_default: bool, // Checkbox state
    delete_profile_modal_open: bool, // NEW: Delete Confirmation Modal

    // Identity & Animation
    logo_texture: Option<egui::TextureHandle>,
    logo_data: Option<egui::ColorImage>,
    tab_changed_at: Instant,

    // Audio
    _audio_stream: Option<rodio::OutputStream>,
    _audio_stream_handle: Option<rodio::OutputStreamHandle>,
    audio_sink: Option<rodio::Sink>,
    volume: f32,



    user_stats: Arc<Mutex<Option<crate::api::UserStats>>>,
    api_last_error: Arc<Mutex<Option<String>>>,
    is_validating_api: Arc<Mutex<bool>>, // New

    // DLC Picker Modal (for large DLC games like Beat Saber)
    dlc_picker_open: bool,
    dlc_picker_candidate: Option<(String, String)>, // (AppID, Name)
    dlc_picker_items: Vec<(String, String, bool)>,  // (DLC ID, DLC Name, Selected)
    dlc_picker_depot_count: usize,                  // Number of base depots (always included)
    dlc_picker_search: String,                      // Search filter
    dlc_picker_pending_library: Option<std::path::PathBuf>,
    dlc_picker_pending_install_dir: Option<String>,
    dlc_picker_cached_bytes: Option<Vec<u8>>,      // NEW: State to hold bytes for finalize_installation

    // Update Detection
    updates_downloading: Arc<Mutex<std::collections::HashSet<String>>>,

    // Goldberg
    goldberg: crate::goldberg::GoldbergGenerator,
    goldberg_modal_open: bool,
    goldberg_candidate_id: Option<String>,
    goldberg_user_input: String,
    goldberg_steamid_input: String,
    goldberg_use_64bit: bool, // NEW: Architecture selection

    // Watcher: Pending Updates (AppID -> (Name, OldBuild, NewBuild))
    watcher_pending_updates: Arc<Mutex<HashMap<String, (String, u64, u64)>>>,
    watcher_updating: Arc<Mutex<HashSet<String>>>, // AppIDs currently being updated

    // Manifestor (The New DLC Selector)
    manifestor_open: bool,
    manifestor_data: Arc<Mutex<Option<crate::api::GameHierarchy>>>,
    manifestor_candidate_id: Option<String>,
    manifestor_candidate_name: String, // Just for header display
    manifestor_target_library: Option<std::path::PathBuf>,
    manifestor_install_name: String,
    manifestor_selections: Vec<String>, // IDs of selected DLCs

    // OTA Update System
    update_available: Arc<Mutex<Option<String>>>, // Contains new version string if available
    is_updating: Arc<Mutex<bool>>, // True during update download
}

impl Default for DarkCoreApp {
    fn default() -> Self {
        Self {
            config: crate::config::load_config(),
            active_tab: 0,
            search_query: String::new(),
            last_searched_query: String::new(),
            last_input_time: None,
            search_results: Arc::new(Mutex::new(Vec::new())),
            active_games: Arc::new(Mutex::new(Vec::new())),
            game_cache: Arc::new(Mutex::new(HashMap::new())),
            update_cache: Arc::new(Mutex::new(HashMap::new())),
            relationships: Arc::new(Mutex::new(HashMap::new())), // New
            target_exe: String::new(),
            include_dlcs: true,
            status_msg: "Ready.".to_string(),
            status_update_queue: Arc::new(Mutex::new(None)),
            system_log: Arc::new(Mutex::new(Vec::new())),
            api_key_glitch_cache: String::new(),
            api_key_glitch_update: Instant::now(),
            cover_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cover_queue: Arc::new(Mutex::new(Vec::new())),
            api_client: None, // Init in new()
            profile_manager: ProfileManager::new("."),
            profile_name_input: String::new(),
            active_profile_name: "Default".to_string(),
            delete_modal_open: false,
            delete_candidate_id: None,
            delete_candidate_name: None,
            delete_associated_dlcs: Vec::new(),
            is_scanning_dlcs: false,
            dlc_scan_result: Arc::new(Mutex::new(None)),
            delete_scan_result: Arc::new(Mutex::new(None)),
            dlc_scan_result_zip: Arc::new(Mutex::new(None)),
            
            // Manifestor Init
            manifestor_open: false,
            manifestor_data: Arc::new(Mutex::new(None)),
            manifestor_candidate_id: None,
            manifestor_candidate_name: String::new(),
            manifestor_target_library: None,
            manifestor_install_name: String::new(),
            manifestor_selections: Vec::new(),
            
            // Install Modal
            install_modal_open: false,
            install_candidate: None,
            install_candidate_is_free: false, // NEW
            detected_libraries: Vec::new(),
            selected_library_index: 0,
            install_dir_input: String::new(), // Init
            
            show_free_content: false, // Default

            create_profile_modal_open: false,
            create_profile_save_default: true,
            
            // NEW:
            delete_profile_modal_open: false,
            
            logo_texture: None,
            logo_data: None,
            tab_changed_at: Instant::now(),
            _audio_stream: None,
            _audio_stream_handle: None,
            audio_sink: None,
            volume: 0.5,


            user_stats: Arc::new(Mutex::new(None)),
            api_last_error: Arc::new(Mutex::new(None)),
            is_validating_api: Arc::new(Mutex::new(false)),
            matrix_trails: Vec::new(),
            config_saved_at: None,
            api_refresh_timer: None,
            
            // DLC Picker
            dlc_picker_open: false,
            dlc_picker_candidate: None,
            dlc_picker_items: Vec::new(),
            dlc_picker_depot_count: 0,
            dlc_picker_search: String::new(),
            dlc_picker_pending_library: None,
            dlc_picker_pending_install_dir: None,
            dlc_picker_cached_bytes: None,

            updates_downloading: Arc::new(Mutex::new(std::collections::HashSet::new())),

            goldberg: crate::goldberg::GoldbergGenerator::new(std::path::Path::new(".")),
            goldberg_modal_open: false,
            goldberg_candidate_id: None,
            goldberg_user_input: String::new(),
            goldberg_steamid_input: String::new(),
            goldberg_use_64bit: true,

            // Watcher
            watcher_pending_updates: Arc::new(Mutex::new(HashMap::new())),
            watcher_updating: Arc::new(Mutex::new(HashSet::new())),

            // OTA Update System
            update_available: Arc::new(Mutex::new(None)),
            is_updating: Arc::new(Mutex::new(false)),
        }
    }
}

impl DarkCoreApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = load_config();

        // Load cache
        let cache_map = load_game_cache();
        // Load relationships
        let rel_map = crate::app_list::load_relationships(".");

        // Always initialize client; it handles empty keys via Fallback to Steam Store API.
        let api_client = Some(ApiClient::new(config.api_key.clone()));

        let system_log = Arc::new(Mutex::new(Vec::new()));
        // Initial log
        if let Ok(mut logs) = system_log.lock() {
            push_log(&mut logs, "System Ready. Darkcore Rust Initialized.".to_string());
        }

        let initial_profile = config.last_active_profile.clone();
        let initial_api_key = config.api_key.clone();

        let mut app = Self {
            config,
            active_tab: 0,
            search_query: String::new(),
            last_searched_query: String::new(),
            last_input_time: None,
            search_results: Arc::new(Mutex::new(Vec::new())),
            active_games: Arc::new(Mutex::new(Vec::new())),
            game_cache: Arc::new(Mutex::new(cache_map)),
            update_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            relationships: Arc::new(Mutex::new(rel_map)),
            target_exe: String::new(),
            include_dlcs: true,
            status_msg: "System Ready".to_string(),
            system_log,
            cover_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cover_queue: Arc::new(Mutex::new(Vec::new())),
            api_client,
            profile_manager: ProfileManager::new("."),
            profile_name_input: String::new(),
            active_profile_name: initial_profile,
            delete_modal_open: false,
            delete_candidate_id: None,
            delete_candidate_name: None,
            delete_associated_dlcs: Vec::new(),
            is_scanning_dlcs: false,
            dlc_scan_result: Arc::new(Mutex::new(None)),
            delete_scan_result: Arc::new(Mutex::new(None)),
            dlc_scan_result_zip: Arc::new(Mutex::new(None)),
            
            // Manifestor Init
            manifestor_open: false,
            manifestor_data: Arc::new(Mutex::new(None)),
            manifestor_candidate_id: None,
            manifestor_candidate_name: String::new(),
            manifestor_target_library: None,
            manifestor_install_name: String::new(),
            manifestor_selections: Vec::new(),
            
            install_modal_open: false,
            install_candidate: None,
            install_candidate_is_free: false, // NEW
            detected_libraries: Vec::new(),
            selected_library_index: 0,
            install_dir_input: String::new(), // Init
            
            show_free_content: false, // Default: Hide F2P
            
            create_profile_modal_open: false,
            create_profile_save_default: true,
            
            // NEW:
            delete_profile_modal_open: false,
            
            logo_texture: None,
            logo_data: {
                // EMBEDDED LOGO (Compile-time check)
                // Relative to manager/src/ui.rs -> manager/logo.png
                let bytes = include_bytes!("../logo.png"); 
                if let Ok(img) = image::load_from_memory(bytes) {
                    let img = img.to_rgba8();
                    Some(egui::ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_flat_samples().as_slice(),
                    ))
                } else {
                    None
                }
            },
            tab_changed_at: Instant::now(),
            
            // Audio Init
            _audio_stream: None,
            _audio_stream_handle: None,
            audio_sink: None,
            volume: 0.02, // Ultra-Quiet Background (Whisper Level)


            status_update_queue: Arc::new(Mutex::new(None)),
            
            user_stats: Arc::new(Mutex::new(None)),
            api_last_error: Arc::new(Mutex::new(None)),
            is_validating_api: Arc::new(Mutex::new(false)),
            matrix_trails: Vec::new(),
            api_key_glitch_cache: String::new(),
            api_key_glitch_update: Instant::now(),
            config_saved_at: None,
            api_refresh_timer: if !initial_api_key.is_empty() { Some(Instant::now() + std::time::Duration::from_millis(500)) } else { None }, // Auto-Start
            
            // DLC Picker
            dlc_picker_open: false,
            dlc_picker_candidate: None,
            dlc_picker_items: Vec::new(),
            dlc_picker_depot_count: 0,
            dlc_picker_search: String::new(),
            dlc_picker_pending_library: None,
            dlc_picker_pending_install_dir: None,
            dlc_picker_cached_bytes: None,

            updates_downloading: Arc::new(Mutex::new(std::collections::HashSet::new())),

            goldberg: crate::goldberg::GoldbergGenerator::new(std::path::Path::new(".")),
            goldberg_modal_open: false,
            goldberg_candidate_id: None,
            goldberg_user_input: "DarkCore User".to_string(),
            goldberg_steamid_input: "76561197960287930".to_string(),
            goldberg_use_64bit: true,

            // Watcher
            watcher_pending_updates: Arc::new(Mutex::new(HashMap::new())),
            watcher_updating: Arc::new(Mutex::new(HashSet::new())),

            // OTA Update System
            update_available: Arc::new(Mutex::new(None)),
            is_updating: Arc::new(Mutex::new(false)),
        };



        // Initialize Audio Thread
        if let Ok((stream, handle)) = rodio::OutputStream::try_default() {
            if let Ok(sink) = rodio::Sink::try_new(&handle) {
                // Load embedded track (Obfuscated as system data)
                let bytes = include_bytes!("../core_data/sys_audio_01.dat");
                let cursor = std::io::Cursor::new(bytes);
                if let Ok(source) = rodio::Decoder::new(cursor) {
                     sink.append(source.repeat_infinite());
                     sink.set_volume(0.02);
                     sink.play();
                     
                     app._audio_stream = Some(stream);
                     app._audio_stream_handle = Some(handle);
                     app.audio_sink = Some(sink);
                }
            }
        }

        app.configure_visuals(&_cc.egui_ctx);

        // --- STARTUP TASK: WUDRM UPDATE SCANNER ---
        let steam_path_clone = app.config.steam_path.clone();
        std::thread::spawn(move || {
            // Give UI time to load / settle
            std::thread::sleep(std::time::Duration::from_secs(3));
            
            let all_libs = crate::game_path::GamePathFinder::get_library_folders(&steam_path_clone);
            for lib in all_libs {
                let steamapps = lib.join("steamapps");
                if let Ok(entries) = std::fs::read_dir(steamapps) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                            if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                                // Parse AppID and StateFlags
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    let mut appid = String::new();
                                    let mut state_flags = 0u32;
                                    
                                    // Quick dirty parse
                                    for line in content.lines() {
                                        if line.contains("\"appid\"") {
                                            if let Some(start) = line.rfind("\"") {
                                                if let Some(prev) = line[..start].rfind("\"") {
                                                    appid = line[prev+1..start].to_string();
                                                }
                                            }
                                        }
                                        if line.contains("\"StateFlags\"") {
                                            if let Some(start) = line.rfind("\"") {
                                                if let Some(prev) = line[..start].rfind("\"") {
                                                    if let Ok(flags) = line[prev+1..start].parse::<u32>() {
                                                        state_flags = flags;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Check "Update Required" (256)
                                    // If Steam flags it, we download manifests to fix it/start download
                                    if !appid.is_empty() && (state_flags & 256 != 0) {
                                        println!("[WUDRM] Detected Update Required for AppID {}. Triggering Recovery...", appid);
                                        // Trigger Download
                                        let _ = download_manifests_wudrm(&appid, &steam_path_clone, &|s| println!("[WUDRM] {}", s));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // Install image loaders
        egui_extras::install_image_loaders(&_cc.egui_ctx);

        app.refresh_library();
        app.resolve_unknown_games();
        
        // AUTO-START WATCHER: Check for updates on startup
        app.start_watcher_check();

        // OTA UPDATE CHECK: Spawn background thread to check for new releases
        {
            let update_arc = app.update_available.clone();
            std::thread::spawn(move || {
                // Give app time to start
                std::thread::sleep(std::time::Duration::from_secs(5));
                match crate::updater::check_for_updates() {
                    Ok(Some(release)) => {
                        if let Ok(mut lock) = update_arc.lock() {
                            *lock = Some(release.version);
                        }
                    }
                    Ok(None) => {} // Already up-to-date
                    Err(e) => {
                        eprintln!("[OTA] Update check failed: {}", e);
                    }
                }
            });
        }
        
        app
    }

    fn configure_visuals(&self, ctx: &egui::Context) {
        // FORCE DARK MODE - Override system theme completely
        ctx.set_visuals(egui::Visuals::dark());
        
        let mut style = (*ctx.style()).clone();
        
        // FONT SIZES
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
            (egui::TextStyle::Button, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Small, egui::FontId::new(12.0, egui::FontFamily::Proportional)),
        ].into();
        
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.button_padding = egui::vec2(20.0, 10.0);
        style.visuals.window_rounding = egui::Rounding::same(12.0);
        
        ctx.set_style(style);

        // CYBERPUNK PALETTE - Apply dark theme with custom colors
        let mut visuals = egui::Visuals::dark();
        
        let bg_app = egui::Color32::from_rgb(11, 12, 16); // Obsidian
        let bg_surface = egui::Color32::from_rgb(24, 26, 33); // Gunmetal
        let accent_cyan = egui::Color32::from_rgb(0, 243, 255); // Neon Cyan
        let accent_pink = egui::Color32::from_rgb(255, 0, 110); // Cyber Pink
        let text_bright = egui::Color32::from_rgb(245, 245, 250); 
        let text_dim = egui::Color32::from_rgb(160, 160, 180);

        // FORCE ALL BACKGROUNDS DARK
        visuals.window_fill = bg_app;
        visuals.panel_fill = bg_app;
        visuals.faint_bg_color = bg_app;
        visuals.extreme_bg_color = bg_app; // This fixes some light areas
        visuals.code_bg_color = bg_surface;
        
        // Force dark text on dark background
        visuals.override_text_color = Some(text_bright);
        
        // Non Interactive
        visuals.widgets.noninteractive.bg_fill = bg_app;
        visuals.widgets.noninteractive.weak_bg_fill = bg_app;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_bright);

        // Buttons (Idle) - "Glassy" look
        visuals.widgets.inactive.bg_fill = bg_surface;
        visuals.widgets.inactive.weak_bg_fill = bg_surface;
        visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text_dim);

        // Buttons (Hover) - "Glow"
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(35, 38, 50);
        visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(35, 38, 50);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, accent_cyan);
        visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
        visuals.widgets.hovered.expansion = 2.0; 

        // Buttons (Active)
        visuals.widgets.active.bg_fill = accent_cyan.linear_multiply(0.15);
        visuals.widgets.active.weak_bg_fill = accent_cyan.linear_multiply(0.15);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, accent_cyan);
        visuals.widgets.active.rounding = egui::Rounding::same(8.0);
        visuals.widgets.active.expansion = 1.0;

        // Open (menus, popups)
        visuals.widgets.open.bg_fill = bg_surface;
        visuals.widgets.open.weak_bg_fill = bg_surface;
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, accent_cyan);

        // Selection
        visuals.selection.bg_fill = accent_pink.linear_multiply(0.3);
        visuals.selection.stroke = egui::Stroke::new(1.0, accent_pink);
        
        ctx.set_visuals(visuals);
    }

    fn log<S: Into<String>>(&self, msg: S) {
        let msg = msg.into();
        if let Ok(mut logs) = self.system_log.lock() {
            push_log(&mut logs, msg);
        }
    }

    fn refresh_library(&mut self) {
        if self.config.gl_path.is_empty() { return; }
        let gl_path = self.config.gl_path.clone();
        let cache_lock = self.game_cache.lock().unwrap();
        let cache_snapshot = cache_lock.clone();
        drop(cache_lock);
        
        let rel_lock = self.relationships.lock().unwrap();
        let rel_snapshot = rel_lock.clone();
        drop(rel_lock);

        let target = self.active_games.clone();
        let steam_path = self.config.steam_path.clone();
        let games = refresh_active_games_list(&gl_path, &steam_path, &cache_snapshot, &rel_snapshot);
        
        // Collect IDs for update checking
        let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
        
        let mut target_guard = target.lock().unwrap();
        *target_guard = games;
        
        // Trigger Update Check
        self.check_updates_for_ids(ids);
    }

    /// Start background update check for all installed games
    fn start_watcher_check(&self) {
        let api_key = self.config.api_key.clone();
        let gl_path = self.config.gl_path.clone();
        let steam_path = self.config.steam_path.clone();
        let pending_arc = self.watcher_pending_updates.clone();
        let game_cache = self.game_cache.clone();
        let relationships = self.relationships.clone();
        let log_arc = self.system_log.clone();
        
        if api_key.is_empty() {
            self.log("[Watcher] Skipped: No API key configured.");
            return;
        }
        
        self.log("[Watcher] Starting update check...");
        
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };
            
            // Get active games
            let (cache_snapshot, rels_snapshot) = {
                let cache = game_cache.lock().unwrap();
                let rels = relationships.lock().unwrap();
                (cache.clone(), rels.clone())
            };
            
            let games = crate::app_list::refresh_active_games_list(
                &gl_path,
                &steam_path,
                &cache_snapshot,
                &rels_snapshot,
            );
            
            if games.is_empty() {
                if let Ok(mut logs) = log_arc.lock() {
                    logs.push("[Watcher] No games to check.".to_string());
                }
                return;
            }
            
            let client = crate::api::ApiClient::new(api_key);
            let mut found_updates = 0;
            
            // Check each game
            rt.block_on(async {
                for game in &games {
                    // Skip depots
                    if game.name.starts_with("Depot (") || game.name.contains("(Content)") {
                        continue;
                    }
                    
                    // Get remote build info
                    if let Ok(info) = client.get_app_info(&game.app_id).await {
                        if let Some(remote_build) = info.buildid {
                            // For now, we don't have local build stored, so check ACF
                            let acf_path = std::path::Path::new(&steam_path)
                                .join("steamapps")
                                .join(format!("appmanifest_{}.acf", game.app_id));
                            
                            let mut local_build: u64 = 0;
                            if acf_path.exists() {
                                if let Ok(content) = std::fs::read_to_string(&acf_path) {
                                    // Parse buildid from ACF
                                    for line in content.lines() {
                                        if line.contains("\"buildid\"") {
                                            if let Some(start) = line.rfind('"') {
                                                if let Some(prev) = line[..start].rfind('"') {
                                                    if let Ok(b) = line[prev+1..start].parse::<u64>() {
                                                        local_build = b;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Compare: if remote > local, update available
                            if remote_build > local_build && local_build > 0 {
                                found_updates += 1;
                                if let Ok(mut p) = pending_arc.lock() {
                                    p.insert(
                                        game.app_id.clone(),
                                        (game.name.clone(), local_build, remote_build)
                                    );
                                }
                            }
                        }
                    }
                    
                    // Rate limit
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            });
            
            if let Ok(mut logs) = log_arc.lock() {
                if found_updates > 0 {
                    logs.push(format!("[Watcher] Found {} games with updates available!", found_updates));
                } else {
                    logs.push("[Watcher] All games are up to date.".to_string());
                }
            }
        });
    }


    fn perform_search(&self) {
        if let Some(_client) = &self.api_client {
            if self.search_query.is_empty() {
                return;
            }
            let query = self.search_query.clone();
            let results_arc = self.search_results.clone();
            let active_games = self.active_games.clone();
            let update_cache = self.update_cache.clone();
            let steam_path = self.config.steam_path.clone();
            
            // Restore missing variables
            let client_key = self.config.api_key.clone();
            let cover_queue = self.cover_queue.clone();
            let cover_cache = self.cover_cache.clone();
            let log_arc = self.system_log.clone();
            let user_stats_arc = self.user_stats.clone(); // Capture Stats Arc

            self.log(format!("Searching for: {}", query));
            if let Ok(mut res) = results_arc.lock() {
                res.clear();
            }
            if let Ok(mut cache) = cover_cache.lock() {
                cache.clear();
            }

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let client = ApiClient::new(client_key.clone());

                // Result of blocking search
                let search_res = rt.block_on(client.search(&query));

                match search_res {
                    Ok(mut res) => {
                        // Intelligent Sorting
                        res.sort_by(|a, b| {
                            let name_a = a
                                .game_name
                                .as_deref()
                                .or(a.name.as_deref())
                                .unwrap_or("")
                                .to_lowercase();
                            let name_b = b
                                .game_name
                                .as_deref()
                                .or(b.name.as_deref())
                                .unwrap_or("")
                                .to_lowercase();
                            let q = query.to_lowercase();

                            let exact_a = name_a == q;
                            let exact_b = name_b == q;

                            // 1. Exact Match Order
                            if exact_a != exact_b {
                                return exact_b.cmp(&exact_a);
                            }

                            // 2. Starts With Query
                            let starts_a = name_a.starts_with(&q);
                            let starts_b = name_b.starts_with(&q);
                            if starts_a != starts_b {
                                return starts_b.cmp(&starts_a);
                            }

                            // 3. Shortest Name First (Main Game vs DLC)
                            let len_a = name_a.len();
                            let len_b = name_b.len();
                            if len_a != len_b {
                                return len_a.cmp(&len_b);
                            }

                            name_a.cmp(&name_b)
                        });

                        if let Ok(mut results) = results_arc.lock() {
                            *results = res.clone();
                        }

                        // Download Covers
                        let dl_client = reqwest::Client::builder()
                            .danger_accept_invalid_certs(true)
                            .user_agent("DarkCore/10.4-Rust")
                            .build()
                            .unwrap_or_default();

                        // Block to spawn and wait for all downloads AND status checks
                        rt.block_on(async {
                            let mut handles = Vec::new();
                            
                            // Get Installed IDs for check
                            let installed: std::collections::HashSet<String> = {
                                if let Ok(g) = active_games.lock() {
                                    g.iter().map(|x| x.app_id.clone()).collect()
                                } else {
                                    std::collections::HashSet::new()
                                }
                            };

                            for item in res {
                                 let id1 = crate::api::val_to_string(&item.game_id);
                                 let id2 = crate::api::val_to_string(&item.app_id);
                                 let appid = if !id1.is_empty() { id1 } else { id2 };
                                 
                                 if !appid.is_empty() && appid != "0" {
                                     let queue = cover_queue.clone();
                                     let appid_clone = appid.clone();
                                     let dl_client = dl_client.clone();
                                     let _log_arc_inner = log_arc.clone();
                                     
                                     // COVER TASK
                                     handles.push(tokio::spawn(async move {
                                         let url_portrait = format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/library_600x900.jpg", appid_clone);
                                         let url_landscape = format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/header.jpg", appid_clone);
                                         
                                         // 1. Try Portrait
                                         let mut success = false;
                                         if let Ok(resp) = dl_client.get(&url_portrait).send().await {
                                             if resp.status().is_success() {
                                                 if let Ok(bytes) = resp.bytes().await {
                                                     if let Ok(img) = image::load_from_memory(&bytes) {
                                                         let img = img.to_rgba8();
                                                         if let Ok(mut q) = queue.lock() {
                                                             q.push((appid_clone.clone(), img.width(), img.height(), img.into_raw()));
                                                             success = true;
                                                         }
                                                     }
                                                 }
                                             }
                                         }
                                         // 2. Try Landscape
                                         if !success {
                                             if let Ok(resp) = dl_client.get(&url_landscape).send().await {
                                                 if resp.status().is_success() {
                                                     if let Ok(bytes) = resp.bytes().await {
                                                          if let Ok(img) = image::load_from_memory(&bytes) {
                                                              let img = img.to_rgba8();
                                                              if let Ok(mut q) = queue.lock() {
                                                                  q.push((appid_clone.clone(), img.width(), img.height(), img.into_raw()));
                                                                  success = true;
                                                              }
                                                          }
                                                     }
                                                 }
                                             }
                                         }
                                         // 3. Fallback
                                         if !success {
                                             let w = 60; let h = 90;
                                             let mut pixels = Vec::with_capacity((w * h * 4) as usize);
                                             for _ in 0..(w*h) { pixels.push(30); pixels.push(30); pixels.push(40); pixels.push(255); }
                                             if let Ok(mut q) = queue.lock() { q.push((appid_clone.clone(), w, h, pixels)); }
                                         }
                                     }));
                                     
                                     // UPDATE CHECK TASK
                                     // Only check if installed
                                     if installed.contains(&appid) {
                                          let client = client.clone(); // ApiClient is cheap clone
                                          let cache = update_cache.clone();
                                          let sp = steam_path.clone();
                                          let aid = appid.clone();
                                          
                                          handles.push(tokio::spawn(async move {
                                               // 1. Get Local StateFlags
                                               let acf = std::path::Path::new(&sp).join("steamapps").join(format!("appmanifest_{}.acf", aid));
                                               let mut state_flags = 0u32;
                                               
                                               if acf.exists() {
                                                   if let Ok(c) = std::fs::read_to_string(&acf) {
                                                       // Parse StateFlags
                                                       if let Some(pos) = c.find("\"StateFlags\"") {
                                                            let key_len = "\"StateFlags\"".len();
                                                            let remainder = &c[pos + key_len..];
                                                            if let Some(qs) = remainder.find("\"") {
                                                                if let Some(qe) = remainder[qs+1..].find("\"") {
                                                                    let val = &remainder[qs+1 .. qs+1+qe];
                                                                    state_flags = val.parse().unwrap_or(0);
                                                                }
                                                            }
                                                       }
                                                   }
                                               }
                                               
                                               // 2. Optimization: Skip API if installed
                                               if (state_flags & 4) != 0 {
                                                   if let Ok(mut c) = cache.lock() { c.insert(aid, false); }
                                                   return;
                                               }

                                               // 3. Get Remote
                                               if let Ok(st) = client.get_status(&aid).await {
                                                    let mut needs = st.needs_update.unwrap_or(false);
                                                    
                                                    // OVERRIDE: Aggressive Trust
                                                    // If installed (4), we force PLAY, even if update required (2).
                                                    if (state_flags & 4) != 0 {
                                                        needs = false;
                                                    }
                                                    
                                                    if let Ok(mut c) = cache.lock() {
                                                        c.insert(aid, needs);
                                                    }
                                               }
                                          }));
                                     }
                                 }
                            }
                            
                            // Wait for all downloads to finish before Runtime drops
                            for h in handles {
                                let _ = h.await; 
                            }
                        });


                        // AUTO-UPDATE STATS (Fix usage counter)
                        if let Ok(stats) = rt.block_on(client.get_user_stats()) {
                            if let Ok(mut s) = user_stats_arc.lock() {
                                *s = Some(stats);
                            }
                        }
                    }
                    Err(e) => {
                        if let Ok(mut logs) = log_arc.lock() {
                            push_log(&mut logs, format!("Search API Error: {}", e));
                        }
                    }
                }
            });
        }
    }

    fn resolve_unknown_games(&mut self) {
        // Hybrid System: Even without key, we can resolve names via Steam Store API.
        let active_games = self.active_games.clone();
        let game_cache = self.game_cache.clone();
        let client_key = self.config.api_key.clone();
        let steam_path = self.config.steam_path.clone();
        let status_queue = self.status_update_queue.clone();
        let relationships = self.relationships.clone(); // Capture relationships

        self.status_msg = "Resolving unknown games & DLCs...".to_string();

        std::thread::spawn(move || {
            let mut ids_to_resolve = Vec::new();

            // Identify unknowns OR orphans (possible unlinked DLCs)
            {
                if let Ok(games) = active_games.lock() {
                    for g in games.iter() {
                        // Check if needs Name Resolution OR Relationship Check
                        if g.name == "Unknown" || g.name.starts_with("Depot of") || g.parent_id.is_none() {
                            ids_to_resolve.push(g.app_id.clone());
                        }
                    }
                }
            }

            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build() 
            {
                Ok(rt) => rt,
                Err(_) => return,
            };

            runtime.block_on(async {
                let mut handles = Vec::new();
                let shared_client = ApiClient::new(client_key.clone());

                for id in ids_to_resolve {
                    let client = shared_client.clone();
                    let game_cache = game_cache.clone();
                    let rel_map = relationships.clone();
                    let id_clone = id.clone();
                    let steam_path_ref = steam_path.clone();

                    handles.push(tokio::spawn(async move {
                        let mut found_name = None;

                        // 0. Hardcoded Common Redists
                        match id_clone.as_str() {
                             "228980" => found_name = Some("Steamworks Common Redistributables".to_string()),
                             "228981" | "228982" | "228983" | "228984" | "228985" | 
                             "228986" | "228987" | "228988" | "228989" | "228990" => {
                                 found_name = Some(format!("Steamworks Redist ({})", id_clone));
                             },
                             "366850" => found_name = Some("Old World".to_string()),
                             "408630" => found_name = Some("Europa Universalis IV".to_string()),
                             _ => {}
                        }

                        // 1. Try Morrenus Search first
                        if found_name.is_none() {
                            if let Ok(results) = client.search(&id_clone).await {
                                use crate::api::val_to_string;
                                let matched = results.iter().find(|r| {
                                    let rid = val_to_string(&r.game_id);
                                    let rid2 = val_to_string(&r.app_id);
                                    rid == id_clone || rid2 == id_clone
                                });

                                if let Some(res) = matched {
                                    let name = res
                                        .game_name
                                        .as_deref()
                                        .or(res.name.as_deref())
                                        .unwrap_or("Unknown")
                                        .to_string();
                                    if name != "Unknown" {
                                        found_name = Some(name);
                                    }
                                }
                            }
                        }

                        // 2. Fallback: Steam Store API & HTML Scraper
                        if found_name.is_none() {
                            let url = format!(
                                "https://store.steampowered.com/api/appdetails?appids={}",
                                id_clone
                            );
                            let mut api_success = false;

                            if let Ok(resp) = reqwest::get(&url).await {
                                if let Ok(json) = resp.json::<serde_json::Value>().await {
                                    if let Some(data) =
                                        json.get(&id_clone).and_then(|v| v.get("data"))
                                    {
                                        if let Some(name_val) = data.get("name") {
                                            if let Some(n) = name_val.as_str() {
                                                found_name = Some(n.to_string());
                                                api_success = true;
                                            }
                                        }
                                    }
                                }
                            }

                            // 2b. HTML Title Scraper (Nuclear Option)
                            if !api_success {
                                let page_url = format!("https://store.steampowered.com/app/{}", id_clone);
                                if let Ok(resp) = reqwest::get(&page_url).await {
                                    if let Ok(text) = resp.text().await {
                                         if let Some(start) = text.find("<title>") {
                                             if let Some(end) = text[start..].find(" on Steam</title>") {
                                                 let raw = &text[start + 7 .. start + end];
                                                 let cleaned = raw.trim()
                                                    .replace("&amp;", "&")
                                                    .replace("&apos;", "'")
                                                    .replace("&#39;", "'");
                                                 
                                                 if !cleaned.is_empty() {
                                                     found_name = Some(cleaned);
                                                 }
                                             }
                                         }
                                    }
                                }
                            }
                        }

                        // 3. Fallback: Local Config VDF (Depot Check)
                        if found_name.is_none() {
                            if let Some(parent_id) = crate::game_path::GamePathFinder::find_parent_for_depot(&steam_path_ref, &id_clone) {
                                // Try to get parent name from cache
                                let parent_name = {
                                    if let Ok(c) = game_cache.lock() {
                                        c.get(&parent_id).cloned()
                                    } else {
                                        None
                                    }
                                };
                                
                                if let Some(p_name) = parent_name {
                                    found_name = Some(format!("{} [Depot]", p_name));
                                } else {
                                    found_name = Some(format!("Depot of AppID {}", parent_id));
                                }
                            }
                        }

                        // 4. Fallback: Deep Manifest Scan (User Mounted Depots)
                        if found_name.is_none() {
                            if let Some(parent_id) = crate::game_path::GamePathFinder::find_parent_by_scanning_manifests(&steam_path_ref, &id_clone) {
                                let parent_name = {
                                    if let Ok(c) = game_cache.lock() {
                                        c.get(&parent_id).cloned()
                                    } else {
                                        None
                                    }
                                };
                                
                                if let Some(p_name) = parent_name {
                                    found_name = Some(format!("{} [Depot]", p_name));
                                } else {
                                    found_name = Some(format!("Depot of AppID {}", parent_id));
                                }
                            }
                        }

                        // 5. DLC Auto-Link (Store API)
                        // This fixes "Standalone DLC" issues by finding the fullgame ID
                        if let Ok(Some(parent_id)) = client.get_details_parent(&id_clone).await {
                             if let Ok(mut map) = rel_map.lock() {
                                 // Only link if not already linked (or orphan)
                                 if !map.contains_key(&id_clone) {
                                     map.insert(id_clone.clone(), parent_id.clone());
                                     crate::app_list::save_relationships(".", &map);
                                     
                                     // If we found a parent, try to make the name nicer if it's still generic
                                     if found_name.is_none() || found_name.as_deref() == Some("Unknown") {
                                          found_name = Some(format!("DLC (Parent: {})", parent_id));
                                     }
                                 }
                             }
                        }

                        // 3. Save if found
                        if let Some(name) = found_name {
                            if let Ok(mut cache) = game_cache.lock() {
                                cache.insert(id_clone.clone(), name.clone());
                                let _ = save_game_cache(&cache);
                            }
                        }
                    }));
                }

                for h in handles {
                    let _ = h.await;
                }
            });
            
            if let Ok(mut guard) = status_queue.lock() {
                *guard = Some("Resolution Complete.".to_string());
            }
        });
    }

    fn check_updates_for_ids(&self, ids: Vec<String>) {
        if ids.is_empty() { return; }
        let client_opt = self.api_client.clone();
        let cache_arc = self.update_cache.clone();
        let steam_path = self.config.steam_path.clone();
        let log_arc = self.system_log.clone();
        let updates_dl = self.updates_downloading.clone();

        std::thread::spawn(move || {
            let client = if let Some(c) = client_opt { c } else { return; };
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build() 
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            
            let mut handles = Vec::new();
            
            for appid in ids {
                let client = client.clone();
                let cache = cache_arc.clone();
                let sp = steam_path.clone();
                let log_clone = log_arc.clone();
                let updates_clone = updates_dl.clone();
                
                handles.push(rt.spawn(async move {
                    // 1. Get Local BuildID & StateFlags
                    let acf_path = std::path::Path::new(&sp).join("steamapps")
                        .join(format!("appmanifest_{}.acf", appid));
                    
                    let mut local_buildid = 0u64;
                    let mut state_flags = 0u32;

                    if acf_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&acf_path) {
                            // Parse StateFlags
                            if let Some(pos) = content.find("\"StateFlags\"") {
                                 let remainder = &content[pos..];
                                 if let Some(start_quote) = remainder.find("\"") {
                                     if let Some(end_label) = remainder[start_quote+1..].find("\"") {
                                          let val_part = &remainder[start_quote+1+end_label+1..];
                                          if let Some(v_start) = val_part.find("\"") {
                                              if let Some(v_end) = val_part[v_start+1..].find("\"") {
                                                  let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                  state_flags = num_str.parse().unwrap_or(0);
                                              }
                                          }
                                     }
                                 }
                            }
                            // Parse buildid
                            if let Some(pos) = content.find("\"buildid\"") {
                                let remainder = &content[pos..];
                                if let Some(start_quote) = remainder.find("\"") {
                                    if let Some(end_label) = remainder[start_quote+1..].find("\"") {
                                         let val_part = &remainder[start_quote+1+end_label+1..];
                                         if let Some(v_start) = val_part.find("\"") {
                                             if let Some(v_end) = val_part[v_start+1..].find("\"") {
                                                 let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                 local_buildid = num_str.parse().unwrap_or(0);
                                             }
                                         }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Always set UI cache to false (PLAY button, never UPDATE)
                    if let Ok(mut c) = cache.lock() {
                        c.insert(appid.clone(), false);
                    }

                    // 3. If installed, check for buildid mismatch and trigger WUDRM
                    let is_installed = (state_flags & 4) != 0;
                    if is_installed && local_buildid > 0 {
                        // Get remote buildid from SteamCMD (FREE)
                        if let Ok(info) = client.get_app_info(&appid).await {
                            if let Some(remote_bid) = info.buildid {
                                if remote_bid > local_buildid {
                                    // Update available!
                                    if let Ok(mut logs) = log_clone.lock() {
                                        push_log(&mut logs, format!("🔄 Update detected for AppID {}. Refreshing manifests...", appid));
                                    }
                                    
                                    // Mark as downloading
                                    if let Ok(mut dl) = updates_clone.lock() {
                                        dl.insert(appid.clone());
                                    }
                                    
                                    // Trigger WUDRM (synchronous call in blocking context)
                                    let sp_ref = sp.clone();
                                    let aid = appid.clone();
                                    let log_ref = log_clone.clone();
                                    let updates_ref = updates_clone.clone();
                                    
                                    // Spawn blocking task for WUDRM
                                    tokio::task::spawn_blocking(move || {
                                        let log_fn = |msg: String| {
                                            if let Ok(mut logs) = log_ref.lock() {
                                                push_log(&mut logs, msg);
                                            }
                                        };
                                        let _ = download_manifests_wudrm(&aid, &sp_ref, &log_fn);
                                        
                                        // Remove from downloading set
                                        if let Ok(mut dl) = updates_ref.lock() {
                                            dl.remove(&aid);
                                        }
                                    });
                                }
                            }
                        }
                    }
                }));
            }
            
            rt.block_on(async {
                for h in handles { let _ = h.await; }
            });
        });
    }

     fn open_manifestor(&mut self, appid: String, name: String) {
        self.manifestor_open = true;
        self.manifestor_candidate_id = Some(appid.clone());
        self.manifestor_candidate_name = name.clone();
        
        // Detect Libraries if not already
        if self.detected_libraries.is_empty() {
            self.detected_libraries = crate::game_path::GamePathFinder::get_library_folders(&self.config.steam_path);
        }
        // Default target library
        self.manifestor_target_library = self.detected_libraries.get(0).cloned();
        
        // Reset data
        if let Ok(mut data) = self.manifestor_data.lock() {
            *data = None;
        }
        
        // Check API Client
        if let Some(client) = &self.api_client {
            let client = client.clone();
            let data_target = self.manifestor_data.clone();
            let appid_target = appid.clone();
            
            // Spawn Fetch Task
            tokio::spawn(async move {
                // Fetch English hierarchy by default
                if let Ok(hierarchy) = client.fetch_full_hierarchy(&appid_target, "english").await {
                    if let Ok(mut target) = data_target.lock() {
                        *target = Some(hierarchy);
                    }
                }
            });
        }
    }

    fn show_manifestor_modal(&mut self, ctx: &egui::Context) {
        if !self.manifestor_open { return; }
        
        let mut close_modal = false;
        let mut launch_params: Option<(String, String, Vec<String>)> = None;
        
        // Scope for Mutex Lock
        {
            let mut open = true;
            let mut should_close_ui = false;
            let mut hierarchy_guard = self.manifestor_data.lock().unwrap();
            let _target_lib = self.manifestor_target_library.clone(); // Unused here now
            let _detected_libs = self.detected_libraries.clone();
    
            egui::Window::new(egui::RichText::new(format!("📦 INSTALL: {}", self.manifestor_candidate_name)).strong())
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(500.0, 600.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if hierarchy_guard.is_none() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(50.0);
                                ui.spinner();
                                ui.label("Fetching Game Hierarchy & DLCs...");
                                ui.label(egui::RichText::new("Querying SteamCMD...").size(10.0).color(egui::Color32::GRAY));
                            });
                            return;
                        }
                        
                        let hierarchy = hierarchy_guard.as_mut().unwrap();
                        let dlc_count = hierarchy.dlcs.len();
                        // Calculate TRUE Slot Usage (AppList Lines)
                        // GreenLuma Limit applies to TOTAL entries (AppIDs + Depots)
                        let mut simulated_ids = Vec::with_capacity(200);
                        simulated_ids.push(hierarchy.root_id.clone());
                        for depot in &hierarchy.base_depots { simulated_ids.push(depot.depot_id.clone()); }
                        
                        for dlc in &hierarchy.dlcs {
                            if dlc.selected {
                                simulated_ids.push(dlc.app_id.clone());
                                for depot in &dlc.depots { simulated_ids.push(depot.depot_id.clone()); }
                            }
                        }
                        simulated_ids.sort();
                        simulated_ids.dedup();
                        
                        let selected_count = simulated_ids.len();
                        let limit_max = 130;
                        let is_over_limit = selected_count > limit_max;
                        
                        ui.add_space(10.0);
                        ui.heading(&hierarchy.root_name);
                        ui.label(egui::RichText::new(format!("AppID: {}", hierarchy.root_id)).monospace().color(egui::Color32::GRAY));
                        ui.separator();
                        
                        if is_over_limit {
                            ui.label(
                                egui::RichText::new(format!("⚠️ CRITICAL: SYSTEM LIMIT EXCEEDED ({}/{})", selected_count, limit_max))
                                .color(egui::Color32::RED).strong().size(16.0)
                            );
                            ui.label("DarkCore/GreenLuma will CRASH if you proceed. Please deselect items.");
                            ui.separator();
                        } else {
                            ui.label(egui::RichText::new(format!("Slots Used: {} / {} (Safe)", selected_count, limit_max)).color(egui::Color32::GREEN));
                        }
                        
                        ui.horizontal(|ui| {
                            if ui.button("Select All").clicked() {
                                for dlc in &mut hierarchy.dlcs { dlc.selected = true; }
                            }
                            if ui.button("Deselect All").clicked() {
                                for dlc in &mut hierarchy.dlcs { dlc.selected = false; }
                            }
                            if ui.button(egui::RichText::new("✨ Essential Content Only").color(egui::Color32::GOLD)).on_hover_text("Selects only Story/Level DLCs.").clicked() {
                                for dlc in &mut hierarchy.dlcs {
                                    let n = dlc.name.to_lowercase();
                                    if n.contains("soundtrack") || n.contains(" ost") || n.contains("artbook") || n.contains("wallpaper") || n.contains("skin") || n.contains("costume") {
                                        dlc.selected = false;
                                    } else {
                                        dlc.selected = true;
                                    }
                                }
                            }
                        });
                        ui.separator();

                        egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                             let mut base_checked = true;
                             ui.add_enabled(false, egui::Checkbox::new(&mut base_checked, egui::RichText::new("Base Game (Core)").strong()));
                             
                             if hierarchy.dlcs.is_empty() {
                                 ui.label("No DLCs found.");
                             } else {
                                 for dlc in &mut hierarchy.dlcs {
                                     ui.horizontal(|ui| {
                                         ui.checkbox(&mut dlc.selected, &dlc.name);
                                         ui.label(egui::RichText::new(format!("({})", dlc.app_id)).size(9.0).color(egui::Color32::GRAY));
                                     });
                                 }
                             }
                        });
                        
                        ui.separator();
                        
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                should_close_ui = true;
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let btn_text = if is_over_limit { "⛔ LIMIT EXCEEDED" } else { "CONFIRM & PROCEED" };
                                let btn = egui::Button::new(egui::RichText::new(btn_text).strong().color(if is_over_limit { egui::Color32::DARK_RED } else { egui::Color32::BLACK }))
                                    .fill(if is_over_limit { egui::Color32::BLACK } else { egui::Color32::GREEN })
                                    .min_size(egui::vec2(150.0, 30.0));
                                    
                                let resp = ui.add_enabled(!is_over_limit, btn);
                                
                                if resp.clicked() {
                                    // SAVE SELECTIONS AND CHAIN TO LIBRARY MODAL
                                    let selections: Vec<String> = hierarchy.dlcs.iter().filter(|d| d.selected).map(|d| d.app_id.clone()).collect();
                                    launch_params = Some((hierarchy.root_id.clone(), hierarchy.root_name.clone(), selections));
                                    should_close_ui = true;
                                }
                            });
                        });
                    });
                });
                
            if !open || should_close_ui {
                close_modal = true;
            }
        }
        
        // EXECUTE CHAIN (Lock Dropped)
        if let Some((app_id, name, selections)) = launch_params {
            self.manifestor_selections = selections;
            self.manifestor_open = false; // Close Manifestor
            
            // Open Library Selection Modal (Next Step)
            self.install_candidate = Some((app_id, name));
            self.install_modal_open = true;
        } else if close_modal {
            self.manifestor_open = false;
        }
    }    
    pub fn install_game(&mut self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>) {
        
        // NEW: Check if Manifestor populated selections. If so, bypass scan.
        // Also check if we should default to AUTO/ALL if manifestor wasn't used?
        // Actually, if self.manifestor_selections is empty BUT the user passed through manifestor, it means "Base Game Only".
        // Use a flag for "UsingManifestor" or check if selections were cleared? 
        // We can just check `!self.manifestor_open` (it's closed now) and assume if we have selections or if we came from that flow...
        // Better: We force use `finalize_installation` directly from the UI for Manifestor path, OR
        // we make `install_game` smarter.
        // Let's make `install_game` smart.

        // If this is a Manifestor install, `manifestor_selections` will be set (even if empty, likely handled by UI clear).
        // Problem: `manifestor_selections` persists. We should clear it after use.
        // Let's assume if it is NOT user initiated scan, we use it.
        // Actually, `manifestor_selections` can be passed to verify.
        
        // Wait, if I chain the UI, `show_install_modal` calls `install_game`.
        // So `install_game` MUST handle the bypassing.
        
        // BYPASS LOGIC
        // We need a reliable way to know "This install uses Manifestor selections".
        // We can check if `self.manifestor_selections` was recently updated?
        // Simplest: If `self.manifestor_selections` isn't empty OR if we assume all flow goes through Manifestor now (which it does for "Install" button).
        // But what about updates?
        
        // Let's rely on `finalize_installation` accepting the selections.
        // If `install_game` is called, we check `self.manifestor_selections`.
        // If we want to support "Default Install" (no manifestor), `manifestor_selections` would be empty?
        // But "Base Game Only" is also empty.
        
        // Solution: `open_manifestor` clears `manifestor_selections`.
        // `show_manifestor_modal` sets them (even empty Vec).
        // We need a flag `manifestor_active_session`?
        // Let's just use `finalize_installation` directly here if we trust the context.
        
        // For now, let's assume if we are calling `install_game`, we want to proceed.
        // If `manifestor_selections` is set (we can change it to Option<Vec> to differentiate "None/Unset" vs "Empty/BaseOnly").
        // But it is Vec<String>.
        
        // HOTFIX: Passing selections directly to finalize if appropriate, skipping scan.
        // Since we enforced Manifestor for ALL installs in Grid, we can trust `manifestor_selections` represents the user's intent.
        // CAUTION: If user cancels manifestor, we don't call this.
        // So:
        
        // Check if we have Manifestor data available to pass
        let hierarchy = if let Ok(data) = self.manifestor_data.lock() {
            data.clone()
        } else {
            None
        };

        self.finalize_installation(appid, name, target_library, install_dir_name, self.manifestor_selections.clone(), None, hierarchy);
        
        // Clear selections after handing off? No, finalize is async thread spawn. Clone is fine.
        // But we should reset them for next time? 
        // `open_manifestor` resets them. Correct.
        return; 
        
        /* SCANNNER BYPASSED
        let client_opt = self.api_client.clone(); 
        ... */
    }

    #[allow(dead_code)]
    fn legacy_install_game(&mut self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>) {
        // Renamed old install_game to logic below if needed
        let client_opt = self.api_client.clone();
        let _log_arc = self.system_log.clone();
        
        // CAPTURE CONTEXT FOR ASYNC
        let appid_c = appid.clone();
        
        // Prepare Scanner State
        let scan_res = self.dlc_scan_result.clone();
        
        // RESET ZIP CACHE
        if let Ok(mut zip) = self.dlc_scan_result_zip.lock() {
            *zip = None;
        }

        // [Scanner State Reset]
        let scan_zip_res = self.dlc_scan_result_zip.clone(); // NEW
        *scan_res.lock().unwrap() = None;
        *scan_zip_res.lock().unwrap() = None; // NEW
        self.is_scanning_dlcs = true;
        
        // Store candidate info for the UI to pick up after scan
        self.dlc_picker_candidate = Some((appid.clone(), name.clone()));
        self.dlc_picker_pending_library = target_library.clone();
        self.dlc_picker_pending_install_dir = install_dir_name.clone();
        
        // Log
        let log_arc = self.system_log.clone();
        if let Ok(mut l) = log_arc.lock() {
            l.push(format!("Checking DLCs for {}...", name));
        }

        std::thread::spawn(move || {
            if let Some(client) = client_opt {
                 if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                      // Fetch .lua from Morrenus
                      match rt.block_on(client.download_manifest(&appid_c)) {
                          Ok(lua_bytes) => {
                              // NEW: Cache bytes
                              *scan_zip_res.lock().unwrap() = Some(lua_bytes.to_vec());

                              let lua_content = String::from_utf8_lossy(&lua_bytes).to_string();
                              
                              // Parse LUA for DLCs (IDs without keys = AppIDs/DLCs)
                              let (applist_ids, keys) = crate::vdf_injector::parse_lua_for_keys(&lua_content);
                              let depot_count = keys.len(); // Depots have keys
                              
                              // Filter: DLCs are IDs in applist_ids that are NOT the base game
                              let dlc_items: Vec<(String, String, bool)> = applist_ids.iter()
                                  .filter(|id| *id != &appid_c)
                                  .map(|id| {
                                      // Try to extract name from LUA comments
                                      let name = extract_dlc_name_from_lua(&lua_content, id)
                                          .unwrap_or_else(|| format!("DLC {}", id));
                                      (id.clone(), name, true)
                                  })
                                  .collect();
                              
                              if !dlc_items.is_empty() {
                                  if let Ok(mut l) = log_arc.lock() {
                                      l.push(format!("Morrenus: Found {} DLCs for picker.", dlc_items.len()));
                                  }
                                  *scan_res.lock().unwrap() = Some((dlc_items, depot_count));
                              } else {
                                  *scan_res.lock().unwrap() = Some((Vec::new(), depot_count));
                              }
                          },
                          Err(e) => {
                               if let Ok(mut l) = log_arc.lock() {
                                   l.push(format!("Morrenus Error: {}. Proceeding without DLC picker.", e));
                               }
                               *scan_res.lock().unwrap() = Some((Vec::new(), 0));
                          }
                      }
                 }
            } else {
                // No Client
                *scan_res.lock().unwrap() = Some((Vec::new(), 0));
            }
        });
    }

    pub fn finalize_installation(
        &self, 
        appid: String, 
        name: String, 
        target_library: Option<std::path::PathBuf>, 
        install_dir_name: Option<String>,
        selected_dlcs: Vec<String>,
        cached_zip: Option<Vec<u8>>, // NEW
        hierarchy: Option<crate::api::GameHierarchy>, // NEW: For precise dependency resolution
    ) {
        // UNIFIED PROTOCOL: Works both Online (Manifests) and Offline (FamSharing/Public) through Fallbacks.
        let log_arc = self.system_log.clone();
        // let api_client_clone = self.api_client.clone(); // Not needed if we re-init
        let steam_path = self.config.steam_path.clone(); // Still need main path for other things
        let gl_path = self.config.gl_path.clone();
        let _include_dlcs = self.include_dlcs;
        let game_cache = self.game_cache.clone(); // Keep this for cache updates
        let api_key = self.config.api_key.clone(); // Keep this for API client creation inside thread
        let relationships_arc = self.relationships.clone(); // New: Capture relationships map for thread
        let enable_stealth = self.config.enable_stealth_mode;
        let is_free = self.install_candidate_is_free; // Capture F2P State
        let user_stats_arc = self.user_stats.clone(); // For refreshing token count after download
        
        // Use Arc/Mutex for status updates
        let status_queue = self.status_update_queue.clone();
        
        let update_status = move |msg: String| {
            if let Ok(mut lock) = status_queue.lock() {
                *lock = Some(msg);
            }
        };

        std::thread::spawn(move || {
            let log = move |msg: String| {
                if let Ok(mut logs) = log_arc.lock() {
                    // Print first (borrow), then push (move)
                    println!("[LOG] {}", msg);
                    push_log(&mut logs, msg);
                }
            };
            
            // Re-initialize client inside thread
            let client = ApiClient::new(api_key.clone());
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            // FETCH DEPOT INFO via SteamCMD (reserved for future update checks)
            let mut _steamcmd_info = None;
            match runtime.block_on(client.get_app_info(&appid)) {
                Ok(info) => {
                    log(format!("SteamCMD: Fetched info. Found {} depots.", info.depots.len()));
                    _steamcmd_info = Some(info);
                },
                Err(e) => {
                    log(format!("Warning: Could not fetch SteamCMD info: {}", e));
                }
            }

            log(format!("START: Protocol for {}", name));
            update_status(format!("Installing {}", name));

            // STEP 0.5: SETUP GREENLUMA CONFIG (Stealth Mode)
            // Ensure .bin files exist
            if let Err(e) = setup_greenluma_config(&gl_path, enable_stealth) {
                 log(format!("Warning: Could not setup GreenLuma config: {}", e));
            } else if enable_stealth {
                log("GreenLuma configured (Stealth Mode: ON).".to_string());
            } else {
                log("GreenLuma configured (Stealth Mode: OFF).".to_string());
            }


            // STEP 1: Kill Steam
            log("STEP 1: Killing Steam Process...".to_string());
            let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "steam.exe"]).output();
            std::thread::sleep(std::time::Duration::from_millis(2000));

            // PATH DEFINITIONS
            // `steam_path` from config is the Steam Installation Root (e.g. C:\Program Files\Steam).
            // We rename it to `steam_root` for clarity.
            let steam_root = steam_path.clone(); 
            
            // `library_path` is the target for the game (e.g. D:\Giochi Steam).
            // If target_library is set, use it. Otherwise, default to steam_root.
            let library_path = if let Some(target) = target_library {
                log(format!("Using selected library: {:?}", target));
                target.to_string_lossy().to_string()
            } else {
                steam_path.clone()
            };

            log(format!("Steam Root (Config): {}", steam_root));
            log(format!("Library Path (Game): {}", library_path));

            // STEP 1.5: GHOST INSTALLATION -> GENERATE ACF
            let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let _timestamp = time.to_string(); 

            let acf_filename = format!("appmanifest_{}.acf", appid);
            let acf_path = std::path::Path::new(&library_path).join("steamapps").join(&acf_filename);
            
            // Check for existing manifest in other libraries (Conflict Cleanup)
            // CRITICAL FIX: CLEANUP CONFLICTS
            let all_libs = crate::game_path::GamePathFinder::get_library_folders(&steam_root);
            for lib in all_libs {
                 let lib_str = lib.to_string_lossy().to_string();
                 if lib_str != library_path {
                     let conflict = lib.join("steamapps").join(&acf_filename);
                     if conflict.exists() {
                         log(format!("Removing conflicting manifest at: {:?}", conflict));
                         let _ = std::fs::remove_file(conflict);
                     }
                 }
            }

            // VAULT RESTORE CHECK
            let vault = VaultManager::new(".");
            let mut skip_ghost = false;
            let mut skip_morrenus = false;  // Skip Morrenus download if Vault has manifests
            
            // HOISTED: Calculate Install Dir Name (Available for both Ghost ACF and Tactical Bypass)
            // Use potentially overridden install dir name, or default to display name
            let final_install_dir = install_dir_name.clone().unwrap_or(name.clone());

            // Use library_path for restore check/logic
            if let Ok((restored_acf, count)) = vault.restore_manifests(&library_path, &appid) {
                if count > 0 { 
                    log(format!("Vault: Restored {} local depot manifests. SKIPPING MORRENUS (Token Saved). 🛡️", count)); 
                    skip_morrenus = true;  // Don't waste token!
                }
                if restored_acf {
                    log("Vault: Restored AppManifest.acf. Skipping Ghost Generation. 🛡️".to_string());
                    skip_ghost = true;
                }
            }

            if !skip_ghost {
                // CRITICAL: Delete existing ACF first to ensure Steam sees fresh state
                if acf_path.exists() {
                    log(format!("Removing old ACF: {:?}", acf_path));
                    let _ = std::fs::remove_file(&acf_path);
                }

                log(format!("Generating Ghost ACF (SMD-Style) at: {:?}", acf_path));

                // Use SMD-style minimal ACF (5 fields only)
                // This matches exactly what SMD does - Steam will fill in the rest during download
                if let Err(e) = generate_smd_style_acf(&acf_path, &appid, &final_install_dir) {
                    log(format!("Error writing ACF: {}", e));
                } else {
                     log("Ghost ACF generated (SMD-Style). Steam will see game as 'Update Required'.".to_string());
                }
            } else {
                log("Using Vaulted AppManifest.".to_string());
            }


            // STEP 2: MORRENUS MANIFEST DOWNLOAD (Or use Vault if available)
            let mut applist_ids = Vec::new();
            let mut keys: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            let depot_cache = std::path::Path::new(&steam_root).join("depotcache");
            if !depot_cache.exists() { let _ = std::fs::create_dir_all(&depot_cache); }
            
            if skip_morrenus {
                // Vault already restored manifests, skip Morrenus download
                log("STEP 2: SKIPPED - Using Vault manifests (0 API Tokens Used). 🛡️".to_string());
                applist_ids.push(appid.clone());
                // Keys should already be in config.vdf from previous installation
            } else {
                log("STEP 2: Fetching game data from Morrenus...".to_string());
                update_status(format!("Downloading manifests for {}", name));
                
                let mut manifests_from_zip = 0usize;
                
                // Fetch ZIP (or raw .lua) from Morrenus API
                // CACHE / VAULT / DOWNLOAD PROTOCOL
                let download_result = if let Some(bytes) = cached_zip {
                     log("📦 Using CACHED data from DLC scan (Saved 1 API Token).".to_string());
                     // Save to Vault
                     let v = crate::vault::VaultManager::new(".");
                     let _ = v.save(&appid, &bytes);
                     Ok(bytes)
                } else {
                     // Check Vault
                     let v = crate::vault::VaultManager::new(".");
                     if let Ok(bytes) = v.get(&appid) {
                          log("📦 Using VAULT data (Saved 1 API Token). 🛡️".to_string());
                          Ok(bytes)
                     } else {
                          // Download
                          match runtime.block_on(client.download_manifest(&appid)) {
                              Ok(bytes) => {
                                  log("📦 Saved Morrenus data to Vault.".to_string());
                                  let vec = bytes.to_vec();
                                  let _ = v.save(&appid, &vec);
                                  Ok(vec)
                              },
                              Err(e) => Err(e)
                          }
                     }
                };

                match download_result {
                    Ok(zip_bytes) => {
                        log(format!("Morrenus: Received {} bytes.", zip_bytes.len()));
                        
                        // Try to extract as ZIP first
                        let lua_content = if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes[..])) {
                                log(format!("Morrenus ZIP contains {} files.", archive.len()));
                                let mut lua_data = String::new();
                                
                                for i in 0..archive.len() {
                                    if let Ok(mut file) = archive.by_index(i) {
                                        let fname = file.name().to_string();
                                        
                                        if fname.ends_with(".lua") {
                                            // Extract .lua content
                                            use std::io::Read;
                                            file.read_to_string(&mut lua_data).ok();
                                            log(format!("  ✓ Extracted LUA: {}", fname));
                                        } else if fname.ends_with(".manifest") {
                                            // Extract .manifest to depotcache AND Vault
                                            use std::io::Read;
                                            let mut manifest_bytes = Vec::new();
                                            if file.read_to_end(&mut manifest_bytes).is_ok() {
                                                let manifest_name = std::path::Path::new(&fname).file_name()
                                                    .map(|n| n.to_string_lossy().to_string())
                                                    .unwrap_or(fname.clone());
                                                let dest = depot_cache.join(&manifest_name);
                                                if std::fs::write(&dest, &manifest_bytes).is_ok() {
                                                    log(format!("  ✓ Extracted Manifest: {}", manifest_name));
                                                    manifests_from_zip += 1;
                                                    
                                                    // Also save to Vault for future reinstalls
                                                    let vault_dir = std::path::Path::new("Vault").join(&appid);
                                                    let _ = std::fs::create_dir_all(&vault_dir);
                                                    let _ = std::fs::write(vault_dir.join(&manifest_name), &manifest_bytes);
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if lua_data.is_empty() {
                                    // No .lua in ZIP, treat whole response as raw text
                                    String::from_utf8_lossy(&zip_bytes).to_string()
                                } else {
                                    lua_data
                                }
                            } else {
                                // Not a valid ZIP, treat as raw .lua text
                                String::from_utf8_lossy(&zip_bytes).to_string()
                            };
                        
                        // Parse LUA for AppIDs and Keys
                        let (parsed_ids, parsed_keys) = crate::vdf_injector::parse_lua_for_keys(&lua_content);
                        log(format!("Parsed: {} AppList entries, {} depot keys.", parsed_ids.len(), parsed_keys.len()));
                        
                        applist_ids = parsed_ids;
                        keys = parsed_keys;
                        
                        // Save .lua for reference
                        let lua_path = depot_cache.join(format!("{}.lua", appid));
                        let _ = std::fs::write(&lua_path, &lua_content);
                        
                        // Download MISSING manifests via Wudrm CDN (only if ZIP didn't have them)
                        if manifests_from_zip == 0 {
                            log("No manifests in ZIP. Downloading via Wudrm CDN...".to_string());
                            match download_manifests_wudrm(&appid, &steam_root, &log) {
                                Ok(count) => {
                                    log(format!("✨ Downloaded {} depot manifests via Wudrm.", count));
                                },
                                Err(e) => {
                                    log(format!("Warning: Wudrm download issue: {}", e));
                                }
                            }
                        } else {
                            log(format!("✨ Got {} manifests from Morrenus ZIP. Skipping Wudrm.", manifests_from_zip));
                        }
                    },
                    Err(e) => {
                        log(format!("Morrenus Error: {}. Falling back to Wudrm-only mode.", e));
                        applist_ids.push(appid.clone());
                        let _ = download_manifests_wudrm(&appid, &steam_root, &log);
                    }
                }
            }
            
            // STEP 3: Filter AppList based on DLC Selection
            let mut final_ids = Vec::new();
            
            if let Some(h) = &hierarchy {
                log("Using GameHierarchy for Mandatory Depot Resolution...".to_string());
                final_ids = resolve_mandatory_depots(h, &selected_dlcs);
                log(format!("Resolved {} mandatory IDs (Base + DLCs + Depots).", final_ids.len()));
            } else {
                // Fallback: Use simple AppID + Selected DLCs logic (Legacy)
                // This might miss separate Depot IDs if Morrenus LUA doesn't group them clearly,
                // but usually works for simple setups.
                final_ids.push(appid.clone()); // CRITICAL FIX: Always include base game
                
                if !selected_dlcs.is_empty() {
                     log(format!("Using {} user-selected DLCs (Fallback Mode).", selected_dlcs.len()));
                     for id in &selected_dlcs {
                         if !final_ids.contains(id) {
                             final_ids.push(id.clone());
                         }
                     }
                } else {
                    log("No DLCs selected (Base Game Only).".to_string());
                }
            }
            
            // STEP 4: Inject ALL depot keys into config.vdf
            if !keys.is_empty() {
                log(format!("Injecting {} depot decryption keys into config.vdf...", keys.len()));
                if let Err(e) = crate::vdf_injector::inject_vdf(&steam_root, &keys) {
                    log(format!("Warning: Key injection failed: {}", e));
                } else {
                    log("✅ Keys injected successfully.".to_string());
                }
            }
            
            log(format!("AppList will contain {} entries.", final_ids.len()));
            log("✅ Morrenus Protocol Complete. Manifests + Keys Ready.".to_string());
            log("   → Steam will download the game files when you click 'Update' in the Library.".to_string());

            // SMD APPROACH: We do NOT regenerate the ACF here.
            // The minimal ACF was already created at the start.
            // Steam will update it automatically during the download process.
            
            // NUKE SQUAD: Preemptively remove installscript.vdf if it exists in the game folder
            let full_install_path = std::path::Path::new(&library_path).join("steamapps").join("common").join(&final_install_dir);
            // This prevents Steam from triggering the "SteamService" install phase which often fails.
            // This prevents Steam from triggering the "SteamService" install phase which often fails.
            {
                let script_path = full_install_path.join("installscript.vdf");
                if script_path.exists()
                     && std::fs::remove_file(&script_path).is_ok() {
                         log("☢️ NUKE: installscript.vdf deleted to bypass SteamService error.".to_string());
                     }
            }

            // VDF Injection (GreenLuma Override)
            // GreenLuma 2025 often uses its own config.vdf in its folder.
            if let Err(e) = inject_vdf(&gl_path, &keys) {
                 log(format!("GreenLuma VDF Warning (Non-Fatal): {}", e));
            } else {
                 log("✅ Depot Keys Injected into GreenLuma config.".to_string());
            }



            // STEP 3.5: LINK DLCs (Intelligent Linking)
            {
                if let Ok(mut map) = relationships_arc.lock() {
                    let mut changed = false;
                    for id in &final_ids {
                        if *id != appid {
                             map.insert(id.clone(), appid.clone());
                             changed = true;
                        }
                    }
                    if changed {
                        crate::app_list::save_relationships(".", &map);
                        log("DLC Relationships linked and saved.".to_string());
                    }
                }
            }

            // STEP 4: UPDATE APPLIST
            if !is_free {
                 log(format!("STEP 3: Injecting {} IDs to AppList...", final_ids.len()));
                 if let Err(e) = add_games_to_list(&gl_path, final_ids) {
                      log(format!("AppList Error: {}", e));
                 } else {
                      log("AppList updated successfully.".to_string());
                 }
            } else {
                 log("ℹ️ F2P Title Detected: Skipping GreenLuma AppList injection.".to_string());
            }

             // Update Cache
             {
                if let Ok(mut cache) = game_cache.lock() {
                    cache.insert(appid.clone(), name.clone());
                    let _ = save_game_cache(&cache);
                }
            }

            // STEP 4.5: BACKUP MANIFESTS TO VAULT (For future reinstalls)
            // This ensures that even if user uninstalls, they can reinstall without using API tokens
            if !skip_morrenus {
                // Only backup if we actually downloaded something new
                let backup_vault = crate::vault::VaultManager::new(".");
                match backup_vault.backup_manifests(&library_path, &appid) {
                    Ok(count) if count > 0 => {
                        log(format!("🛡️ Vault: Saved {} manifests for future reinstalls.", count));
                    },
                    _ => {}
                }
            }

            // STEP 5: STEALTH INJECTION & LAUNCH
            log("STEP 4: Initiating Stealth Launch Sequence (x64)...".to_string());
            
            // STEP 5: STEALTH INJECTION & LAUNCH (SUSPENDED)
            log("STEP 4: Initiating Stealth Launch Sequence (Suspended x64)...".to_string());
            
            let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
            let dll_name = "GreenLuma_2025_x64.dll";
            let dll_path = std::path::Path::new(&gl_path).join(dll_name);
            
            if steam_exe.exists() {
                 if dll_path.exists() {
                     // 3. Launch with EXTERNAL DLL (Legacy Behavior)
                     log("Launching Steam Suspended (External DLL - Phase 1)...".to_string());
                     
                     // Use Original DLL Path
                     let target_dll = std::path::Path::new(&gl_path).join(dll_name);
                     
                     if target_dll.exists() {
                         // PHASE 1: Launch Steam Injected (No AppLaunch yet)
                         match crate::injector::launch_injected(
                             steam_exe.to_str().unwrap_or(""),
                             target_dll.to_str().unwrap_or(""),
                             Some("-inhibitbootstrap")
                         ) {
                             Ok(_) => {
                                 log("✅ INJECTION SUCCESSFUL. Steam starting with GreenLuma...".to_string());
                                 log("Steam will open in a few seconds. The game should appear ready to 'Update'.".to_string());
                                 log("✅ INSTALLATION COMPLETE.".to_string());
                             },
                             Err(e) => log(format!("❌ LAUNCH FAILED: {}", e)),
                         }
                     } else {
                         log(format!("❌ CRITICAL: {} not found in GreenLuma folder!", dll_name));
                     }
                 } else {
                     log(format!("❌ CRITICAL: {} source not found!", dll_name));
                 }
            } else {
                log("❌ Error: steam.exe not found.".to_string());
            }

            // Refresh Stats
            let client = crate::api::ApiClient::new(api_key.clone());
            if let Ok(new_stats) = runtime.block_on(client.get_user_stats()) {
                if let Ok(mut stats_lock) = user_stats_arc.lock() {
                    *stats_lock = Some(new_stats);
                }
            }

            // Remove legacy open::that call - logic handled by args now
        });
    }


}

impl eframe::App for DarkCoreApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll Status Updates from Threads
        if let Ok(mut guard) = self.status_update_queue.lock() {
            if let Some(msg) = guard.take() {
                self.status_msg = msg;
            }
        }

        // Poll DLC Scanner (for DLC Picker during install)
        if self.is_scanning_dlcs && !self.delete_modal_open {
             let mut scan_done = false;
             if let Ok(res) = self.dlc_scan_result.lock() {
                  if res.is_some() {
                      scan_done = true;
                  }
             }
             
             if scan_done {
                 self.is_scanning_dlcs = false;
                 
                 // NEW: Read cached ZIP
                 if let Ok(mut zip_lock) = self.dlc_scan_result_zip.lock() {
                     if let Some(bytes) = zip_lock.take() {
                         self.dlc_picker_cached_bytes = Some(bytes);
                     }
                 }

                 if let Ok(mut res_lock) = self.dlc_scan_result.lock() {
                     if let Some((items, depot_count)) = res_lock.take() {
                         if !items.is_empty() {
                              self.dlc_picker_items = items;
                              // Select first 130 DLCs
                              self.dlc_picker_depot_count = depot_count;
                              self.dlc_picker_open = true;
                         } else {
                              // Auto Proceed (No DLCs found)
                              if let (Some(target), Some(dir)) = (self.dlc_picker_pending_library.take(), self.dlc_picker_pending_install_dir.take()) {
                                  if let Some((appid, name)) = self.dlc_picker_candidate.take() {
                                       // Pass cached bytes (if any)
                                       let cached = self.dlc_picker_cached_bytes.take();
                                       self.finalize_installation(appid, name, Some(target), Some(dir), Vec::new(), cached, None);
                                  }
                              }
                         }
                     }
                 }
             }
             ctx.request_repaint(); // Animation/Polling
        }
        
        // Poll Delete Scanner (for delete modal DLC association)
        if self.is_scanning_dlcs && self.delete_modal_open {
            let mut scan_done = false;
            if let Ok(res) = self.delete_scan_result.lock() {
                if res.is_some() {
                    scan_done = true;
                }
            }
            
            if scan_done {
                self.is_scanning_dlcs = false;
                if let Ok(mut res_lock) = self.delete_scan_result.lock() {
                    if let Some(associated) = res_lock.take() {
                        self.delete_associated_dlcs = associated;
                    }
                }
            }
            ctx.request_repaint();
        }

        // Custom Colors for this specific layout override
        let bg_sidebar = egui::Color32::from_rgb(18, 20, 28);
        let accent_cyan = egui::Color32::from_rgb(0, 243, 255);
        let accent_pink = egui::Color32::from_rgb(255, 0, 110);
        let _text_dim = egui::Color32::from_rgb(140, 140, 160);

            if self.logo_texture.is_none() {
                if let Some(data) = &self.logo_data {
                    self.logo_texture = Some(ctx.load_texture(
                        "logo_v5_final",
                        data.clone(),
                        egui::TextureOptions {
                            magnification: egui::TextureFilter::Linear,
                            minification: egui::TextureFilter::Linear,
                            mipmap_mode: Some(egui::TextureFilter::Linear),
                            ..egui::TextureOptions::LINEAR
                        }
                    ));
                }
            }

        // --- SIDEBAR ---
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(240.0)
            .frame(egui::containers::Frame::default().fill(bg_sidebar).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                // LOGO & IDENTITY
            ui.vertical_centered(|ui| {
                if let Some(texture) = &self.logo_texture {
                     // Animation State
                     let time = ui.input(|i| i.time);
                     let hover = (time * 1.5).sin() * 5.0; // +/- 5px Float
                     let pulse = (time * 2.0).sin() * 0.1 + 0.9; // 0.8-1.0 Opacity

                     // Continuous Repaint for Animation
                     ui.ctx().request_repaint();

                     // Dynamic Spacing (Floating Effect)
                     ui.add_space(15.0 + hover as f32);

                     let size = texture.size_vec2();
                     let target_width = 180.0;
                     let scale = target_width / size.x;
                     let target_height = size.y * scale;
                     
                     // Draw Animated Image
                     ui.add(
                        egui::Image::new((texture.id(), egui::vec2(target_width, target_height)))
                            .tint(egui::Color32::WHITE.linear_multiply(pulse as f32))
                     );
                     
                     // Counter-act spacing to keep header stable
                     ui.add_space(8.0 - hover as f32);
                } else {
                     ui.add_space(10.0);
                }

                // ARTISTIC HEADER
                ui.label(
                    egui::RichText::new("D A R K C O R E")
                            .family(egui::FontFamily::Monospace)
                            .size(22.0)
                            .strong()
                            .color(accent_cyan)
                    );
                });
                
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format!("MANAGER v{}", env!("CARGO_PKG_VERSION")))
                            .size(10.0)
                            .color(accent_pink)
                            .extra_letter_spacing(2.0),
                    );
                });
                
                ui.add_space(20.0);

                // --- COMMAND STRIP (TACTICAL HEADER) ---
                // "Cyber-Minimalism": Two buttons, 50% width, Ghost Style
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let available_w = ui.available_width();
                    let btn_w = (available_w - 4.0) / 2.0;
                    
                    // 1. GL STEALTH [GHOST GREEN]
                    let btn_stealth = egui::Button::new(
                        egui::RichText::new("👻 GL STEALTH")
                            .size(11.0)
                            .color(egui::Color32::GREEN)
                            .strong()
                    )
                    .min_size(egui::vec2(btn_w, 28.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 50))) // Subtle Green Border
                    .fill(egui::Color32::from_black_alpha(50)) // Transparent/Dark
                    .rounding(2.0);

                    if ui.add(btn_stealth).on_hover_text("Launch GreenLuma Stealth Mode (Safe Injection)").clicked() {
                         // Trigger Logic - Identical to previous implementation
                         let steam_path = self.config.steam_path.clone();
                         let gl_path = self.config.gl_path.clone();
                         let log_arc = self.system_log.clone();
                         let enable_stealth = self.config.enable_stealth_mode;
    
                         std::thread::spawn(move || {
                             let log = move |msg: String| {
                                 if let Ok(mut logs) = log_arc.lock() {
                                     push_log(&mut logs, msg);
                                 }
                             };
                             log("🚀 Manual Launch: Initiating Stealth Sequence (x64)...".to_string());
                             
                             let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                             let dll_name = "GreenLuma_2025_x64.dll";
                             let dll_path = std::path::Path::new(&gl_path).join(dll_name);
    
                             if steam_exe.exists() {
                                if dll_path.exists() {
                                     let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "steam.exe"]).output();
                                     std::thread::sleep(std::time::Duration::from_millis(1500));
                                     let _ = crate::ui::setup_greenluma_config(&gl_path, enable_stealth);
                                     match crate::injector::launch_injected(
                                         steam_exe.to_str().unwrap_or(""),
                                         dll_path.to_str().unwrap_or(""), 
                                         Some("-inhibitbootstrap")
                                     ) {
                                         Ok(_) => log("✅ Steam Launched with GreenLuma.".to_string()),
                                         Err(e) => log(format!("❌ Launch Failed: {}", e)),
                                     }
                                } else {
                                    log(format!("❌ Missing: {}", dll_name));
                                }
                             } else {
                                log("❌ steam.exe not found.".to_string());
                             }
                         });
                    }

                    // 2. RESET STEAM [GHOST RED]
                    let btn_reset = egui::Button::new(
                        egui::RichText::new("💀 RESET")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(255, 100, 100)) // Light Red
                            .strong()
                    )
                    .min_size(egui::vec2(btn_w, 28.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 50, 50))) // Subtle Red Border
                    .fill(egui::Color32::from_black_alpha(50))
                    .rounding(2.0);

                    if ui.add(btn_reset).on_hover_text("Force Kill Steam & Relaunch Normally (Emergency)").clicked() {
                        self.relaunch_steam_protocol();
                    }
                });

                // UPDATE AVAILABLE BUTTON
                if let Ok(update_lock) = self.update_available.lock() {
                    if let Some(new_ver) = update_lock.clone() {
                        drop(update_lock); // Release lock before UI
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let btn_text = format!("⬇ UPDATE AVAILABLE: v{}", new_ver);
                            let update_btn = egui::Button::new(
                                egui::RichText::new(btn_text)
                                    .color(egui::Color32::BLACK)
                                    .strong()
                            )
                            .fill(egui::Color32::from_rgb(0, 255, 128)) // FLUO GREEN
                            .min_size(egui::vec2(ui.available_width(), 32.0))
                            .rounding(4.0);

                            if ui.add(update_btn).clicked() {
                                // Trigger update in background
                                let log_arc = self.system_log.clone();
                                let updating_arc = self.is_updating.clone();
                                std::thread::spawn(move || {
                                    if let Ok(mut updating) = updating_arc.lock() {
                                        *updating = true;
                                    }
                                    let log = move |msg: String| {
                                        if let Ok(mut logs) = log_arc.lock() {
                                            push_log(&mut logs, msg);
                                        }
                                    };
                                    log("🔄 Starting OTA Update...".to_string());
                                    match crate::updater::perform_update() {
                                        Ok(_) => {
                                            log("✅ Update downloaded successfully!".to_string());
                                            log("🔄 Restarting application...".to_string());
                                            crate::updater::restart_application();
                                        }
                                        Err(e) => {
                                            log(format!("❌ Update failed: {}", e));
                                        }
                                    }
                                    if let Ok(mut updating) = updating_arc.lock() {
                                        *updating = false;
                                    }
                                });
                            }
                        });
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // NAV BUTTONS HELPER
                let mut nav_btn = |label: &str, icon: &str, tab_idx: usize| {
                   let is_active = self.active_tab == tab_idx;
                   let bg = if is_active { accent_cyan.linear_multiply(0.15) } else { egui::Color32::TRANSPARENT };
                   let fg = if is_active { accent_cyan } else { egui::Color32::from_gray(180) };
                   let stroke = if is_active { egui::Stroke::new(1.0, accent_cyan) } else { egui::Stroke::NONE };
                   
                   let btn = egui::Button::new(
                       egui::RichText::new(format!("{}  {}", icon, label))
                           .size(16.0)
                           .color(fg)
                   )
                   .fill(bg)
                   .stroke(stroke)
                   .frame(true)
                   .min_size(egui::vec2(200.0, 45.0));
                   
                   let response = ui.add(btn);
                   
                   // HOVER / CLICK NAVIGATION
                if (response.clicked() || response.hovered())
                       && self.active_tab != tab_idx {
                            self.active_tab = tab_idx;
                            self.tab_changed_at = Instant::now(); // Trigger Fade
                            if tab_idx == 2 {
                                self.refresh_library();
                            }
                       }
                   
                   // Ensure smooth animation when interacting
                   if response.hovered() {
                       ui.ctx().request_repaint();
                   }
                   ui.add_space(8.0);
                };

                nav_btn("INSTALL", "🚀", 0);
                nav_btn("LIBRARY", "📂", 2);
                // nav_btn("PROFILES", "💾", 3); // Removed
                // nav_btn("DRM INTEL", "🔍", 1); // MOVED: Steamless now integrated into Library
                nav_btn("SETTINGS", "⚙", 4);
                nav_btn("ABOUT", "💻", 5);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    // STATUS
                    ui.label(
                        egui::RichText::new(&self.status_msg)
                            .size(10.0)
                            .color(egui::Color32::from_gray(100)),
                    );

                    // AUDIO CONTROLS
                    if let Some(sink) = &self.audio_sink {
                        ui.separator();
                        ui.add_space(5.0);
                        
                        // CUSTOM NEON VOLUME BAR
                        let bar_height = 24.0;
                        let (rect, response) = ui.allocate_at_least(egui::vec2(ui.available_width(), bar_height), egui::Sense::click_and_drag());
                        
                        // INTERACTION
                        let mut volume_changed = false;
                        
                        // 1. Mouse Wheel (Requested Feature)
                        if response.hovered() {
                             let scroll = ui.input(|i| i.raw_scroll_delta.y);
                             if scroll != 0.0 {
                                  // Scroll up = Volume Up
                                  self.volume = (self.volume + scroll * 0.005).clamp(0.0, 1.0);
                                  volume_changed = true;
                             }
                        }
                        
                        // 2. Click/Drag
                        if response.dragged() || response.clicked() {
                             if let Some(ptr) = response.interact_pointer_pos() {
                                 let rel = (ptr.x - rect.min.x) / rect.width();
                                 self.volume = rel.clamp(0.0, 1.0);
                                 volume_changed = true;
                             }
                        }
                        
                        if volume_changed {
                            sink.set_volume(self.volume);
                            ui.ctx().request_repaint();
                        }

                        // VISUALS ("Extremely Cool")
                        let painter = ui.painter();
                        let time = ui.input(|i| i.time);
                        
                        // Background Groove
                        painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(200));
                        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::from_gray(40)));
                        
                        // Dynamic Fill
                        let fill_w = rect.width() * self.volume;
                        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
                        
                        // Neon Color Pulse
                        let pulse = (time * 3.0).sin() * 0.2 + 0.8;
                        let neon_base = egui::Color32::from_rgb(0, 255, 200); // Cyan-Green
                        let neon_color = neon_base.linear_multiply(pulse as f32);
                        
                        if self.volume > 0.0 {
                            painter.rect_filled(fill_rect, 4.0, neon_color.linear_multiply(0.3)); // Glow halo
                            painter.rect_filled(fill_rect.shrink(2.0), 3.0, neon_color); // Core
                        }
                        
                        // FAKE AUDIO WAVES (Spectrum Visualizer Effect)
                        let bars = 18;
                        let bar_w = rect.width() / bars as f32;
                        for i in 0..bars {
                             let x = rect.min.x + i as f32 * bar_w;
                             // Simulation: Sine wave based on time + index + volume loudness
                             let phase = time * 8.0 + (i as f64 * 0.8);
                             // Amplitude modulated by volume (so it flattens when quiet)
                             let raw_amp = (phase.sin() * 0.5 + 0.5) as f32; 
                             let amp = raw_amp * (self.volume * 1.5).min(1.0); 
                             
                             let h = rect.height() * 0.7 * amp;
                             if h < 2.0 { continue; }
                             
                             let y_base = rect.max.y - 4.0;
                             let y_top = y_base - h;

                             // Only draw bars essentially "inside" the fill for contrast? 
                             // Or draw everywhere?
                             // Let's draw white bars inside the fill, gray outside?
                             let bar_rect = egui::Rect::from_min_max(egui::pos2(x + 1.0, y_top), egui::pos2(x + bar_w - 1.0, y_base));
                             
                             if x < rect.min.x + fill_w {
                                 // Active Spectrum
                                 painter.rect_filled(bar_rect, 1.0, egui::Color32::WHITE.linear_multiply(0.6));
                             } else {
                                 // Passive (Dark)
                                 painter.rect_filled(bar_rect, 1.0, egui::Color32::from_white_alpha(10));
                             }
                        }
                        
                        // Text Overlay (Volume %)
                        let vol_pct = (self.volume * 100.0) as u32;
                        painter.text(
                            rect.center(), 
                            egui::Align2::CENTER_CENTER, 
                            format!("VOL {}%", vol_pct), 
                            egui::FontId::proportional(10.0), 
                            egui::Color32::WHITE
                        );

                        // PLAY/PAUSE Toggle
                        ui.add_space(4.0);
                        let btn_txt = if sink.is_paused() { "▶ RESUME AUDIO" } else { "⏸ PAUSE AUDIO" };
                        let btn = egui::Button::new(egui::RichText::new(btn_txt).size(10.0).strong())
                            .min_size(egui::vec2(rect.width(), 16.0))
                            .fill(egui::Color32::from_black_alpha(100))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)));
                            
                        if ui.add(btn).clicked() {
                             if sink.is_paused() { sink.play(); } else { sink.pause(); }
                        }
                        ui.add_space(5.0);
                    }
                    ui.separator();
                });
            });

        // --- CENTRAL CONTENT ---
        egui::CentralPanel::default()
            .frame(egui::containers::Frame::default().fill(egui::Color32::from_rgb(11, 12, 16)).inner_margin(24.0))
            .show(ctx, |ui| {
                // ANIMATION
                let dt = self.tab_changed_at.elapsed().as_secs_f32();
                let alpha = (dt / 0.25).clamp(0.0, 1.0); // 250ms fade
                ui.set_opacity(alpha);
                if alpha < 1.0 {
                    ui.ctx().request_repaint();
                }
                // WARNING - SUPER ANIMATED CONFIGURATION REQUIRED
                if self.config.steam_path.is_empty() || self.config.gl_path.is_empty() {
                    let time = ui.input(|i| i.time);
                    
                    // Pulsing red glow effect
                    let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
                    let glow_alpha = (pulse * 100.0) as u8 + 50;
                    let border_color = egui::Color32::from_rgba_unmultiplied(255, 50, 50, glow_alpha + 100);
                    let bg_color = egui::Color32::from_rgba_unmultiplied(80, 0, 0, glow_alpha);
                    
                    // Animated border thickness
                    let border_width = 2.0 + pulse * 2.0;
                    
                    egui::Frame::none()
                        .fill(bg_color)
                        .stroke(egui::Stroke::new(border_width, border_color))
                        .rounding(8.0)
                        .inner_margin(15.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Animated warning icon (alternating)
                                let icon = if (time * 2.0) as i32 % 2 == 0 { "⚠" } else { "🔧" };
                                ui.label(
                                    egui::RichText::new(icon)
                                        .size(28.0)
                                        .color(egui::Color32::from_rgb(255, (100.0 + pulse * 155.0) as u8, 50))
                                );
                                
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("CONFIGURATION REQUIRED")
                                            .size(18.0)
                                            .strong()
                                            .color(egui::Color32::from_rgb(255, (200.0 - pulse * 100.0) as u8, (200.0 - pulse * 100.0) as u8))
                                    );
                                    ui.label(
                                        egui::RichText::new("Steam and GreenLuma paths must be configured.")
                                            .size(12.0)
                                            .color(egui::Color32::from_gray(180))
                                    );
                                });
                                
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Animated button with glow
                                    let btn_color = egui::Color32::from_rgb(
                                        (100.0 + pulse * 155.0) as u8,
                                        255,
                                        (100.0 + pulse * 155.0) as u8
                                    );
                                    
                                    let btn = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new("⚙ GO TO SETTINGS")
                                                .size(14.0)
                                                .strong()
                                                .color(egui::Color32::BLACK)
                                        )
                                        .fill(btn_color)
                                        .rounding(6.0)
                                    );
                                    
                                    if btn.clicked() {
                                        self.active_tab = 4; // Settings tab
                                        self.tab_changed_at = std::time::Instant::now();
                                    }
                                    
                                    if btn.hovered() {
                                        ui.ctx().request_repaint();
                                    }
                                });
                            });
                        });
                    
                    ui.add_space(15.0);
                    // Only request continuous repaint for animated tabs (Info tab with Matrix Rain)
                    // Other tabs use on-demand repaint to save GPU
                    if self.active_tab == 5 {
                        ui.ctx().request_repaint();
                    }
                }

                // GLOBAL HUD (Persistent Console)
                // Bottom Panel inside Central Panel
                if self.active_tab != 5 { // Hide on About/Info tab for full immersion
                    egui::TopBottomPanel::bottom("global_hud_console")
                        .resizable(true)
                        .default_height(140.0)
                        .show_inside(ui, |ui| {
                            self.render_global_logs(ui);
                        });
                }

                // CONTENT AREA (Remaining Space)
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    match self.active_tab {
                        0 => self.ui_installation(ui),
                        // 1 was DRM INTEL - now integrated into Library per-game
                        2 => self.ui_library(ui),
                        // 3 was Profiles
                        4 => self.ui_settings(ui),
                        5 => self.ui_info(ui),
                        _ => self.ui_installation(ui),
                    }
                });
                
                // Global Footer Removed (Logs are now per-tab or sidebar)
                ui.add_space(5.0);
            });

        // POLL DELETE SCAN RESULT
        {
            let mut res = self.delete_scan_result.lock().unwrap();
            if let Some(data) = res.take() {
                self.delete_associated_dlcs = data;
                // Don't set is_scanning_dlcs=false here, that's for DLC picker
            }
        }

        // DELETE MODAL
        if self.delete_modal_open {
            egui::Window::new("CONFIRM DELETION")
                .collapsible(false)
                .resizable(false)
                .fixed_size([400.0, 200.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading(format!(
                        "Delete '{}'?",
                        self.delete_candidate_name.as_deref().unwrap_or("Unknown")
                    ));
                    ui.label(format!(
                        "ID: {}",
                        self.delete_candidate_id.as_deref().unwrap_or("?")
                    ));

                    ui.add_space(10.0);

                    if self.is_scanning_dlcs {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Scanning for associated DLCs...");
                        });
                    } else if !self.delete_associated_dlcs.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "⚠ Found {} associated DLCs/Depots installed.",
                                self.delete_associated_dlcs.len()
                            ))
                            .color(egui::Color32::YELLOW),
                        );
                        ui.label("They will be deleted automatically.");
                    } else {
                        ui.label("No associated DLCs found in library.");
                    }

                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        if ui.button("CANCEL").clicked() {
                            self.delete_modal_open = false;
                            self.delete_associated_dlcs.clear();
                        }

                        if !self.is_scanning_dlcs {
                            // OPTION 1: UNLINK (SAFE)
                            if ui
                                .button(
                                    egui::RichText::new("🗑 UNLINK ID (SAFE)").color(egui::Color32::from_rgb(255, 165, 0)),
                                )
                                .on_hover_text("Removes from AppList & Config only.\nKEEPS game files and manifests on disk.")
                                .clicked()
                            {
                                let mut to_delete = vec![self.delete_candidate_id.clone().unwrap()];
                                to_delete.extend(self.delete_associated_dlcs.iter().cloned());

                                self.remove_games_by_id(to_delete, false);

                                self.delete_modal_open = false;
                                self.refresh_library();
                            }

                            // OPTION 2: FULL WIPE
                            if ui
                                .button(
                                    egui::RichText::new("🔥 FULL UNINSTALL").color(egui::Color32::RED).strong(),
                                )
                                .on_hover_text("DESTRUCTIVE.\nRemoves AppList, Config, Manifests AND DELETES GAME FILES.")
                                .clicked()
                            {
                                let mut to_delete = vec![self.delete_candidate_id.clone().unwrap()];
                                to_delete.extend(self.delete_associated_dlcs.iter().cloned());

                                self.remove_games_by_id(to_delete, true);

                                self.delete_modal_open = false;
                                self.refresh_library();
                            }
                        }
                    });
                });
        } // Close if self.delete_modal_open
        
        // MODALS
        self.show_install_modal(ctx);
        self.show_dlc_picker_modal(ctx);
        self.show_manifestor_modal(ctx);
    }
}

impl DarkCoreApp {
    fn relaunch_steam_protocol(&self) {
        let steam_path = self.config.steam_path.clone();
        let log_arc = self.system_log.clone();
        
        std::thread::spawn(move || {
            let log = move |msg: String| {
                if let Ok(mut logs) = log_arc.lock() {
                    push_log(&mut logs, msg);
                }
            };
            
            log("⚠ STEAM PURGE PROTOCOL INITIATED...".to_string());
            
            // 1. Kill Steam
            let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "steam.exe"]).output();
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

    fn render_global_logs(&self, ui: &mut egui::Ui) {
         // 1. HEADER BAR "HUD STYLE"
         ui.horizontal(|ui| {
             ui.label(egui::RichText::new("📟 SYSTEM TERMINAL").size(10.0).font(egui::FontId::monospace(10.0)).color(egui::Color32::from_gray(100)));
             
             ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                 // COPY RAW BUTTON
                 if ui.button(egui::RichText::new("📋 COPY RAW").size(10.0)).clicked() {
                     if let Ok(logs) = self.system_log.lock() {
                         let full_log = logs.join("\n");
                         ui.ctx().output_mut(|o| o.copied_text = full_log);
                     }
                 }
             });
         });
         
         ui.separator();
         
         // 2. SCROLLABLE LOG AREA
         egui::ScrollArea::vertical()
             .stick_to_bottom(true)
             .auto_shrink([false, false])
             .show(ui, |ui| {
                 // Dark background for terminal effect
                 ui.painter().rect_filled(
                     ui.available_rect_before_wrap(),
                     0.0,
                     egui::Color32::from_rgb(10, 10, 12) // Very dark gray/black
                 );

                 if let Ok(logs) = self.system_log.lock() {
                     for entry in logs.iter() {
                         // Colorize based on content
                         let color = if entry.contains("❌") || entry.contains("Error") || entry.contains("Failed") {
                             egui::Color32::from_rgb(255, 80, 80)
                         } else if entry.contains("✅") || entry.contains("Success") {
                             egui::Color32::from_rgb(80, 255, 80)
                         } else if entry.contains("⚠") || entry.contains("Warning") {
                             egui::Color32::from_rgb(255, 200, 50)
                         } else if entry.contains("🚀") {
                             egui::Color32::from_rgb(0, 255, 255) // Cyan
                         } else {
                             egui::Color32::from_gray(180)
                         };
                         
                         ui.label(egui::RichText::new(entry)
                             .font(egui::FontId::monospace(11.0)) // Slightly larger monospace
                             .color(color)
                         );
                     }
                 }
             });
    }

    fn process_cover_queue(&mut self, ctx: &egui::Context) {
        let mut queue_guard = self.cover_queue.lock().unwrap();
        if queue_guard.is_empty() {
            return;
        }

        // Process up to 5 images per frame to avoid lag
        let count = queue_guard.len().min(5);
        let items: Vec<_> = queue_guard.drain(0..count).collect();
        drop(queue_guard); // Release lock

        if let Ok(mut cache) = self.cover_cache.lock() {
            for (appid, w, h, pixels) in items {
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                let texture = ctx.load_texture(
                    format!("cover_{}", appid),
                    image,
                    egui::TextureOptions::LINEAR, // High-res 1440p+ rendering
                );
                cache.insert(appid, Some(texture));
            }
        }
        ctx.request_repaint();
    }

    fn ui_installation(&mut self, ctx_ui: &mut egui::Ui) {
        self.process_cover_queue(ctx_ui.ctx()); // Process queue here

        // SYSTEM LOGS (Pinned Bottom)
        // Logs moved to bottom.


        // MAIN CONTENT
        egui::CentralPanel::default().show_inside(ctx_ui, |ui| {
        ui.label(
            egui::RichText::new("SEARCH & AUTOMATION")
                .color(egui::Color32::from_rgb(0, 200, 255))
                .strong(),
        );
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .min_size(egui::vec2(200.0, 25.0))
                    .hint_text("Enter Game Name...")
                    .font(egui::FontId::proportional(14.0)),
            );

            if response.changed() {
                self.last_input_time = Some(Instant::now());
            }
            // ... (Debounce Logic same as before)
            if let Some(last_time) = self.last_input_time {
                if last_time.elapsed() > Duration::from_millis(500) {
                    if self.search_query != self.last_searched_query {
                        self.perform_search();
                    }
                    self.last_input_time = None;
                } else {
                    ui.ctx().request_repaint();
                }
            }

            if ui
                .button(egui::RichText::new("🔎 SEARCH").size(14.0))
                .clicked()
            {
                self.perform_search();
                self.last_input_time = None;
            }

            ui.add_space(20.0);
            // Launch Button moved to Sidebar Command Center
        });

        ui.add_space(5.0);
        ui.checkbox(
            &mut self.include_dlcs,
            egui::RichText::new("Include DLCs/Depots Automatically")
                .color(egui::Color32::LIGHT_GRAY),
        );
        ui.add_space(5.0);
        // NOISE FILTER CHECKBOX
        ui.checkbox(
            &mut self.show_free_content, 
            egui::RichText::new("Show Free/Demo Content").color(egui::Color32::from_gray(140))
        )
        .on_hover_text("If unchecked, hides Free-to-Play games to reduce noise.\nChecking this allows installing free games without GreenLuma injection.");
        ui.add_space(10.0);

        let search_results = self.search_results.clone();
        let results = search_results.lock().unwrap();

        let available = ui.available_height();
        // Logs are now in a dedicated panel, so use full available height.
        let results_h = available.max(100.0);

        // Cache installed IDs for O(1) lookup
        let installed_ids: std::collections::HashSet<String> = {
            if let Ok(games) = self.active_games.lock() {
                games.iter().map(|g| g.app_id.clone()).collect()
            } else {
                std::collections::HashSet::new()
            }
        };

        egui::ScrollArea::vertical().id_salt("results_scroll").max_height(results_h).show(ui, |ui| {
            let avail_width = ui.available_width();
            
            // RESPONSIVE GRID CALCULATION
            // Target: Dense, immersive layout for 1440p+
            let min_card_width = 180.0_f32;  // Minimum card width
            let spacing = 6.0_f32;           // Tight spacing between cards
            
            // Calculate optimal columns that fill the width
            let cols = ((avail_width + spacing) / (min_card_width + spacing)).floor().max(1.0) as usize;
            
            // Dynamic card width: fills available space exactly
            let card_w = (avail_width - (spacing * (cols as f32 - 1.0))) / cols as f32;
            
            // Aspect ratio 2:3 (Steam Vertical Capsule standard)
            let cover_h = card_w * 1.5;  // 2:3 ratio
            let info_h = 75.0;           // Expanded footer: title (2 lines) + ID + robust button
            let card_h = cover_h + info_h;

            egui::Grid::new("results_grid_manual")
                .spacing(egui::vec2(spacing, spacing))
                .min_col_width(card_w)
                .show(ui, |ui| {
                    for (i, res) in results.iter().enumerate() {
                        // NOISE FILTER LOGIC
                        if !self.show_free_content && res.is_free {
                            continue;
                        }
                        
                        use crate::api::val_to_string;
                        let name = res.game_name.as_deref().or(res.name.as_deref()).unwrap_or("Unknown");
                        let id1 = val_to_string(&res.game_id);
                        let id2 = val_to_string(&res.app_id);
                        let id = if !id1.is_empty() { id1 } else { id2 };
                        let display_id = if id.is_empty() { "0".to_string() } else { id.clone() };
                        let is_installed = installed_ids.contains(&display_id);

                        let card_id = ui.make_persistent_id(format!("card_{}_{}", i, display_id));
                        let _ = ui.ctx().animate_bool(card_id, false);

                        ui.push_id(card_id, |ui| {
                            // CINEMATIC CARD: Zero-gap, borderless, full-bleed artwork
                            // Container has NO stroke, only subtle rounding at bottom
                            let frame_style = egui::Frame::none()
                                .fill(egui::Color32::from_rgb(20, 20, 24))
                                .inner_margin(0.0)
                                .outer_margin(0.0)
                                .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 4.0, se: 4.0 })
                                .stroke(egui::Stroke::NONE); // NO BORDER
                                
                            let response = frame_style.show(ui, |ui| {
                                ui.set_min_size(egui::vec2(card_w, card_h));
                                ui.set_max_size(egui::vec2(card_w, card_h));
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0); // Zero internal spacing
                                
                                ui.vertical(|ui| {
                                    // COVER IMAGE - ABSOLUTE FULL BLEED
                                    // No inner frame, image IS the container
                                    ui.allocate_ui(egui::vec2(card_w, cover_h), |ui| {
                                        if !display_id.is_empty() && display_id != "0" {
                                            let cache = self.cover_cache.lock().unwrap();
                                            if let Some(Some(texture)) = cache.get(&display_id) {
                                                // Aspect Ratio Check
                                                let w = texture.size()[0] as f32;
                                                let h = texture.size()[1] as f32;
                                                let ratio = w / h.max(1.0);
                                                
                                                if ratio > 1.2 {
                                                    // LANDSCAPE / HEADER (Letterbox) - "Box Diverso"
                                                    egui::Frame::none()
                                                        .fill(egui::Color32::from_rgb(10, 10, 12)) // Letterbox Bars
                                                        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                                                        .show(ui, |ui| {
                                                            ui.set_min_size(egui::vec2(card_w, cover_h));
                                                            ui.centered_and_justified(|ui| {
                                                                ui.add(egui::Image::new(texture)
                                                                    .fit_to_exact_size(egui::vec2(card_w, cover_h)) // This sets bounds
                                                                    .maintain_aspect_ratio(true) // This prevents stretching
                                                                );
                                                            });
                                                        });
                                                } else {
                                                    // PORTRAIT / POSTER (Full Bleed)
                                                    ui.add(egui::Image::new(texture)
                                                        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                                                        .fit_to_exact_size(egui::vec2(card_w, cover_h))
                                                        .maintain_aspect_ratio(false)
                                                    );
                                                }
                                            } else {
                                                // Loading placeholder - minimal, dark
                                                egui::Frame::none()
                                                    .fill(egui::Color32::from_rgb(15, 15, 18))
                                                    .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                                                    .show(ui, |ui| {
                                                        ui.set_min_size(egui::vec2(card_w, cover_h));
                                                        ui.centered_and_justified(|ui| {
                                                            ui.spinner();
                                                        });
                                                    });
                                            }
                                        } else {
                                            // No ID placeholder
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgb(12, 12, 14))
                                                .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                                                .show(ui, |ui| {
                                                    ui.set_min_size(egui::vec2(card_w, cover_h));
                                                });
                                        }
                                    });
                                    
                                    // COMPACT INFO FOOTER - Tight, elegant
                                    ui.add_space(4.0);
                                    
                                    // TITLE - Label with truncation
                                    ui.horizontal(|ui| {
                                        ui.add_space(4.0);
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(name)
                                                    .size(11.0)
                                                    .color(egui::Color32::from_rgb(220, 220, 225))
                                                    .strong()
                                            )
                                            .truncate()
                                        );
                                    });

                                    // APP ID - Subtitle
                                    ui.horizontal(|ui| {
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new(&display_id)
                                                .size(9.0)
                                                .color(egui::Color32::from_gray(100))
                                                .monospace()
                                        );
                                    });
                                    
                                    ui.add_space(4.0);

                                    // BUTTON STATE CALCULATION
                                    let mut is_dlc_linked = false;
                                    let mut parent_game_id = String::new();
                                    
                                    if is_installed {
                                         if let Ok(rel) = self.relationships.lock() {
                                             if let Some(pid) = rel.get(&display_id) {
                                                 is_dlc_linked = true;
                                                 parent_game_id = pid.clone();
                                             }
                                         }
                                    }

                                    let text = if is_installed { 
                                         if is_dlc_linked { "🔗 LINKED" }
                                         else { "▶ PLAY" }
                                    } else { "🚀 INSTALL" };

                                    let bg_color = if is_installed {
                                         if is_dlc_linked { egui::Color32::from_rgb(50, 60, 75) } 
                                         else { egui::Color32::from_rgb(0, 200, 100) }
                                    } else {
                                        let time = ui.input(|i| i.time);
                                        let alpha = (time * 3.0).sin().abs() as f32 * 0.3 + 0.7; 
                                        egui::Color32::from_rgba_premultiplied(0, (255.0 * alpha) as u8, (140.0 * alpha) as u8, 255)
                                    };
                                    
                                    let limit_reached = self.active_games.lock().unwrap().len() >= 134;

                                    // ROBUST BUTTON - Readable, clickable
                                    let btn_resp = ui.horizontal(|ui| {
                                        ui.add_space(3.0);
                                        let btn_width = card_w - 6.0;
                                        
                                        if limit_reached && !is_installed {
                                             ui.add(egui::Button::new(egui::RichText::new("⛔ LIMIT REACHED").size(10.0))
                                                .fill(egui::Color32::from_rgb(45, 45, 50))
                                                .min_size(egui::vec2(btn_width, 24.0))
                                                .rounding(4.0))
                                                .on_hover_text("Max AppList limit (134) reached.")
                                        } else {
                                             let btn_txt_size = 11.0; 
                                             let btn_resp = ui.add(egui::Button::new(egui::RichText::new(text).size(btn_txt_size).color(egui::Color32::BLACK).strong())
                                                .fill(bg_color)
                                                .min_size(egui::vec2(btn_width, 24.0))
                                                .rounding(4.0));
                                                
                         if btn_resp.clicked() {
                             if is_installed {
                                 // PLAY / LAUNCH
                                 // existing launch logic...
                                 let _ = std::process::Command::new("explorer")
                                    .arg(format!("steam://rungameid/{}", display_id))
                                    .spawn();
                             } else {
                                 // INSTALL -> MANIFESTOR
                                 self.install_candidate_is_free = res.is_free;
                                 self.open_manifestor(display_id.clone(), name.to_string());
                             }
                         }
                         btn_resp
                                        }
                                    }).inner;
                                    
                                    ui.add_space(2.0);

                                    // CONTEXT MENU
                                    btn_resp.context_menu(|ui: &mut egui::Ui| {
                                        let is_godmode = self.config.family_godmode_ids.contains(&display_id);
                                        if is_godmode {
                                            ui.label(egui::RichText::new("⚡ GODMODE ACTIVE").color(egui::Color32::GREEN).size(10.0));
                                            if ui.button("💀 Disable Godmode").clicked() {
                                                ui.close_menu();
                                                self.disable_family_godmode(display_id.clone());
                                            }
                                        } else {
                                            if is_installed {
                                                if ui.button("🛠 Force Repair").clicked() {
                                                    ui.close_menu();
                                                    self.detected_libraries = crate::game_path::GamePathFinder::get_library_folders(&self.config.steam_path);
                                                    self.selected_library_index = 0;
                                                    self.install_candidate = Some((display_id.clone(), name.to_string()));
                                                    self.install_dir_input = name.to_string(); 
                                                    self.install_modal_open = true;
                                                }
                                                // RESTORED STEAMLESS BUTTON
                                                if ui.button("🔨 Unpack Game (Steamless)").clicked() {
                                                    ui.close_menu();
                                                    let appid_run = display_id.clone();
                                                    let steam_path = self.config.steam_path.clone();
                                                    let steamless_path = self.config.steamless_path.clone();
                                                    let log_arc = self.system_log.clone();
                                                    
                                                    std::thread::spawn(move || {
                                                        let log = move |msg: String| {
                                                            if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, msg); }
                                                        };
                                                        
                                                        log(format!("Steamless: Locating folder for {}...", appid_run));
                                                        // 1. Find Game Path using the correct method
                                                        if let Some(game_folder) = crate::game_path::GamePathFinder::find_game_path(&steam_path, &appid_run) {
                                                            log(format!("Target Folder: {}", game_folder.display()));
                                                            
                                                            // 2. Run Steamless on Folder
                                                            log("Scanning for EXEs to unpack...".to_string());
                                                            let (success, total, results) = crate::steamless::run_steamless_folder(&game_folder, &steamless_path, &appid_run);
                                                            
                                                            for res in results {
                                                                if res.success {
                                                                    log(format!("✅ {}: {}", res.exe_path, res.message));
                                                                } else {
                                                                    log(format!("⚠️ {}: {}", res.exe_path, res.message));
                                                                }
                                                            }
                                                            
                                                            log(format!("Steamless Complete. Unpacked {}/{} files.", success, total));
                                                        } else {
                                                            log("❌ Game folder not found. Is the game installed?".to_string());
                                                        }
                                                    });
                                                }
                                                if ui.button("👨‍👩‍👧 Enable Godmode").clicked() {
                                                    ui.close_menu();
                                                    self.install_game_family_godmode(display_id.clone());
                                                }
                                            } else {
                                                if ui.button("👨‍👩‍👧 Install (Godmode Only)").clicked() {
                                                    ui.close_menu();
                                                    self.install_game_family_godmode(display_id.clone());
                                                }
                                            }
                                        }
                                    });

                                    // CLICK HANDLER
                                    if btn_resp.clicked() {
                                        if is_dlc_linked {
                                            self.log(format!("Linked to {}. Launch base game.", parent_game_id));
                                        } else if !is_installed {
                                             // INSTALL TRIGGER
                                             if let Some(path) = crate::game_path::GamePathFinder::find_manifest_path(&self.config.steam_path, &display_id) {
                                                 self.install_game(display_id.clone(), name.to_string(), Some(path.parent().and_then(|p| p.parent()).unwrap_or(std::path::Path::new(&self.config.steam_path)).to_path_buf()), None);
                                             } else {
                                                 let libraries = crate::game_path::GamePathFinder::get_library_folders(&self.config.steam_path);
                                                 let (found_dir, found_lib, _) = self.detect_auto_install_path(name, &libraries);
                                                 if let Some(dir_name) = found_dir {
                                                     self.install_game(display_id.clone(), name.to_string(), Some(found_lib.unwrap_or(std::path::Path::new(&self.config.steam_path).to_path_buf())), Some(dir_name));
                                                 } else {
                                                     let sanitized = name.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().trim().to_string();
                                                     self.detected_libraries = libraries;
                                                     self.selected_library_index = 0;
                                                     self.install_candidate = Some((display_id.clone(), name.to_string()));
                                                     self.install_dir_input = sanitized; 
                                                     self.install_modal_open = true;
                                                 }
                                             }
                                        } else {
                                            // PLAY TRIGGER
                                            let steam_path = self.config.steam_path.clone();
                                            let gl_path = self.config.gl_path.clone();
                                            let app_id_run = display_id.clone();
                                            let enable_stealth = self.config.enable_stealth_mode;
                                            
                                            std::thread::spawn(move || {
                                                let _ = crate::ui::setup_greenluma_config(&gl_path, enable_stealth);
                                                let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                                                let is_injected = crate::injector::is_greenluma_injected();
                                                let is_running = crate::injector::is_process_running("steam.exe");

                                                if is_running {
                                                    if is_injected {
                                                        let _ = std::process::Command::new(steam_exe).arg("-applaunch").arg(&app_id_run).spawn();
                                                    } else {
                                                        let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "steam.exe"]).output();
                                                        std::thread::sleep(std::time::Duration::from_millis(2000));
                                                        let dll_path = std::path::Path::new(&gl_path).join("GreenLuma_2025_x64.dll");
                                                        let _ = crate::injector::launch_injected(
                                                            steam_exe.to_str().unwrap_or(""),
                                                            dll_path.to_str().unwrap_or(""),
                                                            Some(&format!("-applaunch {}", app_id_run))
                                                        );
                                                    }
                                                } else {
                                                    let dll_path = std::path::Path::new(&gl_path).join("GreenLuma_2025_x64.dll");
                                                    let _ = crate::injector::launch_injected(
                                                        steam_exe.to_str().unwrap_or(""),
                                                        dll_path.to_str().unwrap_or(""),
                                                        Some(&format!("-applaunch {}", app_id_run))
                                                    );
                                                }
                                            });
                                        }
                                    }
                                });
                            });
                            
                            if response.response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        }); // push_id

                        // Force new row after `cols` items
                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        ui.separator();
        ui.horizontal(|ui| { 
             ui.label("📜"); 
             ui.label(egui::RichText::new("SYSTEM LOGS").size(12.0).strong().color(egui::Color32::GRAY)); 
        });

        egui::ScrollArea::vertical().id_salt("log_scroll").max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
             if let Ok(log) = self.system_log.lock() {
                 for line in log.iter() {
                     ui.label(egui::RichText::new(line).font(egui::FontId::monospace(10.0)).color(egui::Color32::LIGHT_GRAY));
                 }
             }
        });
        });
    }

    // --- HELPER METHODS ---

    fn install_game_family_godmode(&mut self, appid: String) {
       // 1. Update Persistent State
       if !self.config.family_godmode_ids.contains(&appid) {
           self.config.family_godmode_ids.push(appid.clone());
           let _ = crate::config::save_config(&self.config);
       }

       let gl_path = self.config.gl_path.clone();
       let include_dlcs = self.include_dlcs;
       // Clone client if it exists, otherwise we will rely on public API inside thread if possible or skip
       let client_opt = self.api_client.clone(); 
       let log_arc = self.system_log.clone();
       let status_queue = self.status_update_queue.clone();

       std::thread::spawn(move || {
           let log = move |msg: String| {
               if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, msg); }
           };
           
           log(format!("Family Godmode: Initializing for {}...", appid));

           // Build ID List
           let mut ids = vec![appid.clone()];

           // FETCH DLCs (Even without API Key, using public store API)
           // We use the method from api_client. If api_client is None (no key), we might need a fallback?
           // Actually api_client is constructed with key, but methods like get_dlc_list use public endpoints mostly?
           // Wait, get_dlc_list in api.rs uses self.client.get but doesn't strictly need API key for store.steampowered.com logic
           // BUT api_client instance might not exist if key was empty? 
           // In `new()`, api_client is Some(...) only if key is valid.
           // However, for Fallback mode, we need to be able to call get_dlc_list.
           // Ideally we should create a temporary client if None.
           
           // Simple workaround: Create a temporary one-off client in the thread if needed, or make get_dlc_list static?
           // Easier: If client_opt is None, try to create a standard reqwest client.
           
           if include_dlcs {
               let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
               log("Fetching DLCs...".to_string());
               
               let dlcs_result = if let Some(client) = client_opt {
                    rt.block_on(client.get_dlc_list(&appid))
               } else {
                    // Fallback URL fetch
                   rt.block_on(async {
                       let client = reqwest::Client::new();
                       let url = format!("https://store.steampowered.com/api/appdetails?appids={}&filters=dlc", appid);
                       if let Ok(resp) = client.get(&url).send().await {
                           if let Ok(root) = resp.json::<serde_json::Value>().await {
                               let mut dlc_ids = Vec::new();
                               if let Some(app_data) = root.get(&appid) {
                                   if let Some(data) = app_data.get("data") {
                                       if let Some(dlc_array) = data.get("dlc").and_then(|v| v.as_array()) {
                                           for item in dlc_array {
                                               if let Some(id) = item.as_u64() { dlc_ids.push(id.to_string()); }
                                           }
                                       }
                                   }
                               }
                               return Ok(dlc_ids);
                           }
                       }
                       Ok(vec![])
                   })
               };

               match dlcs_result {
                    Ok(dlcs) => {
                        log(format!("Found {} DLCs to unlock.", dlcs.len()));
                        ids.extend(dlcs);
                    },
                    Err(e) => log(format!("DLC Fetch Warning: {}", e)),
               }
           }

           // Add to AppList
           match crate::app_list::add_games_to_list(&gl_path, ids) {
               Ok(_) => {
                   log("✅ Family Shared Godmode Active.".to_string());
                   if let Ok(mut q) = status_queue.lock() {
                       *q = Some("REFRESH_LIB".to_string());
                   }
               },
               Err(e) => log(format!("❌ Error writing AppList: {}", e)),
           }
       });
    }

    fn disable_family_godmode(&mut self, appid: String) {
        // 1. Update Persistent State
        if let Some(pos) = self.config.family_godmode_ids.iter().position(|x| *x == appid) {
            self.config.family_godmode_ids.remove(pos);
            let _ = crate::config::save_config(&self.config);
        }

        let gl_path = self.config.gl_path.clone();
        let client_opt = self.api_client.clone();
        let log_arc = self.system_log.clone();
        let status_queue = self.status_update_queue.clone();

        std::thread::spawn(move || {
            let log = move |msg: String| {
                if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, msg); }
            };
            
            log(format!("Disabling Family Godmode for {}...", appid));
            
            // To clean up, we need to know what to remove (AppID + DLCs).
            // So we must fetch DLCs again to ensure we remove them.
            let mut ids_to_remove = vec![appid.clone()];
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

            // Generic Fetch Logic (Duplicated slightly but safe)
            let dlcs_result = if let Some(client) = client_opt {
                 rt.block_on(client.get_dlc_list(&appid))
            } else {
                rt.block_on(async {
                    let client = reqwest::Client::new();
                    let url = format!("https://store.steampowered.com/api/appdetails?appids={}&filters=dlc", appid);
                     if let Ok(resp) = client.get(&url).send().await {
                           if let Ok(root) = resp.json::<serde_json::Value>().await {
                               let mut dlc_ids = Vec::new();
                               if let Some(app_data) = root.get(&appid) {
                                   if let Some(data) = app_data.get("data") {
                                       if let Some(dlc_array) = data.get("dlc").and_then(|v| v.as_array()) {
                                           for item in dlc_array {
                                               if let Some(id) = item.as_u64() { dlc_ids.push(id.to_string()); }
                                           }
                                       }
                                   }
                               }
                               return Ok(dlc_ids);
                           }
                       }
                       Ok(vec![])
                })
            };

            if let Ok(dlcs) = dlcs_result {
                ids_to_remove.extend(dlcs);
            }

            // Call Removal
            match crate::app_list::remove_games_from_list(&gl_path, ids_to_remove) {
                Ok(_) => {
                    log("🚫 Family Godmode Disabled.".to_string());
                    if let Ok(mut q) = status_queue.lock() {
                       *q = Some("REFRESH_LIB".to_string());
                   }
                },
                Err(e) => log(format!("❌ Error stripping AppList: {}", e)),
            }
        });
    }



    // Legacy: Manual DRM INTEL tab (functionality migrated to Library per-game)
    #[allow(dead_code)]
    fn ui_drm(&mut self, ui: &mut egui::Ui) {
        ui.heading("STEAMLESS AUTOMATION");
        ui.add_space(10.0);

        ui.label("Target Executable:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.target_exe);
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("exe", &["exe"])
                    .pick_file()
                {
                    self.target_exe = path.to_string_lossy().to_string();
                }
            }
        });
        
        ui.add_space(15.0);

        if ui.button(egui::RichText::new("UNPACK & PATCH").strong().size(16.0)).clicked() {
            if self.target_exe.is_empty() {
                self.log("Error: No executable selected.".to_string());
                return;
            }

            match steamless::run_steamless(&self.target_exe, &self.config.steamless_path) {
                Ok(msg) => {
                    self.log(msg);
                },
                Err(e) => self.log(format!("Steamless Error: {}", e)),
            }
        }
    }

    fn ui_library(&mut self, ui: &mut egui::Ui) {
        // PROFILE MANAGER HEADER
        ui.vertical(|ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                 ui.label(egui::RichText::new("PROFILE MANAGER & LIBRARY").size(16.0).strong().color(egui::Color32::from_rgb(0, 200, 255)));
                 ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                      if ui.button(egui::RichText::new("➕ CREATE NEW PROFILE").strong().color(egui::Color32::GREEN)).clicked() {
                          self.profile_name_input.clear(); // Reset input
                          self.create_profile_modal_open = true;
                      }
                      
                      // CHECK UPDATES BUTTON
                      let pending_count = self.watcher_pending_updates.lock()
                          .map(|p| p.len())
                          .unwrap_or(0);
                      
                      let btn_text = if pending_count > 0 {
                          format!("🔄 CHECK UPDATES ({})", pending_count)
                      } else {
                          "🔄 CHECK UPDATES".to_string()
                      };
                      
                      let btn_color = if pending_count > 0 {
                          egui::Color32::from_rgb(255, 165, 0) // Orange
                      } else {
                          egui::Color32::from_rgb(100, 200, 255) // Cyan
                      };
                      
                      if ui.button(egui::RichText::new(btn_text).strong().color(btn_color).size(11.0))
                          .on_hover_text("Manually check for game updates")
                          .clicked()
                      {
                          self.start_watcher_check();
                      }
                 });
            });
            
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_black_alpha(100))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(40)))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                         // PROFILE SELECTOR
                         ui.label("Profile:");
                         let profiles = self.profile_manager.list_profiles();
                         let current_sel = self.active_profile_name.clone();
                         
                         // 1. WIDER COMBO & AUTO-LOAD
                         egui::ComboBox::from_id_salt("profile_combo")
                             .selected_text(if current_sel.is_empty() { "Select Profile..." } else { &current_sel })
                             .width(250.0) // Aesthetic Width
                             .show_ui(ui, |ui| {
                                 for name in &profiles {
                                     // AUTO-LOAD LOGIC
                                     if ui.selectable_value(&mut self.active_profile_name, name.clone(), name).clicked() {
                                         // User clicked a new profile -> Auto Load
                                         match self.profile_manager.load_profile(name) {
                                             Ok(p) => {
                                                 if p.app_ids.len() > 133 {
                                                     self.status_msg = format!("⚠ LIMIT EXCEEDED ({} > 133). Steam may crash.", p.app_ids.len());
                                                 }
                                                 use crate::app_list::overwrite_app_list;
                                                 if let Err(e) = overwrite_app_list(&self.config.gl_path, p.app_ids) {
                                                     self.log(format!("Error applying profile: {}", e));
                                                 } else {
                                                     self.config.last_active_profile = p.name.clone();
                                                     if let Err(e) = save_config(&self.config) {
                                                         self.log(format!("Config Save Error: {}", e));
                                                     }
                                                     self.refresh_library(); // Auto Refresh
                                                     self.log(format!("Profile '{}' loaded automatically.", p.name));
                                                 }
                                             },
                                             Err(e) => self.log(format!("Load Error: {}", e)),
                                         }
                                     }
                                 }
                             });

                         ui.add_space(10.0);
                         
                         // SAVE (UPDATE) BUTTON
                         if ui.button(egui::RichText::new("💾 SAVE").strong().color(egui::Color32::GREEN)).on_hover_text("Save current library to SELECTED profile").clicked() {
                             if !self.active_profile_name.is_empty() {
                                 let games = self.active_games.lock().unwrap();
                                 let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
                                 drop(games);
                                 
                                 // 133 CHECK
                                 if ids.len() > 133 {
                                     self.log(format!("⚠ Warning: Saving {} apps (Limit 133).", ids.len()));
                                 }
                                 
                                 let p = Profile { name: self.active_profile_name.clone(), app_ids: ids };
                                 if let Err(e) = self.profile_manager.save_profile(&p) {
                                     self.log(format!("Save Error: {}", e));
                                 } else {
                                     self.log(format!("Profile '{}' updated!", p.name));
                                 }
                             } else {
                                 self.log("Please select a profile to save to first.".to_string());
                             }
                         }

                         // DELETE BUTTON (Protected)
                         let is_default = self.active_profile_name == "Default";
                         let btn = egui::Button::new(egui::RichText::new("🗑").color(if is_default { egui::Color32::GRAY } else { egui::Color32::RED }));
                         
                         if ui.add_enabled(!is_default, btn)
                             .on_hover_text(if is_default { "Cannot delete Default profile" } else { "Delete selected profile" })
                             .clicked()
                             && !self.active_profile_name.is_empty() {
                                 self.delete_profile_modal_open = true;
                             }
                    });
                });
        });
        
        // NEW PROFILE MODAL
        // NEW PROFILE MODAL (ANIMATED)
        // 1. Calculate Ease-Out-Back (Bounce)
        let ctx = ui.ctx().clone();
        let anim_t = ctx.animate_bool(egui::Id::new("create_profile_anim"), self.create_profile_modal_open);
        
        if anim_t > 0.0 {
            // cubic-bezier approximation for backOut(1.7)
            // t = anim_t
            // c1 = 1.70158
            // c3 = c1 + 1
            // 1 + c3 * (t-1)^3 + c1 * (t-1)^2
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            let t = anim_t - 1.0;
            let ease_out_back = 1.0 + c3 * t.powi(3) + c1 * t.powi(2);
            
            // Drop In: Start -300px (Top), End 0px (Center)
            let y_offset = (1.0 - ease_out_back) * -300.0;
            
             egui::Window::new(egui::RichText::new("➕ CREATE NEW PROFILE").strong().color(egui::Color32::GREEN))
                 .collapsible(false)
                 .resizable(false)
                 .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
                 .show(&ctx, |ui| {
                      ui.label("Enter name for new profile:");
                      ui.text_edit_singleline(&mut self.profile_name_input).request_focus();
                      
                      ui.add_space(10.0);
                      ui.label(egui::RichText::new("⚠ This will WIPE the current AppList.").color(egui::Color32::YELLOW));
                      
                      // SAFETY CHECKBOX
                      if !self.active_profile_name.is_empty() {
                          ui.add_space(5.0);
                          ui.checkbox(&mut self.create_profile_save_default, 
                              format!("Save changes to '{}' before wiping?", self.active_profile_name)
                          );
                      }
                      
                      ui.add_space(15.0);

                      ui.horizontal(|ui| {
                          if ui.button("CANCEL").clicked() {
                              self.create_profile_modal_open = false;
                          }
                          
                          if ui.button(egui::RichText::new("✅ CREATE & WIPE").strong().color(egui::Color32::RED)).clicked()
                              && !self.profile_name_input.is_empty() {
                                  // 1. AUTO-SAVE CURRENT (Safety) - CONDITIONAL
                                  if !self.active_profile_name.is_empty() && self.create_profile_save_default {
                                      let games = self.active_games.lock().unwrap();
                                      let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
                                      let p = Profile { name: self.active_profile_name.clone(), app_ids: ids };
                                      let _ = self.profile_manager.save_profile(&p); 
                                      self.log(format!("Safety Save: Updated '{}'.", p.name));
                                  } else {
                                      self.log("Safety Save skipped by user.".to_string());
                                  }
                                  
                                  // 2. CREATE NEW EMPTY PROFILE
                                  let new_p = Profile { name: self.profile_name_input.clone(), app_ids: Vec::new() };
                                  if let Err(e) = self.profile_manager.save_profile(&new_p) {
                                      self.log(format!("Error creating profile: {}", e));
                                  } else {
                                      // 3. WIPE APPLIST
                                      let res = {
                                           use crate::app_list::overwrite_app_list;
                                           overwrite_app_list(&self.config.gl_path, Vec::new())
                                      };
                                      
                                      if let Err(e) = res {
                                          self.log(format!("Error wiping AppList: {}", e));
                                      } else {
                                          // 4. SWITCH & REFRESH
                                          self.active_profile_name = self.profile_name_input.clone();
                                          
                                          // PERSIST CONFIG
                                          self.config.last_active_profile = self.active_profile_name.clone();
                                          if let Err(e) = save_config(&self.config) {
                                              self.log(format!("Config Save Error: {}", e));
                                          }

                                          self.refresh_library();
                                          self.log(format!("Switched to new profile '{}'. List cleared.", self.active_profile_name));
                                          self.create_profile_modal_open = false;
                                      }
                                  }
                              }
                      });
                 });
        }
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // DELETE CONFIRMATION MODAL
        if self.delete_profile_modal_open {
             // Animate or simple overlay
             egui::Window::new(egui::RichText::new("🗑 DELETE PROFILE?").strong().color(egui::Color32::RED))
                 .collapsible(false)
                 .resizable(false)
                 .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                 .show(&ctx, |ui| {
                      ui.label(egui::RichText::new(format!("Are you sure you want to delete '{}'?", self.active_profile_name)).size(16.0));
                      ui.add_space(5.0);
                      ui.label(egui::RichText::new("⚠ This action cannot be undone.").color(egui::Color32::YELLOW));
                      
                      ui.add_space(15.0);
                      ui.horizontal(|ui| {
                          if ui.button("CANCEL").clicked() {
                              self.delete_profile_modal_open = false;
                          }
                          
                          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                              if ui.button(egui::RichText::new("💀 DELETE FOREVER").strong().color(egui::Color32::RED)).clicked() {
                                   if !self.active_profile_name.is_empty() {
                                       if let Err(e) = self.profile_manager.delete_profile(&self.active_profile_name) {
                                           self.log(format!("Delete Error: {}", e));
                                       } else {
                                           self.log(format!("Profile '{}' deleted.", self.active_profile_name));
                                           self.active_profile_name.clear();
                                       }
                                   }
                                   self.delete_profile_modal_open = false;
                              }
                          });
                      });
                 });
        }

        // Standard Library Controls (Refresh, Nuke, Resolve)
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new("🔄 Refresh").strong())
                .clicked()
            {
                self.refresh_library();
            }
            if ui
                .button(
                    egui::RichText::new("🔃 Reorder List")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                )
                .on_hover_text("Sorts the AppList alphabetically without deleting unknown items.")
                .clicked()
            {
                let result = {
                    let guard = self.game_cache.lock().ok();
                    nuke_reorder(&self.config.gl_path, &self.config.steam_path, None, guard.as_deref())
                };

                if let Err(e) = result {
                    self.log(format!("Error: {}", e));
                } else {
                    self.log("Library Reordered (Alphabetical).".to_string());
                    self.refresh_library();
                }
            }

            // REMOVED: "NUKE UNKNOWNS" and "Resolve Unknown" buttons
            // These are no longer needed because:
            // 1. Linked depot/DLC IDs are now hidden from the main list
            // 2. The relationship system properly tracks child→parent links
        });
        ui.add_space(5.0);

        // Headers
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("AppID")
                    .strong()
                    .color(egui::Color32::GRAY)
                    .size(14.0),
            );
            ui.add_space(30.0);
            ui.label(
                egui::RichText::new("Game Name")
                    .strong()
                    .color(egui::Color32::GRAY)
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Actions")
                        .strong()
                        .color(egui::Color32::GRAY)
                        .size(14.0),
                );
            });
        });
        ui.separator();

        let active_games = self.active_games.clone();
        let games = active_games.lock().unwrap();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Collect delete request to avoid borrow issues
            let mut delete_req = None;

            for (idx, game) in games.iter().enumerate() {
                // IDs are shown so user knows exact AppList usage count
                let bg_color = if idx % 2 == 0 {
                    egui::Color32::from_rgb(25, 25, 30)
                } else {
                    egui::Color32::from_rgb(32, 32, 38)
                };

                egui::Frame::none()
                    .fill(bg_color)
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&game.app_id)
                                    .monospace()
                                    .color(egui::Color32::from_rgb(0, 255, 200)),
                            );
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new(&game.name).color(egui::Color32::WHITE));
                            
                            // Update Indicator - Check if update available from watcher
                            let has_pending_update = self.watcher_pending_updates.lock()
                                .map(|pu| pu.contains_key(&game.app_id))
                                .unwrap_or(false);
                            
                            let is_updating = self.watcher_updating.lock()
                                .map(|dl| dl.contains(&game.app_id))
                                .unwrap_or(false);
                            
                            if is_updating {
                                ui.label(
                                    egui::RichText::new("⏳")
                                        .color(egui::Color32::YELLOW)
                                ).on_hover_text("Downloading new manifests...");
                            } else if has_pending_update {
                                ui.label(
                                    egui::RichText::new("🔔")
                                        .color(egui::Color32::from_rgb(255, 165, 0)) // Orange
                                ).on_hover_text("Update available! Click AGGIORNA to download new manifest.");
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(egui::RichText::new("🗑").color(egui::Color32::RED))
                                        .on_hover_text("Delete File")
                                        .clicked()
                                    {
                                        delete_req = Some((game.app_id.clone(), game.name.clone()));
                                    }
                                    
                                    // AGGIORNA MANIFEST BUTTON (only if update pending and not already updating)
                                    if has_pending_update && !is_updating {
                                        let update_btn = ui.button(
                                            egui::RichText::new("⬇ AGGIORNA")
                                                .color(egui::Color32::from_rgb(0, 255, 150))
                                                .size(11.0)
                                        ).on_hover_text("Download new manifest files for this game.\nWill use configured target language.");
                                        
                                        if update_btn.clicked() {
                                            // Start update
                                            let app_id = game.app_id.clone();
                                            let api_key = self.config.api_key.clone();
                                            let steam_path = self.config.steam_path.clone();
                                            let target_language = self.config.target_language.clone();
                                            let updating_arc = self.watcher_updating.clone();
                                            let pending_arc = self.watcher_pending_updates.clone();
                                            let log_arc = self.system_log.clone();
                                            
                                            // Mark as updating
                                            if let Ok(mut u) = updating_arc.lock() {
                                                u.insert(app_id.clone());
                                            }
                                            
                                            // Spawn update thread
                                            std::thread::spawn(move || {
                                                let rt = match tokio::runtime::Runtime::new() {
                                                    Ok(rt) => rt,
                                                    Err(_) => {
                                                        if let Ok(mut u) = updating_arc.lock() { u.remove(&app_id); }
                                                        return;
                                                    }
                                                };
                                                
                                                if let Ok(mut logs) = log_arc.lock() {
                                                    logs.push(format!("[Watcher] Starting manifest update for {}...", app_id));
                                                }
                                                
                                                // Create downloader and trigger update
                                                let downloader = crate::manifest_downloader::ManifestDownloader::new();
                                                let steam_path = std::path::Path::new(&steam_path);
                                                
                                                let result = rt.block_on(async {
                                                    crate::watcher::trigger_update_download(
                                                        &api_key,
                                                        &downloader,
                                                        &app_id,
                                                        steam_path,
                                                        &target_language,
                                                    ).await
                                                });
                                                
                                                match result {
                                                    Ok(count) => {
                                                        if let Ok(mut logs) = log_arc.lock() {
                                                            logs.push(format!("[Watcher] ✅ Updated {} depot manifests for {}", count, app_id));
                                                        }
                                                        // Remove from pending
                                                        if let Ok(mut p) = pending_arc.lock() {
                                                            p.remove(&app_id);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        if let Ok(mut logs) = log_arc.lock() {
                                                            logs.push(format!("[Watcher] ❌ Update failed for {}: {}", app_id, e));
                                                        }
                                                    }
                                                }
                                                
                                                // Remove from updating
                                                if let Ok(mut u) = updating_arc.lock() {
                                                    u.remove(&app_id);
                                                }
                                            });
                                        }
                                    }

                                    // STEAMLESS AUTOMATION BUTTON
                                    let steam_path = self.config.steam_path.clone();
                                    
                                    // SKIP Steamless for Family Shared games
                                    let is_family_shared = self.config.family_godmode_ids.contains(&game.app_id);
                                    
                                    // Show STEAMLESS button only if game path exists and not family shared
                                    if !is_family_shared && crate::game_path::GamePathFinder::find_game_path(&steam_path, &game.app_id).is_some() {
                                        let steamless_btn = ui.button(
                                            egui::RichText::new("⚡ STEAMLESS")
                                                .color(egui::Color32::from_rgb(255, 150, 0))
                                                .size(10.0)
                                        ).on_hover_text("Auto-patch all DRM-protected EXEs in game folder.\nGenerates steam_appid.txt.");
                                        
                                        if steamless_btn.clicked() {
                                            if let Some(game_path) = crate::game_path::GamePathFinder::find_game_path(&steam_path, &game.app_id) {
                                                let steamless_cli = self.config.steamless_path.clone();
                                                let app_id = game.app_id.clone();
                                                let log_arc = self.system_log.clone();
                                                
                                                if steamless_cli.is_empty() || !std::path::Path::new(&steamless_cli).exists() {
                                                    self.log("❌ Steamless CLI not configured. Go to Settings.".to_string());
                                                } else {
                                                    // Log start
                                                    self.log(format!("⚡ Starting Steamless on: {:?}", game_path));
                                                    
                                                    // Find all EXEs first (for logging)
                                                    let exes = crate::steamless::find_game_executables(&game_path);
                                                    self.log(format!("   Found {} potential game executables", exes.len()));
                                                    
                                                    // Run in thread to not block UI
                                                    let path_clone = game_path.clone();
                                                    std::thread::spawn(move || {
                                                        let log = move |msg: String| {
                                                            if let Ok(mut logs) = log_arc.lock() {
                                                                push_log(&mut logs, msg);
                                                            }
                                                        };
                                                        
                                                        let (success, total, results) = crate::steamless::run_steamless_folder(
                                                            &path_clone,
                                                            &steamless_cli,
                                                            &app_id,
                                                        );
                                                        
                                                        // Log results
                                                        for r in results {
                                                            if r.success {
                                                                log(format!("   ✅ {}: {}", r.exe_path, r.message));
                                                            } else {
                                                                log(format!("   ⚠️ {}: {}", r.exe_path, r.message));
                                                            }
                                                        }
                                                        
                                                        log(format!("⚡ Steamless Complete: {}/{} EXEs patched", success, total));
                                                    });
                                                }
                                            } else {
                                                self.log("❌ Game folder not found. Is it installed?".to_string());
                                            }
                                        }
                                        
                                        // GOLDBERG BUTTON
                                        if ui.button(egui::RichText::new("\u{1F6E1} GOLDBERG").color(egui::Color32::YELLOW).size(10.0))
                                            .on_hover_text("Deploy Offline Fix (Goldberg Emulator).\nEnsures Saves and Achievements work offline.")
                                            .clicked() 
                                        {
                                            self.goldberg_candidate_id = Some(game.app_id.clone());
                                            self.goldberg_modal_open = true;
                                        }
                                    } else {
                                         // Not installed or check if DLC
                                         if game.parent_id.is_some() || self.is_probable_dlc(&game.name) {
                                            let label = if let Some(pid) = &game.parent_id {
                                                format!("📦 DLC / CONTENT (Linked to {})", pid)
                                            } else {
                                                "📦 DLC / CONTENT".to_string()
                                            };

                                            ui.label(
                                                egui::RichText::new(&label)
                                                    .color(egui::Color32::from_rgb(150, 150, 255))
                                                    .size(10.0)
                                            ).on_hover_text("Detected as Downloadable Content (Linked to Parent).");
                                         } else if is_family_shared {
                                             // Family Shared game - show special label
                                             ui.label(
                                                 egui::RichText::new("👨‍👩‍👧 FAMILY GODMODE")
                                                     .color(egui::Color32::from_rgb(100, 255, 255))
                                                     .size(10.0)
                                             ).on_hover_text("Game activated via Steam Family Sharing.\nNo patching needed - works natively!");
                                         } else {
                                             ui.label(
                                                 egui::RichText::new("NOT INSTALLED")
                                                     .color(egui::Color32::DARK_GRAY)
                                                     .size(10.0)
                                             );
                                         }
                                    }

                                    ui.label(
                                        egui::RichText::new(&game.filename)
                                            .size(10.0)
                                            .color(egui::Color32::GRAY),
                                    );
                                },
                            );
                        });
                    });
            }

            if let Some((aid, name)) = delete_req {
                drop(games); // Drop lock before mutating self
                self.initiate_delete(aid, name);
            }
        }); // End ScrollArea

            // GOLDBERG MODAL
            let cand_id = self.goldberg_candidate_id.clone();
            if self.goldberg_modal_open {
                if let Some(appid) = cand_id {
                    let ctx = ui.ctx().clone();
                    egui::Window::new(egui::RichText::new("\u{1F6E1} GOLDBERG EMULATOR SETUP").strong().color(egui::Color32::YELLOW))
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                        .show(&ctx, |ui| {
                            ui.label("Configure Offline Wrapper Settings:");
                            ui.add_space(10.0);
                            
                            ui.label("Username (Visible inside game):");
                            ui.text_edit_singleline(&mut self.goldberg_user_input);
                            
                            ui.label("SteamID (64-bit ID):");
                            ui.text_edit_singleline(&mut self.goldberg_steamid_input);
                            ui.small("Default is recommended for compatibility.");

                            ui.add_space(5.0);
                            ui.checkbox(&mut self.goldberg_use_64bit, "Deploy 64-bit DLL (Standard)");
                            
                            ui.add_space(15.0);
                            ui.horizontal(|ui| {
                                if ui.button("CANCEL").clicked() {
                                    self.goldberg_modal_open = false;
                                }
                                
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(egui::RichText::new("🚀 DEPLOY FIX").strong().color(egui::Color32::GREEN)).clicked() {
                                         // DEPLOYMENT LOGIC
                                         let steam_path = self.config.steam_path.clone();
                                         if let Some(game_path) = crate::game_path::GamePathFinder::find_game_path(&steam_path, &appid) {
                                             let mut success = true;
                                             
                                             // 1. Core Files
                                             let aid_u32 = appid.parse::<u32>().unwrap_or(0);
                                             if let Err(e) = self.goldberg.deploy(&game_path, aid_u32, self.goldberg_use_64bit) { 
                                                 self.log(format!("Goldberg Deploy Error: {}", e));
                                                 success = false;
                                             }
                                             
                                             // 2. Ticket Gen
                                             if success {
                                                 if let Err(e) = self.goldberg.generate_ticket(aid_u32, &game_path) {
                                                     self.log(format!("Ticket Gen Error: {}", e));
                                                     // Non-fatal, but warn
                                                 } else {
                                                     self.log("✅ Encrypted AppTicket generated successfully.".to_string());
                                                 }
                                             }
                                             
                                             // 3. User Config (Username/ID)
                                             if success {
                                                 let settings_dir = game_path.join("steam_settings");
                                                 let _ = std::fs::create_dir_all(&settings_dir);
                                                 
                                                 // force_account_name.txt
                                                 if !self.goldberg_user_input.is_empty() {
                                                     let _ = std::fs::write(settings_dir.join("force_account_name.txt"), &self.goldberg_user_input);
                                                 }
                                                 
                                                 // force_steamid.txt (optional, usually user_steam_id.txt)
                                                 // Goldberg uses user_steam_id.txt usually containing just the ID
                                                  if !self.goldberg_steamid_input.is_empty() && self.goldberg_steamid_input.chars().all(char::is_numeric) {
                                                     let _ = std::fs::write(settings_dir.join("user_steam_id.txt"), &self.goldberg_steamid_input);
                                                 }
                                                 
                                                 
                                                 // 4. Achievement Downloader (Async Background)
                                                 let client_opt = self.api_client.clone();
                                                 let g_gen = self.goldberg.clone();
                                                 let appid_c = appid.clone();
                                                 let gp_c = game_path.clone();
                                                 let log_arc = self.system_log.clone();

                                                 std::thread::spawn(move || {
                                                     if let Some(client) = client_opt {
                                                         if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                                                             .enable_all()
                                                             .build() 
                                                         {
                                                             // Log start
                                                             if let Ok(mut logs) = log_arc.lock() {
                                                                  push_log(&mut logs, format!("🏆 Fetching Achievements for {}...", appid_c));
                                                             }

                                                             match rt.block_on(g_gen.download_achievements(&appid_c, &client, &gp_c)) {
                                                                 Ok(msg) => {
                                                                      if let Ok(mut logs) = log_arc.lock() {
                                                                          push_log(&mut logs, format!("✅ Achievements: {}", msg));
                                                                      }
                                                                 },
                                                                 Err(e) => {
                                                                      if let Ok(mut logs) = log_arc.lock() {
                                                                          push_log(&mut logs, format!("⚠️ Achievement Download Error: {}", e));
                                                                      }
                                                                 }
                                                             }

                                                             // 5. DLC Unlocker (Async) - DISABLED per user request (redundant with GreenLuma/Picker)
                                                             /*
                                                             match rt.block_on(g_gen.generate_dlc_config(&appid_c, &client, &gp_c)) {
                                                                Ok(msg) => {
                                                                      if let Ok(mut logs) = log_arc.lock() {
                                                                          push_log(&mut logs, format!("✅ DLC Config: {}", msg));
                                                                      }
                                                                 },
                                                                 Err(e) => {
                                                                     // Not critical
                                                                     if let Ok(mut logs) = log_arc.lock() {
                                                                          push_log(&mut logs, format!("ℹ️ DLC Config: {}", e));
                                                                     }
                                                                 }
                                                             }
                                                             */
                                                         }
                                                     }
                                                 });

                                                 self.log(format!("🛡️ Goldberg Emulator applied to AppID {}. Achievements & DLCs processing in background.", appid));
                                             }
                                         } else {
                                             self.log("Error: Game path not found.".to_string());
                                         }
                                         
                                         self.goldberg_modal_open = false;
                                    }
                                });
                            });
                        });
                }
            }

    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("SYSTEM CONFIGURATION")
                .color(egui::Color32::from_rgb(0, 200, 255))
                .strong(),
        );
        ui.add_space(10.0);

        let path_row =
            |ui: &mut egui::Ui,
             label: &str,
             valid: bool,
             txt: &mut String,
             is_dir: bool,
             hint: Option<&str>| {
                ui.label(label);
                ui.horizontal(|ui| {
                    let _tint = if valid {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };
                    // Auto-clean UNC prefix if present
                    if txt.starts_with(r"\\?\") {
                        *txt = txt.replace(r"\\?\", "");
                    }

                    ui.add(
                        egui::TextEdit::singleline(txt)
                            .desired_width(400.0)
                            .text_color(egui::Color32::WHITE),
                    );
                    if ui.button("📂").clicked() {
                        if is_dir {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                let p_str = path.to_string_lossy().to_string();
                                *txt = p_str.replace(r"\\?\", "");
                            }
                        } else if let Some(path) = rfd::FileDialog::new()
                            .add_filter("exe", &["exe"])
                            .pick_file()
                        {
                            let p_str = path.to_string_lossy().to_string();
                            *txt = p_str.replace(r"\\?\", "");
                        }
                    }
                    if let Some(h) = hint {
                        ui.label("❓").on_hover_text(h);
                    }
                });
                ui.add_space(5.0);
            };

        path_row(
            ui,
            "Steam Path:",
            Path::new(&self.config.steam_path).exists(),
            &mut self.config.steam_path,
            true,
            None,
        );
        path_row(
            ui,
            "GreenLuma Path:",
            Path::new(&self.config.gl_path).exists(),
            &mut self.config.gl_path,
            true,
            Some("Folder containing GreenLuma_2025_x64.dll and AppList folder.\nSearch for 'GreenLuma 2025' on specialized forums."),
        );
        path_row(
            ui,
            "Steamless CLI Path:",
            Path::new(&self.config.steamless_path).exists(),
            &mut self.config.steamless_path,
            false,
            Some("Steamless.CLI.exe required for DRM analysis.\nSearch for 'Steamless' on GitHub (atom0s)."),
        );

        ui.add_space(5.0);
        
        // Settings Toggles
        ui.horizontal(|ui| {
             ui.checkbox(&mut self.config.enable_stealth_mode, egui::RichText::new("Enable GreenLuma Stealth Mode").strong());
             ui.label("ℹ").on_hover_text("Enables 'StealthMode.bin' for GreenLuma.\nDisables some file system hooks to reduce ban risk.\nDisable this if you have issues with downloads or installation errors.");
        });

        ui.add_space(5.0);

        // STEALTH MODE WARNING
        if !self.config.steam_path.is_empty() && !self.config.gl_path.is_empty() {
             let sp = Path::new(&self.config.steam_path);
             let gp = Path::new(&self.config.gl_path);
             // Simple contains check logic
             if gp.starts_with(sp) || sp.starts_with(gp) {
                 ui.group(|ui| {
                      ui.horizontal(|ui| {
                          ui.label(egui::RichText::new("⚠ STEALTH RISK:").color(egui::Color32::RED).strong());
                          ui.label("GreenLuma is located INSIDE or CONTAINS the Steam folder.");
                      });
                      ui.label("For maximum safety, please move GreenLuma to a completely separate folder (e.g. C:\\GreenLuma).");
                 });
                 ui.add_space(10.0);
             }
        }

        // LEGACY IMPORT RECOVERY
        if !self.config.steam_path.is_empty() {
             let legacy_alist = Path::new(&self.config.steam_path).join("AppList");
             if legacy_alist.exists() && legacy_alist.is_dir() {
                  // Check if it has txt files (naive check)
                  let has_files = std::fs::read_dir(&legacy_alist).ok().map(|mut d| d.any(|e| e.ok().map(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false)).unwrap_or(false))).unwrap_or(false);
                  
                  if has_files {
                       ui.group(|ui| {
                           ui.horizontal(|ui| {
                               ui.label(egui::RichText::new("📂 LEGACY CONFIG FOUND").color(egui::Color32::YELLOW).strong());
                               if ui.add(egui::Button::new(egui::RichText::new("📥 IMPORT LEGACY APPLIST").strong().color(egui::Color32::BLACK)).fill(egui::Color32::YELLOW)).clicked() {
                                    // IMPORT LOGIC
                                    let mut count = 0;
                                    let mut new_ids = Vec::new();
                                    if let Ok(entries) = std::fs::read_dir(&legacy_alist) {
                                         for entry in entries.flatten() {
                                             let path = entry.path();
                                             if path.extension().map(|s| s == "txt").unwrap_or(false) {
                                                  if let Ok(content) = std::fs::read_to_string(&path) {
                                                      let clean = content.trim().to_string();
                                                      if !clean.is_empty() && clean.chars().all(char::is_numeric) {
                                                           new_ids.push(clean);
                                                           count += 1;
                                                      }
                                                  }
                                             }
                                         }
                                    }
                                    
                                    if count > 0 {
                                        // Write to current GL AppList
                                        if let Err(e) = crate::app_list::add_games_to_list(&self.config.gl_path, new_ids) {
                                            self.log(format!("Import Error: {}", e));
                                        } else {
                                            self.refresh_library();
                                            self.log(format!("Imported {} legacy games. Please SAVE PROFILE to keep them.", count));
                                        }
                                    } else {
                                        self.log("No valid AppIDs found in legacy folder.".to_string());
                                    }
                               }
                           });
                           ui.label("Old GreenLuma AppList detected inside Steam. Migrate now?");
                       });
                       ui.add_space(10.0);
                  }
             }
        }

        ui.separator();
        
        // Glitch Logic for API Key
        // Force repaint if we have a key (to drive animation loop)
        if !self.config.api_key.is_empty() {
             ui.ctx().request_repaint();
        }

        // Update Glitch String (High Speed)
        let now = Instant::now();
        if !self.config.api_key.is_empty() && (
             now.duration_since(self.api_key_glitch_update).as_millis() > 50 || 
             self.api_key_glitch_cache.len() != self.config.api_key.len()
        ) {
             self.api_key_glitch_update = now;
             
             // High-Tech Glyph Set (Very Distinct)
             let glyphs = "ABCDEF0123456789!@#$%^&*()_+-=[]{}|;:,.<>?§";
             let time = ui.input(|i| i.time);
             let seed = (time * 10000.0) as usize;
             
             self.api_key_glitch_cache = self.config.api_key.chars().enumerate().map(|(i, _)| {
                 let idx = (seed.wrapping_add(i * 13).wrapping_add(now.elapsed().as_nanos() as usize)) % glyphs.len();
                 glyphs.chars().nth(idx).unwrap_or('?')
             }).collect();
        }

        ui.label(egui::RichText::new("API Key (Secure Sandbox):").color(egui::Color32::from_rgb(0, 255, 100)));
        
        let frame = egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 50)))
            .fill(egui::Color32::from_rgb(5, 15, 5))
            .inner_margin(6.0)
            .rounding(4.0);

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                 ui.label("🔒");
                 
                 let glitch_text = self.api_key_glitch_cache.clone();
                 
                 let response = ui.add(
                      egui::TextEdit::singleline(&mut self.config.api_key)
                          .font(egui::FontId::monospace(14.0))
                          .desired_width(320.0)
                          .layouter(&mut |ui, string, _| {
                               let display_text = if string.is_empty() { 
                                   "" 
                               } else if string.len() == glitch_text.len() {
                                   &glitch_text
                               } else {
                                   string // Fallback
                               };

                               let mut job = egui::text::LayoutJob::default();
                               job.append(
                                   display_text,
                                   0.0,
                                   egui::TextFormat {
                                       font_id: egui::FontId::monospace(14.0),
                                       color: egui::Color32::from_rgb(50, 255, 50),
                                       background: egui::Color32::from_black_alpha(150),
                                       ..Default::default()
                                   }
                               );
                               ui.fonts(|f| f.layout_job(job))
                          })
                 );
                 
                 if response.changed() {
                      self.api_key_glitch_update = Instant::now() - Duration::from_millis(100);
                      // AUTO-REFRESH TIMER
                      // Provide 1.5s debounce for typing entire key
                      self.api_refresh_timer = Some(Instant::now() + Duration::from_millis(1500));
                 }
                 
                 ui.label(egui::RichText::new("❓").size(12.0))
                   .on_hover_text("Optional API Key for Manifest Downloads.\nSearch for 'Morrenus API' on Google/Discord.");
            });
        });

        ui.add_space(10.0);

        // API STATS DASHBOARD & AUTOMATION CHECK
        // Check Timer
        if let Some(timer) = self.api_refresh_timer {
            if Instant::now() > timer {
                self.api_refresh_timer = None; // Reset
                if !self.config.api_key.is_empty() {
                     // TRIGGER SEARCH
                     let stats_arc = self.user_stats.clone();
                     let status_queue = self.status_update_queue.clone();
                     let error_arc = self.api_last_error.clone();
                     let validating_arc = self.is_validating_api.clone(); // Capture
                     let cfg_key = self.config.api_key.clone(); 
                     
                     // Set VALIDATING flag immediately
                     if let Ok(mut v) = self.is_validating_api.lock() { *v = true; }

                     std::thread::spawn(move || {
                         let client = ApiClient::new(cfg_key.clone()); 
                         
                         let rt = tokio::runtime::Runtime::new().unwrap();
                         let result = rt.block_on(client.get_user_stats());
                         
                         // Clear Validating Flag
                         if let Ok(mut v) = validating_arc.lock() { *v = false; }
                         
                         match result {
                             Ok(stats) => {
                                 *error_arc.lock().unwrap() = None; // Clear error
                                 *stats_arc.lock().unwrap() = Some(stats);
                                 if let Ok(mut q) = status_queue.lock() {
                                     *q = Some("API Connection Established.".to_string());
                                 }
                             },
                             Err(e) => {
                                 // Parse Error
                                 let err_str = e.to_string();
                                 *error_arc.lock().unwrap() = Some(err_str.clone());
                                 
                                 if let Ok(mut q) = status_queue.lock() {
                                     if err_str.contains("401") || err_str.contains("403") {
                                         *q = Some("⛔ API KEY INVALID OR EXPIRED.".to_string());
                                     } else {
                                         *q = Some(format!("API Error: {}", err_str));
                                     }
                                 }
                             }
                         }
                     });
                     self.log("Auto-Refreshing API Stats...".to_string());
                }
            } else {
                 ui.ctx().request_repaint(); // Keep animating for timer
            }
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("📊 API USAGE:").strong().color(egui::Color32::from_rgb(0, 255, 255)));
            
            // Check Validation Flag
            let mut is_validating = false;
            if let Ok(v) = self.is_validating_api.lock() { is_validating = *v; }
            
            if is_validating || self.api_refresh_timer.is_some() {
                ui.spinner();
                ui.label(egui::RichText::new("Verifying Key...").italics().color(egui::Color32::YELLOW));
            }
        });


        // NEON STATS FRAME
        // NEON STATS / ERROR FRAME
        let mut api_error_msg = None;
        if let Ok(guard) = self.api_last_error.lock() {
            api_error_msg = guard.clone();
        }

        if let Some(err_msg) = api_error_msg {
             // RENDER ERROR FRAME
             let theme_color = egui::Color32::from_rgb(255, 30, 30);
             egui::Frame::none()
                 .fill(egui::Color32::from_black_alpha(200))
                 .stroke(egui::Stroke::new(1.5, theme_color))
                 .rounding(6.0)
                 .inner_margin(12.0)
                 .show(ui, |ui| {
                      ui.set_min_width(320.0);
                      ui.horizontal(|ui| {
                          ui.label("⛔");
                          ui.label(egui::RichText::new("API STATUS CRITICAL").strong().color(theme_color));
                      });
                      ui.separator();
                      ui.add_space(5.0);
                      ui.label(egui::RichText::new(err_msg)
                          .font(egui::FontId::monospace(12.0))
                          .color(egui::Color32::WHITE)
                          .strong());
                 });
        }
        else if let Ok(guard) = self.user_stats.lock() {
            if let Some(stats) = guard.as_ref() {
                let limit_ratio = if stats.daily_limit > 0 {
                    stats.daily_usage as f32 / stats.daily_limit as f32
                } else {
                    0.0
                };
                
                let is_critical = limit_ratio >= 1.0;
                let theme_color = if is_critical { egui::Color32::from_rgb(255, 30, 30) } else { egui::Color32::from_rgb(0, 255, 200) };
                
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_black_alpha(200))
                    .stroke(egui::Stroke::new(1.5, theme_color))
                    .rounding(6.0)
                    .inner_margin(12.0);

                frame.show(ui, |ui| {
                     ui.set_min_width(320.0);
                     
                     // Header
                     ui.horizontal(|ui| {
                         ui.label(egui::RichText::new(if is_critical { "⚠ SYSTEM HALT" } else { "⚡ ONLINE" })
                             .font(egui::FontId::monospace(12.0))
                             .color(theme_color));
                         
                         ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                             ui.label(egui::RichText::new(format!("[{}]", stats.role.clone().unwrap_or("USER".to_string()).to_uppercase()))
                                 .font(egui::FontId::monospace(10.0))
                                 .color(egui::Color32::GRAY));
                         });
                     });
                     
                     ui.add_space(8.0);
                     
                     // Usage Numbers
                     ui.horizontal(|ui| {
                         ui.label(egui::RichText::new(format!("{:02}", stats.daily_usage))
                             .font(egui::FontId { size: 24.0, family: egui::FontFamily::Proportional }) 
                             .color(egui::Color32::WHITE));
                         
                         ui.label(egui::RichText::new("/")
                             .size(18.0)
                             .color(egui::Color32::GRAY));
                             
                         ui.label(egui::RichText::new(format!("{:02}", stats.daily_limit))
                             .font(egui::FontId::monospace(18.0))
                             .color(theme_color));
                             
                         ui.label(egui::RichText::new("REQUESTS")
                             .size(10.0)
                             .color(egui::Color32::GRAY));
                     });
                     
                     ui.add_space(4.0);

                     // Cyberpunk Progress Bar
                     let (rect, _resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
                     ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(20, 20, 30)); // Track
                     
                     if limit_ratio > 0.0 {
                         let fill_width = rect.width() * limit_ratio.clamp(0.0, 1.0);
                         let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, rect.height()));
                         
                         // Glow effect
                         if !is_critical {
                             ui.painter().rect_filled(fill_rect, 3.0, theme_color);
                             ui.painter().rect_stroke(fill_rect.expand(1.0), 3.0, egui::Stroke::new(2.0, theme_color.linear_multiply(0.3)));
                         } else {
                             // Glitch Pattern for Critical
                             ui.painter().rect_filled(fill_rect, 3.0, theme_color); 
                         }
                     }
                     
                     if is_critical {
                         ui.add_space(4.0);
                         ui.label(egui::RichText::new("⛔ UPLINK SEVERED due to protocol limits.")
                             .font(egui::FontId::monospace(10.0))
                             .color(egui::Color32::from_rgb(255, 100, 100)));
                     }
                });
            } else {
                // Empty State with Style
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_black_alpha(150))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50)))
                    .rounding(4.0)
                    .inner_margin(8.0);
                    
                frame.show(ui, |ui| {
                    ui.label(egui::RichText::new("Awaiting Downlink...").font(egui::FontId::monospace(12.0)).italics().color(egui::Color32::GRAY));
                });
            }
        }

        ui.add_space(15.0);
        ui.add_space(20.0);
        
        // CUSTOM ANIMATED SAVE BUTTON
        let now = Instant::now();
        let is_recently_saved = self.config_saved_at.map(|t| now.duration_since(t).as_secs_f32() < 2.0).unwrap_or(false);
        
        if is_recently_saved {
            ui.ctx().request_repaint(); // Animation Loop
        }

        let btn_text = if is_recently_saved { "✅ CONFIGURATION SAVED" } else { "💾 SAVE CONFIGURATION" };
        let btn_size = egui::vec2(280.0, 45.0);
        
        let (rect, response) = ui.allocate_at_least(btn_size, egui::Sense::click());
        
        if response.clicked() {
             if let Err(e) = save_config(&self.config) {
                self.status_msg = format!("Save error: {}", e);
            } else {
                self.config_saved_at = Some(Instant::now());
                self.status_msg = "Config saved.".to_string();
                self.api_client = Some(ApiClient::new(self.config.api_key.clone()));
                self.refresh_library();
                self.resolve_unknown_games();
            }
        }

        // Animation Factors
        let hover_factor = ui.ctx().animate_bool(response.id.with("hover"), response.hovered());
        let save_factor = if let Some(t) = self.config_saved_at {
             let elapsed = now.duration_since(t).as_secs_f32();
             if elapsed < 1.5 {
                 1.0 - (elapsed / 1.5).powf(0.5) // Sqrt fade
             } else { 0.0 }
        } else { 0.0 };

        let painter = ui.painter();
        let center = rect.center();
        
        // Colors
        let cyan = egui::Color32::from_rgb(0, 243, 255);
        let green = egui::Color32::from_rgb(50, 255, 100);
        
        let target_color = if save_factor > 0.0 { green } else { cyan };
        
        // Dynamic Rect
        let visual_rect = rect.shrink(2.0).expand(2.0 * hover_factor);
        let corner_radius = 6.0;

        // Background Fill (Glassy)
        if hover_factor > 0.0 {
            painter.rect_filled(visual_rect, corner_radius, target_color.linear_multiply(0.1));
        }
        
        // Border Stroke
        let stroke_width = 1.0 + (1.0 * hover_factor) + (2.0 * save_factor);
        painter.rect_stroke(visual_rect, corner_radius, egui::Stroke::new(stroke_width, target_color));
        
        // SHOCKWAVE EFFECT (The "Figa" part)
        if save_factor > 0.0 {
            let expansion = (1.0 - save_factor) * 40.0; // Expand outwards
            let alpha = save_factor * 0.6;
            painter.rect_stroke(
                visual_rect.expand(expansion),
                corner_radius + expansion,
                egui::Stroke::new(2.0, green.linear_multiply(alpha))
            );
        }

        // Text
        painter.text(
            center, 
            egui::Align2::CENTER_CENTER, 
            btn_text, 
            egui::FontId::proportional(16.0), 
            target_color
        );
    }

    // ui_profiles Removed - Integrated into ui_library
    
    // Renders the Drive/Library Selection Modal
    fn show_install_modal(&mut self, ctx: &egui::Context) {
        if self.install_modal_open {
             // Clone data upfront to release borrow on self
             let candidate = self.install_candidate.clone();
             let libraries = self.detected_libraries.clone();
             
             if let Some((app_id, name)) = candidate {
                 let mut open = true;
                 egui::Window::new(egui::RichText::new("💾 Select Installation Library").strong())
                     .open(&mut open)
                     .collapsible(false)
                     .resizable(false)
                     .fixed_size(egui::vec2(400.0, 200.0))
                     .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                     .show(ctx, |ui| {
                         ui.vertical_centered(|ui| {
                             ui.add_space(10.0);
                             ui.label(egui::RichText::new(format!("Installing/Repairing: {}", name)).size(14.0));
                             ui.label(egui::RichText::new("Please select the Steam Library where the game files are located:").color(egui::Color32::GRAY));
                             ui.add_space(15.0);
                             
                             if libraries.is_empty() {
                                 ui.label(egui::RichText::new("⚠ No libraries detected!").color(egui::Color32::RED));
                             }
                             
                             egui::ComboBox::from_label("Target Drive")
                                 .selected_text(format!("{:?}", libraries.get(self.selected_library_index).unwrap_or(&std::path::PathBuf::from("None"))))
                                 .show_ui(ui, |ui| {
                                     for (i, lib) in libraries.iter().enumerate() {
                                         ui.selectable_value(&mut self.selected_library_index, i, format!("{:?}", lib));
                                     }
                                 });
                             
                             ui.add_space(20.0);
                             
                             // INSTALL DIR OVERRIDE
                             ui.label(egui::RichText::new("Installation Directory Name (Important!)").strong());
                             ui.label(egui::RichText::new("Use the exact folder name matching your 'common' folder (e.g. 'Expedition 33')").size(10.0).color(egui::Color32::GRAY));
                             ui.horizontal(|ui| {
                                 ui.text_edit_singleline(&mut self.install_dir_input);
                                 
                                 // SCAN BUTTON
                                 if ui.button("🔍 Scan").on_hover_text("Try to find existing folder in common").clicked() {
                                     if let Some(lib) = libraries.get(self.selected_library_index) {
                                          let common = lib.join("steamapps").join("common");
                                          if let Ok(entries) = std::fs::read_dir(common) {
                                              let mut best_match = String::new();
                                              let mut highest_score = 0;
                                              
                                              // Advanced "Brain" Scan Logic
                                              let clean_tokenize = |s: &str| -> Vec<String> {
                                                  s.to_lowercase()
                                                   .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "")
                                                   .split_whitespace()
                                                   .map(|s| s.to_string())
                                                   .collect()
                                              };
                                              
                                              let name_tokens = clean_tokenize(&name);
                                              let name_clean = name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");

                                              for entry in entries.flatten() {
                                                  if let Ok(meta) = entry.metadata() {
                                                      if meta.is_dir() {
                                                          let folder_name = entry.file_name().to_string_lossy().to_string();
                                                          // Skip common utility folders
                                                          if folder_name.eq_ignore_ascii_case("common") || folder_name.eq_ignore_ascii_case("Steamworks Shared") { continue; }

                                                          let folder_tokens = clean_tokenize(&folder_name);
                                                          let folder_clean = folder_name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");

                                                          // 1. Token Overlap
                                                          let matches = folder_tokens.iter().filter(|ft| name_tokens.contains(ft)).count();
                                                          
                                                          // 2. Substring Check (Robust against "The", ":", "-")
                                                          let is_substring = name_clean.contains(&folder_clean) && folder_clean.len() > 3;
                                                          
                                                          // Score Calculation
                                                          let mut score = matches * 10;
                                                          if is_substring { score += 50; }
                                                          if folder_clean == name_clean { score += 100; }
                                                          
                                                          // Update Candidate
                                                          if score > highest_score {
                                                              highest_score = score;
                                                              best_match = folder_name;
                                                          } else if score == highest_score && score > 0 {
                                                              // Tie-breaker: Prefer shorter names (usually the main game vs soundtrack/demo)
                                                              // UNLESS the name is extremely short (<3 chars)
                                                              if folder_name.len() < best_match.len() {
                                                                  best_match = folder_name;
                                                              }
                                                          }
                                                      }
                                                  }
                                              }
                                              
                                              if !best_match.is_empty() {
                                                  self.install_dir_input = best_match;
                                              }
                                          }
                                     }
                                 }
                             });
                             
                             ui.add_space(20.0);
                             
                             ui.horizontal(|ui| {
                                 if ui.button("❌ Cancel").clicked() {
                                     self.install_modal_open = false;
                                     self.install_candidate = None;
                                 }
                                 
                                 if ui.button(egui::RichText::new("✅ CONFIRM & INSTALL").strong().color(egui::Color32::GREEN)).clicked() {
                                     // Proceed with selected library and user-specified install dir
                                     if let Some(target) = libraries.get(self.selected_library_index) {
                                         self.install_game(app_id.clone(), name.clone(), Some(target.clone()), Some(self.install_dir_input.clone()));
                                         self.install_modal_open = false;
                                         self.install_candidate = None;
                                     }
                                 }
                             });
                         });
                     });
                     
                 if !open {
                     self.install_modal_open = false;
                     self.install_candidate = None;
                 }
             }
        }
    }



    /// Renders the DLC Picker Modal for games with many DLCs
    fn show_dlc_picker_modal(&mut self, ctx: &egui::Context) {
        if !self.dlc_picker_open {
            return;
        }
        
        // Ensure modal stays open
        let mut open = true;
        
        let candidate = self.dlc_picker_candidate.clone();
        
        if let Some((app_id, name)) = candidate {
            
            // Count current AppList entries
            let current_count = {
                let games = self.active_games.lock().unwrap();
                games.len()
            };
            let available_slots = APPLIST_LIMIT.saturating_sub(current_count);
            // Rough estimation
            let base_slots = self.dlc_picker_depot_count + 1; 
            let dlc_slots = available_slots.saturating_sub(base_slots);
            
            egui::Window::new(egui::RichText::new("🎮 DLC Picker").strong())
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(600.0, 500.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(5.0);
                        ui.label(egui::RichText::new(format!("Installing: {}", name)).size(16.0).strong());
                        ui.add_space(5.0);
                        
                        // Stats bar
                        ui.horizontal(|ui| {
                            ui.label(format!("📊 AppList: {}/{}", current_count, APPLIST_LIMIT));
                            ui.separator();
                            ui.label(format!("📦 Base Depots: {}", base_slots));
                            ui.separator();
                            let selected = self.dlc_picker_items.iter().filter(|(_, _, s)| *s).count();
                            let color = if selected > dlc_slots {
                                egui::Color32::RED
                            } else {
                                egui::Color32::GREEN
                            };
                            ui.label(egui::RichText::new(format!("✅ Selected: {}/{} DLCs", selected, dlc_slots)).color(color));
                        });
                        
                        ui.add_space(5.0);
                        
                        // Warning if over limit
                        let selected = self.dlc_picker_items.iter().filter(|(_, _, s)| *s).count();
                        if selected > dlc_slots {
                            ui.label(egui::RichText::new(format!(
                                "⚠️ You've selected {} DLCs but only have {} slots available!",
                                selected, dlc_slots
                            )).color(egui::Color32::RED).strong());
                        }
                        
                        ui.add_space(5.0);
                        
                        // Search bar
                        ui.horizontal(|ui| {
                            ui.label("🔍 Filter:");
                            ui.text_edit_singleline(&mut self.dlc_picker_search);
                            
                            if ui.button("Select All").clicked() {
                                for (_, _, selected) in &mut self.dlc_picker_items {
                                    *selected = true;
                                }
                            }
                            if ui.button("Deselect All").clicked() {
                                for (_, _, selected) in &mut self.dlc_picker_items {
                                    *selected = false;
                                }
                            }
                            if ui.button(format!("Select First {}", dlc_slots)).clicked() {
                                for (i, (_, _, selected)) in self.dlc_picker_items.iter_mut().enumerate() {
                                    *selected = i < dlc_slots;
                                }
                            }
                        });
                        
                        ui.separator();
                        
                        // List
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            let filter = self.dlc_picker_search.to_lowercase();
                             // Use index for mutation
                             // We need to iterate mutable items
                             // But we also want to filter. 
                             // Egui pattern:
                             for (_id, name, selected) in &mut self.dlc_picker_items {
                                 if filter.is_empty() || name.to_lowercase().contains(&filter) {
                                     ui.checkbox(selected, name.as_str());
                                 }
                             }
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                             if ui.button("Cancel").clicked() {
                                 self.dlc_picker_open = false;
                                 self.dlc_picker_pending_library = None; 
                             }

                             let selected_count = self.dlc_picker_items.iter().filter(|(_, _, s)| *s).count();
                             let enabled = selected_count <= dlc_slots;
                             
                             if ui.add_enabled(enabled, egui::Button::new(egui::RichText::new("🚀 INSTALL SELECTED").strong().color(egui::Color32::GREEN))).clicked() {
                                 // FINALIZE
                                 let selected_dlc_ids: Vec<String> = self.dlc_picker_items.iter()
                                    .filter(|(_, _, s)| *s)
                                    .map(|(id, _, _)| id.clone())
                                    .collect();

                                     if let (Some(tpl_lib), Some(tpl_dir)) = (self.dlc_picker_pending_library.clone(), self.dlc_picker_pending_install_dir.clone()) {
                                         let cached = self.dlc_picker_cached_bytes.take(); // Take first (consume)
                                         // Pass None for hierarchy since DLC picker uses scraped LUA data
                                         self.finalize_installation(
                                             app_id.clone(), 
                                             name.clone(), 
                                             Some(tpl_lib), 
                                             Some(tpl_dir), 
                                             selected_dlc_ids,
                                             cached,
                                             None // No Hierarchy in Legacy Flow
                                         );
                                     }
                                     self.dlc_picker_open = false;
                                     self.dlc_picker_pending_library = None;
                                     self.dlc_picker_pending_install_dir = None;
                             }
                        });
                    });
                });

        if !open {
            self.dlc_picker_open = false;
        }
        } // Close if let Some((app_id, name))
    }


    fn initiate_delete(&mut self, app_id: String, name: String) {
        self.delete_modal_open = true;
        self.delete_candidate_id = Some(app_id.clone());
        self.delete_candidate_name = Some(name.clone());
        self.delete_associated_dlcs.clear();
        self.is_scanning_dlcs = true;

        // Local Relationship Scan
        let mut known_child_ids = Vec::new();
        if let Ok(rel) = self.relationships.lock() {
            for (child, parent) in rel.iter() {
                if parent == &app_id {
                    known_child_ids.push(child.clone());
                }
            }
        }

        // Heuristic Name Scan (For "Borderlands 4" vs "Borderlands®4: ...")
        let target_clean = name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
        if target_clean.len() >= 4 { 
             if let Ok(games) = self.active_games.lock() {
                 for game in games.iter() {
                     if game.app_id == app_id { continue; } // Skip self
                     
                     // Detect if candidate is likely a DLC based on name overlap
                     let candidate_clean = game.name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
                     
                     if candidate_clean.starts_with(&target_clean) {
                         // Additional content check
                         if (self.is_probable_dlc(&game.name) || candidate_clean.contains("pack") || candidate_clean.contains("content") || candidate_clean.contains("season"))
                            && !known_child_ids.contains(&game.app_id) {
                                known_child_ids.push(game.app_id.clone());
                            }
                     }
                 }
             }
        }

        // Spawn scan
        if let Some(client) = self.api_client.clone() {
            let app_id_clone = app_id.clone();
            let result_arc = self.delete_scan_result.clone();
            let active_games_arc = self.active_games.clone();

            std::thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let found: Vec<String> = runtime.block_on(async {
                    client.get_dlc_list(&app_id_clone).await.unwrap_or_default()
                });

                // Filter: Keep only installed
                let installed_ids: HashSet<String> = {
                    let games = active_games_arc.lock().unwrap();
                    games.iter().map(|g| g.app_id.clone()).collect()
                };

                let mut associated: Vec<String> = found
                    .into_iter()
                    .filter(|id| installed_ids.contains(id))
                    .collect();
                
                // Merge Local Knowledge
                for kid in known_child_ids {
                    if !associated.contains(&kid) && installed_ids.contains(&kid) {
                         associated.push(kid);
                    }
                }

                *result_arc.lock().unwrap() = Some(associated);
            });
        } else {
            self.is_scanning_dlcs = false;
        }
    }

    fn remove_games_by_id(&self, mut ids: Vec<String>, full_wipe: bool) {
        // AUTO-DETECT CHILDREN (Fix for Hidden Orphans)
        // If we are deleting a Parent, we must also delete its Children (DLCs/Depots)
        // because they are now hidden in the UI and can't be deleted manually!
        {
            let mut children_to_add = Vec::new();
            if let Ok(map) = self.relationships.lock() {
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
                self.log(format!("♻ Linked Deletion: Found {} attached DLCs/Depots.", children_to_add.len()));
                ids.extend(children_to_add);
            }
        }

        let gl_path = self.config.gl_path.clone();
        let steam_path = self.config.steam_path.clone();
        let al_path = Path::new(&gl_path).join("AppList");

        // 1. Remove from AppList (Always logic)
        if let Ok(paths) = glob::glob(&al_path.join("*.txt").to_string_lossy()) {
            for path in paths.flatten() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if ids.contains(&content.trim().to_string()) {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
        
        // 2. Full Wipe: Manifests AND Content (Surgical - Check All Libraries)
        if full_wipe {
            let libraries = crate::game_path::GamePathFinder::get_library_folders(&steam_path);
            let vault = crate::vault::VaultManager::new("."); // Initialize Vault

            for id in &ids {
                 // Scan ALL libraries for the game to backup
                 let mut backed_up = false;
                 for lib in &libraries {
                      if let Ok(c) = vault.backup_manifests(&lib.to_string_lossy(), id) {
                          if c > 0 { 
                              self.log(format!("Vault: Secured {} manifests for {} from {:?}.", c, id, lib)); 
                              backed_up = true;
                              break; // Found and backed up
                          }
                      }
                 }
                 if !backed_up {
                     // Try default steam path as fallback
                     let _ = vault.backup_manifests(&steam_path, id);
                 }

                 // Define potential locations (Main + External Libs)
                 let mut locations = libraries.clone();
                 locations.push(std::path::Path::new(&steam_path).to_path_buf());
                 
                 for lib in &locations {
                     let acf = lib.join("steamapps").join(format!("appmanifest_{}.acf", id));
                     if acf.exists() {
                         // A. READ MANIFEST TO FIND INSTALL DIR
                         if let Ok(content) = std::fs::read_to_string(&acf) {
                             // Simple parsing for "installdir"
                             let mut install_dir = String::new();
                             for line in content.lines() {
                                 if line.to_lowercase().contains("installdir") {
                                     let parts: Vec<&str> = line.split('"').collect();
                                     if parts.len() >= 4 {
                                         install_dir = parts[3].to_string();
                                     }
                                 }
                             }
                             
                             // B. DELETE CONTENT FOLDER
                             if !install_dir.is_empty() {
                                 let content_path = lib.join("steamapps").join("common").join(&install_dir);
                                 if content_path.exists() {
                                     self.log(format!("Deleting Game Files: {:?}", content_path));
                                     let _ = std::fs::remove_dir_all(&content_path);
                                 }
                             }
                         }
                         
                         // C. DELETE MANIFEST
                         let _ = std::fs::remove_file(acf); 
                     }
                 }
            }
        }

        // 3. Remove from config.vdf (Surgical)
        if let Err(e) = crate::vdf_injector::remove_vdf_keys(&steam_path, &ids) {
            self.log(format!("VDF Cleanup Warning: {}", e));
        }
        
        // 4. Update Relationships
        if let Ok(mut map) = self.relationships.lock() {
            let initial_len = map.len();
            map.retain(|k, _| !ids.contains(k));
            if map.len() != initial_len {
                crate::app_list::save_relationships(".", &map);
            }
        }

        // 5. Automatic Reorder (Fix gaps in 0.txt, 1.txt...)
        self.log("Reordering AppList...".to_string());
        let cache_guard = self.game_cache.lock().ok();
        let cache_ref = cache_guard.as_deref();
        
        if let Err(e) = crate::app_list::nuke_reorder(&gl_path, &steam_path, None, cache_ref) {
            self.log(format!("Reorder Warning: {}", e));
        }

        self.log(format!("Deleted {} items. Full Wipe: {}", ids.len(), full_wipe));
    }

    fn is_probable_dlc(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        let keywords = [
            "dlc", "pack", "soundtrack", " ost", "artbook", "upgrade", 
            "season pass", "expansion", "ticket", "skin", "costume", 
            "bonus", "content", "kit", "bundle", "edition"
        ];
        for kw in keywords {
            if lower.contains(kw) {
                return true;
            }
        }
        false
    }

    fn ui_info(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let time = ui.input(|i| i.time);
        
        if self.active_tab == 5 {
             ui.ctx().request_repaint();
        }

        // Deep Black Background
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(2, 2, 5));

        let rand_pseudo = |seed: usize| -> usize {
            (seed.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fffffff
        };
        
        // Extended Glyph Set (Katakana-ish + numbers)
        // Note: Standard Fonts might not have all chars, using safe set + some extras
        let glyphs = "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM0123456789<>:;[]{}!@#$%^&*=+-_|?"; 
        let random_matrix_char = |seed: usize| -> char {
             glyphs.chars().nth(seed % glyphs.chars().count()).unwrap_or('X')
        };

        // INITIAL POPULATION (Heavy Density)
        if self.matrix_trails.is_empty() {
             for i in 0..450 {
                 let layer = (i % 3) as u8;
                 // Front layer (2) is sparse but impactful
                 // Back layer (0) is dense
                 
                 let speed_base = match layer { 0 => 1.0, 1 => 2.5, _ => 4.5 };
                 let speed = speed_base + (i % 7) as f32 * 0.3;
                 // Random X
                 let x = (i as f32 * 13.0 * (layer as f32 + 1.2) + (time * 100.0) as f32) % rect.width() + rect.min.x;
                 let h_y = rect.min.y + (i as f32 * 7.0) % rect.height();
                 let len = 10 + (i % 30);
                 
                 let mut chars = Vec::new();
                 for k in 0..len { chars.push(random_matrix_char(i + k)); }
                 
                 self.matrix_trails.push(MatrixTrail { x, head_y: h_y, speed, len, chars, layer });
             }
        }
        
        // SPAWN NEW TRAILS
        // Maintain ~450 trails
        if self.matrix_trails.len() < 450 {
             let seed = (time * 10000.0) as usize;
             // Spawn mostly back/mid layers, occasionally front
             if rand_pseudo(seed) % 100 < 60 { 
                 let layer_roll = rand_pseudo(seed + 1) % 100;
                 let layer = if layer_roll < 50 { 0 } else if layer_roll < 85 { 1 } else { 2 };
                 
                 let x = rect.min.x + (rand_pseudo(seed + 2) % (rect.width() as usize)) as f32;
                 let speed_base = match layer { 0 => 1.0, 1 => 2.5, _ => 4.5 };
                 let speed = speed_base + (rand_pseudo(seed + 3) as f32 % 5.0) * 0.4;
                 let len = 10 + (rand_pseudo(seed + 4) % 40);
                 
                 let mut chars = Vec::new();
                 for k in 0..len { chars.push(random_matrix_char(seed + k)); }
                 
                 self.matrix_trails.push(MatrixTrail {
                     x, head_y: rect.min.y - 150.0, speed, len, chars, layer
                 });
             }
        }

        // UPDATE & RENDER
        let painter = ui.painter();
        
        // Layer Configs
        let font_small = egui::FontId::monospace(10.0);
        let font_mid = egui::FontId::monospace(14.0);
        let font_large = egui::FontId::monospace(18.0); // Big Front

        let white = egui::Color32::WHITE;
        let neon_green = egui::Color32::from_rgb(50, 255, 50);

        // Sort trails by layer so Front draws on top of Back
        // But for performance with retain_mut we can't sort easily every frame.
        // It's digital rain, depth overlap is usually chaotic anyway.
        // We'll iterate. Painter works in order.
        // To do generic depth sort, we'd need to separate list. 
        // Let's just draw mixed. It adds to the chaos.

        self.matrix_trails.retain_mut(|trail| {
            trail.head_y += trail.speed;
            
            // Random mutation
            if rand_pseudo((trail.head_y * 10.0) as usize) % 15 == 0 {
                let idx = rand_pseudo((time * 1000.0) as usize) % trail.len;
                trail.chars[idx] = random_matrix_char((time * 999.0) as usize);
            }

            let (font, char_h, opacity_mult) = match trail.layer {
                0 => (&font_small, 10.0, 0.3),
                1 => (&font_mid, 14.0, 0.7),
                _ => (&font_large, 18.0, 1.0),
            };

            // Draw Chars
             for (i, &c) in trail.chars.iter().enumerate() {
                let y_pos = trail.head_y - (i as f32 * char_h);
                if y_pos > rect.max.y { continue; }
                if y_pos < rect.min.y - char_h { break; }

                let color;
                if i == 0 {
                    color = white.linear_multiply(opacity_mult);
                    // Fake Bloom for head
                    if trail.layer == 2 {
                         // Double draw for glow
                         painter.text(egui::pos2(trail.x, y_pos), egui::Align2::CENTER_TOP, c, font.clone(), white.linear_multiply(0.4));
                    }
                } else if i < 3 {
                    color = neon_green.linear_multiply(opacity_mult);
                } else {
                     let fade = 1.0 - (i as f32 / trail.len as f32);
                     // Quadratic fade out
                     color = neon_green.linear_multiply((fade * fade) * opacity_mult);
                }
                
                painter.text(
                    egui::pos2(trail.x, y_pos),
                    egui::Align2::CENTER_TOP,
                    c,
                    font.clone(),
                    color
                );
             }

            let tail_y = trail.head_y - (trail.len as f32 * char_h);
            tail_y < rect.max.y
        });

        // MANIFESTO OVERLAY (Optimized)
        let center = rect.center();
        let wrap_width = 550.0;
        
        let galley = painter.layout_job(
            egui::text::LayoutJob::simple(
                "WE ARE THE ORCHESTRATORS.\n\nSteam is the cage. DarkCore is the key.\nWe build bridges where they built walls.\nWe play what we want, when we want.\n\nPower to the Players.\n\nSigned, SEBASTIAN.".to_string(),
                egui::FontId::monospace(15.0),
                egui::Color32::from_rgb(220, 255, 220),
                wrap_width
            )
        );

        let text_rect = egui::Rect::from_center_size(center, galley.size() + egui::vec2(80.0, 80.0));
        
        // Advanced Box Rendering
        painter.rect_filled(text_rect, 2.0, egui::Color32::from_black_alpha(245)); // Darker bg
        painter.rect_stroke(text_rect, 2.0, egui::Stroke::new(2.0, neon_green)); // Crisp border
        
        // Outer Glow
        for i in 1..5 {
            let width = 2.0 + i as f32 * 2.0;
            let alpha = 60 / i; // Brighter glow
            painter.rect_stroke(
                text_rect.expand(i as f32), 
                2.0, 
                egui::Stroke::new(width, neon_green.linear_multiply(alpha as f32 / 255.0))
            );
        }

        painter.galley(text_rect.min + egui::vec2(40.0, 40.0), galley, egui::Color32::WHITE);
    }
}

impl DarkCoreApp {
    fn detect_auto_install_path(&self, game_name: &str, libraries: &[std::path::PathBuf]) -> (Option<String>, Option<std::path::PathBuf>, String) {
        // Returns: (DirName, LibraryPath, ConfidenceLevel)
        let target_tokens = clean_tokenize(game_name);
        let target_clean = game_name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
        
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
                              if folder_name.eq_ignore_ascii_case("common") || folder_name.eq_ignore_ascii_case("Steamworks Shared") { continue; }

                              let folder_clean = folder_name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
                              
                              // 1. Exact Match (Sanitized)
                              if folder_clean == target_clean {
                                  return (Some(folder_name.to_string()), Some(lib.clone()), "EXACT".to_string());
                              }

                              // 2. Token Overlap
                              let folder_tokens = clean_tokenize(folder_name);
                              let mut overlap = 0;
                              for t in &target_tokens {
                                  if folder_tokens.contains(t) { overlap += 1; }
                              }
                              
                              let score = if !target_tokens.is_empty() {
                                  (overlap * 100) / target_tokens.len()
                              } else { 0 };

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
}

// Helper Tokenizer
fn clean_tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

// Helper function to write the ACF file content
// DEPRECATED: Use generate_smd_style_acf instead for minimal ACF (SMD approach)
// Kept for potential future use if detailed ACF generation is needed
#[allow(dead_code)]
pub fn generate_acf(
    steam_path: &str, 
    acf_path: &std::path::Path, 
    appid: &str, 
    name: &str, 
    timestamp: &str,
    installed_depots: &Vec<(String, u64, String)>,
    total_size: u64
) -> std::io::Result<()> {
    // Ensure parent dir exists
    if let Some(parent) = acf_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let steam_exe = std::path::Path::new(steam_path).join("steam.exe");
    let steam_exe_str = steam_exe.to_str().unwrap_or("steam.exe").replace("\\", "\\\\");

    // Sanitize installdir (Matches SteamDB convention: Remove non-alphanumeric, keep spaces)
    let install_dir_sanitized: String = name.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .trim()
        .to_string();

    // Create the game directory in steamapps/common
    if let Some(parent) = acf_path.parent() {
        let common_dir = parent.join("common");
        let game_dir = common_dir.join(&install_dir_sanitized);
        if !game_dir.exists() {
            let _ = std::fs::create_dir_all(&game_dir);
        }
    }
    
    // Check for InstallScript
    // Check for InstallScript
    // BYPASS: We deliberately SKIP injecting the InstallScript to prevent "SteamService.exe" errors.
    // This assumes the user has standard VCRedists installed.
    // Use "Repair" to re-generate the ACF without this section if stuck.
    let install_script_entry = String::new();
    /*
    if let Some(parent) = acf_path.parent() {
         let common_dir = parent.join("common");
         let game_dir = common_dir.join(&install_dir_sanitized);
         if game_dir.join("installscript.vdf").exists() {
             // Heuristic: Usually the first depot ID is the main one that has the script
             if let Some((first_depot, _, _)) = installed_depots.first() {
                 install_script_entry = format!("\n\t\"InstallScripts\"\n\t{{\n\t\t\"{}\"		\"installscript.vdf\"\n\t}}", first_depot);
             }
         }
    }
    */

    // Build InstalledDepots Section
    let mut depots_section = String::from("\n\t\"InstalledDepots\"\n\t{");
    for (d_id, d_size, d_manifest) in installed_depots {
        depots_section.push_str(&format!(r#"
		"{}"
		{{
			"manifest"		"{}"
			"size"		"{}"
		}}"#, d_id, d_manifest, d_size));
    }
    depots_section.push_str("\n\t}");

    // StateFlags 4 = Fully Installed.
    let content = format!(r#""AppState"
{{
	"appid"		"{}"
	"Universe"		"1"
	"LauncherPath"		"{}"
	"name"		"{}"
	"StateFlags"		"4"
	"installdir"		"{}"
	"LastUpdated"		"{}"
	"SizeOnDisk"		"{}"
	"StagingSize"		"0"
	"buildid"		"0"
	"LastOwner"		"0"
	"UpdateResult"		"0"
	"BytesToDownload"		"{}"
	"BytesDownloaded"		"{}"
	"BytesToStage"		"0"
	"BytesStaged"		"0"
	"TargetBuildID"		"0"
	"AutoUpdateBehavior"		"0"
	"AllowOtherDownloadsWhileRunning"		"0"
	"ScheduledAutoUpdate"		"0"{}{}
	"UserConfig"
	{{
		"language"		"english"
	}}
	"MountedConfig"
	{{
		"language"		"english"
	}}
}}
"#,
        appid,
        steam_exe_str.replace("\\", "\\\\"),
        name,
        install_dir_sanitized,
        timestamp,
        total_size,
        total_size, // BytesToDownload
        total_size, // BytesDownloaded
        depots_section,
        install_script_entry
    );

    std::fs::write(acf_path, content)?;
    Ok(())
}

/// Generate a MINIMAL ACF file matching SMD's format exactly.
/// This creates a "ghost" ACF that tells Steam the game needs to be downloaded.
/// Steam will populate all the other fields (InstalledDepots, etc.) during download.
/// 
/// SMD Reference (smd/lua/writer.py lines 35-44):
/// ```python
/// acf_contents = {
///     "AppState": {
///         "AppID": lua.app_id,
///         "Universe": "1",
///         "name": app_name,
///         "installdir": sanitize_filename(app_name),
///         "StateFlags": "4",
///     }
/// }
/// ```
pub fn generate_smd_style_acf(
    acf_path: &std::path::Path, 
    appid: &str, 
    game_name: &str,
) -> std::io::Result<()> {
    // Ensure parent dir exists
    if let Some(parent) = acf_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Sanitize installdir (Remove non-alphanumeric except spaces, similar to pathvalidate)
    let install_dir_sanitized: String = game_name.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string();

    // NOTE: We do NOT create the game directory here.
    // SMD doesn't create it either. Steam will create it during download.
    // Creating an empty folder causes Steam to think the game is "installed but corrupt".

    // MINIMAL ACF - Exactly 5 fields like SMD
    // StateFlags "4" = Fully Installed (tells Steam game is ready but needs update)
    let content = format!(r#""AppState"
{{
	"appid"		"{}"
	"Universe"		"1"
	"name"		"{}"
	"installdir"		"{}"
	"StateFlags"		"4"
}}
"#,
        appid,
        game_name,
        install_dir_sanitized,
    );

    std::fs::write(acf_path, content)?;
    Ok(())
}




pub fn setup_greenluma_config(gl_path: &str, enable_stealth: bool) -> std::io::Result<()> {
    let path = std::path::Path::new(gl_path);
    if !path.exists() { return Ok(()); }

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
    // The .ini file was for debug logging (LogFile=1) but it breaks
    // DLLInjector.exe for users who want to use the original tool.
    // Our Rust APC injector does not need this file at all.
    
    Ok(())
}

// --- WUDRM HELPER ---
pub fn download_manifests_wudrm(appid: &str, steam_root: &str, log: &dyn Fn(String)) -> Result<usize, Box<dyn std::error::Error>> {
    use crate::api::ApiClient;
    
    // Check if module exists - assuming we imported it or use crate path
    let runtime = tokio::runtime::Runtime::new()?;
    // Anonymous client for public SteamCMD API
    let client = ApiClient::new("".to_string()); 
    let downloader = crate::manifest_downloader::ManifestDownloader::new();
    let depot_cache_dir = std::path::Path::new(steam_root).join("depotcache");
    
    if !depot_cache_dir.exists() { std::fs::create_dir_all(&depot_cache_dir)?; }

    log(format!("Wudrm: Connecting to SteamCMD for AppID {}...", appid));
    
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
                     log(format!("   - Skipping Wudrm (Found local): {}", expected_name));
                     valid_manifests += 1;
                     // Ensure it is in Vault too (Sync)
                     let _ = vault.store_manifest(appid, &expected_path);
                     continue;
                }

                log(format!("   - Downloading Manifest: Depot {} | GID: {}", depot_id, gid));
                match runtime.block_on(downloader.download_manifest(&depot_id, &gid, &depot_cache_dir)) {
                    Ok(path) => {
                        log(format!("      ✅ Success! Saved to {:?}", path));
                        valid_manifests += 1;
                        // 2. Save to Vault immediately
                        let _ = vault.store_manifest(appid, &path);
                    },
                    Err(e) => {
                        log(format!("      ❌ Failed to download {}: {}", depot_id, e));
                    }
                }
        }
    }
    
    Ok(valid_manifests)
}

/// Extract DLC name from LUA comments
/// Morrenus LUA files often have comments like: addappid(123456) -- DLC Name Here
fn extract_dlc_name_from_lua(lua_content: &str, dlc_id: &str) -> Option<String> {
    for line in lua_content.lines() {
        // Look for addappid(ID) on this line
        if line.contains(&format!("addappid({})", dlc_id)) || 
           line.contains(&format!("addappid({},", dlc_id)) {
            // Check for comment after the call
            if let Some(comment_start) = line.find("--") {
                let comment = line[comment_start + 2..].trim();
                if !comment.is_empty() {
                    return Some(comment.to_string());
                }
            }
        }
    }
    None
}

fn resolve_mandatory_depots(
    hierarchy: &crate::api::GameHierarchy,
    selected_dlcs: &[String],
) -> Vec<String> {
    let mut ids = Vec::new();
    
    // 1. Root AppID (The Game Itself)
    ids.push(hierarchy.root_id.clone());
    
    // 2. Base Depots (Mandatory for the game to run)
    for depot in &hierarchy.base_depots {
        ids.push(depot.depot_id.clone());
    }
    
    // 3. Selected DLCs and THEIR Depots
    for dlc in &hierarchy.dlcs {
        if selected_dlcs.contains(&dlc.app_id) {
            ids.push(dlc.app_id.clone());
            // Include Depots for this DLC
            for depot in &dlc.depots {
                ids.push(depot.depot_id.clone());
            }
        }
    }
    
    // Dedup just in case
    ids.sort();
    ids.dedup();
    
    ids
}
