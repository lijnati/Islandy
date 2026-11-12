//! Metadata normalization: text cleanup and app display names.
//!
//! Apps report metadata of very uneven quality. These functions only clean
//! what is there; they never invent a title or an artist.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

/// Upper bound for any metadata string, in characters. Longer values are cut
/// (the UI truncates visually anyway; this bounds memory and IPC size).
pub const MAX_TEXT_CHARS: usize = 300;

/// Trims, turns control characters (newlines, tabs, …) into spaces, collapses
/// whitespace runs, and caps the length. Empty results become `None`.
pub fn clean_text(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len().min(MAX_TEXT_CHARS * 4));
    let mut count = 0;
    let mut pending_space = false;
    for c in raw.chars() {
        // Zero-width and bidi-control characters are kept: they are legitimate
        // in many scripts. Only C0/C1 controls are replaced.
        let c = if c.is_control() { ' ' } else { c };
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            if count + 1 >= MAX_TEXT_CHARS {
                break;
            }
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        if count >= MAX_TEXT_CHARS {
            break;
        }
        out.push(c);
        count += 1;
    }
    (!out.is_empty()).then_some(out)
}

/// Track numbers are reported as `0` when unknown.
pub fn track_number(raw: i32) -> Option<u32> {
    u32::try_from(raw).ok().filter(|n| *n > 0)
}

/// A readable app name derived from an AppUserModelID when Windows cannot
/// resolve one. Generic rules only; no per-app special cases.
///
/// - Packaged apps (`Family_hash!AppId`): the app id after `!`, last dotted part.
/// - Executables (`Player.exe`, `C:\…\Player.exe`): the file stem.
/// - Opaque hashes (some apps register a hex id): no readable name.
pub fn app_display_name(aumid: &str) -> String {
    const FALLBACK: &str = "Media app";
    let id = aumid.trim();
    let name = if let Some((_, app)) = id.split_once('!') {
        app.rsplit('.').next().unwrap_or(app)
    } else {
        let file = id.rsplit(['\\', '/']).next().unwrap_or(id);
        strip_suffix_ignore_case(file, ".exe")
    };
    let name = name.trim();
    if name.is_empty() || looks_opaque(name) {
        return FALLBACK.to_owned();
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if name.chars().all(|c| !c.is_uppercase()) => {
            first.to_uppercase().chain(chars).collect()
        }
        _ => name.to_owned(),
    }
}

fn strip_suffix_ignore_case<'a>(value: &'a str, suffix: &str) -> &'a str {
    let split = value.len().wrapping_sub(suffix.len());
    match (value.get(..split), value.get(split..)) {
        (Some(stem), Some(tail)) if tail.eq_ignore_ascii_case(suffix) => stem,
        _ => value,
    }
}

/// Long all-hex identifiers (e.g. `308046B0AF4A39CB`) or GUIDs.
fn looks_opaque(name: &str) -> bool {
    let hex = name.chars().filter(|c| *c != '-' && *c != '{' && *c != '}');
    let len = hex.clone().count();
    len >= 12 && hex.into_iter().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_trims_collapses_and_drops_empty() {
        assert_eq!(clean_text("  Song  Title \n"), Some("Song Title".into()));
        assert_eq!(clean_text("a\tb\r\nc"), Some("a b c".into()));
        assert_eq!(clean_text(""), None);
        assert_eq!(clean_text(" \n\t "), None);
        assert_eq!(clean_text("\u{0}\u{1b}"), None);
    }

    #[test]
    fn clean_text_keeps_unicode_and_caps_length_on_char_boundaries() {
        assert_eq!(
            clean_text("夜に駆ける — YOASOBI 🎵"),
            Some("夜に駆ける — YOASOBI 🎵".into())
        );
        assert_eq!(clean_text("Ünïcödé"), Some("Ünïcödé".into()));
        let long = "é".repeat(1000);
        let cleaned = clean_text(&long).unwrap();
        assert_eq!(cleaned.chars().count(), MAX_TEXT_CHARS);
        let spaced = "ab ".repeat(500);
        let cleaned = clean_text(&spaced).unwrap();
        assert!(cleaned.chars().count() <= MAX_TEXT_CHARS);
        assert!(!cleaned.ends_with(' '));
    }

    #[test]
    fn track_numbers_ignore_unknown_values() {
        assert_eq!(track_number(0), None);
        assert_eq!(track_number(-3), None);
        assert_eq!(track_number(7), Some(7));
    }

    #[test]
    fn app_names_follow_generic_rules() {
        assert_eq!(app_display_name("Spotify.exe"), "Spotify");
        assert_eq!(
            app_display_name("C:\\Program Files\\Foo\\foobar2000.EXE"),
            "Foobar2000"
        );
        assert_eq!(
            app_display_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "ZuneMusic"
        );
        assert_eq!(app_display_name("MSEdge"), "MSEdge");
        assert_eq!(app_display_name("chrome"), "Chrome");
        assert_eq!(app_display_name("308046B0AF4A39CB"), "Media app");
        assert_eq!(
            app_display_name("{6D809377-6AF0-444B-8957-A3773F02200E}"),
            "Media app"
        );
        assert_eq!(app_display_name(""), "Media app");
        assert_eq!(app_display_name("Pkg_hash!"), "Media app");
    }
}
