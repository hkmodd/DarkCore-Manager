use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::api::ApiClient;
use crate::cache::load_game_cache;
use crate::config::load_config;
use crate::direct_download::state::DownloadState;
use crate::profiles::ProfileManager;
use crate::ui::helpers::download_manifests_wudrm;
use crate::ui::state::push_log;
use crate::ui::state::DarkCoreApp;

pub fn create_app(cc: &eframe::CreationContext<'_>) -> DarkCoreApp {
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
        push_log(
            &mut logs,
            "System Ready. Darkcore Rust Initialized.".to_string(),
        );
    }

    let initial_profile = config.last_active_profile.clone();
    let initial_api_key = config.api_key.clone();

    let mut app = DarkCoreApp {
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
        download_method_modal_open: false,
        download_state: Arc::new(Mutex::new(DownloadState::new())),
        pending_install: None,
        dlc_scan_result_zip: Arc::new(Mutex::new(None)),

        // Manifestor Init
        manifestor_open: false,
        manifestor_data: Arc::new(Mutex::new(None)),
        manifestor_candidate_id: None,
        manifestor_candidate_name: String::new(),
        manifestor_target_library: None,
        manifestor_install_name: String::new(),
        manifestor_selections: Vec::new(),
        
        // FIX 8: Library search filter
        library_search_query: String::new(),
        
        // FIX 4: Auto-scan flag
        install_modal_auto_scanned: false,

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
        
        // Family Shared vs Download choice modal
        family_or_download_modal_open: false,

        logo_texture: None,
        logo_data: {
            // EMBEDDED LOGO (Compile-time check)
            // Relative to manager/src/ui/app.rs -> manager/logo.png requires ../../
            let bytes = include_bytes!("../../logo.png");
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
        request_api_refresh: Arc::new(Mutex::new(false)),
        matrix_trails: Vec::new(),
        api_key_glitch_cache: String::new(),
        api_key_glitch_update: Instant::now(),
        config_saved_at: None,
        api_refresh_timer: if !initial_api_key.is_empty() {
            Some(Instant::now() + std::time::Duration::from_millis(500))
        } else {
            None
        }, // Auto-Start

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

        // Import ZIP Feature (Phase 3A)
        import_zip_data: None,
        import_modal_open: false,

        // Hover Card Details (Phase 17)
        hover_start_time: None,
        hover_details_cache: Arc::new(Mutex::new(HashMap::new())),
        hover_loading: Arc::new(Mutex::new(HashSet::new())),
        show_detail_popup: None,

        // Premium Hover Animation (Phase 18)
        card_hover_scale: HashMap::new(),
        card_rects: HashMap::new(),
        popup_fade_alpha: 0.0,
    };

    // Initialize Audio Thread
    if let Ok((stream, handle)) = rodio::OutputStream::try_default() {
        if let Ok(sink) = rodio::Sink::try_new(&handle) {
            // Load embedded track (Obfuscated as system data)
            // Relative from src/ui/app.rs -> src/../core_data -> ../../core_data
            let bytes = include_bytes!("../../core_data/sys_audio_01.dat");
            let cursor = std::io::Cursor::new(bytes);
            if let Ok(source) = rodio::Decoder::new(cursor) {
                use rodio::Source; // Import Source trait
                sink.append(source.repeat_infinite());
                sink.set_volume(0.02);
                sink.play();

                app._audio_stream = Some(stream);
                app._audio_stream_handle = Some(handle);
                app.audio_sink = Some(sink);
            }
        }
    }

    configure_visuals(&cc.egui_ctx);

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
                                                appid = line[prev + 1..start].to_string();
                                            }
                                        }
                                    }
                                    if line.contains("\"StateFlags\"") {
                                        if let Some(start) = line.rfind("\"") {
                                            if let Some(prev) = line[..start].rfind("\"") {
                                                if let Ok(flags) =
                                                    line[prev + 1..start].parse::<u32>()
                                                {
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
                                    let _ =
                                        download_manifests_wudrm(&appid, &steam_path_clone, &|s| {
                                            println!("[WUDRM] {}", s)
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // Install image loaders
    egui_extras::install_image_loaders(&cc.egui_ctx);

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

pub fn configure_visuals(ctx: &egui::Context) {
    // FORCE DARK MODE - Override system theme completely
    ctx.set_visuals(egui::Visuals::dark());

    let mut style = (*ctx.style()).clone();

    // FONT SIZES
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            egui::FontId::new(24.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            egui::FontId::new(14.0, egui::FontFamily::Monospace),
        ),
        (
            egui::TextStyle::Button,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
        ),
    ]
    .into();

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

impl DarkCoreApp {
    /// Refreshes the active games list from GreenLuma config and local cache.
    pub fn refresh_library(&mut self) {
        let gl_path = self.config.gl_path.clone();

        let cache_lock = self.game_cache.lock().unwrap();
        let cache_snapshot = cache_lock.clone();
        drop(cache_lock);

        let rel_lock = self.relationships.lock().unwrap();
        let rel_snapshot = rel_lock.clone();
        drop(rel_lock);

        let target = self.active_games.clone();
        let steam_path = self.config.steam_path.clone();
        let games = crate::app_list::refresh_active_games_list(
            &gl_path,
            &steam_path,
            &cache_snapshot,
            &rel_snapshot,
        );

        // Collect IDs for update checking
        let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();

        let mut target_guard = target.lock().unwrap();
        *target_guard = games;

        // Trigger Update Check
        // We defer this because check_updates_for_ids is in ui/watcher.rs
        // And we cannot call it easily if it's a standalone function taking &DarkCoreApp
        // UNLESS we import it.
        // But if we are inside impl DarkCoreApp, we can't call standalone methods as self methods unless defined.
        // Wait, check_updates_for_ids IS standalone in ui/watcher.rs.
        crate::ui::watcher::check_updates_for_ids(self, ids);
    }
}

impl DarkCoreApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        create_app(cc)
    }

    pub(crate) fn perform_search(&mut self) {
        // Since we moved this method, we need access to fields.
        // Most fields are directly accessible via &self.
        // Logic copied from ui_old.rs
        if let Some(_client) = &self.api_client {
            if self.search_query.is_empty() {
                return;
            }
            // CRITICAL FIX: Update last_searched_query to prevent infinite loop
            self.last_searched_query = self.search_query.clone();

            let query = self.search_query.clone();
            let results_arc = self.search_results.clone();
            let active_games = self.active_games.clone();
            let update_cache = self.update_cache.clone();
            let steam_path = self.config.steam_path.clone();
            
            let client_key = self.config.api_key.clone();
            let cover_queue = self.cover_queue.clone();
            let cover_cache = self.cover_cache.clone();
            let log_arc = self.system_log.clone();
            let user_stats_arc = self.user_stats.clone(); 
            let refresh_arc = self.request_api_refresh.clone();

            // Use self.log helper? We just added it to state.rs, but is it visible?
            // Yes, if we import DarkCoreApp.
            // But self.log calls lock().
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

                let search_res = rt.block_on(client.search(&query));

                // Always request a fresh stats update after search, just in case
                if let Ok(mut req) = refresh_arc.lock() { *req = true; }

                match search_res {
                    Ok(mut res) => {
                        res.sort_by(|a, b| {
                             let name_a = a.game_name.as_deref().or(a.name.as_deref()).unwrap_or("").to_lowercase();
                             let name_b = b.game_name.as_deref().or(b.name.as_deref()).unwrap_or("").to_lowercase();
                             let q = query.to_lowercase();
                             let exact_a = name_a == q;
                             let exact_b = name_b == q;
                             if exact_a != exact_b { return exact_b.cmp(&exact_a); }
                             let starts_a = name_a.starts_with(&q);
                             let starts_b = name_b.starts_with(&q);
                             if starts_a != starts_b { return starts_b.cmp(&starts_a); }
                             let len_a = name_a.len();
                             let len_b = name_b.len();
                             if len_a != len_b { return len_a.cmp(&len_b); }
                             name_a.cmp(&name_b)
                        });

                        if let Ok(mut results) = results_arc.lock() {
                            *results = res.clone();
                        }

                        let dl_client = reqwest::Client::builder()
                            .danger_accept_invalid_certs(true)
                            .user_agent("DarkCore/10.4-Rust")
                            .build()
                            .unwrap_or_default();

                        rt.block_on(async {
                            let mut handles = Vec::new();
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
                                     let tiny_image = item.tiny_image.clone(); // Capture from search result
                                     
                                     handles.push(tokio::spawn(async move {
                                         // 1. Get Base URLs from covers.rs
                                         let mut urls = crate::ui::covers::get_cover_urls(&appid_clone);
                                         
                                         // 2. Insert Steam Store API fallback (if exists) 
                                         // Priority: 600x900 -> Header -> *StoreAPI* -> Capsule -> Logo
                                         if let Some(t) = tiny_image {
                                             if !t.is_empty() {
                                                 urls.insert(2, t);
                                             }
                                         }
                                         
                                         let mut success = false;
                                         for url in urls {
                                             if let Ok(resp) = dl_client.get(&url).send().await { 
                                                 if resp.status().is_success() { 
                                                     if let Ok(b) = resp.bytes().await { 
                                                         if let Ok(img) = image::load_from_memory(&b) { 
                                                             let img = img.to_rgba8(); 
                                                             if let Ok(mut q) = queue.lock() { 
                                                                 q.push((appid_clone.clone(), img.width(), img.height(), img.into_raw())); 
                                                                 success = true; 
                                                                 break; // Found one!
                                                             } 
                                                         } 
                                                     } 
                                                 } 
                                             }
                                         }
                                         
                                         // 3. Last Resort: Colored Placeholder
                                         if !success { 
                                             let (w, h, pixels) = crate::ui::covers::generate_placeholder(&appid_clone);
                                             if let Ok(mut q) = queue.lock() { 
                                                 q.push((appid_clone.clone(), w, h, pixels)); 
                                             } 
                                         }
                                     }));
                                     
                                     if installed.contains(&appid) {
                                          let client = client.clone();
                                          let cache = update_cache.clone();
                                          let sp = steam_path.clone();
                                          let aid = appid.clone();
                                          handles.push(tokio::spawn(async move {
                                               let acf = std::path::Path::new(&sp).join("steamapps").join(format!("appmanifest_{}.acf", aid));
                                               let mut flags = 0u32;
                                               if acf.exists() { if let Ok(c) = std::fs::read_to_string(&acf) { if let Some(p) = c.find("\"StateFlags\"") { let rem = &c[p+12..]; if let Some(s) = rem.find("\"") { if let Some(e) = rem[s+1..].find("\"") { flags = rem[s+1..s+1+e].parse().unwrap_or(0); } } } } }
                                               if (flags & 4) != 0 { if let Ok(mut c) = cache.lock() { c.insert(aid, false); } return; }
                                               if let Ok(st) = client.get_status(&aid).await { 
                                                    let mut n = st.needs_update.unwrap_or(false);
                                                    if (flags & 4) != 0 { n = false; }
                                                    if let Ok(mut c) = cache.lock() { c.insert(aid, n); }
                                               }
                                          }));
                                     }
                                 }
                            }
                            for h in handles { let _ = h.await; }
                        });
                        
                        if let Ok(s) = rt.block_on(client.get_user_stats()) { if let Ok(mut guard) = user_stats_arc.lock() { *guard = Some(s); } }
                    }
                    Err(e) => {
                         if let Ok(mut logs) = log_arc.lock() { push_log(&mut logs, format!("Search API Error: {}", e)); }
                    }
                }
            });
        }
    }

    pub fn install_game(&mut self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>) {
        crate::ui::install_logic::install_game(self, appid, name, target_library, install_dir_name);
    }

    #[allow(dead_code)]
    fn legacy_install_game(&mut self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>) {
        crate::ui::install_logic::legacy_install_game(self, appid, name, target_library, install_dir_name);
    }

    pub fn finalize_installation(&mut self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>, selected_dlcs: Vec<String>, cached_zip: Option<Vec<u8>>, hierarchy: Option<crate::api::GameHierarchy>) {
        crate::ui::install_logic::finalize_installation(self, appid, name, target_library, install_dir_name, selected_dlcs, cached_zip, hierarchy);
    }

    pub fn spawn_direct_install(&self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>, selected_dlcs: Vec<String>, cached_zip: Option<Vec<u8>>, hierarchy: Option<crate::api::GameHierarchy>) {
        crate::ui::install_logic::spawn_direct_install(self, appid, name, target_library, install_dir_name, selected_dlcs, cached_zip, hierarchy);
    }

    pub fn spawn_steam_install(&self, appid: String, name: String, target_library: Option<std::path::PathBuf>, install_dir_name: Option<String>, selected_dlcs: Vec<String>, cached_zip: Option<Vec<u8>>, hierarchy: Option<crate::api::GameHierarchy>) {
        crate::ui::install_logic::spawn_steam_install(self, appid, name, target_library, install_dir_name, selected_dlcs, cached_zip, hierarchy);
    }

    pub fn relaunch_steam_protocol(&self) {
        crate::ui::helpers::relaunch_steam_protocol(self);
    }

    pub fn is_probable_dlc(&self, name: &str) -> bool {
        crate::ui::helpers::is_probable_dlc(name)
    }

    #[allow(dead_code)] // Fallback utility, auto-scan logic moved inline
    pub fn detect_auto_install_path(&self, game_name: &str, libraries: &[std::path::PathBuf]) -> (Option<String>, Option<std::path::PathBuf>, String) {
        crate::ui::helpers::detect_auto_install_path(game_name, libraries)
    }

    pub fn remove_games_by_id(&self, ids: Vec<String>, full_wipe: bool) {
        crate::ui::helpers::remove_games_by_id(self, ids, full_wipe);
    }

    pub fn initiate_delete(&mut self, app_id: String, name: String) {
        crate::ui::modals::delete::initiate_delete(self, app_id, name);
    }
    
    pub fn install_game_family_godmode(&mut self, appid: String) {
        crate::ui::install_logic::install_game_family_godmode(self, appid);
    }

    pub fn disable_family_godmode(&mut self, appid: String) {
        crate::ui::install_logic::disable_family_godmode(self, appid);
    }

    pub fn handle_import_zip(&mut self, path: std::path::PathBuf) {
        crate::ui::modals::import_zip::handle_import_zip(self, path);
    }

    pub fn process_cover_queue(&mut self, ctx: &egui::Context) {
        let mut queue_guard = self.cover_queue.lock().unwrap();
        if queue_guard.is_empty() { return; }

        let count = queue_guard.len().min(5);
        let items: Vec<_> = queue_guard.drain(0..count).collect();
        drop(queue_guard);

        if let Ok(mut cache) = self.cover_cache.lock() {
            for (appid, w, h, pixels) in items {
                let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                let texture = ctx.load_texture(
                    format!("cover_{}", appid),
                    image,
                    egui::TextureOptions::LINEAR,
                );
                cache.insert(appid, Some(texture));
            }

            // CLEANUP: Prevent unbounded growth
            if cache.len() > crate::ui::state::MAX_COVER_CACHE_SIZE {
                let overflow = cache.len() - crate::ui::state::MAX_COVER_CACHE_SIZE;
                let keys_to_remove: Vec<_> = cache.keys().take(overflow).cloned().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }
        ctx.request_repaint();
    }
}

impl eframe::App for DarkCoreApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // --- GLOBAL API REFRESH LOGIC ---
        // Check for signals from background threads
        let mut trigger_refresh = false;
        if let Ok(mut req) = self.request_api_refresh.lock() {
            if *req {
                *req = false;
                trigger_refresh = true;
            }
        }

        // If signal received, set timer for immediate refresh (debounce 500ms? No, immediate)
        if trigger_refresh {
            self.api_refresh_timer = Some(Instant::now());
        }

        // Check if timer expired
        if let Some(timer) = self.api_refresh_timer {
            if Instant::now() > timer {
                self.api_refresh_timer = None; // Reset
                self.helper_refresh_api_stats();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        
        crate::ui::render::render(self, ctx, frame);
    }
}

impl DarkCoreApp {
    /// Spawns background thread to refresh user stats
    pub fn helper_refresh_api_stats(&mut self) {
        if self.config.api_key.is_empty() { return; }

        let stats_arc = self.user_stats.clone();
        let status_queue = self.status_update_queue.clone();
        let error_arc = self.api_last_error.clone();
        let validating_arc = self.is_validating_api.clone();
        let cfg_key = self.config.api_key.clone();
        
        // Set VALIDATING flag immediately
        if let Ok(mut v) = self.is_validating_api.lock() { *v = true; }

        std::thread::spawn(move || {
            let client = crate::api::ApiClient::new(cfg_key);
            let result = crate::ui::state::ASYNC_RUNTIME.block_on(client.get_user_stats());
            
            // Clear Validating Flag
            if let Ok(mut v) = validating_arc.lock() { *v = false; }
            
            match result {
                Ok(stats) => {
                    if let Ok(mut e) = error_arc.lock() { *e = None; }
                    if let Ok(mut s) = stats_arc.lock() { *s = Some(stats); }
                    if let Ok(mut q) = status_queue.lock() {
                        *q = Some("API Stats Refreshed.".to_string());
                    }
                },
                Err(e) => {
                    let err_str = e.to_string();
                    if let Ok(mut er) = error_arc.lock() { *er = Some(err_str.clone()); }
                    // Only log error if it's strictly an API failure, not just network blip
                    // But we want to know if it fails.
                }
            }
        });
        self.log("Refreshing API Stats...".to_string());
    }
}
