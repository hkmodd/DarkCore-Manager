#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console in release

mod api;
mod app_list;
mod cache;
mod config;
mod profiles;
mod steamless;
mod game_path;
mod injector;
mod ui;
mod vdf_injector;

use ui::DarkCoreApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Load Icon
    let icon_data = if let Ok(img) = image::open("icon.ico") {
        let img = img.to_rgba8();
        Some(eframe::egui::IconData {
            rgba: img.as_raw().to_vec(),
            width: img.width(),
            height: img.height(),
        })
    } else {
        None
    };

    let viewport = eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]) // Wider for Sidebar
            .with_resizable(true)
            .with_title("DarkCore Manager v1.2");

    let viewport = if let Some(icon) = icon_data {
        viewport.with_icon(icon)
    } else {
        viewport
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "DarkCore Manager v1.2",
        options,
        Box::new(|cc| Ok(Box::new(DarkCoreApp::new(cc)))),
    )
}
