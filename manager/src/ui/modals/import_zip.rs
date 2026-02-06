use crate::ui::state::DarkCoreApp;
use crate::ui::state::ImportedZipData;
use eframe::egui;

pub fn render(app: &mut DarkCoreApp, ctx: &egui::Context) {
    if !app.import_modal_open {
        return;
    }
    let Some(ref data) = app.import_zip_data else {
        return;
    };

    let game_name = data
        .script_data
        .app_name
        .clone()
        .unwrap_or("Unknown Game".to_string());
    let app_id = data.script_data.app_id.unwrap_or(0);
    let depot_count = data.script_data.depots.len();
    let manifest_count = data.manifest_bytes.len();

    let mut close_modal = false;
    let mut start_steam = false;
    let mut start_direct = false;

    egui::Window::new("📦 Import Morrenus ZIP")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.heading(&game_name);
            ui.label(format!("App ID: {}", app_id));
            ui.label(format!(
                "Depots: {} | Manifests found: {}",
                depot_count, manifest_count
            ));
            ui.separator();

            ui.label("Select Download Method:");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new("🎮 STEAM DOWNLOAD").strong())
                    .on_hover_text("Use GreenLuma + Steam to download files")
                    .clicked()
                {
                    start_steam = true;
                    close_modal = true;
                }

                if ui
                    .button(
                        egui::RichText::new("⚡ DIRECT DOWNLOAD")
                            .strong()
                            .color(egui::Color32::LIGHT_BLUE),
                    )
                    .on_hover_text("Download directly from Steam CDN (experimental)")
                    .clicked()
                {
                    start_direct = true;
                    close_modal = true;
                }
            });

            ui.separator();
            if ui.button("Cancel").clicked() {
                close_modal = true;
            }
        });

    // We can't move self methods easily if we are borrowing app.
    // So we use flag logic similar to original.

    if start_steam {
        start_steam_download_from_import(app);
    }
    if start_direct {
        start_direct_download_from_import(app);
    }
    if close_modal {
        app.import_modal_open = false;
        if !start_steam && !start_direct {
            app.import_zip_data = None;
        }
    }
}

pub fn handle_import_zip(app: &mut DarkCoreApp, path: std::path::PathBuf) {
    use std::io::Read;

    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            app.log(format!("Failed to open ZIP: {}", e));
            return;
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            app.log(format!("Invalid ZIP file: {}", e));
            return;
        }
    };

    let mut script_data: Option<crate::direct_download::lua_parser::ScriptData> = None;
    let mut manifest_bytes: std::collections::HashMap<u32, Vec<u8>> =
        std::collections::HashMap::new();

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let name = file.name().to_string();

            // Parse .lua or .txt file for script data
            if name.ends_with(".lua") || name.ends_with(".txt") {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(data) = crate::direct_download::lua_parser::parse_content(&content) {
                        script_data = Some(data);
                        app.log(format!("Parsed script: {}", name));
                    }
                }
            }

            // Store .manifest files (format: {depot_id}_{manifest_id}.manifest)
            if name.ends_with(".manifest") {
                // Extract depot_id from filename
                let basename = std::path::Path::new(&name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&name);
                if let Some(depot_id_str) = basename.split('_').next() {
                    if let Ok(depot_id) = depot_id_str.parse::<u32>() {
                        let mut bytes = Vec::new();
                        if file.read_to_end(&mut bytes).is_ok() {
                            manifest_bytes.insert(depot_id, bytes);
                            app.log(format!("Stored manifest for depot {}", depot_id));
                        }
                    }
                }
            }
        }
    }

    if let Some(data) = script_data {
        let game_name = data.app_name.clone().unwrap_or("Unknown Game".to_string());
        let app_id_str = data.app_id.map(|id| id.to_string()).unwrap_or_default();

        app.log(format!(
            "ZIP imported: {} ({} depots, {} manifests)",
            game_name,
            data.depots.len(),
            manifest_bytes.len()
        ));

        // Store ZIP data for later use
        app.import_zip_data = Some(ImportedZipData {
            script_data: data,
            manifest_bytes,
            source_path: path,
        });

        // FIX 9: Route through Manifestor for DLC selection (same as Search flow)
        if !app_id_str.is_empty() {
            crate::ui::modals::manifestor::open_manifestor(app, app_id_str, game_name);
        } else {
            // Fallback: Open legacy import modal if no AppID
            app.import_modal_open = true;
        }
    } else {
        app.log("No valid .lua/.txt file found in ZIP".to_string());
    }
}

fn start_steam_download_from_import(app: &mut DarkCoreApp) {
    let Some(data) = app.import_zip_data.take() else {
        return;
    };
    let app_id = data
        .script_data
        .app_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let name = data.script_data.app_name.unwrap_or("Unknown".to_string());

    app.log(format!(
        "Starting Steam Download for: {} ({})",
        name, app_id
    ));
    app.install_game(app_id, name, None, None);
}

fn start_direct_download_from_import(app: &mut DarkCoreApp) {
    let Some(data) = app.import_zip_data.take() else {
        return;
    };
    let app_id = data
        .script_data
        .app_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let name = data.script_data.app_name.unwrap_or("Unknown".to_string());

    app.log(format!(
        "Starting Direct Download for: {} ({})",
        name, app_id
    ));

    // Re-read ZIP as cached bytes for spawn_direct_install
    let cached_zip = std::fs::read(&data.source_path).ok();

    app.spawn_direct_install(
        app_id,
        name,
        None,   // target_library
        None,   // install_dir_name
        vec![], // selected_dlcs
        cached_zip,
        None, // hierarchy
    );
}
