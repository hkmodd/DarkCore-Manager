use crate::ui::state::DarkCoreApp;
use crate::ui::state::APPLIST_LIMIT;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.dlc_picker_open {
        return;
    }

    // Ensure modal stays open
    let mut open = true;

    let candidate = app.dlc_picker_candidate.clone();

    if let Some((app_id, name)) = candidate {
        // Count current AppList entries
        let current_count = {
            let games = app.active_games.lock().unwrap();
            games.len()
        };
        let available_slots = APPLIST_LIMIT.saturating_sub(current_count);
        // Rough estimation
        let base_slots = app.dlc_picker_depot_count + 1;
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
                    ui.label(
                        egui::RichText::new(format!("Installing: {}", name))
                            .size(16.0)
                            .strong(),
                    );
                    ui.add_space(5.0);

                    // Stats bar
                    ui.horizontal(|ui| {
                        ui.label(format!("📊 AppList: {}/{}", current_count, APPLIST_LIMIT));
                        ui.separator();
                        ui.label(format!("📦 Base Depots: {}", base_slots));
                        ui.separator();
                        let selected = app
                            .dlc_picker_items
                            .iter()
                            .filter(|(_, _, s, _)| *s)
                            .count();
                        let color = if selected > dlc_slots {
                            egui::Color32::RED
                        } else {
                            egui::Color32::GREEN
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "✅ Selected: {}/{} DLCs",
                                selected, dlc_slots
                            ))
                            .color(color),
                        );
                    });

                    ui.add_space(5.0);

                    // Warning if over limit
                    let selected = app
                        .dlc_picker_items
                        .iter()
                        .filter(|(_, _, s, _)| *s)
                        .count();
                    if selected > dlc_slots {
                        ui.label(
                            egui::RichText::new(format!(
                                "⚠️ You've selected {} DLCs but only have {} slots available!",
                                selected, dlc_slots
                            ))
                            .color(egui::Color32::RED)
                            .strong(),
                        );
                    }

                    ui.add_space(5.0);

                    // Search bar
                    ui.horizontal(|ui| {
                        ui.label("🔍 Filter:");
                        ui.text_edit_singleline(&mut app.dlc_picker_search);

                        if ui.button("Select All").clicked() {
                            for (_, _, selected, available) in &mut app.dlc_picker_items {
                                if *available {
                                    *selected = true;
                                }
                            }
                        }
                        if ui.button("Deselect All").clicked() {
                            for (_, _, selected, _) in &mut app.dlc_picker_items {
                                *selected = false;
                            }
                        }
                        if ui.button(format!("Select First {}", dlc_slots)).clicked() {
                            let mut count = 0;
                            for (_, _, selected, available) in app.dlc_picker_items.iter_mut() {
                                if *available && count < dlc_slots {
                                    *selected = true;
                                    count += 1;
                                } else {
                                    *selected = false;
                                }
                            }
                        }
                    });

                    ui.separator();

                    // List
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let filter = app.dlc_picker_search.to_lowercase();
                            // Use index for mutation
                            // We need to iterate mutable items
                            // But we also want to filter.
                            // Egui pattern:
                            for (_id, name, selected, available) in &mut app.dlc_picker_items {
                                if filter.is_empty() || name.to_lowercase().contains(&filter) {
                                    if *available {
                                        ui.checkbox(selected, name.as_str());
                                    } else {
                                        // Unavailable DLC
                                        ui.add_enabled(
                                            false,
                                            egui::Checkbox::new(
                                                selected,
                                                egui::RichText::new(format!(
                                                    "{} (Not Cracked)",
                                                    name
                                                ))
                                                .color(egui::Color32::DARK_RED),
                                            ),
                                        );
                                    }
                                }
                            }
                        });

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.dlc_picker_open = false;
                            app.dlc_picker_pending_library = None;
                        }

                        let selected_count = app
                            .dlc_picker_items
                            .iter()
                            .filter(|(_, _, s, _)| *s)
                            .count();
                        let enabled = selected_count <= dlc_slots;

                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(
                                    egui::RichText::new("🚀 INSTALL SELECTED")
                                        .strong()
                                        .color(egui::Color32::GREEN),
                                ),
                            )
                            .clicked()
                        {
                            // FINALIZE
                            let selected_dlc_ids: Vec<String> = app
                                .dlc_picker_items
                                .iter()
                                .filter(|(_, _, s, _)| *s)
                                .map(|(id, _, _, _)| id.clone())
                                .collect();

                            if let (Some(tpl_lib), Some(tpl_dir)) = (
                                app.dlc_picker_pending_library.clone(),
                                app.dlc_picker_pending_install_dir.clone(),
                            ) {
                                let cached = app.dlc_picker_cached_bytes.take(); // Take first (consume)
                                                                                 // Pass None for hierarchy since DLC picker uses scraped LUA data
                                app.finalize_installation(
                                    app_id.clone(),
                                    name.clone(),
                                    Some(tpl_lib),
                                    Some(tpl_dir),
                                    selected_dlc_ids,
                                    cached,
                                    None, // No Hierarchy in Legacy Flow
                                );
                            }
                            app.dlc_picker_open = false;
                            app.dlc_picker_pending_library = None;
                            app.dlc_picker_pending_install_dir = None;
                        }
                    });
                });
            });

        if !open {
            app.dlc_picker_open = false;
        }
    }
}
