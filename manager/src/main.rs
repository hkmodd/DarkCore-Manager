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
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 950.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "DARKCORE MANAGER v10.4 Rust",
        options,
        Box::new(|cc| Ok(Box::new(DarkCoreApp::new(cc)))),
    )
}
