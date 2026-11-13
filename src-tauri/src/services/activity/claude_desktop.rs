//! Claude Desktop (the chat app) has no hooks, so on Windows Islandy watches
//! for its "Stop response" button through UI Automation: the button exists
//! exactly while Claude is writing a reply.
//!
//! Privacy: the probe asks the Claude window one question, "is there a
//! button named Stop response?". It reads no message text, titles or other
//! content, and nothing leaves the machine. It only runs while a Claude
//! Desktop window exists, and only while the Claude source is on.
//!
//! The probe result feeds a small state machine that speaks the same
//! language as the Claude Code hooks (prompt submitted → working, stop →
//! finished), so the island, peeks and history treat both alike.

use std::time::Duration;

use super::protocol::{session_key, ClaudeHook, ClaudeWire};

/// How often the Claude window is probed.
pub const POLL: Duration = Duration::from_millis(1200);
/// While Claude works, a heartbeat keeps the session from turning stale.
const HEARTBEAT_MS: u64 = 60_000;
/// Consecutive probes without the button before the reply counts as done
/// (the button can blink out between steps of a long reply).
const DONE_AFTER: u8 = 2;
/// Shown as the session's name in the island.
pub const PROJECT: &str = "Claude Desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Probe {
    /// No Claude Desktop window.
    NotRunning,
    Running {
        responding: bool,
    },
}

/// Turns probe results into hook-like events.
#[derive(Debug, Default)]
pub struct Tracker {
    responding: bool,
    misses: u8,
    last_beat: u64,
    /// A session exists in the engine (something was sent).
    announced: bool,
}

impl Tracker {
    pub fn update(&mut self, probe: Probe, now: u64) -> Option<ClaudeWire> {
        match probe {
            Probe::Running { responding: true } => {
                self.misses = 0;
                if self.responding {
                    if now.saturating_sub(self.last_beat) >= HEARTBEAT_MS {
                        self.last_beat = now;
                        return Some(wire(ClaudeHook::PreToolUse));
                    }
                    return None;
                }
                self.responding = true;
                self.announced = true;
                self.last_beat = now;
                Some(wire(ClaudeHook::UserPromptSubmit))
            }
            Probe::Running { responding: false } => {
                if !self.responding {
                    return None;
                }
                self.misses += 1;
                if self.misses < DONE_AFTER {
                    return None;
                }
                self.responding = false;
                self.misses = 0;
                Some(wire(ClaudeHook::Stop))
            }
            Probe::NotRunning => {
                self.responding = false;
                self.misses = 0;
                if !self.announced {
                    return None;
                }
                self.announced = false;
                Some(wire(ClaudeHook::SessionEnd))
            }
        }
    }
}

fn wire(event: ClaudeHook) -> ClaudeWire {
    ClaudeWire {
        event,
        session: session_key("islandy:claude-desktop"),
        project: Some(PROJECT.to_owned()),
        tool: None,
        notification: None,
        error: None,
        subagent: false,
    }
}

#[cfg(windows)]
pub use self::windows_probe::Prober;

#[cfg(not(windows))]
pub struct Prober;

#[cfg(not(windows))]
impl Prober {
    pub fn new() -> Option<Self> {
        None
    }
    pub fn probe(&mut self) -> Probe {
        Probe::NotRunning
    }
}

#[cfg(windows)]
mod windows_probe {
    use windows::{
        core::{Interface, BOOL},
        Win32::{
            Foundation::{CloseHandle, HWND, LPARAM},
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
                },
                Threading::{
                    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
                Variant::VARIANT,
            },
            UI::{
                Accessibility::{
                    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationCondition,
                    TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId,
                    UIA_NamePropertyId,
                },
                WindowsAndMessaging::{
                    EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowThreadProcessId,
                },
            },
        },
    };

    use super::Probe;

    /// The accessible names of Claude's stop button.
    const STOP_NAMES: [&str; 2] = ["Stop response", "Stop generating"];

    pub struct Prober {
        uia: IUIAutomation,
        condition: IUIAutomationCondition,
    }

    impl Prober {
        /// `None` if UI Automation is unavailable. Call on the thread that
        /// will probe (it joins the COM multithreaded apartment).
        pub fn new() -> Option<Self> {
            // SAFETY: plain COM calls on this thread; failures are handled.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let uia: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                // A hung Claude window must not hang Islandy's probe thread.
                if let Ok(uia2) = uia.cast::<IUIAutomation2>() {
                    let _ = uia2.SetConnectionTimeout(1500);
                    let _ = uia2.SetTransactionTimeout(1500);
                }
                let button = uia
                    .CreatePropertyCondition(
                        UIA_ControlTypePropertyId,
                        &VARIANT::from(UIA_ButtonControlTypeId.0),
                    )
                    .ok()?;
                let mut names: Option<IUIAutomationCondition> = None;
                for name in STOP_NAMES {
                    let c = uia
                        .CreatePropertyCondition(UIA_NamePropertyId, &VARIANT::from(name))
                        .ok()?;
                    names = Some(match names {
                        None => c,
                        Some(prev) => uia.CreateOrCondition(&prev, &c).ok()?,
                    });
                }
                let condition = uia.CreateAndCondition(&button, &names?).ok()?;
                Some(Self { uia, condition })
            }
        }

        pub fn probe(&mut self) -> Probe {
            let windows = claude_windows();
            if windows.is_empty() {
                return Probe::NotRunning;
            }
            let responding = windows.into_iter().any(|hwnd| {
                // SAFETY: COM calls on a window handle that may have closed
                // meanwhile; UI Automation then returns an error.
                unsafe {
                    self.uia
                        .ElementFromHandle(hwnd)
                        .and_then(|root| root.FindFirst(TreeScope_Descendants, &self.condition))
                        .is_ok()
                }
            });
            Probe::Running { responding }
        }
    }

    /// Titled Chromium top-level windows owned by `claude.exe`.
    fn claude_windows() -> Vec<HWND> {
        unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: `lparam` is the `Vec` passed below, alive for the call.
            let found = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
            // SAFETY: read-only window queries.
            if unsafe { is_claude_window(hwnd) } {
                found.push(hwnd);
            }
            BOOL(1)
        }
        let mut found: Vec<HWND> = Vec::new();
        // SAFETY: the callback only touches `found` during this call.
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut found as *mut Vec<HWND> as isize));
        }
        found
    }

    unsafe fn is_claude_window(hwnd: HWND) -> bool {
        let mut class = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, &mut class) };
        if n <= 0 || String::from_utf16_lossy(&class[..n as usize]) != "Chrome_WidgetWin_1" {
            return false;
        }
        if unsafe { GetWindowTextLengthW(hwnd) } == 0 {
            return false;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 {
            return false;
        }
        let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        else {
            return false;
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        }
        .is_ok();
        let _ = unsafe { CloseHandle(process) };
        if !ok {
            return false;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .is_some_and(|exe| exe.eq_ignore_ascii_case("claude.exe"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(tracker: &mut Tracker, probes: &[(Probe, u64)]) -> Vec<ClaudeHook> {
        probes
            .iter()
            .filter_map(|&(p, at)| tracker.update(p, at))
            .map(|w| w.event)
            .collect()
    }

    const ON: Probe = Probe::Running { responding: true };
    const OFF: Probe = Probe::Running { responding: false };

    #[test]
    fn a_reply_is_a_turn() {
        let mut t = Tracker::default();
        assert_eq!(
            events(
                &mut t,
                &[(OFF, 0), (ON, 1), (ON, 2), (OFF, 3), (OFF, 4), (OFF, 5)]
            ),
            [ClaudeHook::UserPromptSubmit, ClaudeHook::Stop]
        );
    }

    #[test]
    fn a_blink_does_not_end_the_reply() {
        let mut t = Tracker::default();
        assert_eq!(
            events(&mut t, &[(ON, 0), (OFF, 1), (ON, 2), (OFF, 3), (OFF, 4)]),
            [ClaudeHook::UserPromptSubmit, ClaudeHook::Stop]
        );
    }

    #[test]
    fn long_replies_send_heartbeats() {
        let mut t = Tracker::default();
        assert_eq!(
            events(&mut t, &[(ON, 0), (ON, 30_000), (ON, 60_000), (ON, 90_000)]),
            [ClaudeHook::UserPromptSubmit, ClaudeHook::PreToolUse]
        );
    }

    #[test]
    fn closing_claude_ends_the_session_once() {
        let mut t = Tracker::default();
        assert_eq!(
            events(
                &mut t,
                &[(Probe::NotRunning, 0), (OFF, 1), (Probe::NotRunning, 2)]
            ),
            []
        );
        assert_eq!(
            events(
                &mut t,
                &[(ON, 3), (Probe::NotRunning, 4), (Probe::NotRunning, 5)]
            ),
            [ClaudeHook::UserPromptSubmit, ClaudeHook::SessionEnd]
        );
    }

    #[test]
    fn events_carry_a_stable_session_and_name() {
        let mut t = Tracker::default();
        let w = t.update(ON, 0).unwrap();
        assert!(super::super::protocol::is_session_key(&w.session));
        assert_eq!(w.project.as_deref(), Some(PROJECT));
        assert!(
            super::super::protocol::validate(&super::super::protocol::WireEvent::Claude(w)).is_ok()
        );
    }
}
