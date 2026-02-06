//! ABOUT panel (Info tab).
//!
//! This panel displays the "Matrix rain" easter egg with the DarkCore manifesto.
//! It's a purely visual/decorative panel with no user interaction.

use crate::ui::state::MatrixTrail;
use eframe::egui;

/// Renders the ABOUT tab with the Matrix rain animation and manifesto overlay.
///
/// # Arguments
/// * `app` - Mutable reference to the application state
/// * `ui` - Mutable reference to the egui UI context
pub fn render(app: &mut crate::ui::state::DarkCoreApp, ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let time = ui.input(|i| i.time);

    if app.active_tab == 5 {
        ui.ctx().request_repaint();
    }

    // Deep Black Background
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_rgb(2, 2, 5));

    let rand_pseudo =
        |seed: usize| -> usize { (seed.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fffffff };

    // Extended Glyph Set (Katakana-ish + numbers)
    let glyphs =
        "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM0123456789<>:;[]{}!@#$%^&*=+-_|?";
    let random_matrix_char = |seed: usize| -> char {
        glyphs
            .chars()
            .nth(seed % glyphs.chars().count())
            .unwrap_or('X')
    };

    // INITIAL POPULATION (Heavy Density)
    if app.matrix_trails.is_empty() {
        for i in 0..crate::ui::state::MAX_MATRIX_TRAILS {
            let layer = (i % 3) as u8;

            let speed_base = match layer {
                0 => 1.0,
                1 => 2.5,
                _ => 4.5,
            };
            let speed = speed_base + (i % 7) as f32 * 0.3;
            let x = (i as f32 * 13.0 * (layer as f32 + 1.2) + (time * 100.0) as f32) % rect.width()
                + rect.min.x;
            let h_y = rect.min.y + (i as f32 * 7.0) % rect.height();
            let len = 10 + (i % 30);

            let mut chars = Vec::new();
            for k in 0..len {
                chars.push(random_matrix_char(i + k));
            }

            app.matrix_trails.push(MatrixTrail {
                x,
                head_y: h_y,
                speed,
                len,
                chars,
                layer,
            });
        }
    }

    // SPAWN NEW TRAILS
    if app.matrix_trails.len() < crate::ui::state::MAX_MATRIX_TRAILS {
        let seed = (time * 10000.0) as usize;
        if rand_pseudo(seed) % 100 < 60 {
            let layer_roll = rand_pseudo(seed + 1) % 100;
            let layer = if layer_roll < 50 {
                0
            } else if layer_roll < 85 {
                1
            } else {
                2
            };

            let x = rect.min.x + (rand_pseudo(seed + 2) % (rect.width() as usize)) as f32;
            let speed_base = match layer {
                0 => 1.0,
                1 => 2.5,
                _ => 4.5,
            };
            let speed = speed_base + (rand_pseudo(seed + 3) as f32 % 5.0) * 0.4;
            let len = 10 + (rand_pseudo(seed + 4) % 40);

            let mut chars = Vec::new();
            for k in 0..len {
                chars.push(random_matrix_char(seed + k));
            }

            app.matrix_trails.push(MatrixTrail {
                x,
                head_y: rect.min.y - 150.0,
                speed,
                len,
                chars,
                layer,
            });
        }
    }

    // UPDATE & RENDER
    let painter = ui.painter();

    // Layer Configs
    let font_small = egui::FontId::monospace(10.0);
    let font_mid = egui::FontId::monospace(14.0);
    let font_large = egui::FontId::monospace(18.0);

    let white = egui::Color32::WHITE;
    let neon_green = egui::Color32::from_rgb(50, 255, 50);

    app.matrix_trails.retain_mut(|trail| {
        trail.head_y += trail.speed;

        // Random mutation
        if rand_pseudo((trail.head_y * 10.0) as usize) % 15 == 0 {
            let idx = rand_pseudo((time * 1000.0) as usize) % trail.len;
            trail.chars[idx] = random_matrix_char((time * 999.0) as usize);
        }

        let (font, char_h, opacity_mult) = match trail.layer {
            0 => (&font_small, 10.0, 0.3),
            1 => (&font_mid, 14.0, 0.7),
            _ => (&font_large, 18.0, 1.0),
        };

        // Draw Chars
        for (i, &c) in trail.chars.iter().enumerate() {
            let y_pos = trail.head_y - (i as f32 * char_h);
            if y_pos > rect.max.y {
                continue;
            }
            if y_pos < rect.min.y - char_h {
                break;
            }

            let color;
            if i == 0 {
                color = white.linear_multiply(opacity_mult);
                if trail.layer == 2 {
                    painter.text(
                        egui::pos2(trail.x, y_pos),
                        egui::Align2::CENTER_TOP,
                        c,
                        font.clone(),
                        white.linear_multiply(0.4),
                    );
                }
            } else if i < 3 {
                color = neon_green.linear_multiply(opacity_mult);
            } else {
                let fade = 1.0 - (i as f32 / trail.len as f32);
                color = neon_green.linear_multiply((fade * fade) * opacity_mult);
            }

            painter.text(
                egui::pos2(trail.x, y_pos),
                egui::Align2::CENTER_TOP,
                c,
                font.clone(),
                color,
            );
        }

        let tail_y = trail.head_y - (trail.len as f32 * char_h);
        tail_y < rect.max.y
    });

    // MANIFESTO OVERLAY
    let center = rect.center();
    let wrap_width = 550.0;

    let galley = painter.layout_job(
        egui::text::LayoutJob::simple(
            "WE ARE THE ORCHESTRATORS.\n\nSteam is the cage. DarkCore is the key.\nWe build bridges where they built walls.\nWe play what we want, when we want.\n\nPower to the Players.\n\nSigned, SEBASTIAN.".to_string(),
            egui::FontId::monospace(15.0),
            egui::Color32::from_rgb(220, 255, 220),
            wrap_width
        )
    );

    let text_rect = egui::Rect::from_center_size(center, galley.size() + egui::vec2(80.0, 80.0));

    // Advanced Box Rendering
    painter.rect_filled(text_rect, 2.0, egui::Color32::from_black_alpha(245));
    painter.rect_stroke(text_rect, 2.0, egui::Stroke::new(2.0, neon_green));

    // Outer Glow
    for i in 1..5 {
        let width = 2.0 + i as f32 * 2.0;
        let alpha = 60 / i;
        painter.rect_stroke(
            text_rect.expand(i as f32),
            2.0,
            egui::Stroke::new(width, neon_green.linear_multiply(alpha as f32 / 255.0)),
        );
    }

    painter.galley(
        text_rect.min + egui::vec2(40.0, 40.0),
        galley,
        egui::Color32::WHITE,
    );
}
