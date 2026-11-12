//! Native Windows backends for the system workers.
//!
//! - [`audio`]   Core Audio (`IMMDeviceEnumerator`, `IAudioEndpointVolume`)
//! - [`monitor`] power (`GetSystemPowerStatus` + WinRT `PowerManager` events),
//!   network (WinRT `NetworkInformation`), radios (WinRT `Radio`),
//!   CPU/RAM (`GetSystemTimes`, `GlobalMemoryStatusEx`)
//! - [`display`] brightness (WMI for built-in panels, DDC/CI for monitors)
//!
//! Every backend lives on its worker thread in the multithreaded apartment.

pub mod audio;
pub mod display;
pub mod monitor;

pub use crate::services::winrt::with_mta;

use super::SystemError;

/// `HRESULT`s meaning "that device is gone".
const GONE: [u32; 3] = [
    0x8889_0004, // AUDCLNT_E_DEVICE_INVALIDATED
    0x8007_0490, // E_NOTFOUND / ERROR_NOT_FOUND
    0x8007_048F, // ERROR_DEVICE_NOT_CONNECTED
];

/// Maps a Windows error to a [`SystemError`] (technical message for logs).
fn platform(context: &str, e: &windows::core::Error) -> SystemError {
    let code = e.code().0 as u32;
    if GONE.contains(&code) {
        SystemError::NotFound
    } else if code == 0x8007_0005 {
        // E_ACCESSDENIED
        SystemError::Denied
    } else {
        SystemError::Platform(format!("{context}: {} ({code:#010x})", e.message()))
    }
}
