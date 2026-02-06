//! Theme and styling constants.
//!
//! Color definitions, font sizes, and styling helpers for consistent UI appearance.

#![allow(dead_code)] // Reserved for future theming system

use eframe::egui;

// === COLOR PALETTE ===

/// Primary accent color (cyan/teal)
pub const COLOR_ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 200, 255);

/// Success color (green)
pub const COLOR_SUCCESS: egui::Color32 = egui::Color32::from_rgb(80, 255, 80);

/// Error color (red)
pub const COLOR_ERROR: egui::Color32 = egui::Color32::from_rgb(255, 80, 80);

/// Warning color (yellow/orange)
pub const COLOR_WARNING: egui::Color32 = egui::Color32::from_rgb(255, 200, 50);

/// Terminal header color
pub const COLOR_TERMINAL_HEADER: egui::Color32 = egui::Color32::from_rgb(80, 180, 180);

/// Card background (dark)
pub const COLOR_CARD_BG: egui::Color32 = egui::Color32::from_rgb(20, 20, 24);

/// Card background alternate (slightly lighter)
pub const COLOR_CARD_BG_ALT: egui::Color32 = egui::Color32::from_rgb(25, 25, 30);

/// Terminal background
pub const COLOR_TERMINAL_BG: egui::Color32 = egui::Color32::from_rgb(8, 8, 10);

// === SIZE CONSTANTS ===

/// Minimum card width in grid
pub const MIN_CARD_WIDTH: f32 = 180.0;

/// Grid spacing
pub const GRID_SPACING: f32 = 6.0;

/// Terminal max height
pub const TERMINAL_MAX_HEIGHT: f32 = 75.0;

/// Row height for virtualized lists
pub const LIST_ROW_HEIGHT: f32 = 35.0;
