//! Main render loop.
//!
//! Contains the `eframe::App` implementation for `DarkCoreApp` (logic extracted),
//! including the main `update()` method that orchestrates all UI rendering.

use crate::ui::state::DarkCoreApp;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // UI: Download Method Modal
    crate::ui::modals::download_method::render(app, ctx);

    // Poll Status Updates from Threads
    if let Ok(mut guard) = app.status_update_queue.lock() {
        if let Some(msg) = guard.take() {
            app.status_msg = msg;
        }
    }

    // Poll DLC Scanner (for DLC Picker during install)
    if app.is_scanning_dlcs && !app.delete_modal_open {
        let mut scan_done = false;
        if let Ok(res) = app.dlc_scan_result.lock() {
            if res.is_some() {
                scan_done = true;
            }
        }

        if scan_done {
            app.is_scanning_dlcs = false;

            // NEW: Read cached ZIP
            if let Ok(mut zip_lock) = app.dlc_scan_result_zip.lock() {
                if let Some(bytes) = zip_lock.take() {
                    app.dlc_picker_cached_bytes = Some(bytes);
                }
            }

            let match_data = {
                let clone = app.dlc_scan_result.clone();
                let res = if let Ok(mut guard) = clone.lock() {
                    guard.take()
                } else {
                    None
                };
                res
            };

            if let Some((items, depot_count)) = match_data {
                if !items.is_empty() {
                    app.dlc_picker_items = items;
                    // Select first 130 DLCs
                    app.dlc_picker_depot_count = depot_count;
                    app.dlc_picker_open = true;
                } else {
                    // Auto Proceed (No DLCs found)
                    if let (Some(target), Some(dir)) = (
                        app.dlc_picker_pending_library.take(),
                        app.dlc_picker_pending_install_dir.take(),
                    ) {
                        if let Some((appid, name)) = app.dlc_picker_candidate.take() {
                            // Pass cached bytes (if any)
                            let cached = app.dlc_picker_cached_bytes.take();
                            app.finalize_installation(
                                appid,
                                name,
                                Some(target),
                                Some(dir),
                                Vec::new(),
                                cached,
                                None,
                            );
                        }
                    }
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100)); // Polling (reduced CPU)
    }

    // Poll Delete Scanner (for delete modal DLC association)
    if app.is_scanning_dlcs && app.delete_modal_open {
        let mut scan_done = false;
        if let Ok(res) = app.delete_scan_result.lock() {
            if res.is_some() {
                scan_done = true;
            }
        }

        if scan_done {
            app.is_scanning_dlcs = false;
            if let Ok(mut res_lock) = app.delete_scan_result.lock() {
                if let Some(associated) = res_lock.take() {
                    app.delete_associated_dlcs = associated;
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100)); // Polling
    }

    if app.logo_texture.is_none() {
        if let Some(data) = &app.logo_data {
            app.logo_texture = Some(ctx.load_texture(
                "logo_v5_final",
                data.clone(),
                egui::TextureOptions {
                    magnification: egui::TextureFilter::Linear,
                    minification: egui::TextureFilter::Linear,
                    mipmap_mode: Some(egui::TextureFilter::Linear),
                    ..egui::TextureOptions::LINEAR
                },
            ));
        }
    }

    // --- SIDEBAR ---
    crate::ui::components::sidebar::render(app, ctx);

    // --- CENTRAL CONTENT ---
    egui::CentralPanel::default()
        .frame(
            egui::containers::Frame::default()
                .fill(egui::Color32::from_rgb(11, 12, 16))
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            // ANIMATION
            let dt = app.tab_changed_at.elapsed().as_secs_f32();
            let alpha = (dt / 0.25).clamp(0.0, 1.0); // 250ms fade
            ui.set_opacity(alpha);
            if alpha < 1.0 {
                ui.ctx().request_repaint();
            }
            // WARNING - SUPER ANIMATED CONFIGURATION REQUIRED
            if app.config.steam_path.is_empty() || app.config.gl_path.is_empty() {
                let time = ui.input(|i| i.time);

                // Pulsing red glow effect
                let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
                let glow_alpha = (pulse * 100.0) as u8 + 50;
                let border_color =
                    egui::Color32::from_rgba_unmultiplied(255, 50, 50, glow_alpha + 100);
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
                            let icon = if (time * 2.0) as i32 % 2 == 0 {
                                "⚠"
                            } else {
                                "🔧"
                            };
                            ui.label(egui::RichText::new(icon).size(28.0).color(
                                egui::Color32::from_rgb(255, (100.0 + pulse * 155.0) as u8, 50),
                            ));

                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("CONFIGURATION REQUIRED")
                                        .size(18.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(
                                            255,
                                            (200.0 - pulse * 100.0) as u8,
                                            (200.0 - pulse * 100.0) as u8,
                                        )),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "Steam and GreenLuma paths must be configured.",
                                    )
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(180)),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Animated button with glow
                                    let btn_color = egui::Color32::from_rgb(
                                        (100.0 + pulse * 155.0) as u8,
                                        255,
                                        (100.0 + pulse * 155.0) as u8,
                                    );

                                    let btn = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new("⚙ GO TO SETTINGS")
                                                .size(14.0)
                                                .strong()
                                                .color(egui::Color32::BLACK),
                                        )
                                        .fill(btn_color)
                                        .rounding(6.0),
                                    );

                                    if btn.clicked() {
                                        app.active_tab = 4; // Settings tab
                                        app.tab_changed_at = std::time::Instant::now();
                                    }

                                    if btn.hovered() {
                                        ui.ctx().request_repaint();
                                    }
                                },
                            );
                        });
                    });

                ui.add_space(15.0);
                // Only request continuous repaint for animated tabs (Info tab with Matrix Rain)
                // Other tabs use on-demand repaint to save GPU
                if app.active_tab == 5 {
                    ui.ctx().request_repaint();
                }
            }

            // GLOBAL HUD (Persistent Console)
            // Bottom Panel inside Central Panel
            if app.active_tab != 5 {
                // Hide on About/Info tab for full immersion
                egui::TopBottomPanel::bottom("global_hud_console")
                    .resizable(true)
                    .default_height(140.0)
                    .show_inside(ui, |ui| {
                        crate::ui::components::terminal::render(app, ui);
                    });
            }

            // CONTENT AREA (Remaining Space)
            egui::CentralPanel::default().show_inside(ui, |ui| {
                match app.active_tab {
                    0 => crate::ui::panels::install::render(app, ui),
                    // 1 was DRM INTEL - now integrated into Library per-game
                    2 => crate::ui::panels::library::render(app, ui),
                    // 3 was Profiles
                    4 => crate::ui::panels::settings::render(app, ui),
                    5 => crate::ui::panels::about::render(app, ui),
                    _ => crate::ui::panels::install::render(app, ui),
                }
            });

            // Global Footer Removed (Logs are now per-tab or sidebar)
            ui.add_space(5.0);
        });

    // MODAL DIMMING (Phase 19)
    // Check if any modal is open
    let is_any_modal_open = app.install_modal_open
        || app.dlc_picker_open
        || app.manifestor_open
        || app.import_modal_open
        || app.delete_modal_open
        || app.download_method_modal_open
        || app.family_or_download_modal_open
        || app.create_profile_modal_open
        || app.delete_profile_modal_open;

    if is_any_modal_open {
        egui::Area::new(egui::Id::new("modal_dimmer"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(true) // Blocks clicks to underlying UI
            .order(egui::Order::Middle) // Draw on top of CentralPanel but below Modals (Modals use specific Area order usually, or we depend on render order)
            // NOTE: Modals in this app usually use `Window::show` or `Area`.
            // `Window` automatically handles Z-order on top of Areas.
            // We set order to Transparent (low priority Area) but defined AFTER CentralPanel, so it covers CentralPanel.
            .show(ctx, |ui| {
                let screen_rect = ctx.input(|i| i.screen_rect());
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 240), // 94% Opacity focus
                );
            });
    }

    // MODALS
    crate::ui::modals::install_modal::render(app, ctx);
    crate::ui::modals::dlc_picker::render(app, ctx);
    crate::ui::modals::manifestor::render(app, ctx);
    crate::ui::modals::import_zip::render(app, ctx); // Phase 3A
    crate::ui::modals::delete::render(app, ctx);
    crate::ui::modals::family_or_download::render(app, ctx); // v1.7.1: Family Shared or Download
}
