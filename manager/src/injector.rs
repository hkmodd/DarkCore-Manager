use std::ffi::OsStr;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{PCSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, FALSE, HANDLE};
// SeDebugPrivilege imports removed (unused)
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, QueueUserAPC, ResumeThread, CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOW,
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
        let si = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();

        // Build Command Line: "ExePath" <Args>
        let mut cmd_str = format!("\"{}\"", exe_abs.to_string_lossy());
        if let Some(arg_str) = args {
            cmd_str.push(' ');
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
            &si,
            &mut pi,
        );

        if success.is_err() {
            return Err("CreateProcessW failed.".to_string());
        }

        // 3. Inject DLL into the suspended process via APC
        // Note: passing both process handle (force memory write) AND thread handle (for APC)
        match inject_dll_apc(pi.hProcess, pi.hThread, dll_abs.to_str().unwrap_or("")) {
            Ok(_) => {
                // 4. Resume Thread - This triggers the APC which executes LoadLibrary
                ResumeThread(pi.hThread);

                let _ = CloseHandle(pi.hProcess);
                let _ = CloseHandle(pi.hThread);
                Ok(())
            }
            Err(e) => {
                // Kill process on failure? For now just close handles and error out
                let _ = CloseHandle(pi.hProcess);
                let _ = CloseHandle(pi.hThread);
                Err(format!("APC Injection failed: {}", e))
            }
        }
    }
}

// Internal helper using APC (Stealthier than CreateRemoteThread)
unsafe fn inject_dll_apc(
    h_process: HANDLE,
    h_thread: HANDLE,
    dll_path: &str,
) -> Result<(), String> {
    // 1. Path Processing
    let path_os = Path::new(dll_path);
    let path_str = path_os.to_string_lossy();

    let mut path_wide: Vec<u16> = OsStr::new(path_str.as_ref()).encode_wide().collect();
    path_wide.push(0);
    let path_len = path_wide.len() * size_of::<u16>();

    // 2. Allocation in Remote Process
    let remote_mem = VirtualAllocEx(
        h_process,
        None,
        path_len,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    if remote_mem.is_null() {
        return Err("VirtualAllocEx failed".to_string());
    }

    // 3. Write DLL Path to Remote Memory
    let mut written = 0;
    if WriteProcessMemory(
        h_process,
        remote_mem,
        path_wide.as_ptr() as *const _,
        path_len,
        Some(&mut written),
    )
    .is_err()
        || written != path_len
    {
        let _ = VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
        return Err("WriteProcessMemory failed".to_string());
    }

    // 4. Resolve LoadLibraryW Address
    let kernel32_str = "kernel32.dll\0";
    let kernel32_wide: Vec<u16> = OsStr::new(kernel32_str).encode_wide().collect();
    let module =
        GetModuleHandleW(PCWSTR(kernel32_wide.as_ptr())).map_err(|_| "GetModuleHandleW failed")?;

    let func_name = "LoadLibraryW\0";
    let load_library_addr = GetProcAddress(module, PCSTR(func_name.as_ptr()));

    if load_library_addr.is_none() {
        let _ = VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
        return Err("Failed to find LoadLibraryW".to_string());
    }

    // Transmute to PAPCFUNC signature: unsafe extern "system" fn(usize)
    let pfn_apc: unsafe extern "system" fn(usize) = std::mem::transmute(load_library_addr);

    // 5. Queue User APC
    // This queues the LoadLibraryW call to the main thread.
    // Since the process is suspended, this will be the VERY FIRST thing it does when resumed.
    let apc_result = QueueUserAPC(
        Some(pfn_apc),
        h_thread,
        remote_mem as usize, // Pass pointer as argument
    );

    if apc_result == 0 {
        let _ = VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
        return Err("QueueUserAPC failed".to_string());
    }

    // Success! We do NOT free memory here because LoadLibraryW needs it when thread resumes.
    // It's a small leak (path string) but standard for injection.
    Ok(())
}

/// Native check if GreenLuma is already injected into Steam
/// uses CreateToolhelp32Snapshot for instantaneous <1ms check.
pub fn is_greenluma_injected() -> bool {
    let target_process = "steam.exe";

    unsafe {
        // 1. Find Steam Process ID
        let h_snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };
        // Check invalid handle if needed (though Ok usually implies valid)
        if h_snapshot.is_invalid() {
            return false;
        }

        let mut pe = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(h_snapshot, &mut pe).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&pe.szExeFile)
                    .trim_matches('\0')
                    .to_lowercase();

                if name == target_process {
                    // Found Steam! Now check Modules
                    let pid = pe.th32ProcessID;

                    // Snapshot Modules for this PID
                    // Note: We need SNAPMODULE | SNAPMODULE32 to see everything
                    let h_mod_snap = match CreateToolhelp32Snapshot(
                        TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                        pid,
                    ) {
                        Ok(h) => Some(h),
                        Err(_) => None,
                    };

                    if let Some(h_mod) = h_mod_snap {
                        let mut me = MODULEENTRY32W {
                            dwSize: size_of::<MODULEENTRY32W>() as u32,
                            ..Default::default()
                        };

                        if Module32FirstW(h_mod, &mut me).is_ok() {
                            loop {
                                let mod_name = String::from_utf16_lossy(&me.szModule)
                                    .trim_matches('\0')
                                    .to_lowercase();

                                if mod_name.contains("greenluma") {
                                    let _ = CloseHandle(h_mod);
                                    let _ = CloseHandle(h_snapshot);
                                    return true; // INJECTED!
                                }

                                if Module32NextW(h_mod, &mut me).is_err() {
                                    break;
                                }
                            }
                        }
                        let _ = CloseHandle(h_mod);
                    }
                    // If we found steam but didn't find module, return false immediately?
                    // Or keep searching if multiple steam processes? (Unlikely)
                    // Let's break and return false, assuming only one steam instance.
                    let _ = CloseHandle(h_snapshot);
                    return false;
                }

                if Process32NextW(h_snapshot, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(h_snapshot);
    }

    false
}

/// Simple native check if a process name is running
pub fn is_process_running(exe_name: &str) -> bool {
    unsafe {
        let h_snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if h_snapshot.is_invalid() {
            return false;
        }

        let mut pe = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(h_snapshot, &mut pe).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&pe.szExeFile)
                    .trim_matches('\0')
                    .to_lowercase();

                if name == exe_name.to_lowercase() {
                    let _ = CloseHandle(h_snapshot);
                    return true;
                }
                if Process32NextW(h_snapshot, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(h_snapshot);
    }
    false
}
