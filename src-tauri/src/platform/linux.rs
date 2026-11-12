//! Linux adjustments. Linux is a development host only; Islandy targets Windows.

use gtk::prelude::*;
use tauri::{Runtime, WebviewWindow};

use crate::error::Result;

/// GTK falls back to a 200px natural height for a window whose content
/// requests no size (WebKitWebView requests 0×0), and a non-resizable GTK
/// window never shrinks below its natural size — so the 44px pill would be
/// 200px tall. An explicit tiny size request on the content box lets
/// `set_size` decide the window size instead.
pub fn allow_small_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    window.default_vbox()?.set_size_request(1, 1);
    Ok(())
}
