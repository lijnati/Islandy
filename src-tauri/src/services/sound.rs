//! Islandy's notification sound: a short, soft two-note chime played when
//! something new lands on the island (an activity peek).
//!
//! The sound is synthesized here from a few sine partials (no audio file,
//! nothing copied from another product): a soft low "tap" as the peek lands,
//! then a rising fourth (D6 → G6) with bell-like decays. It lasts under half
//! a second and is mastered quietly.
//!
//! On Windows it plays through `PlaySoundW` from memory, asynchronously, as a
//! *system* sound (`SND_SYSTEM`), so it follows the "System sounds" volume in
//! the Windows mixer and never blocks the caller. Elsewhere (and in tests) it
//! is silent.

use std::{f32::consts::TAU, sync::OnceLock};

use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: u32 = 44_100;
/// Total length of the chime.
const LENGTH_S: f32 = 0.46;

/// How loud the chime is, relative to the Windows system-sounds volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoundVolume {
    Soft,
    #[default]
    Normal,
    Loud,
}

impl SoundVolume {
    /// Peak amplitude (full scale = 1.0).
    fn peak(self) -> f32 {
        match self {
            Self::Soft => 0.28,
            Self::Normal => 0.5,
            Self::Loud => 0.8,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Soft => 0,
            Self::Normal => 1,
            Self::Loud => 2,
        }
    }
}

/// One bell-like voice: a few harmonics under a fast attack and an
/// exponential decay.
struct Voice {
    start: f32,
    freq: f32,
    gain: f32,
    /// Decay time constant (seconds).
    tau: f32,
    /// Relative levels of the 1st, 2nd and 3rd harmonic.
    partials: [f32; 3],
}

const ATTACK_S: f32 = 0.004;

const VOICES: [Voice; 3] = [
    // The landing "tap": low, very short, mostly fundamental.
    Voice {
        start: 0.0,
        freq: 587.33, // D5
        gain: 0.35,
        tau: 0.022,
        partials: [1.0, 0.15, 0.0],
    },
    // First note.
    Voice {
        start: 0.012,
        freq: 1174.66, // D6
        gain: 0.8,
        tau: 0.075,
        partials: [1.0, 0.22, 0.06],
    },
    // Second note, a fourth up, rings a little longer.
    Voice {
        start: 0.098,
        freq: 1567.98, // G6
        gain: 1.0,
        tau: 0.12,
        partials: [1.0, 0.18, 0.05],
    },
];

/// The chime as 16-bit mono samples, normalized to `volume`'s peak.
pub fn chime_samples(volume: SoundVolume) -> Vec<i16> {
    let n = (LENGTH_S * SAMPLE_RATE as f32) as usize;
    let fade = (0.012 * SAMPLE_RATE as f32) as usize;
    let mut buf = vec![0f32; n];
    for (i, sample) in buf.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        for v in &VOICES {
            let local = t - v.start;
            if local < 0.0 {
                continue;
            }
            let env = (local / ATTACK_S).min(1.0) * (-(local - ATTACK_S).max(0.0) / v.tau).exp();
            let tone: f32 = v
                .partials
                .iter()
                .enumerate()
                .map(|(k, level)| level * (TAU * v.freq * (k as f32 + 1.0) * local).sin())
                .sum();
            *sample += v.gain * env * tone;
        }
        // A short fade at the very end: no click when the buffer stops.
        if i + fade > n {
            *sample *= (n - i) as f32 / fade as f32;
        }
    }
    let max = buf
        .iter()
        .fold(0f32, |m, s| m.max(s.abs()))
        .max(f32::EPSILON);
    let scale = volume.peak() / max;
    buf.iter()
        .map(|s| (s * scale * f32::from(i16::MAX)).round() as i16)
        .collect()
}

/// A complete in-memory WAV file (PCM, 16-bit, mono).
pub fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// The three rendered chimes, built once. `PlaySoundW(SND_MEMORY|SND_ASYNC)`
/// reads the buffer while it plays, so it must live for the whole process.
fn rendered(volume: SoundVolume) -> &'static [u8] {
    static CACHE: OnceLock<[Vec<u8>; 3]> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        [SoundVolume::Soft, SoundVolume::Normal, SoundVolume::Loud].map(|v| wav(&chime_samples(v)))
    });
    &all[volume.index()]
}

/// Plays the chime without waiting for it. Silent off Windows and in tests.
pub fn play(volume: SoundVolume) {
    let bytes = rendered(volume);
    #[cfg(all(windows, not(test)))]
    {
        use windows::{
            core::PCWSTR,
            Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT, SND_SYSTEM},
        };
        // SAFETY: with SND_MEMORY the "name" is a pointer to a complete WAV
        // image; `bytes` is 'static, so it outlives the asynchronous playback.
        let ok = unsafe {
            PlaySoundW(
                PCWSTR(bytes.as_ptr().cast()),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT | SND_SYSTEM,
            )
        };
        if !ok.as_bool() {
            crate::log_warn!("sound", "the notification sound couldn't be played");
        }
    }
    #[cfg(not(all(windows, not(test))))]
    let _ = bytes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chime_is_short_quiet_and_not_clipped() {
        for volume in [SoundVolume::Soft, SoundVolume::Normal, SoundVolume::Loud] {
            let s = chime_samples(volume);
            assert_eq!(s.len(), (LENGTH_S * SAMPLE_RATE as f32) as usize);
            let peak = s.iter().map(|x| i32::from(*x).abs()).max().unwrap();
            let expected = (volume.peak() * f32::from(i16::MAX)) as i32;
            assert!(
                (peak - expected).abs() <= 2,
                "{volume:?}: {peak} vs {expected}"
            );
            // Starts and ends in silence (no clicks).
            assert_eq!(s[0], 0);
            assert!(i32::from(*s.last().unwrap()).abs() < 50);
        }
        let soft = chime_samples(SoundVolume::Soft);
        let loud = chime_samples(SoundVolume::Loud);
        let energy = |s: &[i16]| s.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>();
        assert!(energy(&soft) < energy(&loud));
    }

    #[test]
    fn wav_has_a_valid_pcm_header() {
        let w = wav(&[0, 1, -1, i16::MAX]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(
            u32::from_le_bytes(w[24..28].try_into().unwrap()),
            SAMPLE_RATE
        );
        assert_eq!(&w[36..40], b"data");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 8);
        assert_eq!(w.len(), 52);
    }

    #[test]
    fn rendering_is_cached_and_playing_is_harmless_in_tests() {
        let a = rendered(SoundVolume::Normal).as_ptr();
        let b = rendered(SoundVolume::Normal).as_ptr();
        assert_eq!(a, b);
        play(SoundVolume::Loud);
    }
}
