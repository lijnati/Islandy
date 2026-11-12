//! Launch at sign-in.
//!
//! On Windows this is one string value, `Islandy`, under the per-user key
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`: no administrator
//! rights, no scheduled task, no service. The command line quotes the
//! executable path (profile paths can contain spaces) and adds
//! [`AUTOSTART_ARG`] so Islandy knows it was started by Windows.
//!
//! The state is read from Windows every time (never cached in settings), so
//! a change made in Task Manager or Settings › Apps › Startup is reflected.
//! The NSIS uninstaller removes the value (see `src-tauri/windows/hooks.nsh`).
//!
//! The official `tauri-plugin-autostart` was evaluated and not used: its
//! Windows backend writes the executable path unquoted and tries
//! `HKEY_LOCAL_MACHINE` first when the process happens to be elevated.

use std::path::Path;

use serde::Serialize;

use crate::{error::Result, platform};

/// The value name in the `Run` key (and the XDG entry name on Linux).
pub const ENTRY_NAME: &str = "Islandy";
pub const AUTOSTART_ARG: &str = "--autostart";

/// Mirrored by `StartupStatus` in `src/services/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub supported: bool,
    /// Registered and not switched off in Task Manager.
    pub enabled: bool,
    /// Registered, but switched off in Task Manager / Settings.
    pub disabled_by_system: bool,
}

/// The exact command line Islandy registers.
pub fn command_line(exe: &Path) -> String {
    format!("\"{}\" {AUTOSTART_ARG}", exe.display())
}

/// Whether this process was started by Windows at sign-in.
pub fn launched_at_login<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> bool {
    args.iter().skip(1).any(|a| a.as_ref() == AUTOSTART_ARG)
}

fn supported() -> bool {
    cfg!(any(windows, target_os = "linux"))
}

pub fn status() -> StartupStatus {
    let registered = platform::startup_command(ENTRY_NAME).is_some();
    let disabled = registered && platform::startup_disabled_by_user(ENTRY_NAME);
    StartupStatus {
        supported: supported(),
        enabled: registered && !disabled,
        disabled_by_system: disabled,
    }
}

pub fn set(enabled: bool) -> Result<StartupStatus> {
    if enabled {
        let exe = std::env::current_exe()?;
        platform::set_startup_command(ENTRY_NAME, &command_line(&exe))?;
        crate::log_info!("startup", "launch at sign-in enabled");
    } else {
        platform::remove_startup_command(ENTRY_NAME)?;
        crate::log_info!("startup", "launch at sign-in disabled");
    }
    Ok(status())
}

/// If Islandy is registered with a different executable path (it was moved
/// or reinstalled elsewhere), points the entry at this executable. Release
/// builds only, so a development build never hijacks the installed entry.
pub fn repair() {
    if cfg!(debug_assertions) {
        return;
    }
    let (Some(current), Ok(exe)) = (
        platform::startup_command(ENTRY_NAME),
        std::env::current_exe(),
    ) else {
        return;
    };
    let wanted = command_line(&exe);
    if current != wanted {
        match platform::set_startup_command(ENTRY_NAME, &wanted) {
            Ok(()) => crate::log_info!("startup", "updated the sign-in entry to this executable"),
            Err(e) => crate::log_warn!("startup", "could not update the sign-in entry: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces() {
        let line = command_line(Path::new(
            r"C:\Users\Ann Lee\AppData\Local\Islandy\islandy.exe",
        ));
        assert_eq!(
            line,
            r#""C:\Users\Ann Lee\AppData\Local\Islandy\islandy.exe" --autostart"#
        );
    }

    #[test]
    fn detects_the_autostart_argument_after_the_program_name() {
        assert!(launched_at_login(&["islandy.exe", "--autostart"]));
        assert!(!launched_at_login(&["islandy.exe"]));
        assert!(
            !launched_at_login(&["--autostart"]),
            "argv[0] is the program"
        );
        assert!(!launched_at_login(&["islandy.exe", "--autostart-x"]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn enables_and_disables_an_entry() {
        let dir = std::env::temp_dir().join(format!("islandy-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Only this test touches XDG_CONFIG_HOME.
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        assert!(!status().enabled);
        let on = set(true).unwrap();
        assert!(on.enabled && on.supported && !on.disabled_by_system);
        let exe = std::env::current_exe().unwrap();
        assert_eq!(
            platform::startup_command(ENTRY_NAME).unwrap(),
            command_line(&exe)
        );
        let off = set(false).unwrap();
        assert!(!off.enabled);
        assert!(set(false).is_ok(), "disabling twice is fine");
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
