use std::ffi::OsStr;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{PCSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, FALSE, HANDLE};
// SeDebugPrivilege imports removed (unused)
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, CreateRemoteThread, ResumeThread, WaitForSingleObject, CREATE_SUSPENDED,
    INFINITE, PROCESS_INFORMATION, STARTUPINFOW,
};

// --- CORE LAUNCHER ---
pub fn launch_injected(exe_path: &str, dll_path: &str, args: Option<&str>) -> Result<(), String> {
    // 1. Validation and Path Conversion
    let exe_path_fs = Path::new(exe_path);
    if !exe_path_fs.exists() {
        return Err(format!("Executable not found: {}", exe_path));
    }
    let dll_path_fs = Path::new(dll_path);
    if !dll_path_fs.exists() {
        return Err(format!("DLL not found: {}", dll_path));
    }

    // Canonicalize paths for robustness
    let exe_abs = exe_path_fs.canonicalize().map_err(|e| e.to_string())?;
    let dll_abs = dll_path_fs.canonicalize().map_err(|e| e.to_string())?;

    // Prepare Working Directory (EXE folder)
    let work_dir = exe_abs.parent().ok_or("Invalid exe parent dir")?;

    // Encode to Wide Strings (UTF-16) for Windows API
    let mut exe_wide: Vec<u16> = OsStr::new(&exe_abs).encode_wide().collect();
    exe_wide.push(0);

    let mut work_dir_wide: Vec<u16> = OsStr::new(&work_dir).encode_wide().collect();
    work_dir_wide.push(0);

    // 2. Create Process Suspended
    unsafe {
        let mut si = STARTUPINFOW::default();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        let mut pi = PROCESS_INFORMATION::default();

        // Build Command Line: "ExePath" <Args>
        let mut cmd_str = format!("\"{}\"", exe_abs.to_string_lossy());
        if let Some(arg_str) = args {
            cmd_str.push_str(" ");
            cmd_str.push_str(arg_str);
        }

        let mut cmd_wide: Vec<u16> = OsStr::new(&cmd_str).encode_wide().collect();
        cmd_wide.push(0);

        let success = CreateProcessW(
            None,
            PWSTR(cmd_wide.as_mut_ptr()),
            None,
            None,
            FALSE,
            CREATE_SUSPENDED,
            None,
            PCWSTR(work_dir_wide.as_ptr()),
            &mut si,
            &mut pi,
        );

        if success.is_err() {
            return Err(format!("CreateProcessW failed."));
        }

        // 3. Inject DLL into the suspended process
        match inject_dll_handle(pi.hProcess, dll_abs.to_str().unwrap_or("")) {
            Ok(_) => {
                // 4. Resume Thread if injection succeeded
                ResumeThread(pi.hThread);

                let _ = CloseHandle(pi.hProcess);
                let _ = CloseHandle(pi.hThread);
                Ok(())
            }
            Err(e) => {
                let _ = CloseHandle(pi.hProcess);
                let _ = CloseHandle(pi.hThread);
                Err(format!("Injection into suspended process failed: {}", e))
            }
        }
    }
}

// Internal helper that takes a raw handle
unsafe fn inject_dll_handle(handle: HANDLE, dll_path: &str) -> Result<(), String> {
    // 1. Path Processing
    let path_os = Path::new(dll_path);
    let path_str = path_os.to_string_lossy();

    // Fix: Cow<str> handling
    let mut path_wide: Vec<u16> = OsStr::new(path_str.as_ref()).encode_wide().collect();
    path_wide.push(0);
    let path_len = path_wide.len() * size_of::<u16>();

    // 2. Allocation
    let remote_mem = VirtualAllocEx(
        handle,
        None,
        path_len,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    if remote_mem.is_null() {
        return Err("VirtualAllocEx failed".to_string());
    }

    // 3. Write
    let mut written = 0;
    if WriteProcessMemory(
        handle,
        remote_mem,
        path_wide.as_ptr() as *const _,
        path_len,
        Some(&mut written),
    )
    .is_err()
        || written != path_len
    {
        let _ = VirtualFreeEx(handle, remote_mem, 0, MEM_RELEASE);
        return Err("WriteProcessMemory failed".to_string());
    }

    // 4. Resolve LoadLibraryW
    let kernel32_str = "kernel32.dll\0";
    let kernel32_wide: Vec<u16> = OsStr::new(kernel32_str).encode_wide().collect();
    let module =
        GetModuleHandleW(PCWSTR(kernel32_wide.as_ptr())).map_err(|_| "GetModuleHandleW failed")?;

    let func_name = "LoadLibraryW\0";
    let load_library_addr = GetProcAddress(module, PCSTR(func_name.as_ptr()));

    if load_library_addr.is_none() {
        let _ = VirtualFreeEx(handle, remote_mem, 0, MEM_RELEASE);
        return Err("Failed to find LoadLibraryW".to_string());
    }

    let start_routine: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 =
        std::mem::transmute(load_library_addr);

    // 5. Execute
    let thread_handle = CreateRemoteThread(
        handle,
        None,
        0,
        Some(start_routine),
        Some(remote_mem),
        0,
        None,
    );

    match thread_handle {
        Ok(th) => {
            WaitForSingleObject(th, INFINITE); // Wait for DllMain

            // Cleanup memory (ACTIVE)
            let _ = VirtualFreeEx(handle, remote_mem, 0, MEM_RELEASE);

            let _ = CloseHandle(th);
            Ok(())
        }
        Err(e) => {
            let _ = VirtualFreeEx(handle, remote_mem, 0, MEM_RELEASE);
            Err(e.to_string())
        }
    }
}
