//! DarkCore Manager UI Module
//!
//! This module is being incrementally extracted from the monolithic `ui_old.rs`.
//!
//! ## Current Status
//! - `state.rs`: Types, constants, and helpers (ACTIVE - used by ui_old.rs)
//! - `theme.rs`: Color and size constants (ready for future use)
//! - `panels/about.rs`: ABOUT tab (Matrix rain) - EXTRACTED
//!
//! ## Usage
//! The main `DarkCoreApp` struct is still in `ui_old.rs`.
//! Types are imported from `ui::state` by `ui_old.rs`.

// Active modules (contain real code)
pub mod app;
pub mod components;
pub mod covers;
pub mod helpers;
pub mod install_logic;
pub mod modals;
pub mod panels;
pub mod render;
pub mod state;
pub mod theme;
pub mod watcher;

// Note: Re-exports are handled by ui_old.rs for now.
// Once migration is complete, this module will re-export DarkCoreApp.
