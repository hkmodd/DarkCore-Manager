pub mod hooks;
pub mod interfaces;

use log::{info, LevelFilter};
use simplelog::{Config, WriteLogger};
use std::fs::File;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

#[no_mangle]
extern "system" fn DllMain(_: HMODULE, call_reason: u32, _: *mut std::ffi::c_void) -> BOOL {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            init_logging();
            info!("TitanHook loaded successfully!");
            unsafe {
                hooks::steam_api::install_hooks();
            }
        }
        DLL_PROCESS_DETACH => {
            info!("TitanHook unloading...");
        }
        _ => {}
    }
    BOOL::from(true)
}

fn init_logging() {
    if let Ok(file) = File::create("titan_hook_debug.log") {
        let _ = WriteLogger::init(LevelFilter::Info, Config::default(), file);
    }
}
