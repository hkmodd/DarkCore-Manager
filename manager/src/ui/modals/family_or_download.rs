//! Modal: Family Shared or Download?
//!
//! Appears after DLC Picker to ask user whether to:
//! - Family Shared: Only add to AppList (no download, no API calls)
//! - Download: Proceed with Library Selection → Steam/Direct

use crate::ui::state::DarkCoreApp;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.family_or_download_modal_open {
        return;
    }

    let mut close_modal = false;
    let mut choice_family = false;
    let mut choice_download = false;

    egui::Window::new("🎮 Installation Method")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Show game info
                if let Some((appid, name)) = &app.install_candidate {
                    ui.label(egui::RichText::new(name).size(18.0).strong());
                    ui.label(
                        egui::RichText::new(format!("AppID: {}", appid))
                            .monospace()
                            .color(egui::Color32::GRAY),
                    );
                }

                ui.add_space(15.0);
                ui.label("How do you want to add this game?");
                ui.add_space(20.0);

                // Option 1: Family Shared
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("👨‍👩‍👧").size(24.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("FAMILY SHARED").strong().size(16.0));
                            ui.label(
                                egui::RichText::new("Add to AppList only - no download")
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                            ui.label(
                                egui::RichText::new("Use when game is shared by another account")
                                    .size(10.0)
                                    .italics()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                    ui.add_space(5.0);
                    if ui
                        .add_sized(
                            egui::vec2(ui.available_width(), 35.0),
                            egui::Button::new(
                                egui::RichText::new("👨‍👩‍👧 FAMILY SHARED")
                                    .strong()
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::from_rgb(255, 200, 100)),
                        )
                        .clicked()
                    {
                        choice_family = true;
                    }
                });

                ui.add_space(15.0);

                // Option 2: Download
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⬇").size(24.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("DOWNLOAD").strong().size(16.0));
                            ui.label(
                                egui::RichText::new("Download game files to your PC")
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                            ui.label(
                                egui::RichText::new("Choose Steam or Direct Download next")
                                    .size(10.0)
                                    .italics()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                    ui.add_space(5.0);
                    if ui
                        .add_sized(
                            egui::vec2(ui.available_width(), 35.0),
                            egui::Button::new(
                                egui::RichText::new("⬇ DOWNLOAD")
                                    .strong()
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::from_rgb(0, 200, 255)),
                        )
                        .clicked()
                    {
                        choice_download = true;
                    }
                });

                ui.add_space(15.0);

                if ui.button("Cancel").clicked() {
                    close_modal = true;
                }
            });
        });

    // Handle choices OUTSIDE the UI closure
    if choice_family {
        // FAMILY SHARED: Only add to AppList, no download
        if let Some((appid, _name)) = app.install_candidate.take() {
            let mut ids = vec![appid.clone()];
            // Add selected DLCs (from manifestor or dlc_picker)
            ids.extend(app.manifestor_selections.clone());

            // Write to AppList
            if let Err(e) = crate::app_list::add_games_to_list(&app.config.gl_path, ids.clone()) {
                app.log(format!("❌ Error: {}", e));
            } else {
                app.log(format!(
                    "✅ Family Shared: Added {} IDs to AppList.",
                    ids.len()
                ));

                // Save in config for godmode tracking
                if !app.config.family_godmode_ids.contains(&appid) {
                    app.config.family_godmode_ids.push(appid);
                    let _ = crate::config::save_config(&app.config);
                }

                app.refresh_library();
            }
        }
        app.family_or_download_modal_open = false;
        // Reset state
        app.manifestor_selections.clear();
    }

    if choice_download {
        // DOWNLOAD: Proceed with Library Selection
        app.family_or_download_modal_open = false;

        // Detect libraries
        app.detected_libraries =
            crate::game_path::GamePathFinder::get_library_folders(&app.config.steam_path);
        app.selected_library_index = 0;

        // Auto-fill install dir with sanitized folder name
        if let Some((_appid, name)) = &app.install_candidate {
            // Simple folder sanitization: replace invalid chars with underscores
            let sanitized: String = name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            app.install_dir_input = sanitized.trim().to_string();
        }

        app.install_modal_open = true; // Open Library Selection
    }

    if close_modal {
        app.family_or_download_modal_open = false;
        app.install_candidate = None;
        app.manifestor_selections.clear();
    }
}
