//! Win32 integrations outside the island window itself:
//! - launch at sign-in (the per-user `Run` key; no admin rights),
//! - the Windows version (registry, because `GetVersionEx` lies),
//! - fullscreen detection for the island's monitor,
//! - opening Islandy's own folders in File Explorer,
//! - how long the user has been idle (updates install while away).

use std::path::Path;

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, RECT},
        System::Registry::{
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER,
            HKEY_LOCAL_MACHINE, REG_BINARY, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD,
            RRF_RT_REG_SZ,
        },
        System::SystemInformation::GetTickCount,
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                GetClassNameW, GetForegroundWindow, GetWindowRect, IsIconic, IsZoomed,
                SW_SHOWNORMAL,
            },
        },
    },
};

use crate::{
    error::{IslandyError, Result},
    window::geometry::PhysicalRect,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Task Manager / Settings › Apps › Startup record a user's choice.
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

fn get_string(root: HKEY, key: &str, name: &str) -> Option<String> {
    let mut buffer = vec![0u16; 2048];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: `buffer` is writable for `size` bytes; strings outlive the call.
    let status = unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..len]);
    Some(text.trim_end_matches('\0').to_owned())
}

fn get_dword(root: HKEY, key: &str, name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    // SAFETY: `value` is 4 writable bytes.
    let status = unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(name),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

fn get_binary(root: HKEY, key: &str, name: &str) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; 64];
    let mut size = buffer.len() as u32;
    // SAFETY: `buffer` is writable for `size` bytes.
    let status = unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(name),
            RRF_RT_REG_BINARY,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then(|| {
        buffer.truncate(size as usize);
        buffer
    })
}

fn registry_error(what: &str, status: windows::Win32::Foundation::WIN32_ERROR) -> IslandyError {
    IslandyError::Platform(format!("{what} (error {})", status.0))
}

/// The `Run` command line, if Islandy is registered.
pub fn startup_command(name: &str) -> Option<String> {
    get_string(HKEY_CURRENT_USER, RUN_KEY, name)
}

/// `true` when the user disabled the entry in Task Manager or Settings
/// (the first byte of the `StartupApproved` value is odd).
pub fn startup_disabled_by_user(name: &str) -> bool {
    get_binary(HKEY_CURRENT_USER, APPROVED_KEY, name)
        .and_then(|v| v.first().copied())
        .is_some_and(|b| b & 1 == 1)
}

pub fn set_startup_command(name: &str, command: &str) -> Result<()> {
    let data: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `data` is a NUL-terminated UTF-16 string of the given size.
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY),
            &HSTRING::from(name),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(registry_error(
            "could not register Islandy to start at sign-in",
            status,
        ));
    }
    // An explicit "on" in Islandy overrides an earlier "disabled" in Task
    // Manager: the user just asked for it.
    if startup_disabled_by_user(name) {
        let enabled = [2u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // SAFETY: `enabled` is 12 readable bytes.
        let _ = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(APPROVED_KEY),
                &HSTRING::from(name),
                REG_BINARY.0,
                Some(enabled.as_ptr().cast()),
                enabled.len() as u32,
            )
        };
    }
    Ok(())
}

pub fn remove_startup_command(name: &str) -> Result<()> {
    let delete = |key: &str| {
        // SAFETY: plain value deletion under HKCU.
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(name)) }
    };
    let status = delete(RUN_KEY);
    let _ = delete(APPROVED_KEY);
    if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(registry_error("could not remove the sign-in entry", status))
    }
}

/// e.g. `Windows 11 Pro 23H2 (build 22631.4460)`.
pub fn windows_version() -> Option<String> {
    let build: u32 = get_string(HKEY_LOCAL_MACHINE, VERSION_KEY, "CurrentBuildNumber")?
        .parse()
        .ok()?;
    let ubr = get_dword(HKEY_LOCAL_MACHINE, VERSION_KEY, "UBR");
    let display = get_string(HKEY_LOCAL_MACHINE, VERSION_KEY, "DisplayVersion");
    // ProductName still says "Windows 10" on Windows 11; keep only the edition.
    let edition = get_string(HKEY_LOCAL_MACHINE, VERSION_KEY, "EditionID");
    let family = if build >= 22_000 {
        "Windows 11"
    } else {
        "Windows 10"
    };
    let mut text = family.to_owned();
    if let Some(edition) = edition.filter(|e| !e.is_empty()) {
        text.push(' ');
        text.push_str(&edition);
    }
    if let Some(display) = display.filter(|d| !d.is_empty()) {
        text.push(' ');
        text.push_str(&display);
    }
    text.push_str(&match ubr {
        Some(ubr) => format!(" (build {build}.{ubr})"),
        None => format!(" (build {build})"),
    });
    Some(text)
}

/// The foreground window covers the whole of `monitor` (physical px) and is
/// neither maximized nor the desktop/taskbar: a borderless or exclusive
/// fullscreen app (game, video, presentation) on that monitor.
pub fn fullscreen_on(monitor: PhysicalRect, own: Option<isize>) -> bool {
    // SAFETY: no preconditions.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() || own == Some(hwnd.0 as isize) {
        return false;
    }
    // SAFETY: `hwnd` came from GetForegroundWindow; a stale handle only
    // makes these calls fail.
    unsafe {
        if IsZoomed(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        crate::window::geometry::covers((rect.left, rect.top, rect.right, rect.bottom), monitor)
    }
}

/// Opens one of Islandy's own directories in File Explorer. The path is
/// always derived by Islandy, never received over IPC.
pub fn open_folder(path: &Path) -> Result<()> {
    let target = HSTRING::from(path.as_os_str());
    // SAFETY: valid NUL-terminated strings that outlive the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(IslandyError::Platform(format!(
            "could not open the folder (code {})",
            result.0 as isize
        )))
    }
}

/// Time since the last keyboard or mouse input in this session.
pub fn user_idle_for() -> Option<std::time::Duration> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid, initialized LASTINPUTINFO with cbSize set.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // SAFETY: no preconditions. Both tick counts wrap every ~49.7 days;
    // wrapping subtraction handles that.
    let now = unsafe { GetTickCount() };
    Some(std::time::Duration::from_millis(u64::from(
        now.wrapping_sub(info.dwTime),
    )))
}
