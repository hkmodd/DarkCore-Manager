use eframe::egui;

pub fn render(app: &crate::ui::state::DarkCoreApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let state = app.download_state.clone();
    let mut progress_text = String::new();
    let mut progress_val = 0.0;
    let mut is_active = false;

    if let Ok(mut s) = state.lock() {
        // SPEED CALCULATION LOGIC
        // Check if we need to update speed stats (every 500ms)
        let mut update_speed = false;
        let mut new_speed = 0.0;
        let cur_bytes: u64;

        if let crate::direct_download::state::DownloadStatus::Downloading {
            bytes_downloaded, ..
        } = &s.status
        {
            let now = std::time::Instant::now();
            if now.duration_since(s.last_update).as_millis() > 500 {
                update_speed = true;
                let elapsed = now.duration_since(s.last_update).as_secs_f32().max(0.001);
                let delta = bytes_downloaded.saturating_sub(s.last_bytes_snapshot);
                new_speed = (delta as f32 / elapsed) / 1_048_576.0;
                cur_bytes = *bytes_downloaded;

                // Update snapshot
                s.last_update = now;
                s.last_bytes_snapshot = cur_bytes;
            }
        }

        if update_speed {
            if let crate::direct_download::state::DownloadStatus::Downloading {
                speed_mbps, ..
            } = &mut s.status
            {
                *speed_mbps = new_speed;
            }
        }

        match &s.status {
            crate::direct_download::state::DownloadStatus::Idle => {}
            crate::direct_download::state::DownloadStatus::Downloading {
                bytes_total,
                bytes_downloaded,
                ..
            } => {
                is_active = true;
                progress_text = format!("{} - {}", s.pretty_bytes(), s.pretty_speed());
                if *bytes_total > 0 {
                    progress_val = *bytes_downloaded as f32 / *bytes_total as f32;
                }
            }
            crate::direct_download::state::DownloadStatus::Initializing => {
                is_active = true;
                progress_text = "Initializing...".to_string();
            }
            crate::direct_download::state::DownloadStatus::FetchingManifest => {
                is_active = true;
                progress_text = "Fetching Manifests...".to_string();
            }
            crate::direct_download::state::DownloadStatus::Decrypting => {
                is_active = true;
                progress_text = "Decrypting...".to_string();
            }
            crate::direct_download::state::DownloadStatus::Verifying => {
                is_active = true;
                progress_text = "Verifying...".to_string();
            }
            crate::direct_download::state::DownloadStatus::Finalizing => {
                is_active = true;
                progress_text = "Finalizing...".to_string();
            }
            crate::direct_download::state::DownloadStatus::Completed => {
                ui.label(egui::RichText::new("✅ Download Complete").color(egui::Color32::GREEN));
            }
            crate::direct_download::state::DownloadStatus::Error(e) => {
                ui.label(egui::RichText::new(format!("❌ Error: {}", e)).color(egui::Color32::RED));
            }
            _ => {}
        }
    }

    if is_active {
        ui.separator();
        ui.label(egui::RichText::new("⚡ Direct Download").strong().small());
        ui.add(
            egui::ProgressBar::new(progress_val)
                .text(progress_text)
                .animate(true),
        );
        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // 60fps Animation
    }
}
