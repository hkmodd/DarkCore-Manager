use crate::ui::state::push_log;
use crate::ui::state::DarkCoreApp;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    // Custom Colors (preserved from original)
    let bg_sidebar = egui::Color32::from_rgb(18, 20, 28);
    let accent_cyan = egui::Color32::from_rgb(0, 243, 255);
    let accent_pink = egui::Color32::from_rgb(255, 0, 110);

    egui::SidePanel::left("sidebar")
        .resizable(false)
        .default_width(240.0)
        .frame(
            egui::containers::Frame::default()
                .fill(bg_sidebar)
                .inner_margin(16.0),
        )
        .show(ctx, |ui| {
            ui.add_space(10.0);

            // LOGO & IDENTITY
            ui.vertical_centered(|ui| {
                if let Some(texture) = &app.logo_texture {
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
                            .tint(egui::Color32::WHITE.linear_multiply(pulse as f32)),
                    );

                    // Counter-act spacing to keep header stable
                    ui.add_space(8.0 - hover as f32);
                } else {
                    ui.add_space(10.0);
                }

                // ARTISTIC HEADER
                // ASCII ART TITLE (Phase 19)
                let ascii_art = r#"
██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗ ██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝██╔════╝██╔═══██╗██╔══██╗██╔════╝
██║  ██║███████║██████╔╝█████╔╝ ██║     ██║   ██║██████╔╝█████╗  
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██║     ██║   ██║██╔══██╗██╔══╝  
██████╔╝██║  ██║██║  ██║██║  ██╗╚██████╗╚██████╔╝██║  ██║███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝
"#;
                ui.label(
                    egui::RichText::new(ascii_art)
                        .family(egui::FontFamily::Monospace)
                        .size(5.0) // Small size to fit 240px sidebar
                        .strong()
                        .color(accent_cyan),
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
                        .strong(),
                )
                .min_size(egui::vec2(btn_w, 28.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 50))) // Subtle Green Border
                .fill(egui::Color32::from_black_alpha(50)) // Transparent/Dark
                .rounding(2.0);

                if ui
                    .add(btn_stealth)
                    .on_hover_text("Launch GreenLuma Stealth Mode (Safe Injection)")
                    .clicked()
                {
                    // Trigger Logic
                    let steam_path = app.config.steam_path.clone();
                    let gl_path = app.config.gl_path.clone();
                    let log_arc = app.system_log.clone();
                    let enable_stealth = app.config.enable_stealth_mode;

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
                                let _ = std::process::Command::new("taskkill")
                                    .args(["/F", "/IM", "steam.exe"])
                                    .output();
                                std::thread::sleep(std::time::Duration::from_millis(1500));
                                let _ = crate::ui::helpers::setup_greenluma_config(
                                    &gl_path,
                                    enable_stealth,
                                );
                                match crate::injector::launch_injected(
                                    steam_exe.to_str().unwrap_or(""),
                                    dll_path.to_str().unwrap_or(""),
                                    Some("-inhibitbootstrap"),
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

                // 2. CLEAN STEAM [GHOST BLUE]
                let btn_reset = egui::Button::new(
                    egui::RichText::new("🔄 CLEAN STEAM")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(100, 180, 255)) // Light Blue
                        .strong(),
                )
                .min_size(egui::vec2(btn_w, 28.0))
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgb(50, 100, 180),
                )) // Blue Border
                .fill(egui::Color32::from_black_alpha(50))
                .rounding(2.0);

                if ui
                    .add(btn_reset)
                    .on_hover_text("Restart Steam WITHOUT GreenLuma injection (Clean Mode)")
                    .clicked()
                {
                    app.relaunch_steam_protocol();
                }
            });

            // UPDATE AVAILABLE BUTTON
            if let Ok(update_lock) = app.update_available.lock() {
                if let Some(new_ver) = update_lock.clone() {
                    drop(update_lock); // Release lock before UI
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let btn_text = format!("⬇ UPDATE AVAILABLE: v{}", new_ver);
                        let update_btn = egui::Button::new(
                            egui::RichText::new(btn_text)
                                .color(egui::Color32::BLACK)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(0, 255, 128)) // FLUO GREEN
                        .min_size(egui::vec2(ui.available_width(), 32.0))
                        .rounding(4.0);

                        if ui.add(update_btn).clicked() {
                            // Trigger update in background
                            let log_arc = app.system_log.clone();
                            let updating_arc = app.is_updating.clone();
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

            // NAV BUTTONS
            // Helper Closure replacement
            let current_tab = app.active_tab;
            let mut requested_tab = None;

            let mut draw_nav_btn = |label: &str, icon: &str, tab_idx: usize| {
                let is_active = current_tab == tab_idx;
                let bg = if is_active {
                    accent_cyan.linear_multiply(0.15)
                } else {
                    egui::Color32::TRANSPARENT
                };
                let fg = if is_active {
                    accent_cyan
                } else {
                    egui::Color32::from_gray(180)
                };
                let stroke = if is_active {
                    egui::Stroke::new(1.0, accent_cyan)
                } else {
                    egui::Stroke::NONE
                };

                let btn = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", icon, label))
                        .size(16.0)
                        .color(fg),
                )
                .fill(bg)
                .stroke(stroke)
                .frame(true)
                .min_size(egui::vec2(200.0, 45.0));

                let response = ui.add(btn);

                if (response.clicked() || response.hovered()) && current_tab != tab_idx {
                    requested_tab = Some(tab_idx);
                }

                if response.hovered() {
                    ui.ctx().request_repaint();
                }
                ui.add_space(8.0);
            };

            draw_nav_btn("INSTALL", "🚀", 0);
            draw_nav_btn("LIBRARY", "📂", 2);
            draw_nav_btn("SETTINGS", "⚙", 4);
            draw_nav_btn("ABOUT", "💻", 5);

            if let Some(idx) = requested_tab {
                app.active_tab = idx;
                app.tab_changed_at = std::time::Instant::now();
                if idx == 2 {
                    app.refresh_library();
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                // STATUS
                ui.label(
                    egui::RichText::new(&app.status_msg)
                        .size(10.0)
                        .color(egui::Color32::from_gray(100)),
                );

                // AUDIO CONTROLS
                if let Some(sink) = &app.audio_sink {
                    ui.separator();
                    ui.add_space(5.0);

                    // CUSTOM NEON VOLUME BAR
                    let bar_height = 24.0;
                    let (rect, response) = ui.allocate_at_least(
                        egui::vec2(ui.available_width(), bar_height),
                        egui::Sense::click_and_drag(),
                    );

                    // INTERACTION
                    let mut volume_changed = false;

                    // 1. Mouse Wheel
                    if response.hovered() {
                        let scroll = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll != 0.0 {
                            // Scroll up = Volume Up
                            app.volume = (app.volume + scroll * 0.005).clamp(0.0, 1.0);
                            volume_changed = true;
                        }
                    }

                    // 2. Click/Drag
                    if response.dragged() || response.clicked() {
                        if let Some(ptr) = response.interact_pointer_pos() {
                            let rel = (ptr.x - rect.min.x) / rect.width();
                            app.volume = rel.clamp(0.0, 1.0);
                            volume_changed = true;
                        }
                    }

                    if volume_changed {
                        sink.set_volume(app.volume);
                        ui.ctx().request_repaint();
                    }

                    // VISUALS
                    let painter = ui.painter();
                    let time = ui.input(|i| i.time);

                    // Background Groove
                    painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(200));
                    painter.rect_stroke(
                        rect,
                        4.0,
                        egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
                    );

                    // Dynamic Fill
                    let fill_w = rect.width() * app.volume;
                    let fill_rect =
                        egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));

                    // Neon Color Pulse
                    let pulse = (time * 3.0).sin() * 0.2 + 0.8;
                    let neon_base = egui::Color32::from_rgb(0, 255, 200); // Cyan-Green
                    let neon_color = neon_base.linear_multiply(pulse as f32);

                    if app.volume > 0.0 {
                        painter.rect_filled(fill_rect, 4.0, neon_color.linear_multiply(0.3)); // Glow halo
                        painter.rect_filled(fill_rect.shrink(2.0), 3.0, neon_color);
                        // Core
                    }

                    // FAKE AUDIO WAVES
                    let bars = 18;
                    let bar_w = rect.width() / bars as f32;
                    for i in 0..bars {
                        let x = rect.min.x + i as f32 * bar_w;
                        let phase = time * 8.0 + (i as f64 * 0.8);
                        let raw_amp = (phase.sin() * 0.5 + 0.5) as f32;
                        let amp = raw_amp * (app.volume * 1.5).min(1.0);

                        let h = rect.height() * 0.7 * amp;
                        if h < 2.0 {
                            continue;
                        }

                        let y_base = rect.max.y - 4.0;
                        let y_top = y_base - h;

                        let bar_rect = egui::Rect::from_min_max(
                            egui::pos2(x + 1.0, y_top),
                            egui::pos2(x + bar_w - 1.0, y_base),
                        );

                        if x < rect.min.x + fill_w {
                            painter.rect_filled(
                                bar_rect,
                                1.0,
                                egui::Color32::WHITE.linear_multiply(0.6),
                            );
                        } else {
                            painter.rect_filled(bar_rect, 1.0, egui::Color32::from_white_alpha(10));
                        }
                    }

                    // Text Overlay (Volume %)
                    let vol_pct = (app.volume * 100.0) as u32;
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("VOL {}%", vol_pct),
                        egui::FontId::proportional(10.0),
                        egui::Color32::WHITE,
                    );

                    // PLAY/PAUSE Toggle
                    ui.add_space(4.0);
                    let btn_txt = if sink.is_paused() {
                        "▶ RESUME AUDIO"
                    } else {
                        "⏸ PAUSE AUDIO"
                    };
                    let btn = egui::Button::new(egui::RichText::new(btn_txt).size(10.0).strong())
                        .min_size(egui::vec2(rect.width(), 16.0))
                        .fill(egui::Color32::from_black_alpha(100))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)));

                    if ui.add(btn).clicked() {
                        if sink.is_paused() {
                            sink.play();
                        } else {
                            sink.pause();
                        }
                    }
                    ui.add_space(5.0);
                }
                ui.separator();

                // UI: Download Progress Bar
                crate::ui::components::progress::render(app, ui, ctx);
            });
        });
}
