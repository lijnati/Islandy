//! Shared WinRT/COM plumbing for the native workers (Windows only).

use std::{sync::mpsc, time::Duration};

use windows::{
    core::RuntimeType,
    Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
};
use windows_future::{IAsyncAction, IAsyncOperation, IAsyncOperationWithProgress};

/// Runs `f` on the current thread inside the multithreaded apartment.
pub fn with_mta<R>(f: impl FnOnce() -> R) -> R {
    // SAFETY: plain COM initialization for the current (worker) thread,
    // balanced by CoUninitialize below when it succeeded.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let result = f();
    if initialized {
        // SAFETY: matches the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

/// A WinRT async operation that can be awaited with a timeout.
///
/// The completion callback runs on a system thread and only signals; the
/// result (often a non-`Send` interface such as `IBuffer`) is fetched with
/// `GetResults` back on the worker thread, so no WinRT object is moved
/// across threads by Islandy.
pub trait Pending {
    type Output;
    fn on_done(&self, f: impl FnOnce() + Send + 'static) -> windows::core::Result<()>;
    fn results(&self) -> windows::core::Result<Self::Output>;
    fn cancel(&self);
}

macro_rules! impl_pending {
    ($([$($generics:tt)*] $ty:ty => $out:ty;)*) => {$(
        impl<$($generics)*> Pending for $ty {
            type Output = $out;
            fn on_done(&self, f: impl FnOnce() + Send + 'static) -> windows::core::Result<()> {
                self.when(move |_| f())
            }
            fn results(&self) -> windows::core::Result<$out> {
                self.GetResults()
            }
            fn cancel(&self) {
                let _ = self.Cancel();
            }
        }
    )*};
}

impl_pending! {
    [T: RuntimeType + 'static] IAsyncOperation<T> => T;
    [T: RuntimeType + 'static, P: RuntimeType + 'static] IAsyncOperationWithProgress<T, P> => T;
    [] IAsyncAction => ();
}

/// Why waiting for an async operation failed.
#[derive(Debug)]
pub enum WaitError {
    Timeout,
    Failed(windows::core::Error),
}

/// Waits for `op` without blocking forever; cancels it on timeout.
pub fn wait<P: Pending>(
    op: windows::core::Result<P>,
    timeout: Duration,
) -> Result<P::Output, WaitError> {
    let op = op.map_err(WaitError::Failed)?;
    let (tx, rx) = mpsc::sync_channel(1);
    op.on_done(move || {
        let _ = tx.send(());
    })
    .map_err(WaitError::Failed)?;
    match rx.recv_timeout(timeout) {
        Ok(()) => op.results().map_err(WaitError::Failed),
        Err(_) => {
            op.cancel();
            Err(WaitError::Timeout)
        }
    }
}
