#[derive(Clone)]
struct MatrixColumn {
    x: f32,
    head_y: f32,
    speed: f32,
    len: usize,
    chars: Vec<char>,
}

impl MatrixColumn {
    fn new(x: f32, h_start: f32) -> Self {
        let mut rng_seed = (x * h_start) as u64;
        let speed = 2.0 + (rng_seed % 5) as f32;
        let len = 10 + (rng_seed % 20) as usize;
        let chars = (0..len).map(|_| random_char(rng_seed)).collect();
        // Shift seed

        Self {
            x,
            head_y: h_start,
            speed,
            len,
            chars,
        }
    }
}

fn random_char(seed: u64) -> char {
    let chars = b"QWERTYUIOPASDFGHJKLZXCVBNM1234567890<>/[];:!@#$%^&*";
    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u64
        + seed) as usize
        % chars.len();
    chars[idx] as char
}
