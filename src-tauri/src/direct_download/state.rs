use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadStatus {
    Idle,
    Initializing,
    FetchingManifest,
    Downloading {
        files_total: usize,
        files_done: usize,
        bytes_total: u64,
        bytes_downloaded: u64,
        speed_mbps: f32,
    },
    Decrypting,
    Verifying,
    Finalizing,
    Error(String),
    Paused,
    Completed,
}

pub struct DownloadState {
    pub status: DownloadStatus,
    pub active_game_id: Option<String>,
    pub start_time: Option<Instant>,
    pub last_update: Instant,
    pub last_bytes_snapshot: u64,
    pub target_dir: std::path::PathBuf,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            status: DownloadStatus::Idle,
            active_game_id: None,
            start_time: None,
            last_update: Instant::now(),
            last_bytes_snapshot: 0,
            target_dir: std::path::PathBuf::from("DarkCore Games"),
        }
    }
}

impl DownloadState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn pretty_speed(&self) -> String {
        match self.status {
            DownloadStatus::Downloading { speed_mbps, .. } => format!("{:.1} MB/s", speed_mbps),
            _ => "0.0 MB/s".to_string(),
        }
    }

    pub fn pretty_bytes(&self) -> String {
        match self.status {
            DownloadStatus::Downloading {
                bytes_downloaded,
                bytes_total,
                ..
            } => {
                format!(
                    "{:.1} / {:.1} MB",
                    bytes_downloaded as f64 / 1_048_576.0,
                    bytes_total as f64 / 1_048_576.0
                )
            }
            _ => "0 B".to_string(),
        }
    }
}
