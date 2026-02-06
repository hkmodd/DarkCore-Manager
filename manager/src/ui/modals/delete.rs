use crate::ui::state::DarkCoreApp;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.delete_modal_open {
        return;
    }

    // Check pending scan results (async return)
    let scan_res = app.delete_scan_result.clone();
    if let Ok(mut res) = scan_res.lock() {
        if let Some(dlcs) = res.take() {
            app.delete_associated_dlcs = dlcs;
            app.is_scanning_dlcs = false;
        }
    }

    let mut close = false;
    let mut action: Option<(bool, Vec<String>)> = None; // (is_full_wipe, ids_to_delete)

    egui::Window::new("CONFIRM DELETION")
        .collapsible(false)
        .resizable(false)
        .fixed_size([400.0, 200.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading(format!(
                "Delete '{}'?",
                app.delete_candidate_name.as_deref().unwrap_or("Unknown")
            ));
            ui.label(format!(
                "ID: {}",
                app.delete_candidate_id.as_deref().unwrap_or("?")
            ));

            ui.add_space(10.0);

            if app.is_scanning_dlcs {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Scanning for associated DLCs...");
                });
            } else if !app.delete_associated_dlcs.is_empty() {
                ui.label(
                    egui::RichText::new(format!(
                        "⚠️ Found {} associated DLCs/Depots installed.",
                        app.delete_associated_dlcs.len()
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
                    close = true;
                }

                if !app.is_scanning_dlcs {
                    // OPTION 1: UNLINK (SAFE)
                    if ui
                        .button(
                            egui::RichText::new("🗑 UNLINK ID (SAFE)").color(egui::Color32::from_rgb(255, 165, 0)),
                        )
                        .on_hover_text("Removes from AppList & Config only.\nKEEPS game files and manifests on disk.")
                        .clicked()
                    {
                        if let Some(id) = app.delete_candidate_id.clone() {
                            let mut to_delete = vec![id];
                            to_delete.extend(app.delete_associated_dlcs.iter().cloned());
                            action = Some((false, to_delete));
                        }
                        close = true;
                    }

                    // OPTION 2: FULL WIPE
                    if ui
                        .button(
                            egui::RichText::new("🔥 FULL UNINSTALL").color(egui::Color32::RED).strong(),
                        )
                        .on_hover_text("DESTRUCTIVE.\nRemoves AppList, Config, Manifests AND DELETES GAME FILES.")
                        .clicked()
                    {
                        if let Some(id) = app.delete_candidate_id.clone() {
                            let mut to_delete = vec![id];
                            to_delete.extend(app.delete_associated_dlcs.iter().cloned());
                            action = Some((true, to_delete));
                        }
                        close = true;
                    }
                }
            });
        });

    if let Some((full_wipe, ids)) = action {
        app.remove_games_by_id(ids, full_wipe);
        app.refresh_library();
    }

    if close {
        app.delete_modal_open = false;
        app.delete_associated_dlcs.clear();
    }
}

pub fn initiate_delete(app: &mut DarkCoreApp, app_id: String, name: String) {
    app.delete_modal_open = true;
    app.delete_candidate_id = Some(app_id.clone());
    app.delete_candidate_name = Some(name.clone());
    app.delete_associated_dlcs.clear();
    app.is_scanning_dlcs = true;

    // Local Relationship Scan
    let mut known_child_ids = Vec::new();
    if let Ok(rel) = app.relationships.lock() {
        for (child, parent) in rel.iter() {
            if parent == &app_id {
                known_child_ids.push(child.clone());
            }
        }
    }

    // Heuristic Name Scan (For "Borderlands 4" vs "Borderlands®4: ...")
    let target_clean = name
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "");
    if target_clean.len() >= 4 {
        if let Ok(games) = app.active_games.lock() {
            for game in games.iter() {
                if game.app_id == app_id {
                    continue;
                } // Skip self

                // Detect if candidate is likely a DLC based on name overlap
                let cand_clean = game
                    .name
                    .to_lowercase()
                    .replace(|c: char| !c.is_alphanumeric(), "");
                if cand_clean.contains(&target_clean) && cand_clean != target_clean {
                    // Likely dlc
                    if !known_child_ids.contains(&game.app_id) {
                        known_child_ids.push(game.app_id.clone());
                    }
                }
            }
        }
    }

    // Also scan GreenLuma AppList for exact matches if needed?
    // Wait, relationships map already loaded from app_list::load_relationships.

    // Start Async Scan specific to Delete (lighter than full scan?) or just use knowns?
    // The original code used a thread.

    let res_arc = app.delete_scan_result.clone();

    // Reset result
    if let Ok(mut r) = res_arc.lock() {
        *r = None;
    }

    std::thread::spawn(move || {
        // Just return the synchronous findings for now after a small delay
        // (Simulating scan or if we need IO later)
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Ok(mut r) = res_arc.lock() {
            *r = Some(known_child_ids);
        }
    });
}
