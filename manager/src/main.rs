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
mod goldberg;
mod manifest_downloader;
mod vdf_injector;
mod vault;
mod watcher;
mod updater; // NEW: OTA Update System

// REMOVED: mod downloader (switched to SMD approach)
// REMOVED: mod manifest_parser (unused native parser)

use ui::DarkCoreApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Load Icon - Embedded at compile time for portability
    let icon_data = {
        let icon_bytes = include_bytes!("../icon.ico");
        // Parse ICO file - extract the largest image
        if let Ok(icon_dir) = ico::IconDir::read(std::io::Cursor::new(icon_bytes)) {
            // Find the largest icon
            if let Some(entry) = icon_dir.entries().iter().max_by_key(|e| e.width() * e.height()) {
                if let Ok(image) = entry.decode() {
                    Some(eframe::egui::IconData {
                        rgba: image.rgba_data().to_vec(),
                        width: image.width(),
                        height: image.height(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    let version_str = format!("DarkCore Manager v{}", env!("CARGO_PKG_VERSION"));

    let viewport = eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 950.0]) // Optimized for 1080p+ and Sidebar content
            .with_min_inner_size([1100.0, 720.0])
            .with_resizable(true)
            .with_title(&version_str); // Dynamic Title

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
        "DarkCore Manager v1.5",
        options,
        Box::new(|cc| Ok(Box::new(DarkCoreApp::new(cc)))),
    )
}
