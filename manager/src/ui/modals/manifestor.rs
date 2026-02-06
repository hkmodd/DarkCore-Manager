use eframe::egui;
use crate::ui::state::DarkCoreApp;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.manifestor_open { return; }
    
    let mut close_modal = false;
    let mut launch_params: Option<(String, String, Vec<String>)> = None;
    
    // Scope for Mutex Lock
    {
        let mut open = true;
        let mut should_close_ui = false;
        let mut hierarchy_guard = app.manifestor_data.lock().unwrap();
        // let _target_lib = app.manifestor_target_library.clone(); // Unused
        // let _detected_libs = app.detected_libraries.clone(); // Unused

        egui::Window::new(egui::RichText::new(format!("📦 INSTALL: {}", app.manifestor_candidate_name)).strong())
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
                    let _dlc_count = hierarchy.dlcs.len();
                    // Calculate TRUE Slot Usage (AppList Lines)
                    // GreenLuma Limit applies to TOTAL entries (AppIDs + Depots)
                    // Note: We need a dedicated helper for this logic eventually
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
                        // NOTE: "Essential Content Only" removed - Select/Deselect All is simpler
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
                            let btn_text = if is_over_limit { "🚫 LIMIT EXCEEDED" } else { "CONFIRM & PROCEED" };
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
        app.manifestor_selections = selections;
        app.manifestor_open = false; // Close Manifestor
        
        // v1.7.1: Open Family/Download choice modal (Next Step)
        app.install_candidate = Some((app_id, name));
        app.family_or_download_modal_open = true;
    } else if close_modal {
        app.manifestor_open = false;
    }
}

pub fn open_manifestor(app: &mut DarkCoreApp, appid: String, name: String) {
    app.manifestor_open = true;
    app.manifestor_candidate_id = Some(appid.clone());
    app.manifestor_candidate_name = name.clone();
    
    // Detect Libraries if not already
    if app.detected_libraries.is_empty() {
        app.detected_libraries = crate::game_path::GamePathFinder::get_library_folders(&app.config.steam_path);
    }
    // Default target library
    app.manifestor_target_library = app.detected_libraries.get(0).cloned();
    
    // Reset data
    if let Ok(mut data) = app.manifestor_data.lock() {
        *data = None;
    }
    
    // FIX: Reset install dir for auto-fill
    app.install_dir_input.clear();
    app.install_modal_auto_scanned = false;
    
    // Check API Client
    if let Some(client) = &app.api_client {
        let client = client.clone();
        let data_target = app.manifestor_data.clone();
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
