use crate::api::{ApiClient, SearchResult};
use crate::app_list::{
    add_games_to_list, nuke_reorder, refresh_active_games_list, GameProfile,
};
use crate::cache::{load_game_cache, save_game_cache};
use crate::config::{load_config, save_config, AppConfig};
use crate::profiles::{Profile, ProfileManager};
use crate::steamless;
use crate::vdf_injector::{inject_vdf, parse_lua_for_keys};
use crate::vault::VaultManager;
use eframe::egui;
use rodio::Source;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

use std::time::{Duration, Instant};

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
    // Queue for loaded images: (AppID, Width, Height, Pixels)
    cover_queue: Arc<Mutex<Vec<(String, u32, u32, Vec<u8>)>>>,

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
    dlc_scan_result: Arc<Mutex<Option<Vec<String>>>>,
    
    // Install Modal
    install_modal_open: bool,
    install_candidate: Option<(String, String)>, // (AppID, Name)
    detected_libraries: Vec<std::path::PathBuf>,
    selected_library_index: usize,
    install_dir_input: String, // NEW: Manual override for Folder Name

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

    // Legacy: Steamless Context (DRM tab removed - now in Library)
    #[allow(dead_code)]
    steamless_app_id: String,
    #[allow(dead_code)]
    steamless_auto_titan: bool,

    user_stats: Arc<Mutex<Option<crate::api::UserStats>>>,
    api_last_error: Arc<Mutex<Option<String>>>,
    is_validating_api: Arc<Mutex<bool>>, // New
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
            
            // Install Modal
            install_modal_open: false,
            install_candidate: None,
            detected_libraries: Vec::new(),
            selected_library_index: 0,
            install_dir_input: String::new(),
            
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
            
            steamless_app_id: String::new(),
            steamless_auto_titan: false,

            user_stats: Arc::new(Mutex::new(None)),
            api_last_error: Arc::new(Mutex::new(None)),
            is_validating_api: Arc::new(Mutex::new(false)),
            matrix_trails: Vec::new(),
            config_saved_at: None,
            api_refresh_timer: None,
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
            logs.push("System Ready. Darkcore Rust Initialized.".to_string());
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
            
            // Install Modal
            install_modal_open: false,
            install_candidate: None,
            detected_libraries: Vec::new(),
            selected_library_index: 0,
            install_dir_input: String::new(), // Init
            
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

            steamless_app_id: String::new(),
            steamless_auto_titan: true,

            status_update_queue: Arc::new(Mutex::new(None)),
            
            user_stats: Arc::new(Mutex::new(None)),
            api_last_error: Arc::new(Mutex::new(None)),
            is_validating_api: Arc::new(Mutex::new(false)),
            matrix_trails: Vec::new(),
            api_key_glitch_cache: String::new(),
            api_key_glitch_update: Instant::now(),
            config_saved_at: None,
            api_refresh_timer: if !initial_api_key.is_empty() { Some(Instant::now() + std::time::Duration::from_millis(500)) } else { None }, // Auto-Start
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

        // Install image loaders
        egui_extras::install_image_loaders(&_cc.egui_ctx);

        app.refresh_library();
        app.resolve_unknown_games();
        app
    }

    fn configure_visuals(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // FONT SIZES
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
            (egui::TextStyle::Button, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Small, egui::FontId::new(12.0, egui::FontFamily::Proportional)),
        ].into();
        
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(15.0, 8.0);
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.button_padding = egui::vec2(20.0, 10.0);
        style.visuals.window_rounding = egui::Rounding::same(12.0);
        // style.visuals.popup_shadow = egui::epaint::Shadow::big_dark(); // removed to avoid error
        
        ctx.set_style(style);

        let mut visuals = egui::Visuals::dark();

        // CYBERPUNK PALETTE
        let bg_app = egui::Color32::from_rgb(11, 12, 16); // Obsidian
        let bg_surface = egui::Color32::from_rgb(24, 26, 33); // Gunmetal
        let accent_cyan = egui::Color32::from_rgb(0, 243, 255); // Neon Cyan
        let accent_pink = egui::Color32::from_rgb(255, 0, 110); // Cyber Pink
        //let accent_green = egui::Color32::from_rgb(0, 255, 136); // Toxic Green
        let text_bright = egui::Color32::from_rgb(245, 245, 250); 
        let text_dim = egui::Color32::from_rgb(160, 160, 180);

        visuals.window_fill = bg_app;
        visuals.panel_fill = bg_app;
        
        // Non Interactive
        visuals.widgets.noninteractive.bg_fill = bg_app;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_bright); // Changed from text_main to text_bright

        // Buttons (Idle) - "Glassy" look
        visuals.widgets.inactive.bg_fill = bg_surface;
        visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text_dim);
        visuals.widgets.inactive.weak_bg_fill = bg_surface;

        // Buttons (Hover) - "Glow"
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(35, 38, 50);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, accent_cyan);
        visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
        visuals.widgets.hovered.expansion = 2.0; 

        // Buttons (Active)
        visuals.widgets.active.bg_fill = accent_cyan.linear_multiply(0.15);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, accent_cyan);
        visuals.widgets.active.rounding = egui::Rounding::same(8.0);
        visuals.widgets.active.expansion = 1.0;

        // Selection
        visuals.selection.bg_fill = accent_pink.linear_multiply(0.3);
        visuals.selection.stroke = egui::Stroke::new(1.0, accent_pink);
        
        ctx.set_visuals(visuals);
    }

    fn log<S: Into<String>>(&self, msg: S) {
        let msg = msg.into();
        if let Ok(mut logs) = self.system_log.lock() {
            logs.push(msg);
            if logs.len() > 50 { logs.remove(0); }
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

            self.log(&format!("Searching for: {}", query));
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
                                               // 1. Get Local
                                               let acf = std::path::Path::new(&sp).join("steamapps").join(format!("appmanifest_{}.acf", aid));
                                               let mut local_ts = 0u64;
                                               if acf.exists() {
                                                   if let Ok(c) = std::fs::read_to_string(&acf) {
                                                       if let Some(pos) = c.find("\"LastUpdated\"") {
                                                            let rem = &c[pos..];
                                                            if let Some(sq) = rem.find("\"") {
                                                                if let Some(el) = rem[sq+1..].find("\"") {
                                                                     let val_p = &rem[sq+1+el+1..];
                                                                     if let Some(qs) = val_p.find("\"") {
                                                                         if let Some(qe) = val_p[qs+1..].find("\"") {
                                                                             let s = &val_p[qs+1..qs+1+qe];
                                                                             local_ts = s.parse().unwrap_or(0);
                                                                         }
                                                                     }
                                                                }
                                                            }
                                                       }
                                                   }
                                               }
                                               
                                               // 2. Get Remote
                                               match client.get_status(&aid).await {
                                                   Ok(st) => {
                                                        let mut needs = st.needs_update.unwrap_or(false);
                                                        if !needs && local_ts > 0 {
                                                            if let Some(ts_str) = st.timestamp {
                                                                use chrono::DateTime;
                                                                if let Ok(dt) = DateTime::parse_from_rfc3339(&ts_str) {
                                                                    if dt.timestamp() as u64 > local_ts {
                                                                        needs = true;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if let Ok(mut c) = cache.lock() {
                                                            c.insert(aid, needs);
                                                        }
                                                   },
                                                   Err(_) => {}
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
                        match rt.block_on(client.get_user_stats()) {
                            Ok(stats) => {
                                if let Ok(mut s) = user_stats_arc.lock() {
                                    *s = Some(stats);
                                }
                            }
                            Err(_) => {} 
                        }
                    }
                    Err(e) => {
                        if let Ok(mut logs) = log_arc.lock() {
                            logs.push(format!("Search API Error: {}", e));
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

        std::thread::spawn(move || {
            let client = if let Some(c) = client_opt { c } else { return; };
            // Safe Runtime
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
                
                handles.push(tokio::spawn(async move {
                    // 1. Get Local LastUpdated
                    let acf_path = std::path::Path::new(&sp).join("steamapps")
                        .join(format!("appmanifest_{}.acf", appid));
                    
                    let mut local_ts = 0u64;
                    if acf_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&acf_path) {
                            // Simple Regex/Find for "LastUpdated"
                            // Format: "LastUpdated" "1234567890"
                            if let Some(pos) = content.find("\"LastUpdated\"") {
                                let remainder = &content[pos..];
                                // Skip label and search for value
                                if let Some(start_quote) = remainder.find("\"") {
                                    // Skip first quote of label, find second
                                    if let Some(end_label) = remainder[start_quote+1..].find("\"") {
                                         let val_part = &remainder[start_quote+1+end_label+1..];
                                         // Find value quotes
                                         if let Some(v_start) = val_part.find("\"") {
                                             if let Some(v_end) = val_part[v_start+1..].find("\"") {
                                                 let num_str = &val_part[v_start+1 .. v_start+1+v_end];
                                                 local_ts = num_str.parse().unwrap_or(0);
                                             }
                                         }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Get Remote Status
                    match client.get_status(&appid).await {
                        Ok(status) => {
                             let mut needs_update = false;
                             
                             // A. Explicit Flag
                             if let Some(true) = status.needs_update {
                                 needs_update = true;
                             }
                             
                             // B. Timestamp comparison
                             if !needs_update && local_ts > 0 {
                                 if let Some(ts_str) = status.timestamp {
                                     // Try parsing ISO or Unix?
                                     // Solus uses DateTime. Assuming ISO 8601.
                                     use chrono::DateTime;
                                     if let Ok(dt) = DateTime::parse_from_rfc3339(&ts_str) {
                                         let remote_ts = dt.timestamp() as u64;
                                         if remote_ts > local_ts {
                                             needs_update = true;
                                         }
                                     }
                                 }
                             }
                             
                             if let Ok(mut c) = cache.lock() {
                                 c.insert(appid, needs_update);
                             }
                        },
                        Err(_) => {
                            // If API fails, assume False (Play) or keep previous
                        }
                    }
                }));
            }
            
            rt.block_on(async {
                for h in handles { let _ = h.await; }
            });
        });
    }

    pub fn install_game(&self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>) {
        // UNIFIED PROTOCOL: Works both Online (Manifests) and Offline (FamSharing/Public) through Fallbacks.
        let log_arc = self.system_log.clone();
        // let api_client_clone = self.api_client.clone(); // Not needed if we re-init
        let steam_path = self.config.steam_path.clone(); // Still need main path for other things
        let gl_path = self.config.gl_path.clone();
        let include_dlcs = self.include_dlcs;
        let game_cache = self.game_cache.clone(); // Keep this for cache updates
        let api_key = self.config.api_key.clone(); // Keep this for API client creation inside thread
        let relationships_arc = self.relationships.clone(); // New: Capture relationships map for thread
        let enable_stealth = self.config.enable_stealth_mode;
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
                    logs.push(msg);
                }
            };
            
            // Re-initialize client inside thread for manifest download
            let client = ApiClient::new(api_key.clone());

            log(format!("START: Protocol for {}", name));
            update_status(format!("Installing {}", name));

            // STEP 0.5: SETUP GREENLUMA CONFIG (Stealth Mode)
            // Ensure .bin files exist
            if let Err(e) = setup_greenluma_config(&gl_path, enable_stealth) {
                 log(format!("Warning: Could not setup GreenLuma config: {}", e));
            } else {
                 if enable_stealth {
                     log("GreenLuma configured (Stealth Mode: ON).".to_string());
                 } else {
                     log("GreenLuma configured (Stealth Mode: OFF).".to_string());
                 }
            }


            // STEP 1: Kill Steam
            log("STEP 1: Killing Steam Process...".to_string());
            let _ = std::process::Command::new("taskkill").args(&["/F", "/IM", "steam.exe"]).output();
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
            
            // HOISTED: Calculate Install Dir Name (Available for both Ghost ACF and Tactical Bypass)
            // Use potentially overridden install dir name, or default to display name
            let final_install_dir = install_dir_name.as_ref().map(|s| s.clone()).unwrap_or(name.clone());

            // Use library_path for restore check/logic
            if let Ok((restored_acf, count)) = vault.restore_manifests(&library_path, &appid) {
                if count > 0 { log(format!("Vault: Restored {} local depot manifests.", count)); }
                if restored_acf {
                    log("Vault: Restored AppManifest.acf. Skipping Ghost Generation. 🛡️".to_string());
                    skip_ghost = true;
                }
            }

            if !skip_ghost {
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



            // STEP 2: TRY MANIFEST (Priority + Vault)
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let mut manifest_success = false;
            let mut lua_content = String::new();
            let vault = VaultManager::new(".");

            // Helper to process ZIP bytes
            let process_zip = |bytes: Vec<u8>| -> (bool, String, Vec<std::path::PathBuf>) {
                let reader = Cursor::new(bytes);
                let mut manifest_paths = Vec::new();
                if let Ok(mut zip) = ZipArchive::new(reader) {
                    let depot_dir = Path::new(&steam_path).join("depotcache");
                    if !depot_dir.exists() {
                         let _ = std::fs::create_dir_all(&depot_dir);
                    }
                    let mut extracted_lua = String::new();
                    for i in 0..zip.len() {
                        if let Ok(mut file) = zip.by_index(i) {
                            let raw_path = file.name().to_string();
                            if raw_path.ends_with(".manifest") {
                                 if let Some(fname) = Path::new(&raw_path).file_name() {
                                     let out_path = depot_dir.join(fname);
                                     if let Ok(mut outfile) = std::fs::File::create(&out_path) {
                                          let _ = std::io::copy(&mut file, &mut outfile);
                                          manifest_paths.push(out_path);
                                     }
                                 }
                            } else if raw_path.ends_with(".lua") {
                                 use std::io::Read;
                                 let _ = file.read_to_string(&mut extracted_lua);
                            }
                        }
                    }
                    return (true, extracted_lua, manifest_paths);
                }
                (false, String::new(), Vec::new())
            };

            // VAULT CHECK
            let mut bytes_opt = None;
            if vault.exists(&appid) {
                log(format!("DarkVault: Found cached manifest for {}. Loading local... 🛡️", appid));
                if let Ok(b) = vault.get(&appid) {
                     bytes_opt = Some(b);
                }
            }

            if bytes_opt.is_none() && !api_key.is_empty() {
                log(format!("STEP 2: Downloading Manifest for ID {} (Online)...", appid));
                match runtime.block_on(client.download_manifest(&appid)) {
                     Ok(bytes) => {
                         // Save to Vault
                         if let Err(e) = vault.save(&appid, &bytes) {
                             log(format!("Vault Save Error: {}", e));
                         } else {
                             log("Download successful. Saved to Vault.".to_string());
                             // EXTRA: Backup Depots
                             if let Ok(c) = vault.backup_manifests(&steam_path, &appid) {
                                  if c > 0 { log(format!("Vault: Secured {} local depot manifests.", c)); }
                             }
                             
                             // AUTO-REFRESH TOKEN COUNTER (so user sees updated count immediately)
                             if let Ok(new_stats) = runtime.block_on(client.get_user_stats()) {
                                 if let Ok(mut stats) = user_stats_arc.lock() {
                                     *stats = Some(new_stats);
                                 }
                             }
                         }
                         bytes_opt = Some(bytes.to_vec());
                     },
                     Err(_) => {
                         log("Manifest download failed (Invalid Key or Server Error). Skipping to Fallback...".to_string());
                     }
                }
            } else if bytes_opt.is_none() {
                 log("OFFLINE MODE: No Key & No Cache. Skipping Manifest.".to_string());
            }

            if let Some(bytes) = bytes_opt {
                let (ok, content, paths) = process_zip(bytes);
                manifest_success = ok;
                lua_content = content;
                
                if !paths.is_empty() {
                    log(format!("✅ Extracted {} manifests. Backing up to Vault...", paths.len()));
                    let mut saved_count = 0;
                    for p in paths {
                        if vault.store_manifest(&appid, &p).is_ok() {
                            saved_count += 1;
                        }
                    }
                    if saved_count > 0 {
                         log(format!("✅ Vault: Secured {}/{} manifests.", saved_count, saved_count));
                    }
                } else if ok {
                    log("Warning: No manifests found in zip bundle.".to_string());
                }
            }

            // STEP 3: PREPARE IDs (Hybrid)
            let mut final_ids = Vec::new();

            // 3A. If Manifest/Lua success -> Use Lua IDs (Best)
            if manifest_success && !lua_content.is_empty() {
                let (all_ids, keys) = parse_lua_for_keys(&lua_content);
                log(format!("🔑 LUA Analysis: Found {} AppIDs and {} Depot Keys.", all_ids.len(), keys.len()));

                // Debug: Log the keys we found (Masked)
                for (d_id, d_key) in &keys {
                    let mask = if d_key.len() > 8 { &d_key[0..8] } else { "???" };
                    log(format!("   - Key for Depot {}: {}...", d_id, mask));
                }

                if keys.is_empty() {
                    log("⚠️ WARNING: No Depot Keys found in LUA! Steam download will likely fail with 'Content Encrypted'.".to_string());
                }
            
            // VDF Injection (Steam Native)
            let vdf_file = std::path::Path::new(&steam_root).join("config").join("config.vdf");
            if !vdf_file.exists() {
                log(format!("⚠️ CRITICAL WARNING: config.vdf NOT FOUND at {:?}.", vdf_file));
            }

            if let Err(e) = inject_vdf(&steam_root, &keys) {
                log(format!("Steam VDF Error: {}", e));
            } else {
                log(format!("✅ Depot Keys Injected. Verifying persistence in {:?}...", vdf_file));
                
                // VERIFICATION: Read back to confirm
                if let Ok(written_content) = std::fs::read_to_string(&vdf_file) {
                    let mut all_found = true;
                    for (d_id, d_key) in &keys {
                         if !written_content.contains(d_key) {
                             log(format!("❌ CRITICAL: Key for Depot {} NOT FOUND in config.vdf after write!", d_id));
                             all_found = false;
                         }
                    }
                    if all_found {
                        log("✨ SUCCESS: All Depot Keys verified physically present in config.vdf.".to_string());
                    } else {
                        log("⚠️ WARNING: Some keys failed to persist. Check permissions or file locks.".to_string());
                    }
                } else {
                    log("❌ Error reading config.vdf for verification.".to_string());
                }
            }

            // VDF Injection (User Local Config) - New Fix
            if let Err(e) = crate::vdf_injector::inject_localconfig_vdf(&steam_root, &keys) {
                 log(format!("LocalConfig VDF Error: {}", e));
            } else {
                 log("✅ Depot Keys Injected into UserData localconfig.vdf.".to_string());
            }

// --- TACTICAL BYPASS: DepotDownloader ---
// --- TACTICAL BYPASS: NATIVE DOWNLOADER (Mk3) ---
            log("⚔️ TACTICAL PROTOCOL: Engaging Native Downloader Bypass (Mk3)...".to_string());

            let full_install_path = std::path::Path::new(&library_path).join("steamapps").join("common").join(&final_install_dir);
            let _install_dir_str = full_install_path.to_string_lossy().to_string();

            // Stats Collection for ACF
            let _installed_depots_data: Vec<(String, u64, String)> = Vec::new(); // (DepotID, Size, ManifestID)
            let _total_bytes_downloaded: u64 = 0;

            for (d_id, _d_key) in &keys {
                log(format!("⬇️ Processing Depot {}...", d_id));

                // 1. Locate Manifest (check multiple locations)
                let mut manifest_path = String::new();
                
                // 1a. Check Steam depotcache first (primary location)
                let depot_cache = std::path::Path::new(&steam_root).join("depotcache");
                if let Ok(entries) = std::fs::read_dir(&depot_cache) {
                    for entry in entries.flatten() {
                       let fname = entry.file_name().to_string_lossy().to_string();
                       // Pattern: {depot_id}_{manifest_id}.manifest
                       if fname.starts_with(d_id) && fname.ends_with(".manifest") {
                            manifest_path = entry.path().to_string_lossy().to_string();
                            log(format!("   - Found Manifest (depotcache): {}", fname));
                            break;
                       }
                    }
                }
                
                // 1b. Check Vault folder (cached from previous API downloads)
                if manifest_path.is_empty() {
                    let vault_dir = std::path::Path::new("Vault").join(&appid);
                    if vault_dir.exists() {
                        if let Ok(entries) = std::fs::read_dir(&vault_dir) {
                            for entry in entries.flatten() {
                               let fname = entry.file_name().to_string_lossy().to_string();
                               if fname.starts_with(d_id) && fname.ends_with(".manifest") {
                                    manifest_path = entry.path().to_string_lossy().to_string();
                                    log(format!("   - Found Manifest (Vault): {}", fname));
                                    break;
                               }
                            }
                        }
                    }
                }

                // NOTE: Per-depot API download REMOVED.
                // The API at /api/v1/manifest/ expects APP_ID, not DEPOT_ID.
                // Calling it with depot_id wastes tokens and returns wrong data.
                // All manifests should be included in the main app bundle.
                // If a manifest is missing here, it means:
                // 1. The app bundle from API doesn't include this depot
                // 2. Steam will need to fetch it during the download phase


                // SMD APPROACH: We do NOT download chunks ourselves.
                // Manifests are already in depotcache, keys are in config.vdf.
                // Steam will handle the actual download when the user clicks "Update" in library.
                if !manifest_path.is_empty() {
                    log(format!("📦 Manifest prepared for Depot {}: {}", d_id, manifest_path));
                } else {
                    log(format!("⚠️ No manifest found for Depot {}. Steam may need to fetch it.", d_id));
                }
            }
            log("✅ SMD-Style Preparation Complete. Manifests + Keys Ready.".to_string());
            log("   → Steam will download the game files when you click 'Update' in the Library.".to_string());

            // SMD APPROACH: We do NOT regenerate the ACF here.
            // The minimal ACF was already created at the start.
            // Steam will update it automatically during the download process.
            
            // NUKE SQUAD: Preemptively remove installscript.vdf if it exists in the game folder
            // This prevents Steam from triggering the "SteamService" install phase which often fails.
            {
                let script_path = full_install_path.join("installscript.vdf");
                if script_path.exists() {
                     if std::fs::remove_file(&script_path).is_ok() {
                         log("☢️ NUKE: installscript.vdf deleted to bypass SteamService error.".to_string());
                     }
                }
            }

            // VDF Injection (GreenLuma Override)
            // GreenLuma 2025 often uses its own config.vdf in its folder.
            if let Err(e) = inject_vdf(&gl_path, &keys) {
                 log(format!("GreenLuma VDF Warning (Non-Fatal): {}", e));
            } else {
                 log("✅ Depot Keys Injected into GreenLuma config.".to_string());
            }

            // Filter IDs - FIX: Always include ALL IDs from Lua (Depots + DLCs)
                // Filtering caused issues where required Depots were skipped if include_dlcs was false.
                for id in all_ids.iter() {
                    final_ids.push(id.clone());
                }
                 log(format!("Lua Intelligence: Found {} IDs (Game + Depots + DLCs).", final_ids.len()));
            } 
            // 3B. If Failed/Offline -> Use Public Store API (Smart Fallback)
            else {
                 log("Using Public Steam Store API for DLC detection...".to_string());
                 final_ids.push(appid.clone()); // Always add main game
                 
                 if include_dlcs {
                     match runtime.block_on(client.get_dlc_list(&appid)) {
                         Ok(dlcs) => {
                             if !dlcs.is_empty() {
                                 log(format!("Found {} DLCs from Steam Store.", dlcs.len()));
                                 final_ids.extend(dlcs);
                             } else {
                                 log("No DLCs found publicly.".to_string());
                             }
                         },
                         Err(_) => log("Could not fetch DLC list (Connection Error).".to_string())
                     }
                 }
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
            log(format!("STEP 3: Injecting {} IDs to AppList...", final_ids.len()));
            if let Err(e) = add_games_to_list(&gl_path, final_ids) {
                 log(format!("AppList Error: {}", e));
            } else {
                 log("AppList updated successfully.".to_string());
            }

             // Update Cache
             {
                if let Ok(mut cache) = game_cache.lock() {
                    cache.insert(appid.clone(), name.clone());
                    let _ = save_game_cache(&cache);
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
                                 log("✅ INJECTION SUCCESSFUL. Steam starting...".to_string());
                                 
                                 // PHASE 2: Wait for GreenLuma Initialization
                                 log("Waiting 5s for GreenLuma to unlock AppID...".to_string());
                                 std::thread::sleep(std::time::Duration::from_secs(5));
    
                                 // PHASE 3: Trigger Install Trigger
                                 // v1.3 Logic Restoration + v1.4 Refinement (StateFlags 4)
                                 // We use -applaunch because it's the native Steam method and v1.3 used it successfully.
                                 // The key is StateFlags=4 which prevents the "Unknown Error" loop.
                                 
                                 log("Triggering via -applaunch (v1.3 Style)...".to_string());
                                 let _ = std::process::Command::new(steam_exe)
                                     .arg("-applaunch")
                                     .arg(&appid)
                                     .spawn();

                                 log("✅ LAUNCH COMMAND SENT.".to_string());
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
                        egui::RichText::new("MANAGER v1.5.0")
                            .size(10.0)
                            .color(accent_pink)
                            .extra_letter_spacing(2.0),
                    );
                });
                
                ui.add_space(30.0);

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
                if response.clicked() || response.hovered() {
                       if self.active_tab != tab_idx {
                            self.active_tab = tab_idx;
                            self.tab_changed_at = Instant::now(); // Trigger Fade
                            if tab_idx == 2 {
                                self.refresh_library();
                            }
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
                    ui.ctx().request_repaint(); // Keep animating
                }

                // CONTENT
                match self.active_tab {
                    0 => self.ui_installation(ui),
                    // 1 was DRM INTEL - now integrated into Library per-game
                    2 => self.ui_library(ui),
                    // 3 was Profiles
                    4 => self.ui_settings(ui),
                    5 => self.ui_info(ui),
                    _ => self.ui_installation(ui),
                }
                
                // Global Footer Removed (Logs are now per-tab or sidebar)
                ui.add_space(5.0);
            });

        // POLL SCAN RESULT
        if self.is_scanning_dlcs {
            let mut res = self.dlc_scan_result.lock().unwrap();
            if let Some(data) = res.take() {
                self.delete_associated_dlcs = data;
                self.is_scanning_dlcs = false;
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
                    } else {
                        if !self.delete_associated_dlcs.is_empty() {
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
        }
        
        self.show_install_modal(ctx);
    }
}

impl DarkCoreApp {
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
                    egui::TextureOptions::default(),
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
            
            // MAJESTIC LAUNCH BUTTON
            let time = ui.input(|i| i.time);
            let pulse = (time * 3.0).sin().abs() as f32; // 0.0 to 1.0 rapid pulse
            
            // Animated Gold/Green Gradient logic (Approximated via pulsing fill)
            let fill_col = egui::Color32::from_rgba_premultiplied(
                0, 
                ((pulse * 30.0) + 40.0) as u8, 
                ((pulse * 10.0) + 20.0) as u8, 
                255
            );
            let text_col = egui::Color32::from_rgb(
                255, 
                ((pulse * 55.0) + 200.0) as u8, 
                ((pulse * 55.0) + 100.0) as u8
            ); 
            
            let btn_launch = egui::Button::new(
                egui::RichText::new("✨ LAUNCH GREENLUMA STEALTH")
                    .size(15.0) // Slightly larger
                    .color(text_col)
                    .strong()
            )
            .fill(fill_col)
            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 255, 100))) // Neon Green Border
            .rounding(6.0);
            
            // Force animation
            ui.ctx().request_repaint(); 

            if ui.add(btn_launch).on_hover_text("Initialize Stealth Injection Procedure (GreenLuma 2025)").clicked() {
                 let steam_path = self.config.steam_path.clone();
                 let gl_path = self.config.gl_path.clone();
                 let log_arc = self.system_log.clone();
                 let enable_stealth = self.config.enable_stealth_mode;

                 std::thread::spawn(move || {
                     let log = move |msg: String| {
                         if let Ok(mut logs) = log_arc.lock() {
                             logs.push(msg);
                         }
                     };
                     log("Manual Launch: Initiating Stealth Sequence (x64)...".to_string());
                     
     let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                     let dll_name = "GreenLuma_2025_x64.dll";
                     let dll_path = std::path::Path::new(&gl_path).join(dll_name);

                     if steam_exe.exists() {
                        if dll_path.exists() {
                             // FORCE KILL STEAM FIRST
                             let _ = std::process::Command::new("taskkill").args(&["/F", "/IM", "steam.exe"]).output();
                             std::thread::sleep(std::time::Duration::from_millis(1000));
                             
                             // SETUP CONFIG (Create .bin files in GL folder)
                             // Helper function is now public
                             let _ = crate::ui::setup_greenluma_config(&gl_path, enable_stealth);

                             // DIRECT INJECTION (No copying to Steam folder)
                             match crate::injector::launch_injected(
                                 steam_exe.to_str().unwrap_or(""),
                                 dll_path.to_str().unwrap_or(""), // Use DLL in GL folder
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
        });

        ui.add_space(5.0);
        ui.checkbox(
            &mut self.include_dlcs,
            egui::RichText::new("Include DLCs/Depots Automatically")
                .color(egui::Color32::LIGHT_GRAY),
        );
        ui.add_space(10.0);

        let search_results = self.search_results.clone();
        let results = search_results.lock().unwrap();

        let available = ui.available_height();
        let log_height = 200.0;
        let results_h = (available - log_height - 20.0).max(100.0);

        // Cache installed IDs for O(1) lookup
        let installed_ids: std::collections::HashSet<String> = {
            if let Ok(games) = self.active_games.lock() {
                games.iter().map(|g| g.app_id.clone()).collect()
            } else {
                std::collections::HashSet::new()
            }
        };

        egui::ScrollArea::vertical().id_salt("results_scroll").max_height(results_h).show(ui, |ui| {
            for res in results.iter() {
                use crate::api::val_to_string;
                let name = res.game_name.as_deref().or(res.name.as_deref()).unwrap_or("Unknown");
                let id1 = val_to_string(&res.game_id);
                let id2 = val_to_string(&res.app_id);
                let id = if !id1.is_empty() { id1 } else { id2 };
                let display_id = if id.is_empty() { "0".to_string() } else { id.clone() };
                let is_installed = installed_ids.contains(&display_id);

                // Animated Card Hover
                let card_id = ui.make_persistent_id(&display_id);
                let _is_hovered = ui.ctx().animate_bool(card_id, 
                     ui.input(|i| i.pointer.hover_pos().map_or(false, |_pos| {
                         false 
                     }))
                ); 

                ui.push_id(display_id.clone(), |ui| {
                    let frame_style = egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(30,30,40))
                        .inner_margin(8.0)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60,60,70)));
                        
                    // Draw Frame
                    let response = frame_style.show(ui, |ui| {
                             ui.horizontal(|ui| {
                                 // CALC DYNAMIC SIZE
                                 let avail_width = ui.ctx().screen_rect().width().max(800.0);
                                 let scale = (avail_width / 1200.0).max(1.0).min(3.0);
                                 let cover_w = 70.0 * scale;
                                 let cover_h = 100.0 * scale;

                                 // COVER IMAGE
                                 if !display_id.is_empty() && display_id != "0" {
                                     let cache = self.cover_cache.lock().unwrap();
                                     if let Some(Some(texture)) = cache.get(&display_id) {
                                         ui.add(egui::Image::new(texture).rounding(5.0 * scale).max_height(cover_h).max_width(cover_w));
                                     } else {
                                         ui.add(egui::Label::new("..."));
                                     }
                                 }

                             ui.vertical(|ui| {
                                 ui.label(egui::RichText::new("MANAGER v1.4").size(10.0).color(egui::Color32::from_rgb(100, 100, 120)));
                                 ui.label(egui::RichText::new(name).size(16.0).color(egui::Color32::WHITE).strong());
                                 ui.label(egui::RichText::new(format!("ID: {}", display_id)).size(10.0).color(egui::Color32::GRAY));
                                 ui.add_space(5.0);
                                 
                                 // PULSING BUTTON
                                 let mut needs_update = false;

                                 let mut is_dlc_linked = false;
                                 let mut parent_game_id = String::new();
                                 
                                 if is_installed {
                                     // Check Update Status
                                     if let Ok(cache) = self.update_cache.lock() {
                                         if let Some(upd) = cache.get(&display_id) {
                                             if *upd { needs_update = true; }

                                         }
                                     }
                                     // Check DLC Link
                                     if let Ok(rel) = self.relationships.lock() {
                                         if let Some(pid) = rel.get(&display_id) {
                                             is_dlc_linked = true;
                                             parent_game_id = pid.clone();
                                         }
                                     }
                                 }

                                 let text = if is_installed { 
                                     if is_dlc_linked { "🔗 DLC LINKED" }
                                     else if needs_update { "♻ UPDATE" }
                                     else { "▶ PLAY" } // Default to PLAY immediately, check in background
                                 } else { "🚀 INSTALL" };

                                 let time = ui.input(|i| i.time);
                                 let alpha = (time * 3.0).sin().abs() as f32 * 0.5 + 0.5; // 0.5 to 1.0
                                 
                                 let bg_color = if is_installed {
                                     if is_dlc_linked {
                                         // Passive Blue-Gray
                                         egui::Color32::from_rgb(50, 60, 75)
                                     } else if needs_update {
                                         // Orange/Yellow for Update
                                         egui::Color32::from_rgba_premultiplied(
                                             (255.0 * alpha) as u8, (140.0 * alpha) as u8, 0, 255
                                         )
                                     } else {
                                         // Green for Play (Solid)
                                         egui::Color32::from_rgb(0, 200, 100)
                                     }
                                 } else {
                                     // Green/Cyan for Install
                                     egui::Color32::from_rgba_premultiplied(
                                         0, (255.0 * alpha) as u8, (100.0 * alpha) as u8, 255
                                     )
                                 };
                                 
                                 let text_color = egui::Color32::BLACK;
                                 
                                 let limit_reached = self.active_games.lock().unwrap().len() >= 134;

                                 if limit_reached && !is_installed {
                                      ui.add(egui::Button::new(egui::RichText::new("⛔ LIMIT (134)").strong())
                                          .fill(egui::Color32::DARK_GRAY)
                                          .rounding(4.0))
                                          .on_hover_text("Max AppList limit reached. Create a Profile to install more.");
                                 } else {
                                      let btn = egui::Button::new(egui::RichText::new(text).color(text_color).strong())
                                         .fill(bg_color)
                                         .rounding(4.0);
                                      
                                      let mut btn_resp = ui.add(btn);
                                      if is_installed {
                                           if is_dlc_linked {
                                               btn_resp = btn_resp.on_hover_text(format!("Linked to Parent ID: {}. Launch the main game to play.", parent_game_id));
                                           } else {
                                               btn_resp = btn_resp.on_hover_text("Game is already installed. Right-click to Repair.");
                                           }
                                      }
                                      
                                      // Right-Click Context Menu
                                      btn_resp.context_menu(|ui| {
                                          let is_godmode_active = self.config.family_godmode_ids.contains(&display_id);

                                          if is_godmode_active {
                                              // GODMODE ACTIVE STATE
                                              ui.label(egui::RichText::new("⚡ FAMILY GODMODE ACTIVE").color(egui::Color32::GREEN).size(10.0));
                                              if ui.button(egui::RichText::new("💀 Disable Steam Family Godmode").color(egui::Color32::from_rgb(255, 100, 100))).clicked() {
                                                  ui.close_menu();
                                                  self.disable_family_godmode(display_id.clone());
                                              }
                                              // Hide "Force Repair" as requested
                                          } else {
                                              // STANDARD STATE
                                              if is_installed {
                                                  if ui.button("🛠 Force Repair (Regenerate ACF)").clicked() {
                                                      ui.close_menu();
                                                      // FORCE MODAL: User explicitly wants to repair
                                                      self.detected_libraries = crate::game_path::GamePathFinder::get_library_folders(&self.config.steam_path);
                                                      self.selected_library_index = 0;
                                                      self.install_candidate = Some((display_id.clone(), name.to_string()));
                                                      self.install_dir_input = name.to_string(); // Pre-fill
                                                      self.install_modal_open = true;
                                                  }
                                                  
                                                  // Offer Godmode Enable for installed games too (e.g. converting a family share title)
                                                  // Valid if API Key is present (if missing, Install button handles uninstalled, but installed ones need this)
                                                  // Actually godmode works without API Key using fallback fetch, so we should allow it always?
                                                  // User requirement: "launcher deve capire che il gioco selezionato è stato messo con Family Share mode"
                                                  // Implies we can PUT it there.
                                                  if ui.button(egui::RichText::new("👨‍👩‍👧 Enable Family Godmode (Unlock DLCs)").color(egui::Color32::from_rgb(100, 255, 255))).on_hover_text("Adds AppID + DLCs to GreenLuma.\nUseful for Family Shared games to unlock full content.").clicked() {
                                                       ui.close_menu();
                                                       self.install_game_family_godmode(display_id.clone());
                                                  }
                                              } else {
                                                  // UNINSTALLED
                                                  // Provide option explicitly
                                                  if ui.button(egui::RichText::new("👨‍👩‍👧 Install (Family Shared Godmode)").color(egui::Color32::from_rgb(100, 255, 255))).on_hover_text("Adds AppID + DLCs only. Skips file download.").clicked() {
                                                       ui.close_menu();
                                                       self.install_game_family_godmode(display_id.clone());
                                                  }
                                              }
                                          }
                                      });
                                      
                                      if btn_resp.clicked() {
                                            if is_dlc_linked {
                                                // Prevent action
                                                self.log(format!("DLC Content (Linked to {}). Please launch the base game.", parent_game_id));
                                            } else if !is_installed || needs_update {
                                                // Check if manifest exists (Automatic Resume)
                                               if let Some(path) = crate::game_path::GamePathFinder::find_manifest_path(&self.config.steam_path, &display_id) {
                                                   // Found it -> Resume/Update in place
                                                   self.install_game(display_id.clone(), name.to_string(), Some(path.parent().and_then(|p| p.parent()).unwrap_or(std::path::Path::new(&self.config.steam_path)).to_path_buf()), None);
                                               } else {
                                                   // 2. AUTO-DETECT FOLDER (Smart Scan)
                                                   // If we can find the folder, we skip the modal entirely.
                                                   let libraries = crate::game_path::GamePathFinder::get_library_folders(&self.config.steam_path);
                                                   let (found_dir, found_lib, confidence) = self.detect_auto_install_path(&name, &libraries);

                                                   if let Some(dir_name) = found_dir {
                                                       self.log(format!("Auto-Detected Install Dir: '{}' (Confidence: {:?})", dir_name, confidence));
                                                       // High confidence or exact match -> One-Click Install
                                                       self.install_game(display_id.clone(), name.to_string(), Some(found_lib.unwrap_or(std::path::Path::new(&self.config.steam_path).to_path_buf())), Some(dir_name));
                                                   } else {
                                                       // 3. FALLBACK -> MODAL (Fresh Install / No Trace Found)
                                                       // Pre-fill with sanitized name for user convenience
                                                       let sanitized = name.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().trim().to_string();
                                                       
                                                       if self.config.api_key.is_empty() {
                                                           // NO API KEY -> Family Godmode Warning/Offer? No, proceed to manual Install.
                                                            self.detected_libraries = libraries;
                                                            self.selected_library_index = 0;
                                                            self.install_candidate = Some((display_id.clone(), name.to_string()));
                                                            self.install_dir_input = sanitized; 
                                                            self.install_modal_open = true;
                                                       } else {
                                                           // STANDARD MODE -> Modal
                                                           self.detected_libraries = libraries;
                                                           self.selected_library_index = 0;
                                                           self.install_candidate = Some((display_id.clone(), name.to_string()));
                                                           self.install_dir_input = sanitized; 
                                                           self.install_modal_open = true;
                                                       }
                                                   }
                                               }
                                            } else {
                                               // SMART LAUNCH SYSTEM
                                               let steam_path = self.config.steam_path.clone();
                                               let gl_path = self.config.gl_path.clone();
                                               let app_id_run = display_id.clone();
                                               
                                               std::thread::spawn(move || {
                                                   let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                                                   
                                                   // 1. Check if Steam is running
                                                   let status_out = std::process::Command::new("tasklist")
                                                       .args(&["/FI", "IMAGENAME eq steam.exe", "/M", "GreenLuma_2025_x64.dll"])
                                                       .output();
                                                       
                                                   let mut is_running = false;
                                                   let mut is_injected = false;
                                                   
                                                   // Check generic running first
                                                   let run_check = std::process::Command::new("tasklist")
                                                        .args(&["/FI", "IMAGENAME eq steam.exe"])
                                                        .output();
                                                   if let Ok(o) = run_check {
                                                       let s = String::from_utf8_lossy(&o.stdout);
                                                       if s.contains("steam.exe") { is_running = true; }
                                                   }
                                                   
                                                   // Check injection
                                                   if let Ok(o) = status_out {
                                                       let s = String::from_utf8_lossy(&o.stdout);
                                                       if s.contains("steam.exe") { is_injected = true; }
                                                   }
                                                   
                                                   if is_running {
                                                       if is_injected {
                                                           // CASE A: Steam Running + GreenLuma -> Direct Launch
                                                           let _ = std::process::Command::new(steam_exe)
                                                               .arg("-applaunch")
                                                               .arg(&app_id_run)
                                                               .spawn();
                                                       } else {
                                                           // CASE B: Steam Running w/o GreenLuma -> RESTART REQUIRED (Automatic)
                                                           // Kill Steam
                                                           let _ = std::process::Command::new("taskkill").args(&["/F", "/IM", "steam.exe"]).output();
                                                           std::thread::sleep(std::time::Duration::from_millis(2000));
                                                           
                                                           // Launch Injected
                                                           let dll_path = std::path::Path::new(&gl_path).join("GreenLuma_2025_x64.dll");
                                                           let _ = crate::injector::launch_injected(
                                                               steam_exe.to_str().unwrap_or(""),
                                                               dll_path.to_str().unwrap_or(""),
                                                               Some(&format!("-applaunch {}", app_id_run))
                                                           );
                                                       }
                                                   } else {
                                                       // CASE C: Steam Closed -> Launch Injected
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
                                 }
                                 // Request repaint for animation
                                 ui.ctx().request_repaint();
                             });
                         });
                    });
                    
                    if response.response.hovered() {
                         ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
                ui.add_space(5.0);
            }
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
               if let Ok(mut logs) = log_arc.lock() { logs.push(msg); }
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
                if let Ok(mut logs) = log_arc.lock() { logs.push(msg); }
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
        
        ui.add_space(5.0);
        
        ui.label("Associated AppID (Optional, for Titan):");
        ui.horizontal(|ui| {
             ui.text_edit_singleline(&mut self.steamless_app_id); 
             ui.label("❓").on_hover_text("Enter the Steam AppID of this game to automatically deploy Titan Hook after unpacking.");
        });

        ui.add_space(5.0);
        ui.checkbox(&mut self.steamless_auto_titan, "Auto-Activate Titan (Hook + Cloud Patch)");

        ui.add_space(15.0);

        if ui.button(egui::RichText::new("UNPACK & PATCH").strong().size(16.0)).clicked() {
            if self.target_exe.is_empty() {
                self.log("Error: No executable selected.".to_string());
                return;
            }

            match steamless::run_steamless(&self.target_exe, &self.config.steamless_path) {
                Ok(msg) => {
                    self.log(msg);
                    // AUTO TITAN TRIGGER
                    if self.steamless_auto_titan && !self.steamless_app_id.is_empty() {
                        // We must clone because deploy mutates self and we are in a mutable borrow??
                        // Actually calling method on self inside match is fine if no conflict?
                        // `steamless::run_steamless` does not borrow self.
                        let appid = self.steamless_app_id.clone();
                        self.deploy_titan_auto(&appid);
                    }
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
                                         match self.profile_manager.load_profile(&name) {
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
                         {
                             if !self.active_profile_name.is_empty() {
                                 self.delete_profile_modal_open = true;
                             }
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
                          
                          if ui.button(egui::RichText::new("✅ CREATE & WIPE").strong().color(egui::Color32::RED)).clicked() {
                              if !self.profile_name_input.is_empty() {
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

            if ui
                .button(
                    egui::RichText::new("☢ NUKE UNKNOWNS")
                        .strong()
                        .color(egui::Color32::RED),
                )
                .on_hover_text("Smart Delete: Removes 'Unknown' items ONLY if they are NOT linked DLCs.\nSafe to use: If a game breaks, simply re-add its AppID.")
                .clicked()
            {
                let result = {
                    let cache = self.game_cache.lock().unwrap();
                    let rel = self.relationships.lock().unwrap();
                    crate::app_list::nuke_unknowns(&self.config.gl_path, &cache, &rel)
                };

                match result {
                    Ok(count) => {
                         self.log(format!("☢ NUKE COMPLETE: Vaporized {} junk files.", count));
                         self.refresh_library();
                    },
                    Err(e) => self.log(format!("Nuke Error: {}", e)),
                }
            }
            if ui
                .button(
                    egui::RichText::new("🔎 Resolve Unknown")
                        .strong()
                        .color(egui::Color32::YELLOW),
                )
                .clicked()
            {
                self.resolve_unknown_games();
            }
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

                                    // TITAN MODE CHECK
                                    let steam_path = self.config.steam_path.clone();
                                    
                                    // SKIP Steamless/Titan for Family Shared games
                                    let is_family_shared = self.config.family_godmode_ids.contains(&game.app_id);
                                    
                                    // Use helper methods from game_path.rs
                                    if !is_family_shared && crate::game_path::GamePathFinder::find_game_path(&steam_path, &game.app_id).is_some() {
                                        if crate::game_path::GamePathFinder::is_titan_active(&steam_path, &game.app_id) {
                                            ui.label(
                                                egui::RichText::new("✅ TITAN ACTIVE")
                                                    .color(egui::Color32::GREEN)
                                                    .size(10.0)
                                            ).on_hover_text("Titan Hook (version.dll) is deployed.");
                                        } else {
                                            let btn = ui.button(
                                                egui::RichText::new("⚔ ACTIVATE TITAN")
                                                    .color(egui::Color32::YELLOW)
                                                    .size(10.0)
                                            ).on_hover_text("Deploy Titan Hook (version.dll) for Cloud Save & Achievements.");
                                            
                                            if btn.clicked() {
                                                // KILL STEAM FIRST (Safety for VDF & File Locks)
                                                let _ = std::process::Command::new("taskkill")
                                                    .args(&["/F", "/IM", "steam.exe"])
                                                    .output();
                                                
                                                // Wait a moment for process death
                                                std::thread::sleep(std::time::Duration::from_millis(1000));

                                                match crate::game_path::GamePathFinder::deploy_titan_hook(&steam_path, &game.app_id) {
                                                    Ok(path) => {
                                                        self.log(format!("Titan deployed to: {:?}", path));
                                                        // Suppress Cloud Error (Safe now that Steam is dead)
                                                        match crate::game_path::GamePathFinder::suppress_cloud_sync(&steam_path, &game.app_id) {
                                                            Ok(_) => {
                                                                self.log("Cloud Sync Suppressed (localconfig patched).".to_string());
                                                                
                                                                // AUTO-RESTART STEAM via GreenLuma Injector
                                                                let steam_path = steam_path.clone(); // Capture from outer
                                                                let gl_path = self.config.gl_path.clone();
                                                                let log_arc = self.system_log.clone();

                                                                std::thread::spawn(move || {
                                                                    let log = move |msg: String| {
                                                                        if let Ok(mut logs) = log_arc.lock() {
                                                                            logs.push(msg);
                                                                        }
                                                                    };
                                                                    log("Titan/Restart: Initiating Stealth Sequence (x64)...".to_string());
                                                                    
                                                                let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                                                                    let dll_name = "GreenLuma_2025_x64.dll";
                                                                    let dll_path = std::path::Path::new(&gl_path).join(dll_name);
                
                                                                    if steam_exe.exists() && dll_path.exists() {
                                                                        // SYNC
                                                                        let target_dll = std::path::Path::new(&steam_path).join(dll_name);
                                                                        let _ = std::fs::copy(&dll_path, &target_dll);
                                                                        
                                                                        let src_applist = std::path::Path::new(&gl_path).join("AppList");
                                                                        let dst_applist = std::path::Path::new(&steam_path).join("AppList");
                                                                        if src_applist.exists() {
                                                                            let _ = std::fs::create_dir_all(&dst_applist);
                                                                            if let Ok(entries) = std::fs::read_dir(src_applist) {
                                                                               for entry in entries.flatten() {
                                                                                   if let Ok(ft) = entry.file_type() {
                                                                                       if ft.is_file() { let _ = std::fs::copy(entry.path(), dst_applist.join(entry.file_name())); }
                                                                                   }
                                                                               }
                                                                            }
                                                                        }

                                                                        match crate::injector::launch_injected(
                                                                            steam_exe.to_str().unwrap_or(""),
                                                                            target_dll.to_str().unwrap_or(""),
                                                                            Some("-inhibitbootstrap")
                                                                        ) {
                                                                            Ok(_) => log("✅ Restarted with GreenLuma.".to_string()),
                                                                            Err(e) => log(format!("❌ Restart Failed: {}", e)),
                                                                        }
                                                                    } else {
                                                                        log("❌ Missing files for restart.".to_string());
                                                                    }
                                                                });
                                                            },
                                                            Err(e) => self.log(format!("Cloud Suppression Warning: {}", e)),
                                                        }
                                                    },
                                                    Err(e) => self.log(format!("Titan Error: {}", e)),
                                                }
                                            }
                                        }
                                        
                                        // STEAMLESS AUTOMATION BUTTON
                                        let steamless_btn = ui.button(
                                            egui::RichText::new("⚡ STEAMLESS")
                                                .color(egui::Color32::from_rgb(255, 150, 0))
                                                .size(10.0)
                                        ).on_hover_text("Auto-patch all DRM-protected EXEs in game folder.\nGenerates steam_appid.txt for Titan.");
                                        
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
                                                                logs.push(msg);
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
        });
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
                        } else {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("exe", &["exe"])
                                .pick_file()
                            {
                                let p_str = path.to_string_lossy().to_string();
                                *txt = p_str.replace(r"\\?\", "");
                            }
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
                         if self.is_probable_dlc(&game.name) || candidate_clean.contains("pack") || candidate_clean.contains("content") || candidate_clean.contains("season") {
                            if !known_child_ids.contains(&game.app_id) {
                                known_child_ids.push(game.app_id.clone());
                            }
                         }
                     }
                 }
             }
        }

        // Spawn scan
        if let Some(client) = self.api_client.clone() {
            let app_id_clone = app_id.clone();
            let result_arc = self.dlc_scan_result.clone();
            let active_games_arc = self.active_games.clone();

            std::thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let found: Vec<String> = runtime.block_on(async {
                    match client.get_dlc_list(&app_id_clone).await {
                        Ok(dlcs) => dlcs,
                        Err(_) => vec![],
                    }
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

    fn remove_games_by_id(&self, ids: Vec<String>, full_wipe: bool) {
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
                 // SAFETY FIRST: Backup Manifests before any deletion
                 if let Ok(c) = vault.backup_manifests(&steam_path, id) {
                     if c > 0 { self.log(format!("Vault: Secured {} manifests for {} before deletion.", c, id)); }
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

    // Legacy: Called from old DRM INTEL tab
    #[allow(dead_code)]
    fn deploy_titan_auto(&mut self, app_id: &str) {
        let steam_path = self.config.steam_path.clone();
        
        self.log(format!("Auto-Deploying Titan for AppID: {}...", app_id));

        // 1. Kill Steam
        let _ = std::process::Command::new("taskkill")
            .args(&["/F", "/IM", "steam.exe"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // 2. Deploy Hook (DLL + AppID txt)
        match crate::game_path::GamePathFinder::deploy_titan_hook(&steam_path, app_id) {
            Ok(path) => {
                self.log(format!("Titan Hook deployed to: {:?}", path));

                // 3. Suppress Cloud
                match crate::game_path::GamePathFinder::suppress_cloud_sync(&steam_path, app_id) {
                    Ok(_) => self.log("Cloud Sync Suppressed.".to_string()),
                    Err(e) => self.log(format!("Cloud Suppression Warning: {}", e)),
                }

                // 4. Auto-Restart
                let steam_path = steam_path.clone();
                let gl_path = self.config.gl_path.clone();
                let log_arc = self.system_log.clone();

                std::thread::spawn(move || {
                     let log = move |msg: String| {
                         if let Ok(mut logs) = log_arc.lock() {
                             logs.push(msg);
                         }
                     };
                     log("Auto-Titan: Initiating Stealth Sequence (x64)...".to_string());
                     
                     let steam_exe = std::path::Path::new(&steam_path).join("steam.exe");
                     let dll_name = "GreenLuma_2025_x64.dll";
                     let dll_path = std::path::Path::new(&gl_path).join(dll_name);

                     if steam_exe.exists() && dll_path.exists() {
                        // SYNC
                        let target_dll = std::path::Path::new(&steam_path).join(dll_name);
                        let _ = std::fs::copy(&dll_path, &target_dll);

                        let src_applist = std::path::Path::new(&gl_path).join("AppList");
                        let dst_applist = std::path::Path::new(&steam_path).join("AppList");
                        if src_applist.exists() {
                            let _ = std::fs::create_dir_all(&dst_applist);
                            if let Ok(entries) = std::fs::read_dir(src_applist) {
                               for entry in entries.flatten() {
                                   if let Ok(ft) = entry.file_type() {
                                       if ft.is_file() { let _ = std::fs::copy(entry.path(), dst_applist.join(entry.file_name())); }
                                   }
                               }
                            }
                        }

                        match crate::injector::launch_injected(
                            steam_exe.to_str().unwrap_or(""),
                            target_dll.to_str().unwrap_or(""),
                            Some("-inhibitbootstrap")
                        ) {
                            Ok(_) => log("✅ Auto-Titan Injected.".to_string()),
                            Err(e) => log(format!("❌ Auto-Titan Failed: {}", e)),
                        }
                     }
                });
            },
            Err(e) => self.log(format!("Titan Deployment Failed: {}", e)),
        }
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
                 let len = 10 + (rand_pseudo(seed + 4) % 40) as usize;
                 
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

    std::fs::write(&acf_path, content)?;
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

    // Create the game directory in steamapps/common (Steam expects this to exist)
    if let Some(parent) = acf_path.parent() {
        let common_dir = parent.join("common");
        let game_dir = common_dir.join(&install_dir_sanitized);
        if !game_dir.exists() {
            let _ = std::fs::create_dir_all(&game_dir);
        }
    }

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

    std::fs::write(&acf_path, content)?;
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
    } else {
        if stealth_bin.exists() {
             let _ = std::fs::remove_file(&stealth_bin);
        }
    }

    // GreenLuma INI Configuration (Analysis Mode)
    // To solve "Unknown Error", we need logs from the DLL.
    let ini_path = path.join("GreenLuma_2025_x64.ini");
    // LogFile=1 -> Creates GreenLuma_2025_x64.log
    // ModifyLauncher=1 -> Standard hook
    let ini_content = "[GreenLuma]\nLogFile=1\nModifyLauncher=1\n";
    let _ = std::fs::write(&ini_path, ini_content);
    
    Ok(())
}
