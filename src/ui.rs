use crate::api::{ApiClient, SearchResult};
use crate::app_list::{
    add_games_to_list, nuke_reorder, overwrite_app_list, refresh_active_games_list, GameProfile,
};
use crate::cache::{load_game_cache, save_game_cache};
use crate::config::{load_config, save_config, AppConfig};
use crate::profiles::{Profile, ProfileManager};
use crate::steamless;
use crate::vdf_injector::{inject_vdf, parse_lua_for_keys};
use eframe::egui;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

use std::time::{Duration, Instant};

pub struct DarkCoreApp {
    config: AppConfig,
    active_tab: usize,

    // UI Variables
    search_query: String,
    last_searched_query: String,
    last_input_time: Option<Instant>,
    search_results: Arc<Mutex<Vec<SearchResult>>>,
    active_games: Arc<Mutex<Vec<GameProfile>>>,
    game_cache: Arc<Mutex<HashMap<String, String>>>,

    // Steamless
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

    // Smart Delete
    delete_modal_open: bool,
    delete_candidate_id: Option<String>,
    delete_candidate_name: Option<String>,
    delete_associated_dlcs: Vec<String>,
    is_scanning_dlcs: bool,
    dlc_scan_result: Arc<Mutex<Option<Vec<String>>>>,
}

impl DarkCoreApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = load_config();

        // Load cache
        let cache_map = load_game_cache();

        // Always initialize client; it handles empty keys via Fallback to Steam Store API.
        let api_client = Some(ApiClient::new(config.api_key.clone()));

        let system_log = Arc::new(Mutex::new(Vec::new()));
        // Initial log
        system_log
            .lock()
            .unwrap()
            .push("System Ready. Darkcore Rust Initialized.".to_string());

        let mut app = Self {
            config,
            active_tab: 0,
            search_query: String::new(),
            last_searched_query: String::new(),
            last_input_time: None,
            search_results: Arc::new(Mutex::new(Vec::new())),
            active_games: Arc::new(Mutex::new(Vec::new())),
            game_cache: Arc::new(Mutex::new(cache_map)),
            target_exe: String::new(),
            include_dlcs: true,
            status_msg: "System Ready".to_string(),
            system_log,
            cover_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cover_queue: Arc::new(Mutex::new(Vec::new())),
            api_client,
            profile_manager: ProfileManager::new("."),
            profile_name_input: String::new(),
            active_profile_name: "Default".to_string(),
            delete_modal_open: false,
            delete_candidate_id: None,
            delete_candidate_name: None,
            delete_associated_dlcs: Vec::new(),
            is_scanning_dlcs: false,
            dlc_scan_result: Arc::new(Mutex::new(None)),
        };

        app.configure_visuals(&_cc.egui_ctx);

        // Install image loaders
        egui_extras::install_image_loaders(&_cc.egui_ctx);

        app.refresh_library();
        app.resolve_unknown_games();
        app
    }

    fn configure_visuals(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();

        // Cyberpunk Palette
        let bg_color = egui::Color32::from_rgb(5, 5, 10); // Deep Black/Blue
        let surface_color = egui::Color32::from_rgb(20, 20, 25);
        let accent_green = egui::Color32::from_rgb(0, 255, 150); // Neon Green
        let accent_cyan = egui::Color32::from_rgb(0, 200, 255);
        let text_color = egui::Color32::from_rgb(220, 220, 220);

        visuals.window_fill = bg_color;
        visuals.panel_fill = bg_color;

        // Widgets
        visuals.widgets.noninteractive.bg_fill = bg_color;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_color);

        visuals.widgets.inactive.bg_fill = surface_color;
        visuals.widgets.inactive.weak_bg_fill = surface_color;
        visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(140));

        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 40, 50);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, accent_green);
        visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);

        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 60, 70);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, accent_cyan);
        visuals.widgets.active.rounding = egui::Rounding::same(4.0);

        // Selection
        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 100, 200);
        visuals.selection.stroke = egui::Stroke::new(1.0, accent_cyan);

        ctx.set_visuals(visuals);
    }

    fn log<S: Into<String>>(&self, msg: S) {
        let msg = msg.into();
        if let Ok(mut logs) = self.system_log.lock() {
            logs.push(msg);
            // Keep last 50 lines
            if logs.len() > 50 {
                logs.remove(0);
            }
        }
    }

    fn refresh_library(&mut self) {
        if self.config.gl_path.is_empty() {
            return;
        }
        let gl_path = self.config.gl_path.clone();

        let cache_lock = self.game_cache.lock().unwrap();
        let cache_snapshot = cache_lock.clone();
        drop(cache_lock);

        let target = self.active_games.clone();
        let steam_path = self.config.steam_path.clone();

        // Blocking for simplicity or could be async
        let games = refresh_active_games_list(&gl_path, &steam_path, &cache_snapshot);
        let mut target_guard = target.lock().unwrap();
        *target_guard = games;
    }

    fn perform_search(&self) {
        if let Some(client) = &self.api_client {
            if self.search_query.is_empty() {
                return;
            }
            let query = self.search_query.clone();
            let results_arc = self.search_results.clone();
            let client_key = self.config.api_key.clone();
            let cover_queue = self.cover_queue.clone();
            let cover_cache = self.cover_cache.clone();
            let log_arc = self.system_log.clone();

            self.log(&format!("Searching for: {}", query));
            results_arc.lock().unwrap().clear();
            cover_cache.lock().unwrap().clear();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let client = ApiClient::new(client_key);

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

                            // 4. Alphabetical
                            name_a.cmp(&name_b)
                        });

                        *results_arc.lock().unwrap() = res.clone();

                        // Download Covers
                        let dl_client = reqwest::Client::builder()
                            .danger_accept_invalid_certs(true)
                            .user_agent("DarkCore/10.4-Rust")
                            .build()
                            .unwrap_or_default();

                        // Block to spawn and wait for all downloads
                        rt.block_on(async {
                            let mut handles = Vec::new();

                            for item in res {
                                 let id1 = crate::api::val_to_string(&item.game_id);
                                 let id2 = crate::api::val_to_string(&item.app_id);
                                 let appid = if !id1.is_empty() { id1 } else { id2 };
                                 
                                 if !appid.is_empty() && appid != "0" {
                                     let queue = cover_queue.clone();
                                     let appid_clone = appid.clone();
                                     let dl_client = dl_client.clone();
                                     let log_arc_inner = log_arc.clone();
                                     
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
                                         
                                         // 2. Try Landscape (Header) if Portrait failed
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
                                         
                                         // 3. Fallback to Placeholder if both failed
                                         if !success {
                                             // Generate a 1x1 dark gray pixel to clear "Loading..."
                                             // or a small 60x90 placeholder
                                             let w = 60;
                                             let h = 90;
                                             let mut pixels = Vec::with_capacity((w * h * 4) as usize);
                                             for _ in 0..(w*h) {
                                                 // r, g, b, a (Dark Gray/Blue)
                                                 pixels.push(30); pixels.push(30); pixels.push(40); pixels.push(255);
                                             }
                                             if let Ok(mut q) = queue.lock() {
                                                  q.push((appid_clone.clone(), w, h, pixels));
                                             }
                                         }
                                     }));
                                 }
                            }
                            
                            // Wait for all downloads to finish before Runtime drops
                            for h in handles {
                                let _ = h.await; 
                            }
                        });
                    }
                    Err(e) => {
                        let _ = log_arc
                            .lock()
                            .unwrap()
                            .push(format!("Search API Error: {}", e));
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
        let client = ApiClient::new(client_key.clone());

        self.status_msg = "Resolving unknown games...".to_string();

        std::thread::spawn(move || {
            let mut ids_to_resolve = Vec::new();

            // Identify unknowns
            {
                let games = active_games.lock().unwrap();
                for g in games.iter() {
                    if g.name == "Unknown" {
                        ids_to_resolve.push(g.app_id.clone());
                    }
                }
            }

            let runtime = tokio::runtime::Runtime::new().unwrap();

            runtime.block_on(async {
                let mut handles = Vec::new();

                for id in ids_to_resolve {
                    let client = ApiClient::new(client_key.clone());
                    let game_cache = game_cache.clone();
                    let id_clone = id.clone();

                    handles.push(tokio::spawn(async move {
                        let mut found_name = None;

                        // 1. Try Morrenus Search first
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

                        // 2. Fallback: Steam Store API
                        if found_name.is_none() {
                            // Try fetching from official Steam API
                            let url = format!(
                                "https://store.steampowered.com/api/appdetails?appids={}",
                                id_clone
                            );
                            if let Ok(resp) = reqwest::get(&url).await {
                                if let Ok(json) = resp.json::<serde_json::Value>().await {
                                    // Path: [id].data.name
                                    if let Some(data) =
                                        json.get(&id_clone).and_then(|v| v.get("data"))
                                    {
                                        if let Some(name_val) = data.get("name") {
                                            if let Some(n) = name_val.as_str() {
                                                found_name = Some(n.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 3. Save if found
                        if let Some(name) = found_name {
                            if let Ok(mut cache) = game_cache.lock() {
                                cache.insert(id_clone.clone(), name.clone());
                                let _ = save_game_cache(&cache); // Save to disk immediately
                            }
                        }
                    }));
                }

                // Wait for all requests to finish
                for h in handles {
                    let _ = h.await;
                }
            });
        });
    }

    fn install_game(&mut self, appid: String, name: String) {
        // UNIFIED PROTOCOL: Works both Online (Manifests) and Offline (FamSharing/Public) through Fallbacks.
        let api_key = self.config.api_key.clone(); // Can be empty
        let client = ApiClient::new(api_key.clone()); 

        let steam_path = self.config.steam_path.clone();
        let gl_path = self.config.gl_path.clone();

        let game_cache = self.game_cache.clone();
        let include_dlcs = self.include_dlcs;
        let log_arc = self.system_log.clone();

        self.status_msg = format!("START: Protocol for {}", name);
        self.log(&format!(
            "Starting installation protocol for: {} ({})",
            name, appid
        ));

        std::thread::spawn(move || {
            let log = |msg: String| {
                if let Ok(mut logs) = log_arc.lock() {
                    logs.push(msg);
                    if logs.len() > 100 {
                        logs.remove(0);
                    }
                }
            };
            
            // STEP 1: Kill Steam
            log("STEP 1: Killing Steam Process...".to_string());
            use std::process::Command;
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "steam.exe"])
                .output();
            std::thread::sleep(std::time::Duration::from_secs(2));

            // STEP 2: TRY MANIFEST (Priority)
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let mut manifest_success = false;
            let mut lua_content = String::new();

            // Only attempt manifest download if we have a key (saves a request) OR we assume empty key fails?
            // Morrenus API definitely needs key.
            if !api_key.is_empty() {
                log(format!("STEP 2: Downloading Manifest for ID {}...", appid));
                match runtime.block_on(client.download_manifest(&appid)) {
                    Ok(bytes) => {
                        log("Download successful. Extracting...".to_string());
                        let reader = Cursor::new(bytes);
                        if let Ok(mut zip) = ZipArchive::new(reader) {
                            let depot_dir = Path::new(&steam_path).join("depotcache");
                            if !depot_dir.exists() {
                                 let _ = std::fs::create_dir_all(&depot_dir);
                            }
                            for i in 0..zip.len() {
                                if let Ok(mut file) = zip.by_index(i) {
                                    let raw_path = file.name().to_string();
                                    if raw_path.ends_with(".manifest") {
                                         if let Some(fname) = Path::new(&raw_path).file_name() {
                                             let out_path = depot_dir.join(fname);
                                             if let Ok(mut outfile) = std::fs::File::create(out_path) {
                                                  let _ = std::io::copy(&mut file, &mut outfile);
                                             }
                                         }
                                    } else if raw_path.ends_with(".lua") {
                                         use std::io::Read;
                                         let _ = file.read_to_string(&mut lua_content);
                                    }
                                }
                            }
                            manifest_success = true;
                        }
                    },
                    Err(_) => {
                        log("Manifest download failed (Invalid Key or Server Error). Skipping to Fallback...".to_string());
                    }
                }
            } else {
                 log("OFFLINE MODE: Skipping Manifest Download (No API Key).".to_string());
            }

            // STEP 3: PREPARE IDs (Hybrid)
            let mut final_ids = Vec::new();

            // 3A. If Manifest/Lua success -> Use Lua IDs (Best)
            if manifest_success && !lua_content.is_empty() {
                let (all_ids, keys) = parse_lua_for_keys(&lua_content);
                // VDF Injection
                if let Err(e) = inject_vdf(&steam_path, &keys) {
                    log(format!("VDF Error: {}", e));
                }
                // Filter IDs
                for id in all_ids.iter() {
                    if include_dlcs || *id == appid {
                        final_ids.push(id.clone());
                    }
                }
                 log(format!("Lua Intelligence: Found {} associated IDs.", final_ids.len()));
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

            // STEP 5: DLL INJECTOR & RESTART
            log("STEP 4: Converting Environment...".to_string());
            let injector_path = Path::new(&gl_path).join("DLLInjector.exe");
            if injector_path.exists() {
                 if let Ok(mut _child) = Command::new(&injector_path).current_dir(&gl_path).spawn() {
                     log("Injector launched. Waiting 15s...".to_string());
                     std::thread::sleep(std::time::Duration::from_secs(15));
                 }
            }

            log("STEP 5: Launching Installation...".to_string());
            let _ = open::that(format!("steam://install/{}", appid));
            log("Protocol Complete. Happy Gaming!".to_string());
        });
    }
}

impl eframe::App for DarkCoreApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Custom styling for Main Container
        let main_frame = egui::containers::Frame::default()
            .inner_margin(20.0)
            .fill(egui::Color32::from_rgb(5, 5, 10));

        // 0. CONFIG WARNING
        if self.config.steam_path.is_empty() || self.config.gl_path.is_empty() {
            egui::TopBottomPanel::top("warning").frame(egui::containers::Frame::default().fill(egui::Color32::RED)).show(ctx, |ui| {
                 ui.centered_and_justified(|ui| {
                      ui.label(egui::RichText::new("⚠️ SYSTEM WARNING: CONFIGURATION INCOMPLETE. PLEASE SET PATHS IN SETTINGS TAB.").color(egui::Color32::WHITE).strong());
                 });
             });
        }

        egui::TopBottomPanel::top("header")
            .frame(main_frame)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("// D4RKC0R3_MANAGER_v10.4")
                            .color(egui::Color32::from_rgb(0, 255, 100))
                            .size(28.0)
                            .family(egui::FontFamily::Monospace)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("[ SYSTEM_STATUS :: PURIFIED ]")
                            .size(12.0)
                            .family(egui::FontFamily::Monospace)
                            .color(egui::Color32::from_rgb(0, 200, 255)),
                    );

                    if self.config.api_key.is_empty() {
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("⚠ FALLBACK MODE: STEAM FAMILY SHARE ONLY (NO API KEY)")
                                .size(10.0)
                                .color(egui::Color32::ORANGE)
                                .strong(),
                        )
                        .on_hover_text("Morrenus API Key is missing. Manifests cannot be downloaded.\nOnly Family Shared games and Public DLCs are available.\nAdd a key in Settings for full unlocking power.");
                    }
                    ui.add_space(5.0);
                });
                ui.separator();
                ui.add_space(5.0);
            });

        egui::TopBottomPanel::bottom("status")
            .min_height(100.0)
            .frame(main_frame)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("SYSTEM LOG")
                            .font(egui::FontId::proportional(14.0))
                            .color(egui::Color32::from_rgb(0, 200, 255)),
                    );

                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(120.0)
                        .show(ui, |ui| {
                            let logs = self.system_log.lock().unwrap();
                            for line in logs.iter() {
                                ui.label(
                                    egui::RichText::new(line)
                                        .font(egui::FontId::monospace(12.0))
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                );
                            }
                        });
                });
            });

        egui::CentralPanel::default()
            .frame(main_frame.inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 20.0;
                    let tab_btn = |ui: &mut egui::Ui, label: &str, active: bool| {
                        let color = if active {
                            egui::Color32::from_rgb(0, 255, 100)
                        } else {
                            egui::Color32::GRAY
                        };
                        let text = egui::RichText::new(label).color(color).size(16.0).strong();
                        if ui.add(egui::Button::new(text).frame(false)).clicked() {
                            return true;
                        }
                        false
                    };

                    if tab_btn(ui, "🚀 INSTALL", self.active_tab == 0) {
                        self.active_tab = 0;
                    }
                    if tab_btn(ui, "🔍 DRM ANALYZER", self.active_tab == 1) {
                        self.active_tab = 1;
                    }
                    if tab_btn(ui, "📂 LIBRARY", self.active_tab == 2) {
                        self.active_tab = 2;
                        self.refresh_library();
                    }
                    if tab_btn(ui, "📂 PROFILES", self.active_tab == 3) {
                        self.active_tab = 3;
                    }
                    if tab_btn(ui, "⚙️ SETTINGS", self.active_tab == 4) {
                        self.active_tab = 4;
                    }
                });
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                match self.active_tab {
                    0 => self.ui_installation(ui),
                    1 => self.ui_drm(ui),
                    2 => self.ui_library(ui),
                    3 => self.ui_profiles(ui),
                    4 => self.ui_settings(ui),
                    _ => self.ui_installation(ui),
                }
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
                            if ui
                                .button(
                                    egui::RichText::new("CONFIRM DELETE").color(egui::Color32::RED),
                                )
                                .clicked()
                            {
                                // EXECUTE DELETE
                                let mut to_delete = vec![self.delete_candidate_id.clone().unwrap()];
                                to_delete.extend(self.delete_associated_dlcs.iter().cloned());

                                self.remove_games_by_id(to_delete);

                                self.delete_modal_open = false;
                                self.refresh_library();
                            }
                        }
                    });
                });
        }
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

    fn ui_installation(&mut self, ui: &mut egui::Ui) {
        self.process_cover_queue(ui.ctx()); // Process queue here
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            for res in results.iter() {
                use crate::api::val_to_string;
                let name = res.game_name.as_deref().or(res.name.as_deref()).unwrap_or("Unknown");
                let id1 = val_to_string(&res.game_id);
                let id2 = val_to_string(&res.app_id);
                let id = if !id1.is_empty() { id1 } else { id2 };
                let display_id = if id.is_empty() { "0".to_string() } else { id.clone() };

                // Animated Card Hover
                let card_id = ui.make_persistent_id(&display_id);
                let is_hovered = ui.ctx().animate_bool(card_id, 
                     ui.input(|i| i.pointer.hover_pos().map_or(false, |pos| {
                         // We don't have rect yet, simple hack: 
                         // Just use "interact" on the frame response below
                         false // We'll set it after
                     }))
                ); // Getting hover before drawing is tricky in immediate mode without 2-pass.
                // Simpler: Just rely on standard sense(Sense::hover()) on the frame.

                ui.push_id(display_id.clone(), |ui| {
                    let frame_style = egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(30,30,40))
                        .inner_margin(8.0)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60,60,70)));
                        
                    // Draw Frame
                    let response = frame_style.show(ui, |ui| {
                             ui.horizontal(|ui| {
                                 // CALC DYNAMIC SIZE
                                 // 1200px -> 80px width
                                 // 2560px -> 180px width
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
                                 ui.label(egui::RichText::new(name).size(16.0).color(egui::Color32::WHITE).strong());
                                 ui.label(egui::RichText::new(format!("ID: {}", display_id)).size(10.0).color(egui::Color32::GRAY));
                                 ui.add_space(5.0);
                                 
                                 // PULSING INSTALL BUTTON
                                 let text = "🚀 INSTALL";
                                 let time = ui.input(|i| i.time);
                                 let alpha = (time * 3.0).sin().abs() as f32 * 0.5 + 0.5; // 0.5 to 1.0
                                 let color = egui::Color32::from_rgba_premultiplied(
                                     0, (255.0 * alpha) as u8, (100.0 * alpha) as u8, 255
                                 );
                                 let text_color = egui::Color32::BLACK;
                                 
                                 let limit_reached = self.active_games.lock().unwrap().len() >= 134;

                                 if limit_reached {
                                      ui.add(egui::Button::new(egui::RichText::new("⛔ LIMIT (134)").strong())
                                          .fill(egui::Color32::DARK_GRAY)
                                          .rounding(4.0))
                                          .on_hover_text("Max AppList limit reached. Create a Profile to install more.");
                                 } else {
                                      let btn = egui::Button::new(egui::RichText::new(text).color(text_color).strong())
                                         .fill(color)
                                         .rounding(4.0);
                                      
                                      if ui.add(btn).clicked() {
                                           self.install_game(display_id.clone(), name.to_string());
                                      }
                                 }
                                 // Request repaint for animation
                                 ui.ctx().request_repaint();
                             });
                         });
                    });
                    
                    // Simple Hover Effect (Brighten border)
                    if response.response.hovered() {
                         ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                         // Since we already drew it, we can't change it this frame easy.
                         // But next frame it will redraw. 
                         // Actually eframe clears every frame. 
                         // To do hover style, we should calculate style BEFORE show.
                         // But Frame::show returns Response AFTER.
                         // Standard Egui pattern: UI is stateful.
                    }
                });
                ui.add_space(5.0);
            }
        });
    }

    fn ui_drm(&mut self, ui: &mut egui::Ui) {
        ui.label("Steamless CLI Unpacker");
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
        if ui.button("Patch DRM").clicked() {
            match steamless::run_steamless(&self.target_exe, &self.config.steamless_path) {
                Ok(msg) => self.log(msg),
                Err(e) => self.log(format!("Error: {}", e)),
            }
        }
    }

    fn ui_library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new("🔄 Refresh").strong())
                .clicked()
            {
                self.refresh_library();
            }
            if ui
                .button(
                    egui::RichText::new("☢ Nuke & Reorder")
                        .strong()
                        .color(egui::Color32::RED),
                )
                .clicked()
            {
                if let Err(e) = nuke_reorder(&self.config.gl_path, &self.config.steam_path, None) {
                    self.log(format!("Error: {}", e));
                } else {
                    self.log("Library Reordered (Safe verification).".to_string());
                    self.refresh_library();
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
        ui.add_space(10.0);

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

        let mut path_row =
            |ui: &mut egui::Ui, label: &str, valid: bool, txt: &mut String, is_dir: bool| {
                ui.label(label);
                ui.horizontal(|ui| {
                    let tint = if valid {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };
                    ui.add(
                        egui::TextEdit::singleline(txt)
                            .desired_width(400.0)
                            .text_color(egui::Color32::WHITE),
                    );
                    if ui.button("📂").clicked() {
                        if is_dir {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                *txt = path.to_string_lossy().to_string();
                            }
                        } else {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("exe", &["exe"])
                                .pick_file()
                            {
                                *txt = path.to_string_lossy().to_string();
                            }
                        }
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
        );
        path_row(
            ui,
            "GreenLuma Path:",
            Path::new(&self.config.gl_path).exists(),
            &mut self.config.gl_path,
            true,
        );
        path_row(
            ui,
            "Steamless CLI Path:",
            Path::new(&self.config.steamless_path).exists(),
            &mut self.config.steamless_path,
            false,
        );

        ui.separator();
        ui.label("API Key (Morrenus):");
        ui.text_edit_singleline(&mut self.config.api_key);

        ui.add_space(15.0);
        if ui
            .button(
                egui::RichText::new("💾 SAVE CONFIGURATION")
                    .color(egui::Color32::BLACK)
                    .strong(),
            )
            .highlight()
            .clicked()
        {
            if let Err(e) = save_config(&self.config) {
                self.status_msg = format!("Save error: {}", e);
            } else {
                self.status_msg = "Config saved.".to_string();
                self.api_client = Some(ApiClient::new(self.config.api_key.clone()));
                self.refresh_library();
                self.resolve_unknown_games();
            }
        }
    }

    fn ui_profiles(&mut self, ui: &mut egui::Ui) {
        ui.heading("PROFILES MANAGER");
        ui.separator();

        ui.add_space(10.0);

        // 1. CREATE NEW
        ui.group(|ui| {
            ui.label(
                egui::RichText::new("SAVE CURRENT LIBRARY AS PROFILE")
                    .strong()
                    .color(egui::Color32::from_rgb(0, 255, 100)),
            );
            ui.horizontal(|ui| {
                ui.label("Profile Name:");
                ui.text_edit_singleline(&mut self.profile_name_input);
                if ui.button("SAVE").clicked() {
                    if !self.profile_name_input.is_empty() {
                        // Gather IDs
                        use crate::app_list::GameProfile;
                        let games = self.active_games.lock().unwrap();
                        let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
                        drop(games);

                        let p = Profile {
                            name: self.profile_name_input.clone(),
                            app_ids: ids,
                        };

                        if let Err(e) = self.profile_manager.save_profile(&p) {
                            self.log(format!("Error saving profile: {}", e));
                        } else {
                            self.log(format!("Profile '{}' saved successfully.", p.name));
                            self.profile_name_input.clear();
                        }
                    }
                }
            });
        });

        ui.add_space(20.0);

        // 2. LIST
        ui.label(egui::RichText::new("YOUR PROFILES").strong().size(18.0));
        let profiles = self.profile_manager.list_profiles();

        if profiles.is_empty() {
            ui.label("No profiles found.");
        }

        for name in profiles {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&name).size(16.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(egui::RichText::new("🗑 DELETE").color(egui::Color32::RED))
                            .clicked()
                        {
                            if let Err(e) = self.profile_manager.delete_profile(&name) {
                                self.log(format!("Error deleting profile: {}", e));
                            } else {
                                self.log(format!("Profile '{}' deleted.", name));
                            }
                        }

                        if ui
                            .button(egui::RichText::new("⚡ LOAD").color(egui::Color32::YELLOW))
                            .clicked()
                        {
                            // LOAD LOGIC
                            match self.profile_manager.load_profile(&name) {
                                Ok(p) => {
                                    use crate::app_list::overwrite_app_list;
                                    if let Err(e) =
                                        overwrite_app_list(&self.config.gl_path, p.app_ids)
                                    {
                                        self.log(format!("Error applying profile: {}", e));
                                    } else {
                                        self.log(format!(
                                            "Profile '{}' loaded! Library updated.",
                                            name
                                        ));
                                        self.refresh_library();
                                        self.active_profile_name = name.clone();
                                    }
                                }
                                Err(e) => self.log(format!("Error loading profile: {}", e)),
                            }
                        }
                    });
                });
            });
        }
    }

    fn initiate_delete(&mut self, app_id: String, name: String) {
        self.delete_modal_open = true;
        self.delete_candidate_id = Some(app_id.clone());
        self.delete_candidate_name = Some(name);
        self.delete_associated_dlcs.clear();
        self.is_scanning_dlcs = true;

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

                let associated: Vec<String> = found
                    .into_iter()
                    .filter(|id| installed_ids.contains(id))
                    .collect();

                *result_arc.lock().unwrap() = Some(associated);
            });
        } else {
            self.is_scanning_dlcs = false;
        }
    }

    fn remove_games_by_id(&self, ids: Vec<String>) {
        let gl_path = self.config.gl_path.clone();
        let al_path = Path::new(&gl_path).join("AppList");

        if let Ok(paths) = glob::glob(&al_path.join("*.txt").to_string_lossy()) {
            for path in paths.flatten() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if ids.contains(&content.trim().to_string()) {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
        self.log(format!("Deleted {} items.", ids.len()));
    }
}
