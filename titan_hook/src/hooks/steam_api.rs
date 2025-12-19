use log::{error, info};
use minhook::MinHook;
use std::ffi::{c_void, CStr};
use std::ptr;
use windows::core::s;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

// We use AtomicPtr for the original function to be thread-safe(ish) and accessible.
static mut ORIGINAL_CREATE_INTERFACE: *mut c_void = ptr::null_mut();

type FnCreateInterface = extern "C" fn(p_version: *const i8) -> *mut c_void;

pub unsafe fn install_hooks() {
    let steam_api_res = GetModuleHandleA(s!("steam_api64.dll"));
    if let Err(e) = steam_api_res {
        error!("steam_api64.dll not found: {:?}", e);
        return;
    }
    let steam_api = steam_api_res.unwrap();

    // Try "SteamInternal_CreateInterface" first, then "CreateInterface"
    let mut proc_name = s!("SteamInternal_CreateInterface");
    let mut proc_addr = GetProcAddress(steam_api, proc_name);

    if proc_addr.is_none() {
        proc_name = s!("CreateInterface");
        proc_addr = GetProcAddress(steam_api, proc_name);
    }

    if let Some(addr) = proc_addr {
        info!("Found CreateInterface at {:p}", addr);

        let target = addr as *mut c_void;
        let detour = detour_create_interface as *mut c_void;

        match MinHook::create_hook(target, detour) {
            Ok(original) => {
                ORIGINAL_CREATE_INTERFACE = original;
                if let Err(e) = MinHook::enable_hook(target) {
                    error!("Failed to enable CreateInterface hook: {:?}", e);
                } else {
                    info!("Successfully hooked CreateInterface");
                }
            }
            Err(e) => {
                error!("Failed to create CreateInterface hook: {:?}", e);
            }
        }
    } else {
        error!("Could not find CreateInterface export");
    }
}

unsafe extern "C" fn detour_create_interface(version: *const i8) -> *mut c_void {
    let original_fn: FnCreateInterface = std::mem::transmute(ORIGINAL_CREATE_INTERFACE);
    let iface = original_fn(version);

    if !iface.is_null() && !version.is_null() {
        if let Ok(v_str) = CStr::from_ptr(version).to_str() {
            info!("Interface requested: {}", v_str);

            // Dispatch to specific interface hookers
            if v_str.contains("STEAMREMOTESTORAGE") {
                crate::interfaces::remote_storage::hook_interface(iface, v_str);
            } else if v_str.contains("STEAMUSERSTATS") {
                crate::interfaces::user_stats::hook_interface(iface, v_str);
            }
        }
    }

    iface
}
