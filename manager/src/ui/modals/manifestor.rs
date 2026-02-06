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
    
    // 1. Check for OFFLINE data (Priority)
    let mut loaded_from_zip = false;
    if let Some(zip_data) = &app.import_zip_data {
        let zip_app_id = zip_data.script_data.app_id.unwrap_or(0).to_string();
        
                if zip_app_id == appid {
            if let Ok(mut data) = app.manifestor_data.lock() {
                // Convert ScriptData -> GameHierarchy
                let mut dlcs = Vec::new();
                for dlc_info in &zip_data.script_data.dlcs {
                    dlcs.push(crate::api::DlcNode {
                        app_id: dlc_info.app_id.to_string(),
                        name: dlc_info.name.clone(),
                        depots: Vec::new(),
                        selected: true,
                    });
                }
                
                let mut base_depots = Vec::new();
                for d_info in &zip_data.script_data.depots {
                     base_depots.push(crate::api::DepotNode {
                         depot_id: d_info.depot_id.to_string(),
                         gid: None, // No GID context from simple ScriptData
                         size: None,
                         language: None,
                         is_common: true,
                     });
                }

                *data = Some(crate::api::GameHierarchy {
                    root_id: appid.clone(),
                    root_name: zip_data.script_data.app_name.clone().unwrap_or(name.clone()),
                    base_depots,
                    dlcs,
                });
                loaded_from_zip = true;
            }
        }
    }

    // 2. Fallback to API (if not loaded from zip)
    if !loaded_from_zip {
        if let Some(client) = &app.api_client {
            let client = client.clone();
            let data_target = app.manifestor_data.clone();
            let appid_target = appid.clone();
            let stats_arc = app.user_stats.clone();
            let refresh_arc = app.request_api_refresh.clone();
            
            tokio::spawn(async move {
                // 1. Try Standard API Fetch (Reverse Lookup included)
                match client.fetch_full_hierarchy(&appid_target, "english").await {
                    Ok(mut hierarchy) => {
                        // [MORRENUS FAILOVER PROTOCOL]
                        // If API finds 0 DLCs, it might be a hidden game (like Risk of Rain 2).
                        // We FORCE check Morrenus to see if there's a script with better info.
                        if hierarchy.dlcs.is_empty() {
                            let vault = crate::vault::VaultManager::new(".");
                            // 1. CHECK VAULT (Token Saver) - 06/02/2026
                            // If we already have the Lua script, USE IT. Do NOT download again.
                            let mut loaded_from_vault = false;
                            
                            if let Ok(lua_bytes) = vault.get_lua(&appid_target) {
                                println!("[Failover] Found cached Lua in Vault. Using local data (0 Tokens).");
                                let lua_content = String::from_utf8_lossy(&lua_bytes).to_string();
                                
                                if let Ok(script_data) = crate::direct_download::lua_parser::parse_content(&lua_content) {
                                    if !script_data.dlcs.is_empty() {
                                        // FOUND HIDDEN DLCs (From Vault)!
                                        for dlc in script_data.dlcs {
                                            if !hierarchy.dlcs.iter().any(|d| d.app_id == dlc.app_id.to_string()) {
                                                hierarchy.dlcs.push(crate::api::DlcNode {
                                                    app_id: dlc.app_id.to_string(),
                                                    name: dlc.name,
                                                    depots: Vec::new(),
                                                    selected: true,
                                                });
                                            }
                                        }
                                        hierarchy.dlcs.sort_by(|a, b| a.app_id.cmp(&b.app_id));
                                        loaded_from_vault = true;
                                    }
                                }
                            }

                            // 2. DOWNLOAD (Only if Vault failed/empty)
                            if !loaded_from_vault {
                                // Don't log to UI here, just do it in background
                                if let Ok(zip_bytes) = client.download_manifest(&appid_target).await {
                                    // [STATS TRIGGER] - Real-time Update
                                    if let Ok(mut stats) = stats_arc.lock() {
                                        if let Some(s) = stats.as_mut() {
                                            s.api_key_usage_count += 1;
                                            s.daily_usage += 1;
                                        }
                                    }
                                    if let Ok(mut req) = refresh_arc.lock() { *req = true; }

                                    // [IMMEDIATE VAULT SAVE] - 06/02/2026
                                    // Strategy: "Extracted Only" (Armored Layout)
                                    // Vault/{AppID}/{AppID}.lua
                                    // Vault/{AppID}/{Depot}_{Gid}.manifest
                                    
                                    let mut real_lua_content = String::new();
                                    if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes)) {
                                         // 1. EXTRACT AND SAVE MANIFESTS + FIND LUA
                                         for i in 0..archive.len() {
                                             if let Ok(mut f) = archive.by_index(i) {
                                                 if f.name().ends_with(".lua") {
                                                     use std::io::Read;
                                                     let _ = f.read_to_string(&mut real_lua_content);
                                                     // Save Lua immediately to Vault
                                                     if let Err(e) = vault.save_lua(&appid_target, real_lua_content.as_bytes()) {
                                                         println!("[Fallback] Warning: Could not save Lua to Vault: {}", e);
                                                     }
                                                 } else if f.name().ends_with(".manifest") {
                                                      // Backup manifests individually
                                                      if let Ok(gids) = crate::api::ApiClient::extract_gids_from_zip(&zip_bytes) {
                                                           let fname = f.name().to_string();
                                                           for (depot_id, gid) in &gids {
                                                               if fname.contains(&format!("{}_{}", depot_id, gid)) {
                                                                   let mut buf = Vec::new();
                                                                   use std::io::Read;
                                                                   if f.read_to_end(&mut buf).is_ok() {
                                                                       let _ = vault.store_manifest_bytes(&appid_target, depot_id.parse().unwrap_or(0), gid.parse().unwrap_or(0), &buf);
                                                                   }
                                                                   break;
                                                               }
                                                           }
                                                      }
                                                 }
                                             }
                                         }
                                    }
                                    
                                    // Fallback if unzip failed but bytes are valid (weird text case)
                                    if real_lua_content.is_empty() {
                                        real_lua_content = String::from_utf8_lossy(&zip_bytes).to_string();
                                        // Try to save this as Lua too, just in case
                                        let _ = vault.save_lua(&appid_target, &zip_bytes); 
                                    }
                                    
                                    println!("[Fallback] Data secured in Vault (Extracted Format). Token preserved.");

                                    if let Ok(script_data) = crate::direct_download::lua_parser::parse_content(&real_lua_content) {
                                        if !script_data.dlcs.is_empty() {
                                            // FOUND HIDDEN DLCs! Merge them in.
                                            for dlc in script_data.dlcs {
                                                // Check duplicates
                                                if !hierarchy.dlcs.iter().any(|d| d.app_id == dlc.app_id.to_string()) {
                                                    hierarchy.dlcs.push(crate::api::DlcNode {
                                                        app_id: dlc.app_id.to_string(),
                                                        name: dlc.name, // Use name from Lua comment!
                                                        depots: Vec::new(),
                                                        selected: true,
                                                    });
                                                }
                                            }
                                            // Sort
                                            hierarchy.dlcs.sort_by(|a, b| a.app_id.cmp(&b.app_id));
                                        }
                                    }
                                }
                            }
                        }

                        if let Ok(mut target) = data_target.lock() {
                            *target = Some(hierarchy);
                        }
                    },
                    Err(_) => {
                         // API Failed completely? Try Morrenus as last resort?
                         // For now, leave as None (Spinner spins forever or we could handle error)
                    }
                }
            });
        }
    }
}
