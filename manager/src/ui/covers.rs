//! Cover image loading with multi-source fallback.
//!
//! Handles cover URL generation and fallback placeholder creation.

/// URLs to try for game covers, in order of preference
pub fn get_cover_urls(appid: &str) -> Vec<String> {
    vec![
        // Best quality - vertical cover (Steam CDN)
        format!(
            "https://steamcdn-a.akamaihd.net/steam/apps/{}/library_600x900.jpg",
            appid
        ),
        // Good quality - horizontal header (Steam CDN)
        format!(
            "https://steamcdn-a.akamaihd.net/steam/apps/{}/header.jpg",
            appid
        ),
        // Medium quality - capsule (Steam CDN - Primary Fallback)
        format!(
            "https://steamcdn-a.akamaihd.net/steam/apps/{}/capsule_231x87.jpg",
            appid
        ),
        // Low quality - small capsule (Steam CDN - Last Resort)
        format!(
            "https://steamcdn-a.akamaihd.net/steam/apps/{}/capsule_184x69.jpg",
            appid
        ),
        // Tiny - logo (Steam CDN)
        format!(
            "https://steamcdn-a.akamaihd.net/steam/apps/{}/logo.png",
            appid
        ),
    ]
}

/// Generate a unique colored placeholder based on appid hash
/// Returns (width, height, rgba_pixels)
pub fn generate_placeholder(appid: &str) -> (u32, u32, Vec<u8>) {
    // Generate deterministic hash from appid
    let hash = appid
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));

    // Generate muted color from hash
    let hue = (hash % 360) as f32;
    // Saturation 0.3 (30%), Value 0.25 (25%) -> Dark muted color
    let (r, g, b) = hsv_to_rgb(hue, 0.4, 0.30);

    let w = 60u32;
    let h = 90u32;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);

    // Gradient effect (lighter at top for "glassy" look)
    for y in 0..h {
        let brightness = 1.0 + (1.0 - (y as f32 / h as f32)) * 0.4;
        let pr = ((r as f32 * brightness) as u8).min(255);
        let pg = ((g as f32 * brightness) as u8).min(255);
        let pb = ((b as f32 * brightness) as u8).min(255);

        for _ in 0..w {
            pixels.extend_from_slice(&[pr, pg, pb, 255]);
        }
    }

    (w, h, pixels)
}

/// Helper: Convert HSV color space to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
