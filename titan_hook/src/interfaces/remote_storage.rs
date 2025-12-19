use log::{error, info};
use minhook::MinHook;
use std::ffi::{c_void, CStr, CString};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::ptr;
use std::slice;

// VTable indices for standard Steamworks SDK (1.37 - 1.50+)
const IDX_FILE_WRITE: usize = 0;
const IDX_FILE_READ: usize = 1;
const IDX_FILE_EXISTS: usize = 13;
const IDX_GET_FILE_COUNT: usize = 15;
const IDX_GET_FILE_NAME_AND_SIZE: usize = 16;

static mut ORIGINAL_FILE_WRITE: *mut c_void = ptr::null_mut();
static mut ORIGINAL_FILE_READ: *mut c_void = ptr::null_mut();
static mut ORIGINAL_FILE_EXISTS: *mut c_void = ptr::null_mut();
static mut ORIGINAL_GET_FILE_COUNT: *mut c_void = ptr::null_mut();
static mut ORIGINAL_GET_FILE_NAME_AND_SIZE: *mut c_void = ptr::null_mut();

fn get_save_path(filename: &str) -> PathBuf {
    let mut path = directories::UserDirs::new()
        .map(|ud| ud.home_dir().join("AppData/Roaming/DarkCore/Saves/Unknown"))
        .unwrap_or_else(|| PathBuf::from("Saves"));

    // In a real impl, read steam_appid.txt or config from current dir
    if let Ok(appid_str) = fs::read_to_string("steam_appid.txt") {
        let appid = appid_str.trim();
        path.pop(); // Remove Unknown
        path.push(appid);
    }

    // Sanitize filename
    let sanitized_name = filename.replace("\\", "/").replace("/", "_");
    path.push(sanitized_name);

    path
}

pub unsafe fn hook_interface(iface: *mut c_void, _version: &str) {
    let vtable = *(iface as *mut *mut *mut c_void);

    // Helpers to install hooks
    let install = |idx: usize, detour: *mut c_void, name: &str| -> *mut c_void {
        let target = *vtable.add(idx);
        match MinHook::create_hook(target, detour) {
            Ok(original) => {
                if MinHook::enable_hook(target).is_ok() {
                    info!("Hooked ISteamRemoteStorage::{} at {:p}", name, target);
                    original
                } else {
                    error!("Failed to enable hook for {}", name);
                    ptr::null_mut()
                }
            }
            Err(e) => {
                error!("Failed to create hook for {}: {:?}", name, e);
                ptr::null_mut()
            }
        }
    };

    if ORIGINAL_FILE_WRITE.is_null() {
        ORIGINAL_FILE_WRITE = install(IDX_FILE_WRITE, detour_file_write as _, "FileWrite");
    }
    if ORIGINAL_FILE_READ.is_null() {
        ORIGINAL_FILE_READ = install(IDX_FILE_READ, detour_file_read as _, "FileRead");
    }
    if ORIGINAL_FILE_EXISTS.is_null() {
        ORIGINAL_FILE_EXISTS = install(IDX_FILE_EXISTS, detour_file_exists as _, "FileExists");
    }
    if ORIGINAL_GET_FILE_COUNT.is_null() {
        ORIGINAL_GET_FILE_COUNT = install(
            IDX_GET_FILE_COUNT,
            detour_get_file_count as _,
            "GetFileCount",
        );
    }
    if ORIGINAL_GET_FILE_NAME_AND_SIZE.is_null() {
        ORIGINAL_GET_FILE_NAME_AND_SIZE = install(
            IDX_GET_FILE_NAME_AND_SIZE,
            detour_get_file_name_and_size as _,
            "GetFileNameAndSize",
        );
    }
}

unsafe extern "C" fn detour_file_write(
    _this: *mut c_void,
    filename: *const i8,
    data: *const u8,
    size: i32,
) -> bool {
    let fname = if !filename.is_null() {
        CStr::from_ptr(filename).to_string_lossy().to_string()
    } else {
        "unknown".to_string()
    };
    info!("FileWrite intercepted: {} ({} bytes)", fname, size);

    let path = get_save_path(&fname);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let data_slice = slice::from_raw_parts(data, size as usize);
    if let Ok(mut f) = fs::File::create(&path) {
        if f.write_all(data_slice).is_ok() {
            return true;
        }
    }
    false
}

unsafe extern "C" fn detour_file_read(
    this: *mut c_void,
    filename: *const i8,
    data: *mut u8,
    size: i32,
) -> i32 {
    let fname = if !filename.is_null() {
        CStr::from_ptr(filename).to_string_lossy().to_string()
    } else {
        "unknown".to_string()
    };
    info!("FileRead intercepted: {} (Max bytes: {})", fname, size);

    let path = get_save_path(&fname);
    if path.exists() {
        if let Ok(content) = fs::read(&path) {
            let bytes_to_copy = std::cmp::min(size as usize, content.len());
            ptr::copy_nonoverlapping(content.as_ptr(), data, bytes_to_copy);
            return bytes_to_copy as i32;
        }
    }

    // Fallback to original? Usually not if we want to be full emu, but for mixed mode...
    // No, if checking local failed, return 0 (not found) to avoid Cloud confusion.
    0
}

unsafe extern "C" fn detour_file_exists(_this: *mut c_void, filename: *const i8) -> bool {
    let fname = if !filename.is_null() {
        CStr::from_ptr(filename).to_string_lossy().to_string()
    } else {
        "unknown".to_string()
    };
    let path = get_save_path(&fname);
    let exists = path.exists();
    info!("FileExists check for {}: {}", fname, exists);
    exists
}

unsafe extern "C" fn detour_get_file_count(_this: *mut c_void) -> i32 {
    let mut count = 0;
    // We need to list files in the save dir
    // This requires us to know the save dir without a filename.
    // Hack: get path for "dummy" and parent
    let path = get_save_path("dummy");
    if let Some(parent) = path.parent() {
        if let Ok(entries) = fs::read_dir(parent) {
            count = entries.count() as i32;
        }
    }
    info!("GetFileCount returned {}", count);
    count
}

unsafe extern "C" fn detour_get_file_name_and_size(
    _this: *mut c_void,
    index: i32,
    size: *mut i32,
) -> *const i8 {
    // This is tricky. We need to return a static/persistent string pointer.
    // And we need to iterate files in consistent order.
    // For now, simple implementation: List all files, pick index.
    // WARNING: This is slow and memory leaky if we leak strings.

    // Static buffer for returned string (not thread safe but okay for single game thread)
    static mut LAST_FILENAME_BUF: Vec<u8> = Vec::new();

    let path = get_save_path("dummy");
    if let Some(parent) = path.parent() {
        if let Ok(entries) = fs::read_dir(parent) {
            let mut files: Vec<PathBuf> =
                entries.filter_map(|e| e.ok().map(|d| d.path())).collect();
            // Sort for consistency
            files.sort();

            if let Some(file_path) = files.get(index as usize) {
                if let Ok(meta) = fs::metadata(file_path) {
                    if !size.is_null() {
                        *size = meta.len() as i32;
                    }
                }
                let name = file_path.file_name().unwrap().to_string_lossy().to_string();
                let c_str = CString::new(name).unwrap();
                let ptr = c_str.as_ptr();
                LAST_FILENAME_BUF = c_str.into_bytes_with_nul();
                return LAST_FILENAME_BUF.as_ptr() as *const i8;
            }
        }
    }

    if !size.is_null() {
        *size = 0;
    }
    ptr::null()
}
