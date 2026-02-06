use eframe::egui;
use crate::ui::state::DarkCoreApp;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if app.install_modal_open {
         // Clone data upfront to release borrow on app
         let candidate = app.install_candidate.clone();
         let libraries = app.detected_libraries.clone();
         
         if let Some((app_id, name)) = candidate {
              // FIX 4: Auto-fill with sanitized game name (Windows-safe folder name)
              if !app.install_modal_auto_scanned && app.install_dir_input.is_empty() {
                  app.install_modal_auto_scanned = true;
                  
                  // Sanitize the game name for Windows folder naming
                  // Invalid chars: \ / : * ? " < > |
                  let sanitized: String = name
                      .chars()
                      .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
                      .collect::<String>()
                      .trim()
                      .to_string();
                  
                  app.install_dir_input = sanitized;
              }
             let mut open = true;
             egui::Window::new(egui::RichText::new("💾 Select Installation Library").strong())
                 .open(&mut open)
                 .collapsible(false)
                 .resizable(false)
                 .fixed_size(egui::vec2(400.0, 200.0))
                 .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                 .show(ctx, |ui| {
                     ui.vertical_centered(|ui| {
                         ui.add_space(10.0);
                         ui.label(egui::RichText::new(format!("Installing/Repairing: {}", name)).size(14.0));
                         ui.label(egui::RichText::new("Please select the Steam Library where the game files are located:").color(egui::Color32::GRAY));
                         ui.add_space(15.0);
                         
                         if libraries.is_empty() {
                             ui.label(egui::RichText::new("⚠️ No libraries detected!").color(egui::Color32::RED));
                         }
                         
                         egui::ComboBox::from_label("Target Drive")
                             .selected_text(format!("{:?}", libraries.get(app.selected_library_index).unwrap_or(&std::path::PathBuf::from("None"))))
                             .show_ui(ui, |ui| {
                                 for (i, lib) in libraries.iter().enumerate() {
                                     ui.selectable_value(&mut app.selected_library_index, i, format!("{:?}", lib));
                                 }
                             });
                         
                         ui.add_space(20.0);
                         
                         // INSTALL DIR OVERRIDE
                         ui.label(egui::RichText::new("Installation Directory Name (Important!)").strong());
                         ui.label(egui::RichText::new("Use the exact folder name matching your 'common' folder (e.g. 'Expedition 33')").size(10.0).color(egui::Color32::GRAY));
                         ui.horizontal(|ui| {
                             ui.text_edit_singleline(&mut app.install_dir_input);
                             
                             // SCAN BUTTON
                             if ui.button("🔍 Scan").on_hover_text("Try to find existing folder in common").clicked() {
                                 if let Some(lib) = libraries.get(app.selected_library_index) {
                                      let common = lib.join("steamapps").join("common");
                                      if let Ok(entries) = std::fs::read_dir(common) {
                                          let mut best_match = String::new();
                                          let mut highest_score = 0;
                                          
                                          // Advanced "Brain" Scan Logic
                                          let clean_tokenize = |s: &str| -> Vec<String> {
                                              s.to_lowercase()
                                               .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "")
                                               .split_whitespace()
                                               .map(|s| s.to_string())
                                               .collect()
                                          };
                                          
                                          let name_tokens = clean_tokenize(&name);
                                          let name_clean = name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");

                                          for entry in entries.flatten() {
                                              if let Ok(meta) = entry.metadata() {
                                                  if meta.is_dir() {
                                                      let folder_name = entry.file_name().to_string_lossy().to_string();
                                                      // Skip common utility folders
                                                      if folder_name.eq_ignore_ascii_case("common") || folder_name.eq_ignore_ascii_case("Steamworks Shared") { continue; }

                                                      let folder_tokens = clean_tokenize(&folder_name);
                                                      let folder_clean = folder_name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");

                                                      // 1. Token Overlap
                                                      let matches = folder_tokens.iter().filter(|ft| name_tokens.contains(ft)).count();
                                                      
                                                      // 2. Substring Check (Robust against "The", ":", "-")
                                                      let is_substring = name_clean.contains(&folder_clean) && folder_clean.len() > 3;
                                                      
                                                      // Score Calculation
                                                      let mut score = matches * 10;
                                                      if is_substring { score += 50; }
                                                      if folder_clean == name_clean { score += 100; }
                                                      
                                                      // Update Candidate
                                                      if score > highest_score {
                                                          highest_score = score;
                                                          best_match = folder_name;
                                                      } else if score == highest_score && score > 0 {
                                                          // Tie-breaker: Prefer shorter names (usually the main game vs soundtrack/demo)
                                                          // UNLESS the name is extremely short (<3 chars)
                                                          if folder_name.len() < best_match.len() {
                                                              best_match = folder_name;
                                                          }
                                                      }
                                                  }
                                              }
                                          }
                                          
                                          if !best_match.is_empty() {
                                              app.install_dir_input = best_match;
                                          }
                                      }
                                 }
                             }
                         });
                         
                         ui.add_space(20.0);
                         
                         ui.horizontal(|ui| {
                             if ui.button("❌ Cancel").clicked() {
                                 app.install_modal_open = false;
                                 app.install_candidate = None;
                             }
                             
                             if ui.button(egui::RichText::new("✅ CONFIRM & INSTALL").strong().color(egui::Color32::GREEN)).clicked() {
                                 // Proceed with selected library and user-specified install dir
                                 if let Some(target) = libraries.get(app.selected_library_index) {
                                     app.install_game(app_id.clone(), name.clone(), Some(target.clone()), Some(app.install_dir_input.clone()));
                                     app.install_modal_open = false;
                                     app.install_candidate = None;
                                 }
                             }
                         });
                     });
                 });
                 
             if !open {
                 app.install_modal_open = false;
                 app.install_candidate = None;
                 app.install_modal_auto_scanned = false; // FIX 4: Reset for next open
             }
         }
    }
}
