//! Linux adjustments. Linux is a development host only; Islandy targets Windows.

use gtk::{cairo, prelude::*};
use tauri::{Runtime, WebviewWindow};

use crate::{error::Result, window::geometry::PhysicalRect};

/// GTK falls back to a 200px natural height for a window whose content
/// requests no size (WebKitWebView requests 0×0), and a non-resizable GTK
/// window never shrinks below its natural size — so the 44px pill would be
/// 200px tall. An explicit tiny size request on the content box lets
/// `set_size` decide the window size instead.
pub fn allow_small_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    window.default_vbox()?.set_size_request(1, 1);
    Ok(())
}

/// X11 equivalent of the hit-testing half of Win32 `SetWindowRgn`: an input
/// shape, so clicks outside the island reach the windows below.
///
/// Only the *input* shape is set. A bounding shape (which would also clip
/// drawing) stops WebKitGTK from painting at all on our Xvfb test host, and
/// it is unnecessary: everything outside the island is transparent anyway
/// (under a compositor). GTK works in application (logical) pixels, so the
/// physical rectangle is divided by GTK's scale.
pub fn set_window_shape<R: Runtime>(window: &WebviewWindow<R>, region: PhysicalRect) -> Result<()> {
    let gtk_window = window.gtk_window()?;
    let scale = gtk_window.scale_factor().max(1);
    let rect = cairo::RectangleInt::new(
        region.x / scale,
        region.y / scale,
        (region.width as i32 + scale - 1) / scale,
        (region.height as i32 + scale - 1) / scale,
    );
    let shape = cairo::Region::create_rectangle(&rect);
    gtk_window.input_shape_combine_region(Some(&shape));
    Ok(())
}

/* Launch at sign-in on the development host: an XDG autostart entry. */

fn autostart_file(name: &str) -> Option<std::path::PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(
        config
            .join("autostart")
            .join(format!("{}.desktop", name.to_lowercase())),
    )
}

pub fn startup_command(name: &str) -> Option<String> {
    let text = std::fs::read_to_string(autostart_file(name)?).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("Exec="))
        .map(str::to_owned)
}

pub fn set_startup_command(name: &str, command: &str) -> Result<()> {
    let file = autostart_file(name).ok_or(crate::error::IslandyError::Unsupported(
        "no configuration directory",
    ))?;
    let platform = |e: std::io::Error| crate::error::IslandyError::Platform(e.to_string());
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(platform)?;
    }
    let entry = format!("[Desktop Entry]\nType=Application\nName={name}\nExec={command}\nX-GNOME-Autostart-enabled=true\n");
    crate::services::paths::write_atomic(&file, entry.as_bytes()).map_err(platform)
}

pub fn remove_startup_command(name: &str) -> Result<()> {
    match autostart_file(name).map(std::fs::remove_file) {
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(crate::error::IslandyError::Platform(e.to_string()))
        }
        _ => Ok(()),
    }
}
