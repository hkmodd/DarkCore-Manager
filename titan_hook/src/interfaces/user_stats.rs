use log::info;
use minhook::MinHook;
use std::ffi::{c_void, CStr};
use std::ptr;

// TODO: Verify indices!
const IDX_SET_ACHIEVEMENT: usize = 7;
const IDX_STORE_STATS: usize = 9;

static mut ORIGINAL_SET_ACHIEVEMENT: *mut c_void = ptr::null_mut();
static mut ORIGINAL_STORE_STATS: *mut c_void = ptr::null_mut();

pub unsafe fn hook_interface(iface: *mut c_void, _version: &str) {
    let vtable = *(iface as *mut *mut *mut c_void);

    // Hook SetAchievement
    let set_ach_addr = *vtable.add(IDX_SET_ACHIEVEMENT);
    if ORIGINAL_SET_ACHIEVEMENT.is_null() {
        if let Ok(original) = MinHook::create_hook(set_ach_addr, detour_set_achievement as _) {
            ORIGINAL_SET_ACHIEVEMENT = original;
            MinHook::enable_hook(set_ach_addr).ok();
            info!(
                "Hooked ISteamUserStats::SetAchievement at {:p}",
                set_ach_addr
            );
        }
    }

    // Hook StoreStats
    let store_stats_addr = *vtable.add(IDX_STORE_STATS);
    if ORIGINAL_STORE_STATS.is_null() {
        if let Ok(original) = MinHook::create_hook(store_stats_addr, detour_store_stats as _) {
            ORIGINAL_STORE_STATS = original;
            MinHook::enable_hook(store_stats_addr).ok();
            info!(
                "Hooked ISteamUserStats::StoreStats at {:p}",
                store_stats_addr
            );
        }
    }
}

// Helper for stats path
fn get_stats_path() -> std::path::PathBuf {
    let mut path = directories::UserDirs::new()
        .map(|ud| ud.home_dir().join("AppData/Roaming/DarkCore/Saves/Unknown"))
        .unwrap_or_else(|| std::path::PathBuf::from("Saves"));

    if let Ok(appid_str) = std::fs::read_to_string("steam_appid.txt") {
        let appid = appid_str.trim();
        path.pop();
        path.push(appid);
    }
    path.push("stats.txt");
    path
}

unsafe extern "C" fn detour_set_achievement(_this: *mut c_void, name: *const i8) -> bool {
    let ach_name = if !name.is_null() {
        CStr::from_ptr(name).to_string_lossy().to_string()
    } else {
        return false;
    };

    info!("Unlock Achievement requested: {}", ach_name);

    // Load existing stats
    let path = get_stats_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let mut stats = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already unlocked
    if !stats.lines().any(|line| line.trim() == ach_name) {
        stats.push_str(&ach_name);
        stats.push('\n');
        if let Ok(_) = std::fs::write(&path, stats) {
            info!("Achievement '{}' persisted to disk.", ach_name);
        }
    }

    true
}

unsafe extern "C" fn detour_store_stats(_this: *mut c_void) -> bool {
    // We persist immediately on SetAchievement, so StoreStats is just a dummy success
    info!("StoreStats requested (Already committed).");
    true
}
// Hook GetAchievement to report true if in file?
// Ideally yes, but depends on game. Many games just fire "Set" when logic dictates.
// If the game checks state, we need `GetAchievement`.
// Let's add basic `GetAchievement` hook for completeness?
// Adding `IDX_GET_ACHIEVEMENT = 8` (usually) or looking it up.
// For now, persistence ensures at least we don't lose the "write".
