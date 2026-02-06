//! INSTALLATION panel (Home/Search tab).
//!
//! This panel handles:
//! - Search input for games
//! - Importing ZIP files
//! - Displaying search results in a responsive grid
//! - Handling "Install" and "Play" actions

use eframe::egui;
use std::time::{Duration, Instant};


/// Renders the MAIN INSTALLATION / SEARCH tab.
///
/// # Arguments
/// * `app` - Mutable reference to the application state
/// * `ui` - Mutable reference to the egui UI context (usually ctx_ui from CentralPanel)
pub fn render(app: &mut crate::ui::state::DarkCoreApp, ctx_ui: &mut egui::Ui) {
    // Process cover download queue
    app.process_cover_queue(ctx_ui.ctx());

    // MAIN CONTENT
    // We reuse the existing CentralPanel logic if possible, or just render content if already inside one.
    // In ui_old.rs, it creates a new CentralPanel inside the tab.
    // But Render is called FROM a CentralPanel in render_central_panel usually? 
    // Wait, let's check ui_old.rs:2959. 
    // It says `0 => self.ui_installation(ui),`. 
    // And `ui_installation` signature is `fn ui_installation(&mut self, ctx_ui: &mut egui::Ui)`.
    // Inside it does `egui::CentralPanel::default().show_inside(ctx_ui, |ui| { ... })`.
    // This seems redundant if we are ALREADY in a CentralPanel?
    // Let's check `render_central_panel` in `ui_old.rs`.
    // It says `egui::CentralPanel::default().show(ctx, |ui| { ... match active_tab ... })`.
    // So if `ui_installation` creates *another* CentralPanel, it's nested.
    // `show_inside` creates a panel inside the parent UI. That's fine.
    
    egui::CentralPanel::default().show_inside(ctx_ui, |ui| {
        ui.label(
            egui::RichText::new("SEARCH & AUTOMATION")
                .color(egui::Color32::from_rgb(0, 200, 255)) // app.accent_color?
                .strong(),
        );
        ui.add_space(5.0);

        // SEARCH BAR ROW
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut app.search_query)
                    .min_size(egui::vec2(200.0, 25.0))
                    .hint_text("Enter Game Name...")
                    .font(egui::FontId::proportional(14.0)),
            );

            // Debounce Logic
            if response.changed() {
                app.last_input_time = Some(Instant::now());
                
                // FIX 5: Reset to empty state when search is cleared
                if app.search_query.trim().is_empty() {
                    if let Ok(mut results) = app.search_results.lock() {
                        results.clear();
                    }
                    app.last_searched_query.clear();
                }
            }

            if let Some(last_time) = app.last_input_time {
                if last_time.elapsed() > Duration::from_millis(500) {
                    if app.search_query != app.last_searched_query && !app.search_query.trim().is_empty() {
                        app.perform_search();
                    }
                    app.last_input_time = None;
                } else {
                    ui.ctx().request_repaint();
                }
            }

            // Search Button
            if ui
                .button(egui::RichText::new("🔍 SEARCH").size(14.0))
                .clicked()
            {
                app.perform_search();
                app.last_input_time = None;
            }

            // Import ZIP Button (Phase 3A)
            if ui
                .button(egui::RichText::new("📂 IMPORT ZIP").size(14.0))
                .on_hover_text("Import a Morrenus ZIP file manually")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Morrenus ZIP", &["zip"])
                    .pick_file()
                {
                    app.handle_import_zip(path);
                }
            }
        });

        ui.add_space(5.0);
        
        // NOTE: "Include DLCs/Depots" checkbox removed - DLC Picker handles selection now
        
        // NOISE FILTER CHECKBOX
        ui.checkbox(
            &mut app.show_free_content, 
            egui::RichText::new("Show Free/Demo Content").color(egui::Color32::from_gray(140))
        )
        .on_hover_text("If unchecked, hides Free-to-Play games to reduce noise.\nChecking this allows installing free games without GreenLuma injection.");
        
        ui.add_space(10.0);

        // GRID VISUALIZATION
        // We need to clone the search results out of the mutex
        let search_results = app.search_results.clone();
        let results = search_results.lock().unwrap();

        let available = ui.available_height();
        let results_h = available.max(100.0);

        // Cache installed IDs for O(1) lookup
        let installed_ids: std::collections::HashSet<String> = {
            if let Ok(games) = app.active_games.lock() {
                games.iter().map(|g| g.app_id.clone()).collect()
            } else {
                std::collections::HashSet::new()
            }
        };

        egui::ScrollArea::vertical().id_salt("results_scroll").max_height(results_h).show(ui, |ui| {
            let avail_width = ui.available_width();
            
            // RESPONSIVE GRID CALCULATION
            let min_card_width = 180.0_f32;  
            let spacing = 6.0_f32;           
            
            let cols = ((avail_width + spacing) / (min_card_width + spacing)).floor().max(1.0) as usize;
            let card_w = (avail_width - (spacing * (cols as f32 - 1.0))) / cols as f32;
            
            let cover_h = card_w * 1.5;  // 2:3 ratio
            let info_h = 75.0;           
            let _card_h = cover_h + info_h;

            egui::Grid::new("results_grid_manual")
                .spacing(egui::vec2(spacing, spacing))
                .min_col_width(card_w)
                .show(ui, |ui| {
                    let hovered_appid = app.show_detail_popup.clone();

                    for (i, res) in results.iter().enumerate() {
                        // NOISE FILTER LOGIC
                        if !app.show_free_content && res.is_free {
                            continue;
                        }
                        
                        use crate::api::val_to_string;
                        let name = res.game_name.as_deref().or(res.name.as_deref()).unwrap_or("Unknown");
                        let id1 = val_to_string(&res.game_id);
                        let id2 = val_to_string(&res.app_id);
                        let id = if !id1.is_empty() { id1 } else { id2 };
                        let display_id = if id.is_empty() { "0".to_string() } else { id.clone() };
                        let is_installed = installed_ids.contains(&display_id);
                        let is_free = res.is_free;

                        let is_this_card_hovered = hovered_appid.as_ref().map(|h| h == &display_id).unwrap_or(false);
    
                        // === SCALE ANIMATION ===
                        let target_scale = if is_this_card_hovered { 1.08 } else { 1.0 };
                        let current_scale = app.card_hover_scale.entry(display_id.clone()).or_insert(1.0);
                        // Smooth lerp
                        *current_scale += (target_scale - *current_scale) * 0.4; // FAST transition
                        let scale = *current_scale;
                        
                        // Calculate scaled dimensions
                        let base_card_w = card_w;
                        let base_card_h = cover_h + info_h;
                        let scaled_card_w = base_card_w * scale;
                        let scaled_card_h = base_card_h * scale;
                        
                        // Request repaint for smooth animation
                        if (scale - target_scale).abs() > 0.001 {
                            ui.ctx().request_repaint();
                        }
                        
                        // Allocate space (use base size for layout, scale visually)
                        let (card_rect, response) = ui.allocate_exact_size(
                            egui::vec2(base_card_w, base_card_h),
                            egui::Sense::click()
                        );
                        
                        // Save card rect for popup anchoring
                        app.card_rects.insert(display_id.clone(), (
                            card_rect.min.x,
                            card_rect.min.y, 
                            card_rect.width(),
                            card_rect.height()
                        ));
                        
                        // Calculate scaled rect (centered scaling)
                        let scale_offset_x = (scaled_card_w - base_card_w) / 2.0;
                        let scale_offset_y = (scaled_card_h - base_card_h) / 2.0;
                        let visual_rect = egui::Rect::from_min_size(
                            egui::pos2(card_rect.min.x - scale_offset_x, card_rect.min.y - scale_offset_y),
                            egui::vec2(scaled_card_w, scaled_card_h)
                        );
                        
                        // Choose painter - foreground for hovered card to prevent clipping
                        let painter = if is_this_card_hovered {
                            ui.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Foreground,
                                egui::Id::new("hovered_card_layer")
                            ))
                        } else {
                            ui.painter().clone()
                        };

                        // === GLOW EFFECT (when hovered) ===
                        if is_this_card_hovered {
                            let glow_color = egui::Color32::from_rgba_unmultiplied(0, 200, 255, 50);
                            let glow_rect = visual_rect.expand(10.0);
                            painter.rect_filled(glow_rect, 14.0, glow_color);
                            
                            let inner_glow = egui::Color32::from_rgba_unmultiplied(0, 200, 255, 80);
                            painter.rect_filled(visual_rect.expand(4.0), 10.0, inner_glow);
                        }
                        
                        // === CARD BACKGROUND ===
                        let bg_color = if is_this_card_hovered {
                            egui::Color32::from_rgb(35, 40, 50)
                        } else {
                            egui::Color32::from_rgb(28, 30, 38)
                        };
                        painter.rect_filled(visual_rect, 8.0, bg_color);
                        
                        // === CARD BORDER ===
                        let border_color = if is_this_card_hovered {
                            egui::Color32::from_rgb(0, 220, 255)
                        } else {
                            egui::Color32::from_rgb(50, 55, 65)
                        };
                        painter.rect_stroke(visual_rect, 8.0, egui::Stroke::new(
                            if is_this_card_hovered { 2.5 } else { 1.0 },
                            border_color
                        ));
                        
                        // === COVER IMAGE ===
                        let cover_rect = egui::Rect::from_min_size(
                            visual_rect.min,
                            egui::vec2(scaled_card_w, cover_h * scale)
                        );
                        
                        if let Ok(cache) = app.cover_cache.lock() {
                            if let Some(Some(texture)) = cache.get(&display_id) {
                                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                painter.image(texture.id(), cover_rect, uv, egui::Color32::WHITE);
                            } else {
                                // Placeholder
                                painter.rect_filled(cover_rect, 8.0, egui::Color32::from_rgb(40, 42, 54));
                            }
                        }
                        
                        // === DLC BADGE ===
                        if is_free {
                            let badge_rect = egui::Rect::from_min_size(
                                visual_rect.min + egui::vec2(6.0 * scale, 6.0 * scale),
                                egui::vec2(28.0 * scale, 16.0 * scale)
                            );
                            painter.rect_filled(badge_rect, 4.0, egui::Color32::from_rgb(80, 200, 80));
                            painter.text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "DLC",
                                egui::FontId::proportional(9.0 * scale),
                                egui::Color32::BLACK
                            );
                        }
                        
                        // === GAME NAME ===
                        let name_pos = egui::pos2(
                            visual_rect.min.x + 6.0 * scale,
                            visual_rect.min.y + cover_h * scale + 4.0 * scale
                        );
                        let truncated_name = if name.len() > 28 {
                            format!("{}...", &name[..25])
                        } else {
                            name.to_string()
                        };
                        painter.text(
                            name_pos,
                            egui::Align2::LEFT_TOP,
                            &truncated_name,
                            egui::FontId::proportional(11.0 * scale),
                            egui::Color32::WHITE
                        );
                        
                        // === APP ID ===
                        let id_pos = egui::pos2(
                            visual_rect.min.x + 6.0 * scale,
                            visual_rect.min.y + cover_h * scale + 20.0 * scale
                        );
                        painter.text(
                            id_pos,
                            egui::Align2::LEFT_TOP,
                            &display_id,
                            egui::FontId::monospace(9.0 * scale),
                            egui::Color32::from_gray(100)
                        );
                        
                        // === INSTALL/PLAY BUTTON ===
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                visual_rect.min.x + 4.0 * scale,
                                visual_rect.max.y - 28.0 * scale
                            ),
                            egui::vec2(scaled_card_w - 8.0 * scale, 24.0 * scale)
                        );
                        
                        let btn_color = if is_installed {
                            egui::Color32::from_rgb(0, 200, 100)
                        } else {
                            egui::Color32::from_rgb(0, 220, 150)
                        };
                        
                        painter.rect_filled(btn_rect, 4.0, btn_color);
                        
                        let btn_text = if is_installed { "PLAY" } else { "INSTALL" };
                        let btn_icon = if is_installed { ">" } else { "+" };
                        painter.text(
                            btn_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{} {}", btn_icon, btn_text),
                            egui::FontId::proportional(11.0 * scale),
                            egui::Color32::BLACK
                        );
                        
                        // === HOVER LOGIC ===
                        let card_hovered = response.hovered();
                        if card_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        
                        // Hover timing logic
                        if card_hovered {
                            match &app.hover_start_time {
                                Some((hovered_id, start_time)) if hovered_id == &display_id => {
                                    let is_cached = app.hover_details_cache.lock().unwrap().contains_key(&display_id);
                                    let duration = if is_cached {
                                        std::time::Duration::from_millis(50)
                                    } else {
                                        std::time::Duration::from_millis(150)
                                    };
                                    
                                    if start_time.elapsed() >= duration {
                                        app.show_detail_popup = Some(display_id.clone());
                                        
                                        // Trigger fetch if needed
                                        let already_loading = app.hover_loading.lock().unwrap().contains(&display_id);
                                        
                                        if !is_cached && !already_loading {
                                            app.hover_loading.lock().unwrap().insert(display_id.clone());
                                            
                                            let appid_clone = display_id.clone();
                                            let cache = app.hover_details_cache.clone();
                                            let loading = app.hover_loading.clone();
                                            
                                            std::thread::spawn(move || {
                                                let rt = tokio::runtime::Runtime::new().unwrap();
                                                rt.block_on(async {
                                                    let client = reqwest::Client::new();
                                                    let url = format!(
                                                        "https://store.steampowered.com/api/appdetails?appids={}&l=english",
                                                        appid_clone
                                                    );
                                                    
                                                    if let Ok(resp) = client.get(&url).send().await {
                                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                            if let Some(data) = json.get(&appid_clone).and_then(|v| v.get("data")) {
                                                                let details = crate::api::GameDetails {
                                                                    app_id: appid_clone.clone(),
                                                                    name: data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                                    short_description: data.get("short_description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                                    developers: data.get("developers")
                                                                        .and_then(|v| v.as_array())
                                                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                                                        .unwrap_or_default(),
                                                                    publishers: Vec::new(),
                                                                    genres: data.get("genres")
                                                                        .and_then(|v| v.as_array())
                                                                        .map(|arr| arr.iter()
                                                                            .filter_map(|v| v.get("description").and_then(|d| d.as_str()).map(String::from))
                                                                            .collect())
                                                                        .unwrap_or_default(),
                                                                    release_date: data.get("release_date")
                                                                        .and_then(|v| v.get("date"))
                                                                        .and_then(|v| v.as_str())
                                                                        .unwrap_or("")
                                                                        .to_string(),
                                                                    metacritic_score: data.get("metacritic")
                                                                        .and_then(|v| v.get("score"))
                                                                        .and_then(|v| v.as_u64())
                                                                        .map(|v| v as u32),
                                                                    recommendations: data.get("recommendations")
                                                                        .and_then(|v| v.get("total"))
                                                                        .and_then(|v| v.as_u64()),
                                                                    platforms: (
                                                                        data.get("platforms").and_then(|v| v.get("windows")).and_then(|v| v.as_bool()).unwrap_or(false),
                                                                        data.get("platforms").and_then(|v| v.get("mac")).and_then(|v| v.as_bool()).unwrap_or(false),
                                                                        data.get("platforms").and_then(|v| v.get("linux")).and_then(|v| v.as_bool()).unwrap_or(false),
                                                                    ),
                                                                    required_age: data.get("required_age").and_then(|v| v.as_u64()).map(|v| v as u8).unwrap_or(0),
                                                                };
                                                                cache.lock().unwrap().insert(appid_clone.clone(), details);
                                                            }
                                                        }
                                                    }
                                                    loading.lock().unwrap().remove(&appid_clone);
                                                });
                                            });
                                        }
                                    }
                                }
                                _ => {
                                    // Reset immediato della card precedente
                                    if let Some((old_id, _)) = &app.hover_start_time {
                                        if old_id != &display_id {
                                            // Nuova card - reset scale della vecchia
                                            if let Some(old_scale) = app.card_hover_scale.get_mut(old_id) {
                                                *old_scale = 1.0; // Reset immediato
                                            }
                                            app.show_detail_popup = None;
                                            app.popup_fade_alpha = 0.0;
                                        }
                                    }
                                    app.hover_start_time = Some((display_id.clone(), std::time::Instant::now()));
                                }
                            }
                        } else {
                            if let Some((hovered_id, _)) = &app.hover_start_time {
                                if hovered_id == &display_id {
                                    app.hover_start_time = None;
                                    app.show_detail_popup = None;
                                    app.popup_fade_alpha = 0.0;
                                }
                            }
                        }

                        // RESTORED CONTEXT MENU
                        response.context_menu(|ui| {
                            let is_godmode = app.config.family_godmode_ids.contains(&display_id);
                            if is_godmode {
                                ui.label(egui::RichText::new("⚡ GODMODE ACTIVE").color(egui::Color32::GREEN).size(10.0));
                                if ui.button("💀 Disable Godmode").clicked() {
                                    ui.close_menu();
                                    app.disable_family_godmode(display_id.clone());
                                }
                            } else {
                                if is_installed {
                                    if ui.button("🛠 Force Repair").clicked() {
                                        ui.close_menu();
                                        app.detected_libraries = crate::game_path::GamePathFinder::get_library_folders(&app.config.steam_path);
                                        app.selected_library_index = 0;
                                        app.install_candidate = Some((display_id.clone(), name.to_string()));
                                        app.install_dir_input = name.to_string(); 
                                        app.install_modal_open = true;
                                    }
                                    if ui.button("🔨 Unpack Game (Steamless)").clicked() {
                                        ui.close_menu();
                                        // Steamless logic placeholder
                                    }
                                    if ui.button("👨‍👩‍👧 Enable Godmode").clicked() {
                                        ui.close_menu();
                                        app.install_game_family_godmode(display_id.clone());
                                    }
                                } else {
                                    if ui.button("👨‍👩‍👧 Install (Godmode Only)").clicked() {
                                        ui.close_menu();
                                        app.install_game_family_godmode(display_id.clone());
                                    }
                                }
                            }
                        });
                        
                        // RESTORED CLICK HANDLING
                        if response.clicked() {
                            if is_installed {
                                // PLAY LOGIC (Simplified call to spawn logic, to avoid duplication)
                                let steam_path = app.config.steam_path.clone();
                                let gl_path = app.config.gl_path.clone();
                                let app_id_run = display_id.clone();
                                let enable_stealth = app.config.enable_stealth_mode;
                                
                                std::thread::spawn(move || {
                                    let _ = crate::ui::helpers::setup_greenluma_config(&gl_path, enable_stealth);
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
                            } else {
                                // Install -> Manifestor
                                crate::ui::modals::manifestor::open_manifestor(app, display_id.clone(), name.to_string());
                            }
                        }

                        // Force new row
                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        // Easter egg for empty search state
        if results.is_empty() && app.last_searched_query.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    egui::RichText::new("🔍")
                        .size(64.0)
                        .color(egui::Color32::from_gray(60))
                );
                ui.add_space(15.0);
                ui.label(
                    egui::RichText::new("It's calm here...")
                        .size(20.0)
                        .color(egui::Color32::from_gray(100))
                );
                ui.label(
                    egui::RichText::new("Search for something above!")
                        .size(14.0)
                        .color(egui::Color32::from_gray(60))
                );
            });
        }
    });

    // ===== PREMIUM HOVER DETAIL POPUP =====
    if let Some(popup_appid) = &app.show_detail_popup.clone() {
        // Fade in animation
        app.popup_fade_alpha += (1.0 - app.popup_fade_alpha) * 0.2;
        if app.popup_fade_alpha < 0.99 {
            ctx_ui.ctx().request_repaint();
        }
        let alpha = (app.popup_fade_alpha * 255.0) as u8;
        
        // Get card position for anchoring
        if let Some((card_x, card_y, card_w, card_h)) = app.card_rects.get(popup_appid).cloned() {
            let popup_width = 300.0;
            let popup_x = card_x + (card_w - popup_width) / 2.0; // Center under card
            let popup_y = card_y + card_h + 8.0; // Below the card
            
            egui::Area::new(egui::Id::new("premium_game_popup"))
                .fixed_pos(egui::pos2(popup_x, popup_y))
                .order(egui::Order::Foreground)
                .show(ctx_ui.ctx(), |ui| {
                    // Outer glow
                    let glow_rect = egui::Rect::from_min_size(
                        egui::pos2(popup_x - 8.0, popup_y - 8.0),
                        egui::vec2(popup_width + 16.0, 200.0)
                    );
                    ui.painter().rect_filled(
                        glow_rect,
                        16.0,
                        egui::Color32::from_rgba_unmultiplied(0, 180, 220, (alpha as f32 * 0.15) as u8)
                    );
                    
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(18, 20, 28, alpha))
                        .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(0, 200, 255, alpha)))
                        .rounding(12.0)
                        .inner_margin(16.0)
                        .shadow(egui::epaint::Shadow {
                            offset: egui::vec2(0.0, 8.0),
                            blur: 24.0,
                            spread: 4.0,
                            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha as f32 * 0.7) as u8),
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(popup_width - 32.0);
                            
                            let is_loading = app.hover_loading.lock().unwrap().contains(popup_appid);
                            let details_opt = app.hover_details_cache.lock().unwrap().get(popup_appid).cloned();
                            
                            if let Some(details) = details_opt {
                                // === HEADER ===
                                ui.label(egui::RichText::new(&details.name)
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha)));
                                
                                ui.add_space(10.0);
                                
                                // === SCORES ROW ===
                                ui.horizontal(|ui| {
                                    // Metacritic
                                    if let Some(score) = details.metacritic_score {
                                        let (bg, fg) = if score >= 75 {
                                            (egui::Color32::from_rgb(102, 204, 102), egui::Color32::BLACK)
                                        } else if score >= 50 {
                                            (egui::Color32::from_rgb(255, 204, 51), egui::Color32::BLACK)
                                        } else {
                                            (egui::Color32::from_rgb(255, 102, 102), egui::Color32::WHITE)
                                        };
                                        
                                        egui::Frame::none()
                                            .fill(bg)
                                            .rounding(4.0)
                                            .inner_margin(egui::vec2(8.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.label(egui::RichText::new(format!("{}", score))
                                                    .size(14.0)
                                                    .strong()
                                                    .color(fg));
                                            });
                                    }
                                    
                                    // Reviews
                                    if let Some(recs) = details.recommendations {
                                        let formatted = if recs >= 1_000_000 {
                                            format!("{:.1}M reviews", recs as f64 / 1_000_000.0)
                                        } else if recs >= 1_000 {
                                            format!("{:.0}K reviews", recs as f64 / 1_000.0)
                                        } else {
                                            format!("{} reviews", recs)
                                        };
                                        ui.label(egui::RichText::new(formatted)
                                            .size(11.0)
                                            .color(egui::Color32::from_rgba_unmultiplied(180, 180, 190, alpha)));
                                    }
                                    
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Platforms (using text instead of broken emoji)
                                        if details.platforms.2 { 
                                            ui.label(egui::RichText::new("LNX").size(9.0).color(egui::Color32::from_rgb(255, 180, 0))); 
                                        }
                                        if details.platforms.1 { 
                                            ui.label(egui::RichText::new("MAC").size(9.0).color(egui::Color32::from_rgb(200, 200, 200))); 
                                        }
                                        if details.platforms.0 { 
                                            ui.label(egui::RichText::new("WIN").size(9.0).color(egui::Color32::from_rgb(0, 180, 255))); 
                                        }
                                    });
                                });
                                
                                ui.add_space(10.0);
                                
                                // === DESCRIPTION ===
                                if !details.short_description.is_empty() {
                                    // Strip HTML tags
                                    let clean_desc = details.short_description
                                        .replace("<br>", " ")
                                        .replace("<br/>", " ")
                                        .replace("&quot;", "\"")
                                        .replace("&amp;", "&");
                                    let desc = if clean_desc.len() > 200 {
                                        format!("{}...", &clean_desc[..200])
                                    } else {
                                        clean_desc
                                    };
                                    ui.label(egui::RichText::new(desc)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgba_unmultiplied(160, 165, 175, alpha)));
                                }
                                
                                ui.add_space(10.0);
                                
                                // === INFO ROW ===
                                ui.horizontal(|ui| {
                                    if let Some(dev) = details.developers.first() {
                                        ui.label(egui::RichText::new(format!("By {}", dev))
                                            .size(10.0)
                                            .italics()
                                            .color(egui::Color32::from_rgba_unmultiplied(120, 120, 130, alpha)));
                                    }
                                    
                                    if !details.release_date.is_empty() {
                                        ui.label(egui::RichText::new("|")
                                            .size(10.0)
                                            .color(egui::Color32::from_rgba_unmultiplied(60, 60, 70, alpha)));
                                        ui.label(egui::RichText::new(&details.release_date)
                                            .size(10.0)
                                            .color(egui::Color32::from_rgba_unmultiplied(120, 120, 130, alpha)));
                                    }
                                });
                                
                                // === GENRES ===
                                if !details.genres.is_empty() {
                                    ui.add_space(8.0);
                                    ui.horizontal_wrapped(|ui| {
                                        for genre in details.genres.iter().take(4) {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_unmultiplied(0, 150, 200, (alpha as f32 * 0.3) as u8))
                                                .rounding(3.0)
                                                .inner_margin(egui::vec2(6.0, 2.0))
                                                .show(ui, |ui| {
                                                    ui.label(egui::RichText::new(genre)
                                                        .size(9.0)
                                                        .color(egui::Color32::from_rgba_unmultiplied(0, 200, 255, alpha)));
                                                });
                                        }
                                    });
                                }
                                
                                // === AGE RATING ===
                                if details.required_age >= 18 {
                                    ui.add_space(6.0);
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(180, 50, 50))
                                        .rounding(3.0)
                                        .inner_margin(egui::vec2(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new("MATURE 18+")
                                                .size(9.0)
                                                .strong()
                                                .color(egui::Color32::WHITE));
                                        });
                                }
                                
                            } else if is_loading {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(egui::RichText::new("Loading game info...")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgba_unmultiplied(150, 150, 160, alpha)));
                                });
                            } else {
                                ui.label(egui::RichText::new(format!("AppID: {}", popup_appid))
                                    .size(11.0)
                                    .color(egui::Color32::GRAY));
                            }
                        });
                });
        }
    } else {
        // Reset fade when no popup
        app.popup_fade_alpha = 0.0;
    }
}
