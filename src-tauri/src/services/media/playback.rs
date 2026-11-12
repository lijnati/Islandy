//! Playback status, capability and timeline normalization (pure).

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use super::models::{MediaAction, MediaControls, PlaybackStatus, Timeline};

/// WinRT `TimeSpan` / `DateTime` use 100 ns ticks.
pub const TICKS_PER_MS: i64 = 10_000;
/// `DateTime` counts from 1601-01-01 UTC; this is 1970-01-01 in those ticks.
const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
/// Positions closer than this to a prediction are not worth a new snapshot.
pub const TIMELINE_TOLERANCE_MS: u64 = 1_500;
/// Rates outside (0, MAX_RATE] are treated as unknown (1×).
const MAX_RATE: f64 = 16.0;

/// Maps `GlobalSystemMediaTransportControlsSessionPlaybackStatus` values.
pub fn map_status(raw: i32) -> PlaybackStatus {
    match raw {
        0 | 1 | 3 => PlaybackStatus::Stopped, // Closed, Opened, Stopped
        2 => PlaybackStatus::Changing,
        4 => PlaybackStatus::Playing,
        5 => PlaybackStatus::Paused,
        _ => PlaybackStatus::Unknown,
    }
}

/// Apps report `null`, `0` or garbage when they don't know the rate.
pub fn effective_rate(raw: Option<f64>) -> f64 {
    match raw {
        Some(rate) if rate.is_finite() && rate > 0.0 && rate <= MAX_RATE => rate,
        _ => 1.0,
    }
}

/// Raw timeline values in 100 ns ticks, as read from Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawTimeline {
    pub start: i64,
    pub end: i64,
    pub position: i64,
    /// `DateTime::UniversalTime` (ticks since 1601), `0` when unknown.
    pub last_updated: i64,
}

pub fn ticks_to_ms(ticks: i64) -> i64 {
    ticks / TICKS_PER_MS
}

/// Converts a WinRT `DateTime` to Unix milliseconds; `None` if not set.
pub fn datetime_to_unix_ms(ticks: i64) -> Option<u64> {
    if ticks <= UNIX_EPOCH_TICKS {
        return None;
    }
    u64::try_from((ticks - UNIX_EPOCH_TICKS) / TICKS_PER_MS).ok()
}

/// Normalizes a timeline. Returns `None` when the app exposes no timeline at
/// all (every value zero), which is common for apps without seek support.
pub fn normalize_timeline(raw: RawTimeline, now_ms: u64) -> Option<Timeline> {
    if raw.start == 0 && raw.end == 0 && raw.position == 0 {
        return None;
    }
    let duration_ms = u64::try_from(ticks_to_ms(raw.end.saturating_sub(raw.start)))
        .ok()
        .filter(|d| *d > 0);
    let relative = u64::try_from(ticks_to_ms(raw.position.saturating_sub(raw.start))).unwrap_or(0);
    let position_ms = duration_ms.map_or(relative, |d| relative.min(d));
    // A timestamp in the future (clock skew) or missing means "as of now".
    let updated_at_ms = datetime_to_unix_ms(raw.last_updated)
        .filter(|t| *t <= now_ms)
        .unwrap_or(now_ms);
    Some(Timeline {
        position_ms,
        duration_ms,
        updated_at_ms,
    })
}

/// The position the UI would show at `at_ms`, interpolating while playing.
pub fn predict_position(timeline: &Timeline, playing: bool, rate: f64, at_ms: u64) -> u64 {
    if !playing {
        return timeline.position_ms;
    }
    let elapsed = at_ms.saturating_sub(timeline.updated_at_ms) as f64 * effective_rate(Some(rate));
    let predicted = timeline.position_ms.saturating_add(elapsed as u64);
    timeline.duration_ms.map_or(predicted, |d| predicted.min(d))
}

/// Whether `new` tells the UI anything its interpolation of `old` does not.
/// Many apps re-report the timeline every second; those updates are dropped.
pub fn timeline_changed(
    old: Option<&Timeline>,
    new: Option<&Timeline>,
    playing: bool,
    rate: f64,
) -> bool {
    match (old, new) {
        (None, None) => false,
        (Some(old), Some(new)) => {
            if old.duration_ms != new.duration_ms {
                return true;
            }
            let predicted = predict_position(old, playing, rate, new.updated_at_ms);
            predicted.abs_diff(new.position_ms) > TIMELINE_TOLERANCE_MS
        }
        _ => true,
    }
}

/// A command as it will be sent to the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAction {
    Play,
    Pause,
    TogglePlayPause,
    Next,
    Previous,
}

/// Resolves a UI action against what the app supports. A toggle falls back to
/// play/pause when the app doesn't implement toggling. `None` = unsupported.
pub fn resolve_action(
    action: MediaAction,
    status: PlaybackStatus,
    controls: &MediaControls,
) -> Option<NativeAction> {
    match action {
        MediaAction::Play => controls.play.then_some(NativeAction::Play),
        MediaAction::Pause => controls.pause.then_some(NativeAction::Pause),
        MediaAction::Next => controls.next.then_some(NativeAction::Next),
        MediaAction::Previous => controls.previous.then_some(NativeAction::Previous),
        MediaAction::TogglePlayPause => {
            if controls.toggle_play_pause {
                Some(NativeAction::TogglePlayPause)
            } else if status == PlaybackStatus::Playing {
                controls.pause.then_some(NativeAction::Pause)
            } else {
                controls.play.then_some(NativeAction::Play)
            }
        }
    }
}

/// Seeking needs both the app's permission and a known duration.
pub fn can_seek(position_enabled: bool, timeline: Option<&Timeline>) -> bool {
    position_enabled && timeline.and_then(|t| t.duration_ms).is_some()
}

/// The absolute `TryChangePlaybackPositionAsync` argument for a requested
/// position (ms from the start), clamped to the media's bounds.
pub fn seek_ticks(position_ms: u64, start_ticks: i64, duration_ms: u64) -> i64 {
    let clamped = i64::try_from(position_ms.min(duration_ms)).unwrap_or(i64::MAX);
    start_ticks.saturating_add(clamped.saturating_mul(TICKS_PER_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: i64 = 1000 * TICKS_PER_MS;
    const NOW: u64 = 1_700_000_000_000;

    fn unix_ms_to_ticks(ms: u64) -> i64 {
        UNIX_EPOCH_TICKS + ms as i64 * TICKS_PER_MS
    }

    #[test]
    fn maps_every_native_status() {
        assert_eq!(map_status(0), PlaybackStatus::Stopped);
        assert_eq!(map_status(1), PlaybackStatus::Stopped);
        assert_eq!(map_status(2), PlaybackStatus::Changing);
        assert_eq!(map_status(3), PlaybackStatus::Stopped);
        assert_eq!(map_status(4), PlaybackStatus::Playing);
        assert_eq!(map_status(5), PlaybackStatus::Paused);
        assert_eq!(map_status(99), PlaybackStatus::Unknown);
    }

    #[test]
    fn rates_fall_back_to_normal_speed() {
        assert_eq!(effective_rate(None), 1.0);
        assert_eq!(effective_rate(Some(0.0)), 1.0);
        assert_eq!(effective_rate(Some(-1.0)), 1.0);
        assert_eq!(effective_rate(Some(f64::NAN)), 1.0);
        assert_eq!(effective_rate(Some(100.0)), 1.0);
        assert_eq!(effective_rate(Some(1.5)), 1.5);
    }

    #[test]
    fn normalizes_timelines_relative_to_start_and_clamps() {
        let raw = RawTimeline {
            start: 10 * SEC,
            end: 190 * SEC,
            position: 70 * SEC,
            last_updated: unix_ms_to_ticks(NOW - 500),
        };
        assert_eq!(
            normalize_timeline(raw, NOW),
            Some(Timeline {
                position_ms: 60_000,
                duration_ms: Some(180_000),
                updated_at_ms: NOW - 500
            })
        );
        // Position past the end, and before the start.
        let past = RawTimeline {
            position: 500 * SEC,
            ..raw
        };
        assert_eq!(normalize_timeline(past, NOW).unwrap().position_ms, 180_000);
        let before = RawTimeline { position: 0, ..raw };
        assert_eq!(normalize_timeline(before, NOW).unwrap().position_ms, 0);
    }

    #[test]
    fn handles_unknown_duration_missing_and_future_timestamps() {
        assert_eq!(
            normalize_timeline(
                RawTimeline {
                    start: 0,
                    end: 0,
                    position: 0,
                    last_updated: 0
                },
                NOW
            ),
            None
        );
        let live = RawTimeline {
            start: 0,
            end: 0,
            position: 42 * SEC,
            last_updated: 0,
        };
        assert_eq!(
            normalize_timeline(live, NOW),
            Some(Timeline {
                position_ms: 42_000,
                duration_ms: None,
                updated_at_ms: NOW
            })
        );
        let future = RawTimeline {
            start: 0,
            end: 60 * SEC,
            position: SEC,
            last_updated: unix_ms_to_ticks(NOW + 60_000),
        };
        assert_eq!(normalize_timeline(future, NOW).unwrap().updated_at_ms, NOW);
        assert_eq!(datetime_to_unix_ms(0), None);
    }

    #[test]
    fn predicts_and_clamps_positions() {
        let t = Timeline {
            position_ms: 10_000,
            duration_ms: Some(20_000),
            updated_at_ms: NOW,
        };
        assert_eq!(predict_position(&t, true, 1.0, NOW + 2_000), 12_000);
        assert_eq!(predict_position(&t, true, 2.0, NOW + 2_000), 14_000);
        assert_eq!(predict_position(&t, false, 1.0, NOW + 2_000), 10_000);
        assert_eq!(predict_position(&t, true, 1.0, NOW + 60_000), 20_000);
        assert_eq!(predict_position(&t, true, 1.0, NOW - 5_000), 10_000);
    }

    #[test]
    fn timeline_changes_ignore_updates_matching_interpolation() {
        let old = Timeline {
            position_ms: 10_000,
            duration_ms: Some(200_000),
            updated_at_ms: NOW,
        };
        let tick = Timeline {
            position_ms: 11_000,
            updated_at_ms: NOW + 1_000,
            ..old
        };
        assert!(!timeline_changed(Some(&old), Some(&tick), true, 1.0));
        // While paused, a small drift stays within tolerance; a jump does not.
        assert!(!timeline_changed(Some(&old), Some(&tick), false, 1.0));
        let jumped = Timeline {
            position_ms: 15_000,
            ..tick
        };
        assert!(timeline_changed(Some(&old), Some(&jumped), false, 1.0));
        let seeked = Timeline {
            position_ms: 90_000,
            updated_at_ms: NOW + 1_000,
            ..old
        };
        assert!(timeline_changed(Some(&old), Some(&seeked), true, 1.0));
        let new_track = Timeline {
            duration_ms: Some(150_000),
            ..old
        };
        assert!(timeline_changed(Some(&old), Some(&new_track), true, 1.0));
        assert!(timeline_changed(None, Some(&old), true, 1.0));
        assert!(timeline_changed(Some(&old), None, true, 1.0));
        assert!(!timeline_changed(None, None, true, 1.0));
    }

    #[test]
    fn resolves_actions_against_capabilities() {
        let all = MediaControls {
            play: true,
            pause: true,
            toggle_play_pause: true,
            next: true,
            previous: true,
            seek: true,
        };
        assert_eq!(
            resolve_action(MediaAction::Next, PlaybackStatus::Playing, &all),
            Some(NativeAction::Next)
        );
        let none = MediaControls::default();
        for action in [
            MediaAction::Play,
            MediaAction::Pause,
            MediaAction::Next,
            MediaAction::Previous,
            MediaAction::TogglePlayPause,
        ] {
            assert_eq!(resolve_action(action, PlaybackStatus::Playing, &none), None);
        }
        let no_toggle = MediaControls {
            play: true,
            pause: true,
            ..none
        };
        assert_eq!(
            resolve_action(
                MediaAction::TogglePlayPause,
                PlaybackStatus::Playing,
                &no_toggle
            ),
            Some(NativeAction::Pause)
        );
        assert_eq!(
            resolve_action(
                MediaAction::TogglePlayPause,
                PlaybackStatus::Paused,
                &no_toggle
            ),
            Some(NativeAction::Play)
        );
        let play_only = MediaControls { play: true, ..none };
        assert_eq!(
            resolve_action(
                MediaAction::TogglePlayPause,
                PlaybackStatus::Playing,
                &play_only
            ),
            None
        );
    }

    #[test]
    fn seek_requires_permission_and_duration_and_is_clamped() {
        let t = Timeline {
            position_ms: 0,
            duration_ms: Some(1_000),
            updated_at_ms: NOW,
        };
        let live = Timeline {
            duration_ms: None,
            ..t
        };
        assert!(can_seek(true, Some(&t)));
        assert!(!can_seek(false, Some(&t)));
        assert!(!can_seek(true, Some(&live)));
        assert!(!can_seek(true, None));
        assert_eq!(
            seek_ticks(500, 5 * SEC, 1_000),
            5 * SEC + 500 * TICKS_PER_MS
        );
        assert_eq!(seek_ticks(9_999, 0, 1_000), 1_000 * TICKS_PER_MS);
    }
}
