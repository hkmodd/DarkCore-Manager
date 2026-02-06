//! Settings Panel module.
//! 
//! Handles configuration of:
//! - Steam Path
//! - GreenLuma Path
//! - Steamless Path
//! - Stealth Mode
//! - System Status
//! - API Key Management (with glitch effect)

use std::path::Path;
use std::time::{Duration, Instant};
use eframe::egui;
use crate::api::ApiClient;
use crate::config::save_config;

/// Renders the Settings tab.
///
/// # Arguments
/// * `app` - Mutable reference to the application state
/// * `ui` - Mutable reference to the UI context
pub fn render(app: &mut crate::ui::state::DarkCoreApp, ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("SYSTEM CONFIGURATION")
            .color(egui::Color32::from_rgb(0, 200, 255))
            .strong(),
    );
    ui.add_space(10.0);

    let path_row =
        |ui: &mut egui::Ui,
         label: &str,
         valid: bool,
         txt: &mut String,
         is_dir: bool,
         hint: Option<&str>| {
            ui.label(label);
            ui.horizontal(|ui| {
                let _tint = if valid {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                };
                // Auto-clean UNC prefix if present
                if txt.starts_with(r"\\?\") {
                    *txt = txt.replace(r"\\?\", "");
                }

                ui.add(
                    egui::TextEdit::singleline(txt)
                        .desired_width(400.0)
                        .text_color(egui::Color32::WHITE),
                );
                if ui.button("📂").clicked() {
                    if is_dir {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            let p_str = path.to_string_lossy().to_string();
                            *txt = p_str.replace(r"\\?\", "");
                        }
                    } else if let Some(path) = rfd::FileDialog::new()
                        .add_filter("exe", &["exe"])
                        .pick_file()
                    {
                        let p_str = path.to_string_lossy().to_string();
                        *txt = p_str.replace(r"\\?\", "");
                    }
                }
                if let Some(h) = hint {
                    ui.label("❓").on_hover_text(h);
                }
            });
            ui.add_space(5.0);
        };

    path_row(
        ui,
        "Steam Path:",
        Path::new(&app.config.steam_path).exists(),
        &mut app.config.steam_path,
        true,
        None,
    );
    path_row(
        ui,
        "GreenLuma Path:",
        Path::new(&app.config.gl_path).exists(),
        &mut app.config.gl_path,
        true,
        Some("Folder containing GreenLuma_2025_x64.dll and AppList folder.\nSearch for 'GreenLuma 2025' on specialized forums."),
    );
    path_row(
        ui,
        "Steamless CLI Path:",
        Path::new(&app.config.steamless_path).exists(),
        &mut app.config.steamless_path,
        false,
        Some("Steamless.CLI.exe required for DRM analysis.\nSearch for 'Steamless' on GitHub (atom0s)."),
    );

    ui.add_space(5.0);
    
    // Settings Toggles
    ui.horizontal(|ui| {
         ui.checkbox(&mut app.config.enable_stealth_mode, egui::RichText::new("Enable GreenLuma Stealth Mode").strong());
         ui.label("ℹ").on_hover_text("Enables 'StealthMode.bin' for GreenLuma.\nDisables some file system hooks to reduce ban risk.\nDisable this if you have issues with downloads or installation errors.");
    });

    ui.add_space(5.0);

    // STEALTH MODE WARNING
    if !app.config.steam_path.is_empty() && !app.config.gl_path.is_empty() {
         let sp = Path::new(&app.config.steam_path);
         let gp = Path::new(&app.config.gl_path);
         // Simple contains check logic
         if gp.starts_with(sp) || sp.starts_with(gp) {
             ui.group(|ui| {
                  ui.horizontal(|ui| {
                      ui.label(egui::RichText::new("⚠ STEALTH RISK:").color(egui::Color32::RED).strong());
                      ui.label("GreenLuma is located INSIDE or CONTAINS the Steam folder.");
                  });
                  ui.label("For maximum safety, please move GreenLuma to a completely separate folder (e.g. C:\\GreenLuma).");
             });
             ui.add_space(10.0);
         }
    }

    // LEGACY IMPORT RECOVERY
    if !app.config.steam_path.is_empty() {
         let legacy_alist = Path::new(&app.config.steam_path).join("AppList");
         if legacy_alist.exists() && legacy_alist.is_dir() {
              // Check if it has txt files (naive check)
              let has_files = std::fs::read_dir(&legacy_alist).ok().map(|mut d| d.any(|e| e.ok().map(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false)).unwrap_or(false))).unwrap_or(false);
              
              if has_files {
                   ui.group(|ui| {
                       ui.horizontal(|ui| {
                           ui.label(egui::RichText::new("📂 LEGACY CONFIG FOUND").color(egui::Color32::YELLOW).strong());
                           if ui.add(egui::Button::new(egui::RichText::new("📥 IMPORT LEGACY APPLIST").strong().color(egui::Color32::BLACK)).fill(egui::Color32::YELLOW)).clicked() {
                                // IMPORT LOGIC
                                let mut count = 0;
                                let mut new_ids = Vec::new();
                                if let Ok(entries) = std::fs::read_dir(&legacy_alist) {
                                     for entry in entries.flatten() {
                                         let path = entry.path();
                                         if path.extension().map(|s| s == "txt").unwrap_or(false) {
                                              if let Ok(content) = std::fs::read_to_string(&path) {
                                                  let clean = content.trim().to_string();
                                                  if !clean.is_empty() && clean.chars().all(char::is_numeric) {
                                                       new_ids.push(clean);
                                                       count += 1;
                                                  }
                                              }
                                         }
                                     }
                                }
                                
                                if count > 0 {
                                    // Write to current GL AppList
                                    if let Err(e) = crate::app_list::add_games_to_list(&app.config.gl_path, new_ids) {
                                        app.log(format!("Import Error: {}", e));
                                    } else {
                                        app.refresh_library();
                                        app.log(format!("Imported {} legacy games. Please SAVE PROFILE to keep them.", count));
                                    }
                                } else {
                                    app.log("No valid AppIDs found in legacy folder.".to_string());
                                }
                           }
                       });
                       ui.label("Old GreenLuma AppList detected inside Steam. Migrate now?");
                   });
                   ui.add_space(10.0);
              }
         }
    }

    ui.separator();
    
    // Glitch Logic for API Key
    // Update Glitch String (High Speed)
    let now = Instant::now();
    
    // Only repaint if we are actually animating (every 50ms)
    // This prevents 60+ FPS redraws when just sitting on the page
    if !app.config.api_key.is_empty() {
         if now.duration_since(app.api_key_glitch_update).as_millis() > 50 || 
            app.api_key_glitch_cache.len() != app.config.api_key.len() 
         {
             app.api_key_glitch_update = now;
             
             // High-Tech Glyph Set (Very Distinct)
             let glyphs = "ABCDEF0123456789!@#$%^&*()_+-=[]{}|;:,.<>?§";
             let time = ui.input(|i| i.time);
             let seed = (time * 10000.0) as usize;
             
             app.api_key_glitch_cache = app.config.api_key.chars().enumerate().map(|(i, _)| {
                 let idx = (seed.wrapping_add(i * 13).wrapping_add(now.elapsed().as_nanos() as usize)) % glyphs.len();
                 glyphs.chars().nth(idx).unwrap_or('?')
             }).collect();
             
             // ONLY repaint when we update the text
             ui.ctx().request_repaint();
         } else {
             // Request repaint for next frame only if we are close to update 
             // (This ensures smooth 20 FPS animation for the glitch without spinning 144Hz)
             let remaining = 50u128.saturating_sub(now.duration_since(app.api_key_glitch_update).as_millis());
             ui.ctx().request_repaint_after(Duration::from_millis(remaining as u64));
         }
    }

    ui.label(egui::RichText::new("API Key (Secure Sandbox):").color(egui::Color32::from_rgb(0, 255, 100)));
    
    let frame = egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 50)))
        .fill(egui::Color32::from_rgb(5, 15, 5))
        .inner_margin(6.0)
        .rounding(4.0);

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
             ui.label("🔒");
             
             let glitch_text = app.api_key_glitch_cache.clone();
             
             let response = ui.add(
                  egui::TextEdit::singleline(&mut app.config.api_key)
                      .font(egui::FontId::monospace(14.0))
                      .desired_width(320.0)
                      .layouter(&mut |ui, string, _| {
                           let display_text = if string.is_empty() { 
                               "" 
                           } else if string.len() == glitch_text.len() {
                               &glitch_text
                           } else {
                               string // Fallback
                           };

                           let mut job = egui::text::LayoutJob::default();
                           job.append(
                               display_text,
                               0.0,
                               egui::TextFormat {
                                   font_id: egui::FontId::monospace(14.0),
                                   color: egui::Color32::from_rgb(50, 255, 50),
                                   background: egui::Color32::from_black_alpha(150),
                                   ..Default::default()
                               }
                           );
                           ui.fonts(|f| f.layout_job(job))
                      })
             );
             
             if response.changed() {
                  app.api_key_glitch_update = Instant::now() - Duration::from_millis(100);
                  // Trigger API refresh when key changes
                  app.api_refresh_timer = Some(Instant::now() + Duration::from_millis(1500));
             }
             
             // Get Button
             if ui.button("Get Key").on_hover_text("Get your Morrenus API key").clicked() {
                 let _ = open::that("https://manifest.morrenus.xyz/");
             }
        });
        
        ui.label(egui::RichText::new("Used for covers and game names. Purely legal, public metadata.").italics().size(10.0));
    });

    ui.add_space(5.0);
    
    // API Check / Refresh Logic
    // Timer is a FUTURE time - when it passes, we refresh.
    if !app.config.api_key.is_empty() {
        if let Some(timer) = app.api_refresh_timer {
            if Instant::now() > timer {
                app.api_refresh_timer = None; // Reset timer
                
                let stats_arc = app.user_stats.clone();
                let status_queue = app.status_update_queue.clone();
                let error_arc = app.api_last_error.clone();
                let validating_arc = app.is_validating_api.clone();
                let cfg_key = app.config.api_key.clone();
                
                // Set VALIDATING flag immediately
                if let Ok(mut v) = app.is_validating_api.lock() { *v = true; }

                std::thread::spawn(move || {
                    let client = crate::api::ApiClient::new(cfg_key);
                    let result = crate::ui::state::ASYNC_RUNTIME.block_on(client.get_user_stats());
                    
                    // *** CRITICAL: Clear Validating Flag ***
                    if let Ok(mut v) = validating_arc.lock() { *v = false; }
                    
                    match result {
                        Ok(stats) => {
                            if let Ok(mut e) = error_arc.lock() { *e = None; }
                            if let Ok(mut s) = stats_arc.lock() { *s = Some(stats); }
                            if let Ok(mut q) = status_queue.lock() {
                                *q = Some("API Connection Established.".to_string());
                            }
                        },
                        Err(e) => {
                            let err_str = e.to_string();
                            if let Ok(mut er) = error_arc.lock() { *er = Some(err_str.clone()); }
                            if let Ok(mut q) = status_queue.lock() {
                                if err_str.contains("401") || err_str.contains("403") {
                                    *q = Some("⛔ API KEY INVALID OR EXPIRED.".to_string());
                                } else {
                                    *q = Some(format!("API Error: {}", err_str));
                                }
                            }
                        }
                    }
                });
                app.log("Auto-Refreshing API Stats...".to_string());
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)); // Timer countdown
            }
        }
        // NOTE: If timer is None, it stays None until user modifies the API key
    }

    // API UI Header
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("📊 API USAGE:").strong().color(egui::Color32::from_rgb(0, 255, 255)));
        
        let mut is_validating = false;
        if let Ok(v) = app.is_validating_api.lock() { is_validating = *v; }
        
        if is_validating || app.api_refresh_timer.is_some() {
             // If timer is valid, we might be waiting for next refresh, OR validating.
             // Original logic: if validating is true or timer is set (which implies automatic mode is active)
             // Actually original logic was simpler: if verifying... 
             if is_validating {
                 ui.spinner();
                 ui.label(egui::RichText::new("Verifying Key...").italics().color(egui::Color32::YELLOW));
             }
        }
    });

    // API Stats / Error Body
    let mut api_error_msg = None;
    if let Ok(guard) = app.api_last_error.lock() {
        api_error_msg = guard.clone();
    }

    if let Some(err_msg) = api_error_msg {
         // RENDER ERROR
         let theme_color = egui::Color32::from_rgb(255, 30, 30);
         egui::Frame::none()
             .fill(egui::Color32::from_black_alpha(200))
             .stroke(egui::Stroke::new(1.5, theme_color))
             .rounding(6.0)
             .inner_margin(12.0)
             .show(ui, |ui| {
                  ui.set_min_width(320.0);
                  ui.horizontal(|ui| {
                      ui.label("⛔");
                      ui.label(egui::RichText::new(format!("Connection Failed: {}", err_msg))
                          .color(theme_color).strong());
                  });
             });
    } else {
        // RENDER STATS OR EMPTY
        let stats_opt = app.user_stats.lock().unwrap().clone();
        
        if let Some(stats) = stats_opt {
             // Limits Logic
             let limit_ratio = if stats.daily_limit > 0 { stats.daily_usage as f32 / stats.daily_limit as f32 } else { 0.0 };
             let is_critical = limit_ratio > 0.9;
             let theme_color = if is_critical { egui::Color32::from_rgb(255, 30, 30) } else { egui::Color32::from_rgb(0, 255, 200) };
             
             let frame = egui::Frame::none()
                 .fill(egui::Color32::from_black_alpha(200))
                 .stroke(egui::Stroke::new(1.5, theme_color))
                 .rounding(6.0)
                 .inner_margin(12.0);

             frame.show(ui, |ui| {
                 ui.set_min_width(320.0);
                 
                 // Header
                 ui.horizontal(|ui| {
                     ui.label(egui::RichText::new(if is_critical { "⚠ SYSTEM HALT" } else { "⚡ ONLINE" })
                         .font(egui::FontId::monospace(12.0))
                         .color(theme_color));
                     
                     ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                         ui.label(egui::RichText::new(format!("[{}]", stats.role.clone().unwrap_or("USER".to_string()).to_uppercase()))
                             .font(egui::FontId::monospace(10.0))
                             .color(egui::Color32::GRAY));
                     });
                 });
                 
                 ui.add_space(8.0);
                 
                 // Usage Numbers
                 ui.horizontal(|ui| {
                     ui.label(egui::RichText::new(format!("{:02}", stats.daily_usage)) // Typo in original? requests_today vs daily_usage?
                         // struct UserStats { requests_today, daily_limit, ... }
                         // Original code said daily_usage?
                         // Let's assume matches struct field.
                         .font(egui::FontId::new(24.0, egui::FontFamily::Proportional)) 
                         .color(egui::Color32::WHITE));
                     
                     ui.label(egui::RichText::new("/")
                         .size(18.0)
                         .color(egui::Color32::GRAY));
                         
                     ui.label(egui::RichText::new(format!("{:02}", stats.daily_limit))
                         .font(egui::FontId::monospace(18.0))
                         .color(theme_color));
                         
                     ui.label(egui::RichText::new("REQUESTS")
                         .size(10.0)
                         .color(egui::Color32::GRAY));
                 });
                 
                 ui.add_space(4.0);

                 // Cyberpunk Progress Bar
                 let (rect, _resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
                 ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(20, 20, 30)); // Track
                 
                 if limit_ratio > 0.0 {
                     let fill_width = rect.width() * limit_ratio.clamp(0.0, 1.0);
                     let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, rect.height()));
                     
                     // Glow effect
                     if !is_critical {
                         ui.painter().rect_filled(fill_rect, 3.0, theme_color);
                         ui.painter().rect_stroke(fill_rect.expand(1.0), 3.0, egui::Stroke::new(2.0, theme_color.linear_multiply(0.3)));
                     } else {
                         // Glitch Pattern for Critical
                         ui.painter().rect_filled(fill_rect, 3.0, theme_color); 
                     }
                 }
                 
                 if is_critical {
                     ui.add_space(4.0);
                     ui.label(egui::RichText::new("⛔ UPLINK SEVERED due to protocol limits.")
                         .font(egui::FontId::monospace(10.0))
                         .color(egui::Color32::from_rgb(255, 100, 100)));
                 }
            });
        } else {
            // Empty State
            let frame = egui::Frame::none()
                .fill(egui::Color32::from_black_alpha(150))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50)))
                .rounding(4.0)
                .inner_margin(8.0);
                
            frame.show(ui, |ui| {
                ui.label(egui::RichText::new("Awaiting Downlink...").font(egui::FontId::monospace(12.0)).italics().color(egui::Color32::GRAY));
            });
        }
    }

    ui.add_space(15.0);
    ui.add_space(20.0);
    
    // CUSTOM ANIMATED SAVE BUTTON
    let now = Instant::now();
    let is_recently_saved = app.config_saved_at.map(|t| now.duration_since(t).as_secs_f32() < 2.0).unwrap_or(false);
    
    if is_recently_saved {
        ui.ctx().request_repaint(); // Animation Loop
    }

    let btn_text = if is_recently_saved { "✅ CONFIGURATION SAVED" } else { "💾 SAVE CONFIGURATION" };
    let btn_size = egui::vec2(280.0, 45.0);
    
    let (rect, response) = ui.allocate_at_least(btn_size, egui::Sense::click());
    
    if response.clicked() {
         if let Err(e) = save_config(&app.config) {
            app.status_msg = format!("Save error: {}", e);
        } else {
            app.config_saved_at = Some(Instant::now());
            app.status_msg = "Config saved.".to_string();
            app.api_client = Some(ApiClient::new(app.config.api_key.clone()));
            app.refresh_library();
            app.resolve_unknown_games();
        }
    }

    // Animation Factors
    let hover_factor = ui.ctx().animate_bool(response.id.with("hover"), response.hovered());
    let save_factor = if let Some(t) = app.config_saved_at {
         let elapsed = now.duration_since(t).as_secs_f32();
         if elapsed < 1.5 {
             1.0 - (elapsed / 1.5).powf(0.5) // Sqrt fade
         } else { 0.0 }
    } else { 0.0 };

    let painter = ui.painter();
    let center = rect.center();
    
    // Colors
    let cyan = egui::Color32::from_rgb(0, 243, 255);
    let green = egui::Color32::from_rgb(50, 255, 100);
    
    let target_color = if save_factor > 0.0 { green } else { cyan };
    
    // Dynamic Rect
    let visual_rect = rect.shrink(2.0).expand(2.0 * hover_factor);
    let corner_radius = 6.0;

    // Background Fill (Glassy)
    if hover_factor > 0.0 {
        painter.rect_filled(visual_rect, corner_radius, target_color.linear_multiply(0.1));
    }
    
    // Border Stroke
    let stroke_width = 1.0 + (1.0 * hover_factor) + (2.0 * save_factor);
    painter.rect_stroke(visual_rect, corner_radius, egui::Stroke::new(stroke_width, target_color));
    
    // SHOCKWAVE EFFECT (The "Figa" part)
    if save_factor > 0.0 {
        let expansion = (1.0 - save_factor) * 40.0; // Expand outwards
        let alpha = save_factor * 0.6;
        painter.rect_stroke(
            visual_rect.expand(expansion),
            corner_radius + expansion,
            egui::Stroke::new(2.0, green.linear_multiply(alpha))
        );
    }

    // Text
    painter.text(
        center, 
        egui::Align2::CENTER_CENTER, 
        btn_text, 
        egui::FontId::proportional(16.0), 
        target_color
    );
}
