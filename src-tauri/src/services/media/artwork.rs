//! Artwork handling shared by all platforms (pure): content keys, size
//! limits, data URLs and a small bounded cache.
//!
//! Artwork stays in memory only. It is transcoded natively (Windows Imaging
//! Component) to a JPEG no larger than [`MAX_EDGE_PX`], then served to the UI
//! as a `data:` URL through `get_media_artwork`, keyed by the hash of the
//! source bytes. Snapshots carry only the key, so artwork is never resent
//! with every playback update, and an unchanged thumbnail is never decoded
//! twice.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    collections::{hash_map::DefaultHasher, VecDeque},
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Thumbnails larger than this are rejected without decoding.
pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
/// Longest edge of the transcoded image (the UI shows at most ~180 px).
pub const MAX_EDGE_PX: u32 = 256;
/// Encoded images above this are dropped (a 256 px JPEG is ~15–40 KB).
pub const MAX_ENCODED_BYTES: usize = 256 * 1024;
/// Entries kept in the cache (current tracks of several sessions + history).
pub const CACHE_ENTRIES: usize = 12;

/// A stable content key for thumbnail bytes.
pub fn artwork_key(bytes: &[u8]) -> String {
    // `DefaultHasher::new()` uses fixed keys, so keys are stable per build.
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("a{:016x}{:x}", hasher.finish(), bytes.len())
}

/// Scales `(w, h)` down (never up) so the longest edge is at most `max`.
pub fn fit_within(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (0, 0);
    }
    let longest = width.max(height);
    if longest <= max {
        return (width, height);
    }
    let scale = |v: u32| {
        ((u64::from(v) * u64::from(max) + u64::from(longest) / 2) / u64::from(longest)).max(1)
            as u32
    };
    (scale(width), scale(height))
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(BASE64[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Only image types Islandy itself produces are turned into URLs.
pub fn data_url(mime: &'static str, bytes: &[u8]) -> Option<String> {
    if !matches!(mime, "image/jpeg" | "image/png")
        || bytes.is_empty()
        || bytes.len() > MAX_ENCODED_BYTES
    {
        return None;
    }
    Some(format!("data:{mime};base64,{}", base64_encode(bytes)))
}

/// Least-recently-used cache of data URLs by artwork key.
#[derive(Debug, Default)]
pub struct ArtworkCache {
    entries: VecDeque<(String, Arc<str>)>,
}

impl ArtworkCache {
    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn get(&mut self, key: &str) -> Option<Arc<str>> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let url = entry.1.clone();
        self.entries.push_back(entry);
        Some(url)
    }

    pub fn insert(&mut self, key: String, url: String) {
        self.entries.retain(|(k, _)| *k != key);
        self.entries.push_back((key, url.into()));
        while self.entries.len() > CACHE_ENTRIES {
            self.entries.pop_front();
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_content_dependent() {
        assert_eq!(artwork_key(b"abc"), artwork_key(b"abc"));
        assert_ne!(artwork_key(b"abc"), artwork_key(b"abd"));
        assert!(artwork_key(b"abc").starts_with('a'));
    }

    #[test]
    fn fits_images_without_upscaling() {
        assert_eq!(fit_within(300, 300, 256), (256, 256));
        assert_eq!(fit_within(1280, 720, 256), (256, 144));
        assert_eq!(fit_within(720, 1280, 256), (144, 256));
        assert_eq!(fit_within(100, 50, 256), (100, 50));
        assert_eq!(fit_within(10_000, 1, 256), (256, 1));
        assert_eq!(fit_within(0, 50, 256), (0, 0));
    }

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected);
        }
        assert_eq!(base64_encode(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn data_urls_are_restricted_and_bounded() {
        assert_eq!(
            data_url("image/jpeg", b"foo").as_deref(),
            Some("data:image/jpeg;base64,Zm9v")
        );
        assert_eq!(data_url("image/svg+xml", b"<svg/>"), None);
        assert_eq!(data_url("image/png", b""), None);
        assert_eq!(data_url("image/png", &vec![0; MAX_ENCODED_BYTES + 1]), None);
    }

    #[test]
    fn cache_is_bounded_and_lru() {
        let mut cache = ArtworkCache::default();
        for i in 0..CACHE_ENTRIES {
            cache.insert(format!("k{i}"), format!("u{i}"));
        }
        assert!(cache.get("k0").is_some()); // k0 is now most recent
        cache.insert("new".into(), "u".into());
        assert_eq!(cache.len(), CACHE_ENTRIES);
        assert!(cache.contains("k0"));
        assert!(!cache.contains("k1"), "least recently used entry evicted");
        assert_eq!(cache.get("new").as_deref(), Some("u"));
        cache.insert("new".into(), "v".into());
        assert_eq!(cache.len(), CACHE_ENTRIES);
        assert_eq!(cache.get("new").as_deref(), Some("v"));
    }
}
