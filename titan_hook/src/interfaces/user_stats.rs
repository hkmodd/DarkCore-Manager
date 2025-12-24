use log::info;
use minhook::MinHook;
use std::ffi::{c_void, CStr};
use std::ptr;

// VTable indices for ISteamUserStats interface
// These indices are based on Steamworks SDK v1.50+ (commonly used 2020+)
// Reference: Goldberg Steam Emu uses similar indices for modern SDK versions
// NOTE: If achievements don't work for a game, it may be using an older SDK version
// Older SDK versions (pre-1.37) may have different vtable layouts
const IDX_SET_ACHIEVEMENT: usize = 7; // bool SetAchievement(const char* pchName)
const IDX_GET_ACHIEVEMENT: usize = 8; // bool GetAchievement(const char* pchName, bool* pbAchieved)
const IDX_STORE_STATS: usize = 9; // bool StoreStats()

static mut ORIGINAL_SET_ACHIEVEMENT: *mut c_void = ptr::null_mut();
static mut ORIGINAL_GET_ACHIEVEMENT: *mut c_void = ptr::null_mut();
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

    // Hook GetAchievement
    let get_ach_addr = *vtable.add(IDX_GET_ACHIEVEMENT);
    if ORIGINAL_GET_ACHIEVEMENT.is_null() {
        if let Ok(original) = MinHook::create_hook(get_ach_addr, detour_get_achievement as _) {
            ORIGINAL_GET_ACHIEVEMENT = original;
            MinHook::enable_hook(get_ach_addr).ok();
            info!(
                "Hooked ISteamUserStats::GetAchievement at {:p}",
                get_ach_addr
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

unsafe extern "C" fn detour_get_achievement(
    _this: *mut c_void,
    name: *const i8,
    pb_achieved: *mut bool,
) -> bool {
    let ach_name = if !name.is_null() {
        CStr::from_ptr(name).to_string_lossy().to_string()
    } else {
        return false;
    };

    // Check local database
    let path = get_stats_path();
    let mut is_unlocked = false;

    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            is_unlocked = content.lines().any(|line| line.trim() == ach_name);
        }
    }

    if !pb_achieved.is_null() {
        *pb_achieved = is_unlocked;
    }

    // Debug log only if True to avoid spam
    if is_unlocked {
        info!("GetAchievement '{}' -> TRUE (Local Override)", ach_name);
    }

    true // Return success (we handled the call)
}

unsafe extern "C" fn detour_store_stats(_this: *mut c_void) -> bool {
    // We persist immediately on SetAchievement, so StoreStats is just a dummy success
    info!("StoreStats requested (Already committed).");
    true
}
