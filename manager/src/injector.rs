use std::ffi::CString;
use std::path::Path;
use std::ptr;
use windows::core::{s, PCSTR, PSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};
use windows::Win32::System::Threading::{
    CreateProcessA, CreateRemoteThread, ResumeThread, WaitForSingleObject, CREATE_SUSPENDED,
    INFINITE, PROCESS_INFORMATION, STARTUPINFOA,
};

#[allow(dead_code)]
pub fn launch_and_inject(exe_path: &str, dll_path: &str, app_id: &str) -> Result<(), String> {
    unsafe {
        // 1. Prepare startup info
        let mut startup_info = STARTUPINFOA::default();
        let mut process_info = PROCESS_INFORMATION::default();

        let exe_cstring = CString::new(exe_path).map_err(|e| e.to_string())?;

        // We might need to set the working directory to the game's folder
        let cwd_path = Path::new(exe_path).parent().unwrap_or(Path::new("."));
        let cwd_cstring = CString::new(cwd_path.to_str().unwrap()).map_err(|e| e.to_string())?;

        // CreateProcessA requires mutable string for command line
        let cmd_line = exe_cstring.clone();

        // 2. Create Process Suspended
        let result = CreateProcessA(
            PCSTR(ptr::null()),                   // No module name (use command line)
            PSTR(cmd_line.into_raw() as *mut u8), // Command line (mutable)
            None,
            None,
            false,
            CREATE_SUSPENDED, // Suspended
            None,
            PCSTR(cwd_cstring.as_ptr() as _), // CWD
            &mut startup_info,
            &mut process_info,
        );

        if result.is_ok() {
            println!("Process created suspended: {}", process_info.dwProcessId);

            // 3. Inject DLL
            // Write AppID file first if needed
            let _ = std::fs::write(cwd_path.join("steam_appid.txt"), app_id);

            match inject_dll(process_info.hProcess, dll_path) {
                Ok(_) => {
                    println!("DLL Injected successfully. Resuming thread.");
                    ResumeThread(process_info.hThread);
                }
                Err(e) => {
                    println!("Injection failed: {}", e);
                }
            }

            // Copy handles to close them safely
            let _ = windows::Win32::Foundation::CloseHandle(process_info.hProcess);
            let _ = windows::Win32::Foundation::CloseHandle(process_info.hThread);

            Ok(())
        } else {
            Err(format!(
                "Failed to create process: {:?}",
                std::io::Error::last_os_error()
            ))
        }
    }
}

#[allow(dead_code)]
unsafe fn inject_dll(h_process: HANDLE, dll_path_str: &str) -> Result<(), String> {
    let dll_path = CString::new(dll_path_str).map_err(|e| e.to_string())?;
    let dll_len = dll_path.as_bytes_with_nul().len();

    // 1. Allocate memory in target
    let remote_mem = VirtualAllocEx(
        h_process,
        None,
        dll_len,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );

    if remote_mem.is_null() {
        return Err("VirtualAllocEx failed".to_string());
    }

    // 2. Write DLL path
    let mut bytes_written = 0;
    let write_res = WriteProcessMemory(
        h_process,
        remote_mem,
        dll_path.as_ptr() as _,
        dll_len,
        Some(&mut bytes_written),
    );

    if write_res.is_err() || bytes_written != dll_len {
        return Err("WriteProcessMemory failed".to_string());
    }

    // 3. Get LoadLibraryA address
    let kernel32 = GetModuleHandleA(s!("kernel32.dll")).map_err(|e| e.to_string())?;
    let load_library = GetProcAddress(kernel32, s!("LoadLibraryA"));

    if load_library.is_none() {
        return Err("Could not find LoadLibraryA".to_string());
    }

    // 4. Create Remote Thread
    let h_thread = CreateRemoteThread(
        h_process,
        None,
        0,
        Some(std::mem::transmute(load_library)), // ThreadProc
        Some(remote_mem),                        // Param
        0,
        None,
    );

    if let Ok(handle) = h_thread {
        if handle.is_invalid() {
            return Err("CreateRemoteThread returned invalid handle".to_string());
        }

        // Wait for thread to finish (DLL load)
        WaitForSingleObject(handle, INFINITE);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        Ok(())
    } else {
        Err("CreateRemoteThread failed".to_string())
    }
}
