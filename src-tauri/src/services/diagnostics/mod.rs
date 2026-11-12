//! Local diagnostics: the bounded log ([`log`]) and a plain-text summary a
//! user can copy into a bug report.
//!
//! The summary contains versions, feature availability, integration status
//! and sanitized recent warnings. It never contains tokens, paths inside
//! the user's profile, hook payloads, prompts, window titles, track titles or
//! any other user activity.

pub mod log;

use std::fmt::Write;

/// Everything the summary reports, gathered by the caller (`commands.rs`).
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub version: String,
    pub build: &'static str,
    pub tauri: &'static str,
    pub webview: Option<String>,
    pub os: String,
    pub arch: &'static str,
    pub frame_strategy: String,
    pub placement: String,
    pub shortcut: String,
    pub launch_at_login: String,
    pub settings: String,
    pub media: String,
    pub system: Vec<(&'static str, String)>,
    pub activity: String,
    pub claude_code: String,
    pub problems: Vec<String>,
}

pub fn format_report(r: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Islandy {} ({})", r.version, r.build);
    let _ = writeln!(
        out,
        "Tauri {} · WebView {}",
        r.tauri,
        r.webview.as_deref().unwrap_or("unknown")
    );
    let _ = writeln!(out, "OS: {} ({})", r.os, r.arch);
    let _ = writeln!(out, "Window: {} · {}", r.frame_strategy, r.placement);
    let _ = writeln!(out, "Shortcut: {}", r.shortcut);
    let _ = writeln!(out, "Launch at login: {}", r.launch_at_login);
    let _ = writeln!(out, "Settings: {}", r.settings);
    let _ = writeln!(out, "Media: {}", r.media);
    for (name, value) in &r.system {
        let _ = writeln!(out, "{name}: {value}");
    }
    let _ = writeln!(out, "Activity: {}", r.activity);
    let _ = writeln!(out, "Claude Code: {}", r.claude_code);
    if r.problems.is_empty() {
        let _ = writeln!(out, "Recent problems: none");
    } else {
        let _ = writeln!(out, "Recent problems:");
        for p in &r.problems {
            let _ = writeln!(out, "  {p}");
        }
    }
    out
}

/// A human-readable OS name and version.
pub fn os_version() -> String {
    #[cfg(windows)]
    {
        crate::platform::windows_version().unwrap_or_else(|| "Windows (version unknown)".into())
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                    .map(|v| v.trim_matches('"').to_owned())
            })
            .unwrap_or_else(|| std::env::consts::OS.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_every_section() {
        let report = Report {
            version: "0.1.0".into(),
            build: "release",
            tauri: "2.12.5",
            webview: Some("131.0".into()),
            os: "Windows 11 23H2 (build 22631)".into(),
            arch: "x86_64",
            frame_strategy: "region".into(),
            placement: "top center on primary, 1920×1032 @ 125%".into(),
            shortcut: "Ctrl+Alt+Space (registered)".into(),
            launch_at_login: "off".into(),
            settings: "loaded".into(),
            media: "ready, 2 sessions".into(),
            system: vec![
                ("Audio", "ready".into()),
                ("Brightness", "unsupported".into()),
            ],
            activity: "receiver listening".into(),
            claude_code: "hooks installed".into(),
            problems: vec!["2026-11-12T10:00:00.000Z WARN media: retrying".into()],
        };
        let text = format_report(&report);
        assert!(text.starts_with("Islandy 0.1.0 (release)\n"));
        assert!(text.contains("Brightness: unsupported\n"));
        assert!(
            text.contains("Recent problems:\n  2026-11-12T10:00:00.000Z WARN media: retrying\n")
        );
        let empty = format_report(&Report::default());
        assert!(empty.contains("Recent problems: none"));
        assert!(empty.contains("WebView unknown"));
    }
}
