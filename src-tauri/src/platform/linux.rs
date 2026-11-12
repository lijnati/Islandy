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
