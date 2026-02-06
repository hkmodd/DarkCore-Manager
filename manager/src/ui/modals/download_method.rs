use eframe::egui;
use crate::ui::state::DarkCoreApp;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.download_method_modal_open { return; }
    
    let mut close = false;
    let mut action: Option<bool> = None; // Some(true) = Direct, Some(false) = Steam
    
    let game_name = app.pending_install.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    
    egui::Window::new(egui::RichText::new("📥 Download Method").strong())
        .open(&mut app.download_method_modal_open)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.heading(format!("Install {}", game_name));
            ui.label("Select how you want to download this game:");
            ui.add_space(10.0);
            
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("☁️ Steam Download").strong()).on_hover_text("Standard method. Tells Steam to download files. Requires Steam running.").clicked() {
                    action = Some(false);
                }
                if ui.button(egui::RichText::new("⚡ Direct Download").strong().color(egui::Color32::LIGHT_BLUE)).on_hover_text("Experimental. Downloads files directly from Steam CDN. Faster, doesn't require Steam to be running during download.").clicked() {
                    action = Some(true);
                }
            });
            
            ui.add_space(10.0);
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        
    if let Some(is_direct) = action {
        if let Some(pending) = app.pending_install.take() {
            if is_direct {
                app.spawn_direct_install(pending.appid, pending.name, pending.target_library, pending.install_dir_name, pending.selected_dlcs, pending.cached_zip, pending.hierarchy);
            } else {
                app.spawn_steam_install(pending.appid, pending.name, pending.target_library, pending.install_dir_name, pending.selected_dlcs, pending.cached_zip, pending.hierarchy);
            }
        }
        close = true;
    }
    
    if close {
        app.download_method_modal_open = false;
    }
}
