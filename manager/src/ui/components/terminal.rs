use eframe::egui;

pub fn render(app: &crate::ui::state::DarkCoreApp, ui: &mut egui::Ui) {
    // POLISHED TERMINAL HEADER
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("▸ TERMINAL")
                .size(10.0)
                .monospace()
                .color(egui::Color32::from_rgb(80, 180, 180)),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Compact icon-only copy button
            if ui
                .small_button("📋")
                .on_hover_text("Copy logs to clipboard")
                .clicked()
            {
                if let Ok(logs) = app.system_log.lock() {
                    let full_log = logs.join("\n");
                    ui.ctx().output_mut(|o| o.copied_text = full_log);
                }
            }
        });
    });

    // Subtle separator line
    ui.add(egui::Separator::default().spacing(2.0));

    // SCROLLABLE LOG AREA - More compact
    egui::ScrollArea::vertical()
        .max_height(75.0) // Slightly more compact
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Dark background for terminal effect
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                egui::Color32::from_rgb(8, 8, 10),
            );

            if let Ok(logs) = app.system_log.lock() {
                for entry in logs.iter() {
                    // Colorize based on content
                    let color = if entry.contains("❌")
                        || entry.contains("Error")
                        || entry.contains("Failed")
                    {
                        egui::Color32::from_rgb(255, 80, 80)
                    } else if entry.contains("✅") || entry.contains("Success") {
                        egui::Color32::from_rgb(80, 255, 80)
                    } else if entry.contains("⚠️") || entry.contains("Warning") {
                        egui::Color32::from_rgb(255, 200, 50)
                    } else if entry.contains("🚀") {
                        egui::Color32::from_rgb(0, 255, 255)
                    } else {
                        egui::Color32::from_gray(140)
                    };

                    ui.label(
                        egui::RichText::new(entry)
                            .font(egui::FontId::monospace(10.0))
                            .color(color),
                    );
                }
            }
        });
}
