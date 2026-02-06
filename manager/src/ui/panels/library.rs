//! Library Panel module.
//! 
//! Handles the main "Library" tab, including:
//! - Profile Management (Load, Save, Create, Delete)
//! - Game List with Virtualized Scrolling
//! - Watcher Updates & Manifest Updates
//! - Integration with Steamless and Goldberg

use eframe::egui;
use crate::profiles::Profile;
use crate::app_list::nuke_reorder;
use crate::config::save_config;
use crate::ui::state::push_log;

/// Renders the Library/Profile Manager tab.
///
/// # Arguments
/// * `app` - Mutable reference to the application state
/// * `ui` - Mutable reference to the UI context
pub fn render(app: &mut crate::ui::state::DarkCoreApp, ui: &mut egui::Ui) {
    // PROFILE MANAGER HEADER
    ui.vertical(|ui| {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
             ui.label(egui::RichText::new("PROFILE MANAGER & LIBRARY").size(16.0).strong().color(egui::Color32::from_rgb(0, 200, 255)));
             ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                  if ui.button(egui::RichText::new("➕ CREATE NEW PROFILE").strong().color(egui::Color32::GREEN)).clicked() {
                      app.profile_name_input.clear(); // Reset input
                      app.create_profile_modal_open = true;
                  }
                  
                  // CHECK UPDATES BUTTON
                  let pending_count = app.watcher_pending_updates.lock()
                      .map(|p| p.len())
                      .unwrap_or(0);
                  
                  let btn_text = if pending_count > 0 {
                      format!("🔄 CHECK UPDATES ({})", pending_count)
                  } else {
                      "🔄 CHECK UPDATES".to_string()
                  };
                  
                  let btn_color = if pending_count > 0 {
                      egui::Color32::from_rgb(255, 165, 0) // Orange
                  } else {
                      egui::Color32::from_rgb(100, 200, 255) // Cyan
                  };
                  
                  if ui.button(egui::RichText::new(btn_text).strong().color(btn_color).size(11.0))
                      .on_hover_text("Manually check for game updates")
                      .clicked()
                  {
                      app.start_watcher_check();
                  }
             });
        });
        
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_black_alpha(100))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                     // PROFILE SELECTOR
                     ui.label("Profile:");
                     let profiles = app.profile_manager.list_profiles();
                     let current_sel = app.active_profile_name.clone();
                     
                     // 1. WIDER COMBO & AUTO-LOAD
                     egui::ComboBox::from_id_salt("profile_combo")
                         .selected_text(if current_sel.is_empty() { "Select Profile..." } else { &current_sel })
                         .width(250.0) // Aesthetic Width
                         .show_ui(ui, |ui| {
                             for name in &profiles {
                                 // AUTO-LOAD LOGIC
                                 if ui.selectable_value(&mut app.active_profile_name, name.clone(), name).clicked() {
                                     // User clicked a new profile -> Auto Load
                                     match app.profile_manager.load_profile(name) {
                                         Ok(p) => {
                                             if p.app_ids.len() > 133 {
                                                 app.status_msg = format!("⚠ LIMIT EXCEEDED ({} > 133). Steam may crash.", p.app_ids.len());
                                             }
                                             use crate::app_list::overwrite_app_list;
                                             if let Err(e) = overwrite_app_list(&app.config.gl_path, p.app_ids) {
                                                 app.log(format!("Error applying profile: {}", e));
                                             } else {
                                                 app.config.last_active_profile = p.name.clone();
                                                 if let Err(e) = save_config(&app.config) {
                                                     app.log(format!("Config Save Error: {}", e));
                                                 }
                                                 app.refresh_library(); // Auto Refresh
                                                 app.log(format!("Profile '{}' loaded automatically.", p.name));
                                             }
                                         },
                                         Err(e) => app.log(format!("Load Error: {}", e)),
                                     }
                                 }
                             }
                         });

                     ui.add_space(10.0);
                     
                     // SAVE (UPDATE) BUTTON
                     if ui.button(egui::RichText::new("💾 SAVE").strong().color(egui::Color32::GREEN)).on_hover_text("Save current library to SELECTED profile").clicked() {
                         if !app.active_profile_name.is_empty() {
                             let games = app.active_games.lock().unwrap();
                             let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
                             drop(games);
                             
                             // 133 CHECK
                             if ids.len() > 133 {
                                 app.log(format!("⚠ Warning: Saving {} apps (Limit 133).", ids.len()));
                             }
                             
                             let p = Profile { name: app.active_profile_name.clone(), app_ids: ids };
                             if let Err(e) = app.profile_manager.save_profile(&p) {
                                 app.log(format!("Save Error: {}", e));
                             } else {
                                 app.log(format!("Profile '{}' updated!", p.name));
                             }
                         } else {
                             app.log("Please select a profile to save to first.".to_string());
                         }
                     }

                     // DELETE BUTTON (Protected)
                     let is_default = app.active_profile_name == "Default";
                     let btn = egui::Button::new(egui::RichText::new("🗑").color(if is_default { egui::Color32::GRAY } else { egui::Color32::RED }));
                     
                     if ui.add_enabled(!is_default, btn)
                         .on_hover_text(if is_default { "Cannot delete Default profile" } else { "Delete selected profile" })
                         .clicked()
                         && !app.active_profile_name.is_empty() {
                             app.delete_profile_modal_open = true;
                         }
                });
            });
    });
    
    // NEW PROFILE MODAL
    // NEW PROFILE MODAL (ANIMATED)
    // 1. Calculate Ease-Out-Back (Bounce)
    let ctx = ui.ctx().clone();
    let anim_t = ctx.animate_bool(egui::Id::new("create_profile_anim"), app.create_profile_modal_open);
    
    if anim_t > 0.0 {
        // cubic-bezier approximation for backOut(1.7)
        // t = anim_t
        // c1 = 1.70158
        // c3 = c1 + 1
        // 1 + c3 * (t-1)^3 + c1 * (t-1)^2
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        let t = anim_t - 1.0;
        let ease_out_back = 1.0 + c3 * t.powi(3) + c1 * t.powi(2);
        
        // Drop In: Start -300px (Top), End 0px (Center)
        let y_offset = (1.0 - ease_out_back) * -300.0;
        
         egui::Window::new(egui::RichText::new("➕ CREATE NEW PROFILE").strong().color(egui::Color32::GREEN))
             .collapsible(false)
             .resizable(false)
             .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
             .show(&ctx, |ui| {
                  ui.label("Enter name for new profile:");
                  ui.text_edit_singleline(&mut app.profile_name_input).request_focus();
                  
                  ui.add_space(10.0);
                  ui.label(egui::RichText::new("⚠ This will WIPE the current AppList.").color(egui::Color32::YELLOW));
                  
                  // SAFETY CHECKBOX
                  if !app.active_profile_name.is_empty() {
                      ui.add_space(5.0);
                      ui.checkbox(&mut app.create_profile_save_default, 
                          format!("Save changes to '{}' before wiping?", app.active_profile_name)
                      );
                  }
                  
                  ui.add_space(15.0);

                  ui.horizontal(|ui| {
                      if ui.button("CANCEL").clicked() {
                          app.create_profile_modal_open = false;
                      }
                      
                      if ui.button(egui::RichText::new("✅ CREATE & WIPE").strong().color(egui::Color32::RED)).clicked()
                          && !app.profile_name_input.is_empty() {
                              // 1. AUTO-SAVE CURRENT (Safety) - CONDITIONAL
                              if !app.active_profile_name.is_empty() && app.create_profile_save_default {
                                  let games = app.active_games.lock().unwrap();
                                  let ids: Vec<String> = games.iter().map(|g| g.app_id.clone()).collect();
                                  let p = Profile { name: app.active_profile_name.clone(), app_ids: ids };
                                  let _ = app.profile_manager.save_profile(&p); 
                                  app.log(format!("Safety Save: Updated '{}'.", p.name));
                              } else {
                                  app.log("Safety Save skipped by user.".to_string());
                              }
                              
                              // 2. CREATE NEW EMPTY PROFILE
                              let new_p = Profile { name: app.profile_name_input.clone(), app_ids: Vec::new() };
                              if let Err(e) = app.profile_manager.save_profile(&new_p) {
                                  app.log(format!("Error creating profile: {}", e));
                              } else {
                                  // 3. WIPE APPLIST
                                  let res = {
                                       use crate::app_list::overwrite_app_list;
                                       overwrite_app_list(&app.config.gl_path, Vec::new())
                                  };
                                  
                                  if let Err(e) = res {
                                      app.log(format!("Error wiping AppList: {}", e));
                                  } else {
                                      // 4. SWITCH & REFRESH
                                      app.active_profile_name = app.profile_name_input.clone();
                                      
                                      // PERSIST CONFIG
                                      app.config.last_active_profile = app.active_profile_name.clone();
                                      if let Err(e) = save_config(&app.config) {
                                          app.log(format!("Config Save Error: {}", e));
                                      }

                                      app.refresh_library();
                                      app.log(format!("Switched to new profile '{}'. List cleared.", app.active_profile_name));
                                      app.create_profile_modal_open = false;
                                  }
                              }
                          }
                  });
             });
    }
    
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // DELETE CONFIRMATION MODAL
    if app.delete_profile_modal_open {
         // Animate or simple overlay
         egui::Window::new(egui::RichText::new("🗑 DELETE PROFILE?").strong().color(egui::Color32::RED))
             .collapsible(false)
             .resizable(false)
             .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
             .show(&ctx, |ui| {
                  ui.label(egui::RichText::new(format!("Are you sure you want to delete '{}'?", app.active_profile_name)).size(16.0));
                  ui.add_space(5.0);
                  ui.label(egui::RichText::new("⚠ This action cannot be undone.").color(egui::Color32::YELLOW));
                  
                  ui.add_space(15.0);
                  ui.horizontal(|ui| {
                      if ui.button("CANCEL").clicked() {
                          app.delete_profile_modal_open = false;
                      }
                      
                      ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                          if ui.button(egui::RichText::new("💀 DELETE FOREVER").strong().color(egui::Color32::RED)).clicked() {
                               if !app.active_profile_name.is_empty() {
                                   if let Err(e) = app.profile_manager.delete_profile(&app.active_profile_name) {
                                       app.log(format!("Delete Error: {}", e));
                                   } else {
                                       app.log(format!("Profile '{}' deleted.", app.active_profile_name));
                                       app.active_profile_name.clear();
                                   }
                               }
                               app.delete_profile_modal_open = false;
                          }
                      });
                  });
             });
    }

    // FIX 8: Library Search/Filter Bar
    ui.horizontal(|ui| {
        ui.label("Filter:");
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.library_search_query)
                .hint_text("Search games...")
                .desired_width(200.0)
        );
        if ui.button("X").on_hover_text("Clear filter").clicked() {
            app.library_search_query.clear();
        }
        
        // Show match count
        if !app.library_search_query.is_empty() {
             // FIX: Avoid unwrap() in UI thread
             if let Ok(games) = app.active_games.try_lock() {
                let q = app.library_search_query.to_lowercase();
                let match_count = games.iter().filter(|g| {
                    g.name.to_lowercase().contains(&q) || g.app_id.contains(&app.library_search_query)
                }).count();
                // implicit drop(games)
                ui.label(egui::RichText::new(format!("({} matches)", match_count)).color(egui::Color32::from_rgb(100, 200, 255)));
             }
        }
        
        if response.changed() {
            ui.ctx().request_repaint();
        }
    });
    
    ui.add_space(5.0);

    // Standard Library Controls (Refresh, Nuke, Resolve)
    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new("🔄 Refresh").strong())
            .clicked()
        {
            app.refresh_library();
        }
        if ui
            .button(
                egui::RichText::new("🔃 Reorder List")
                    .strong()
                    .color(egui::Color32::LIGHT_BLUE),
            )
            .on_hover_text("Sorts the AppList alphabetically without deleting unknown items.")
            .clicked()
        {
            let result = {
                let guard = app.game_cache.lock().ok();
                nuke_reorder(&app.config.gl_path, &app.config.steam_path, None, guard.as_deref())
            };

            if let Err(e) = result {
                app.log(format!("Error: {}", e));
            } else {
                app.log("Library Reordered (Alphabetical).".to_string());
                app.refresh_library();
            }
        }
    });
    ui.add_space(5.0);

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

    let active_games = app.active_games.clone();
    let games = match active_games.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Log isn't easily accessible here without cloning app, so just recover
            poisoned.into_inner()
        }
    };
    
    // Filter games based on search query
    // NOTE: Must use SAME logic as match counter above (lines 287-290)
    let search_query = &app.library_search_query;
    let filtered_games: Vec<usize> = if search_query.is_empty() {
        (0..games.len()).collect()
    } else {
        let query_lower = search_query.to_lowercase();
        games.iter().enumerate()
            .filter(|(_, g)| {
                g.name.to_lowercase().contains(&query_lower) || 
                g.app_id.contains(search_query)
            })
            .map(|(i, _)| i)
            .collect()
    };

    // VIRTUALIZED SCROLLING: Only render visible rows for 60fps performance
    let row_height = 35.0_f32; // Approximate height per game row
    let total_rows = filtered_games.len();

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show_rows(ui, row_height, total_rows, |ui, row_range| {
        // Collect delete request to avoid borrow issues
        let mut delete_req = None;

        for row_idx in row_range {
            if let Some(&game_idx) = filtered_games.get(row_idx) {
            if let Some(game) = games.get(game_idx) {
            // IDs are shown so user knows exact AppList usage count
            let bg_color = if row_idx % 2 == 0 {
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
                        
                        // Update Indicator - Check if update available from watcher
                        let has_pending_update = app.watcher_pending_updates.lock()
                            .map(|pu| pu.contains_key(&game.app_id))
                            .unwrap_or(false);
                        
                        let is_updating = app.watcher_updating.lock()
                            .map(|dl| dl.contains(&game.app_id))
                            .unwrap_or(false);
                        
                        if is_updating {
                            ui.label(
                                egui::RichText::new("⏳")
                                    .color(egui::Color32::YELLOW)
                            ).on_hover_text("Downloading new manifests...");
                        } else if has_pending_update {
                            ui.label(
                                egui::RichText::new("🔔")
                                    .color(egui::Color32::from_rgb(255, 165, 0)) // Orange
                            ).on_hover_text("Update available! Click AGGIORNA to download new manifest.");
                        }

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
                                
                                // AGGIORNA MANIFEST BUTTON (only if update pending and not already updating)
                                if has_pending_update && !is_updating {
                                    let update_btn = ui.button(
                                        egui::RichText::new("⬇ AGGIORNA")
                                            .color(egui::Color32::from_rgb(0, 255, 150))
                                            .size(11.0)
                                    ).on_hover_text("Download new manifest files for this game.\nWill use configured target language.");
                                    
                                    if update_btn.clicked() {
                                        // Start update
                                        let app_id = game.app_id.clone();
                                        let api_key = app.config.api_key.clone();
                                        let steam_path = app.config.steam_path.clone();
                                        let target_language = app.config.target_language.clone();
                                        let updating_arc = app.watcher_updating.clone();
                                        let pending_arc = app.watcher_pending_updates.clone();
                                        let log_arc = app.system_log.clone();
                                        
                                        // Mark as updating
                                        if let Ok(mut u) = updating_arc.lock() {
                                            u.insert(app_id.clone());
                                        }
                                        
                                        // Spawn update thread
                                        std::thread::spawn(move || {
                                            let rt = match tokio::runtime::Runtime::new() {
                                                Ok(rt) => rt,
                                                Err(_) => {
                                                    if let Ok(mut u) = updating_arc.lock() { u.remove(&app_id); }
                                                    return;
                                                }
                                            };
                                            
                                            if let Ok(mut logs) = log_arc.lock() {
                                                logs.push(format!("[Watcher] Starting manifest update for {}...", app_id));
                                            }
                                            
                                            // Create downloader and trigger update
                                            let downloader = crate::manifest_downloader::ManifestDownloader::new();
                                            let steam_path = std::path::Path::new(&steam_path);
                                            
                                            let result = rt.block_on(async {
                                                crate::watcher::trigger_update_download(
                                                    &api_key,
                                                    &downloader,
                                                    &app_id,
                                                    steam_path,
                                                    &target_language,
                                                ).await
                                            });
                                            
                                            match result {
                                                Ok(count) => {
                                                    if let Ok(mut logs) = log_arc.lock() {
                                                        logs.push(format!("[Watcher] ✅ Updated {} depot manifests for {}", count, app_id));
                                                    }
                                                    // Remove from pending
                                                    if let Ok(mut p) = pending_arc.lock() {
                                                        p.remove(&app_id);
                                                    }
                                                }
                                                Err(e) => {
                                                    if let Ok(mut logs) = log_arc.lock() {
                                                        logs.push(format!("[Watcher] ❌ Update failed for {}: {}", app_id, e));
                                                    }
                                                }
                                            }
                                            
                                            // Remove from updating
                                            if let Ok(mut u) = updating_arc.lock() {
                                                u.remove(&app_id);
                                            }
                                        });
                                    }
                                }

                                // STEAMLESS AUTOMATION BUTTON
                                let steam_path = app.config.steam_path.clone();
                                
                                // SKIP Steamless for Family Shared games
                                let is_family_shared = app.config.family_godmode_ids.contains(&game.app_id);
                                
                                // Show STEAMLESS button only if game path exists and not family shared
                                if !is_family_shared && crate::game_path::GamePathFinder::find_game_path(&steam_path, &game.app_id).is_some() {
                                    let steamless_btn = ui.button(
                                        egui::RichText::new("⚡ STEAMLESS")
                                            .color(egui::Color32::from_rgb(255, 150, 0))
                                            .size(10.0)
                                    ).on_hover_text("Auto-patch all DRM-protected EXEs in game folder.\nGenerates steam_appid.txt.");
                                    
                                    if steamless_btn.clicked() {
                                        if let Some(game_path) = crate::game_path::GamePathFinder::find_game_path(&steam_path, &game.app_id) {
                                            let steamless_cli = app.config.steamless_path.clone();
                                            let app_id = game.app_id.clone();
                                            let log_arc = app.system_log.clone();
                                            
                                            if steamless_cli.is_empty() || !std::path::Path::new(&steamless_cli).exists() {
                                                app.log("❌ Steamless CLI not configured. Go to Settings.".to_string());
                                            } else {
                                                // Log start
                                                app.log(format!("⚡ Starting Steamless on: {:?}", game_path));
                                                
                                                // Find all EXEs first (for logging)
                                                let exes = crate::steamless::find_game_executables(&game_path);
                                                app.log(format!("   Found {} potential game executables", exes.len()));
                                                
                                                // Run in thread to not block UI
                                                let path_clone = game_path.clone();
                                                std::thread::spawn(move || {
                                                    let log = move |msg: String| {
                                                        if let Ok(mut logs) = log_arc.lock() {
                                                            push_log(&mut logs, msg);
                                                        }
                                                    };
                                                    
                                                    let (success, total, results) = crate::steamless::run_steamless_folder(
                                                        &path_clone,
                                                        &steamless_cli,
                                                        &app_id,
                                                    );
                                                    
                                                    // Log results
                                                    for r in results {
                                                        if r.success {
                                                            log(format!("   ✅ {}: {}", r.exe_path, r.message));
                                                        } else {
                                                            log(format!("   ⚠️ {}: {}", r.exe_path, r.message));
                                                        }
                                                    }
                                                    
                                                    log(format!("⚡ Steamless Complete: {}/{} EXEs patched", success, total));
                                                });
                                            }
                                        } else {
                                            app.log("❌ Game folder not found. Is it installed?".to_string());
                                        }
                                    }
                                    
                                    // GOLDBERG BUTTON
                                    if ui.button(egui::RichText::new("🛡 GOLDBERG").color(egui::Color32::YELLOW).size(10.0))
                                        .on_hover_text("Deploy Offline Fix (Goldberg Emulator).\nEnsures Saves and Achievements work offline.")
                                        .clicked() 
                                    {
                                        app.goldberg_candidate_id = Some(game.app_id.clone());
                                        app.goldberg_modal_open = true;
                                    }
                                } else {
                                     // Not installed or check if DLC
                                     if game.parent_id.is_some() || app.is_probable_dlc(&game.name) {
                                        let label = if let Some(pid) = &game.parent_id {
                                            format!("📦 DLC / CONTENT (Linked to {})", pid)
                                        } else {
                                            "📦 DLC / CONTENT".to_string()
                                        };

                                        ui.label(
                                            egui::RichText::new(&label)
                                                .color(egui::Color32::from_rgb(150, 150, 255))
                                                .size(10.0)
                                        ).on_hover_text("Detected as Downloadable Content (Linked to Parent).");
                                     } else if is_family_shared {
                                         // Family Shared game - show special label
                                         ui.label(
                                             egui::RichText::new("👨‍👩‍👧 FAMILY GODMODE")
                                                 .color(egui::Color32::from_rgb(100, 255, 255))
                                                 .size(10.0)
                                         ).on_hover_text("Game activated via Steam Family Sharing.\nNo patching needed - works natively!");
                                     } else {
                                         ui.label(
                                             egui::RichText::new("NOT INSTALLED")
                                                 .color(egui::Color32::DARK_GRAY)
                                                 .size(10.0)
                                         );
                                     }
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
            } // if let Some(game)
            } // if let Some(&game_idx) = filtered_games
        }

        if let Some((aid, name)) = delete_req {
            drop(games); // Drop lock before mutating self
            app.initiate_delete(aid, name);
        }
    }); // End ScrollArea

        // GOLDBERG MODAL
        let cand_id = app.goldberg_candidate_id.clone();
        if app.goldberg_modal_open {
            if let Some(appid) = cand_id {
                let ctx = ui.ctx().clone();
                egui::Window::new(egui::RichText::new("🛡 GOLDBERG EMULATOR SETUP").strong().color(egui::Color32::YELLOW))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(&ctx, |ui| {
                        ui.label("Configure Offline Wrapper Settings:");
                        ui.add_space(10.0);
                        
                        ui.label("Username (Visible inside game):");
                        ui.text_edit_singleline(&mut app.goldberg_user_input);
                        
                        ui.label("SteamID (64-bit ID):");
                        ui.text_edit_singleline(&mut app.goldberg_steamid_input);
                        ui.small("Default is recommended for compatibility.");

                        ui.add_space(5.0);
                        ui.checkbox(&mut app.goldberg_use_64bit, "Deploy 64-bit DLL (Standard)");
                        
                        ui.add_space(15.0);
                        ui.horizontal(|ui| {
                            if ui.button("CANCEL").clicked() {
                                app.goldberg_modal_open = false;
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(egui::RichText::new("🚀 DEPLOY FIX").strong().color(egui::Color32::GREEN)).clicked() {
                                     // DEPLOYMENT LOGIC
                                     let steam_path = app.config.steam_path.clone();
                                     if let Some(game_path) = crate::game_path::GamePathFinder::find_game_path(&steam_path, &appid) {
                                         let mut success = true;
                                         
                                         // 1. Core Files
                                         let aid_u32 = appid.parse::<u32>().unwrap_or(0);
                                         if let Err(e) = app.goldberg.deploy(&game_path, aid_u32, app.goldberg_use_64bit) { 
                                             app.log(format!("Goldberg Deploy Error: {}", e));
                                             success = false;
                                         }
                                         
                                         // 2. Ticket Gen
                                         if success {
                                             if let Err(e) = app.goldberg.generate_ticket(aid_u32, &game_path) {
                                                 app.log(format!("Ticket Gen Error: {}", e));
                                                 // Non-fatal, but warn
                                             } else {
                                                 app.log("✅ Encrypted AppTicket generated successfully.".to_string());
                                             }
                                         }
                                         
                                         // 3. User Config (Username/ID)
                                         if success {
                                             let settings_dir = game_path.join("steam_settings");
                                             let _ = std::fs::create_dir_all(&settings_dir);
                                             
                                             // force_account_name.txt
                                             if !app.goldberg_user_input.is_empty() {
                                                 let _ = std::fs::write(settings_dir.join("force_account_name.txt"), &app.goldberg_user_input);
                                             }
                                             
                                             // force_steamid.txt (optional, usually user_steam_id.txt)
                                             // Goldberg uses user_steam_id.txt usually containing just the ID
                                              if !app.goldberg_steamid_input.is_empty() && app.goldberg_steamid_input.chars().all(char::is_numeric) {
                                                 let _ = std::fs::write(settings_dir.join("user_steam_id.txt"), &app.goldberg_steamid_input);
                                             }
                                             
                                             
                                             // 4. Achievement Downloader (Async Background)
                                             let client_opt = app.api_client.clone();
                                             let g_gen = app.goldberg.clone();
                                             let appid_c = appid.clone();
                                             let gp_c = game_path.clone();
                                             let log_arc = app.system_log.clone();

                                             std::thread::spawn(move || {
                                                 if let Some(client) = client_opt {
                                                     if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                                                         .enable_all()
                                                         .build() 
                                                     {
                                                         // Log start
                                                         if let Ok(mut logs) = log_arc.lock() {
                                                              push_log(&mut logs, format!("🏆 Fetching Achievements for {}...", appid_c));
                                                         }

                                                         match rt.block_on(g_gen.download_achievements(&appid_c, &client, &gp_c)) {
                                                             Ok(msg) => {
                                                                  if let Ok(mut logs) = log_arc.lock() {
                                                                      push_log(&mut logs, format!("✅ Achievements: {}", msg));
                                                                  }
                                                              },
                                                             Err(e) => {
                                                                  if let Ok(mut logs) = log_arc.lock() {
                                                                      push_log(&mut logs, format!("⚠️ Achievement Download Error: {}", e));
                                                                  }
                                                             }
                                                         }
                                                     }
                                                 } else {
                                                      if let Ok(mut logs) = log_arc.lock() {
                                                          push_log(&mut logs, "❌ API Client not initiated. Cannot download achievements.".to_string());
                                                      }
                                                 }
                                             });
                                             
                                             app.log("Goldberg Emulator deployed.".to_string());
                                         }
                                     } else {
                                        app.log("Game path not found!".to_string());
                                     }
                                     app.goldberg_modal_open = false;
                                }
                            });
                        });
                    });
            }
        }
}
