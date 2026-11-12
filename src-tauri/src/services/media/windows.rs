//! The Windows backend: Global System Media Transport Controls (GSMTC).
//!
//! Runs only on the media worker thread, which joins the multithreaded COM
//! apartment. Every WinRT async call is awaited with a timeout, so an app
//! that never answers cannot stall the worker. Event handlers do no work:
//! they post a [`NativeEvent`] to the worker and return.

use std::{collections::HashMap, sync::mpsc::Sender, time::Duration};

use windows::{
    core::{IUnknown, Interface, HSTRING},
    ApplicationModel::AppInfo,
    Foundation::TypedEventHandler,
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
        BitmapTransform, ColorManagementMode, ExifOrientationMode,
    },
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
        TimelinePropertiesChangedEventArgs,
    },
    Storage::Streams::{
        Buffer, DataReader, DataWriter, IInputStream, IRandomAccessStreamReference,
        InMemoryRandomAccessStream, InputStreamOptions,
    },
};

pub use crate::services::winrt::with_mta;
use crate::services::winrt::{self, Pending, WaitError};

use super::{
    artwork,
    events::NativeEvent,
    identity, metadata,
    models::{MediaControls, Metadata, SessionId, Timeline},
    playback::{self, NativeAction, RawTimeline},
    worker::{Backend, Message, NativeMetadata, NativePlayback, SessionInfo, SessionSync},
    MediaError,
};

const MANAGER_TIMEOUT: Duration = Duration::from_secs(5);
const QUERY_TIMEOUT: Duration = Duration::from_secs(2);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
/// Decoded images above this edge length are refused (decompression bombs).
const MAX_DECODE_EDGE: u32 = 8192;

/// HRESULTs that mean "the app (and its session) went away".
fn is_gone(code: i32) -> bool {
    const GONE: [u32; 4] = [
        0x8001_0108, // RPC_E_DISCONNECTED
        0x8007_06BA, // RPC_S_SERVER_UNAVAILABLE
        0x8007_06BF, // RPC_S_CALL_FAILED_DNE
        0x8007_0490, // ERROR_NOT_FOUND
    ];
    GONE.contains(&(code as u32))
}

fn platform(e: windows::core::Error) -> MediaError {
    let code = e.code().0;
    if is_gone(code) {
        MediaError::SessionNotFound
    } else {
        MediaError::Platform(format!("{} ({:#010x})", e.message(), code as u32))
    }
}

/// Waits for `op` with a timeout, mapping failures to [`MediaError`].
fn wait<P: Pending>(
    op: windows::core::Result<P>,
    timeout: Duration,
) -> Result<P::Output, MediaError> {
    winrt::wait(op, timeout).map_err(|e| match e {
        WaitError::Timeout => MediaError::Timeout,
        WaitError::Failed(e) => platform(e),
    })
}

/// Reads exactly `size` bytes (or until the stream ends).
fn read_all(stream: &IInputStream, size: u32) -> Result<Vec<u8>, MediaError> {
    let mut bytes = Vec::with_capacity(size as usize);
    while (bytes.len() as u32) < size {
        let remaining = size - bytes.len() as u32;
        let buffer = Buffer::Create(remaining).map_err(platform)?;
        let read = wait(
            stream.ReadAsync(&buffer, remaining, InputStreamOptions::None),
            QUERY_TIMEOUT,
        )?;
        let len = read.Length().map_err(platform)?;
        if len == 0 {
            break;
        }
        let mut chunk = vec![0u8; len as usize];
        DataReader::FromBuffer(&read)
            .and_then(|reader| reader.ReadBytes(&mut chunk))
            .map_err(platform)?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn read_thumbnail(reference: &IRandomAccessStreamReference) -> Result<Vec<u8>, MediaError> {
    let stream = wait(reference.OpenReadAsync(), QUERY_TIMEOUT)?;
    let size = stream.Size().map_err(platform)?;
    if size == 0 || size > artwork::MAX_SOURCE_BYTES {
        return Err(MediaError::Platform(format!(
            "thumbnail size {size} is out of range"
        )));
    }
    let input = stream.GetInputStreamAt(0).map_err(platform)?;
    read_all(&input, size as u32)
}

fn text(value: windows::core::Result<HSTRING>) -> Option<String> {
    value
        .ok()
        .and_then(|h| metadata::clean_text(&h.to_string()))
}

struct Tracked {
    id: SessionId,
    session: Session,
    identity: usize,
    app_id: String,
    /// Media properties, playback info, timeline handler tokens.
    tokens: Option<[i64; 3]>,
}

pub struct WindowsBackend {
    manager: Manager,
    manager_tokens: Option<[i64; 2]>,
    tracked: Vec<Tracked>,
    next_id: u64,
    events: Sender<Message>,
    app_names: HashMap<String, String>,
}

fn identity_of(session: &Session) -> Result<usize, MediaError> {
    // COM identity: QueryInterface(IUnknown) returns the same pointer for the
    // same object. We hold a reference, so the value stays unique meanwhile.
    Ok(session.cast::<IUnknown>().map_err(platform)?.as_raw() as usize)
}

fn notify(tx: &Sender<Message>, event: NativeEvent) {
    let _ = tx.send(Message::Native(event));
}

impl WindowsBackend {
    pub fn connect(events: Sender<Message>) -> Result<Self, MediaError> {
        let manager = wait(Manager::RequestAsync(), MANAGER_TIMEOUT)?;
        let tx = events.clone();
        let sessions = manager
            .SessionsChanged(
                &TypedEventHandler::<Manager, SessionsChangedEventArgs>::new(move |_, _| {
                    notify(&tx, NativeEvent::SessionsChanged);
                    Ok(())
                }),
            )
            .map_err(platform)?;
        let tx = events.clone();
        let current = manager.CurrentSessionChanged(&TypedEventHandler::<
            Manager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            notify(&tx, NativeEvent::CurrentSessionChanged);
            Ok(())
        }));
        let current = match current {
            Ok(token) => token,
            Err(e) => {
                let _ = manager.RemoveSessionsChanged(sessions);
                return Err(platform(e));
            }
        };
        Ok(Self {
            manager,
            manager_tokens: Some([sessions, current]),
            tracked: Vec::new(),
            next_id: 1,
            events,
            app_names: HashMap::new(),
        })
    }

    fn subscribe(&self, session: &Session, id: SessionId) -> windows::core::Result<[i64; 3]> {
        let tx = self.events.clone();
        let properties = session.MediaPropertiesChanged(&TypedEventHandler::<
            Session,
            MediaPropertiesChangedEventArgs,
        >::new(move |_, _| {
            notify(&tx, NativeEvent::MediaPropertiesChanged(id));
            Ok(())
        }))?;
        let tx = self.events.clone();
        let playback = session.PlaybackInfoChanged(&TypedEventHandler::<
            Session,
            PlaybackInfoChangedEventArgs,
        >::new(move |_, _| {
            notify(&tx, NativeEvent::PlaybackInfoChanged(id));
            Ok(())
        }));
        let playback = playback.inspect_err(|_| {
            let _ = session.RemoveMediaPropertiesChanged(properties);
        })?;
        let tx = self.events.clone();
        let timeline = session.TimelinePropertiesChanged(&TypedEventHandler::<
            Session,
            TimelinePropertiesChangedEventArgs,
        >::new(move |_, _| {
            notify(&tx, NativeEvent::TimelinePropertiesChanged(id));
            Ok(())
        }));
        let timeline = timeline.inspect_err(|_| {
            let _ = session.RemoveMediaPropertiesChanged(properties);
            let _ = session.RemovePlaybackInfoChanged(playback);
        })?;
        Ok([properties, playback, timeline])
    }

    fn unsubscribe(tracked: &mut Tracked) {
        if let Some([properties, playback, timeline]) = tracked.tokens.take() {
            let _ = tracked.session.RemoveMediaPropertiesChanged(properties);
            let _ = tracked.session.RemovePlaybackInfoChanged(playback);
            let _ = tracked.session.RemoveTimelinePropertiesChanged(timeline);
        }
    }

    fn track(&self, session: Session, identity: usize, app_id: String, id: SessionId) -> Tracked {
        let tokens = self
            .subscribe(&session, id)
            .inspect_err(|e| {
                eprintln!(
                    "[islandy] media: subscribing to session events failed: {}",
                    platform(e.clone())
                )
            })
            .ok();
        Tracked {
            id,
            session,
            identity,
            app_id,
            tokens,
        }
    }

    fn app_name(&mut self, app_id: &str) -> String {
        if let Some(name) = self.app_names.get(app_id) {
            return name.clone();
        }
        // Resolves packaged apps; fails for most classic desktop apps.
        let resolved = AppInfo::GetFromAppUserModelId(&HSTRING::from(app_id))
            .and_then(|info| info.DisplayInfo())
            .and_then(|display| display.DisplayName())
            .ok()
            .and_then(|name| metadata::clean_text(&name.to_string()));
        let name = resolved.unwrap_or_else(|| metadata::app_display_name(app_id));
        if self.app_names.len() > 64 {
            self.app_names.clear();
        }
        self.app_names.insert(app_id.to_owned(), name.clone());
        name
    }

    fn session(&self, id: SessionId) -> Result<Session, MediaError> {
        self.tracked
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.session.clone())
            .ok_or(MediaError::SessionNotFound)
    }
}

impl Backend for WindowsBackend {
    fn sync_sessions(&mut self) -> Result<SessionSync, MediaError> {
        let list = self.manager.GetSessions().map_err(platform)?;
        let count = list.Size().map_err(platform)?;
        let mut incoming = Vec::with_capacity(count as usize);
        for i in 0..count {
            let session = list.GetAt(i).map_err(platform)?;
            let identity = identity_of(&session)?;
            let app_id = session
                .SourceAppUserModelId()
                .map(|h| h.to_string())
                .unwrap_or_default();
            incoming.push((session, identity, app_id));
        }

        let known: Vec<_> = self
            .tracked
            .iter()
            .map(|t| identity::Known {
                id: t.id,
                identity: t.identity,
                app_id: t.app_id.as_str(),
            })
            .collect();
        let candidates: Vec<_> = incoming
            .iter()
            .map(|(_, identity, app_id)| identity::Incoming {
                identity: *identity,
                app_id: app_id.as_str(),
            })
            .collect();
        let plan = identity::reconcile(&known, &candidates);

        let mut removed = Vec::new();
        for id in plan.removed {
            if let Some(index) = self.tracked.iter().position(|t| t.id == id) {
                let mut tracked = self.tracked.swap_remove(index);
                Self::unsubscribe(&mut tracked);
                removed.push(id);
            }
        }
        for (index, id) in plan.matched {
            let (session, identity, _) = &incoming[index];
            let Some(position) = self.tracked.iter().position(|t| t.id == id) else {
                continue;
            };
            if self.tracked[position].identity != *identity {
                // Same session, new object: move the event registrations.
                let mut old = self.tracked.swap_remove(position);
                Self::unsubscribe(&mut old);
                let replacement = self.track(session.clone(), *identity, old.app_id, id);
                self.tracked.push(replacement);
                for event in [
                    NativeEvent::MediaPropertiesChanged(id),
                    NativeEvent::PlaybackInfoChanged(id),
                    NativeEvent::TimelinePropertiesChanged(id),
                ] {
                    notify(&self.events, event);
                }
            }
        }
        let mut added = Vec::new();
        for index in plan.added {
            let (session, identity, app_id) = &incoming[index];
            let id = SessionId(self.next_id);
            self.next_id += 1;
            let app_name = self.app_name(app_id);
            let tracked = self.track(session.clone(), *identity, app_id.clone(), id);
            self.tracked.push(tracked);
            added.push(SessionInfo {
                id,
                app_id: app_id.clone(),
                app_name,
            });
        }

        // `GetCurrentSession` fails (null) when there is no current session.
        let current = self
            .manager
            .GetCurrentSession()
            .ok()
            .and_then(|session| identity_of(&session).ok())
            .and_then(|identity| self.tracked.iter().find(|t| t.identity == identity))
            .map(|t| t.id);
        Ok(SessionSync {
            added,
            removed,
            current,
        })
    }

    fn read_metadata(&mut self, id: SessionId) -> Result<NativeMetadata, MediaError> {
        let session = self.session(id)?;
        let props = wait(session.TryGetMediaPropertiesAsync(), QUERY_TIMEOUT)?;
        let metadata = Metadata {
            title: text(props.Title()),
            artist: text(props.Artist()),
            album: text(props.AlbumTitle()),
            album_artist: text(props.AlbumArtist()),
            track_number: props.TrackNumber().ok().and_then(metadata::track_number),
        };
        let thumbnail = match props.Thumbnail() {
            Ok(reference) => read_thumbnail(&reference)
                .inspect_err(|e| eprintln!("[islandy] media: reading artwork failed: {e}"))
                .ok(),
            Err(_) => None, // no thumbnail (null reference)
        };
        Ok(NativeMetadata {
            metadata,
            thumbnail,
        })
    }

    fn read_playback(&mut self, id: SessionId) -> Result<NativePlayback, MediaError> {
        let info = self.session(id)?.GetPlaybackInfo().map_err(platform)?;
        let status = playback::map_status(info.PlaybackStatus().map_err(platform)?.0);
        let rate = playback::effective_rate(info.PlaybackRate().ok().and_then(|r| r.Value().ok()));
        let c = info.Controls().map_err(platform)?;
        let flag = |v: windows::core::Result<bool>| v.unwrap_or(false);
        Ok(NativePlayback {
            status,
            rate,
            controls: MediaControls {
                play: flag(c.IsPlayEnabled()),
                pause: flag(c.IsPauseEnabled()),
                toggle_play_pause: flag(c.IsPlayPauseToggleEnabled()),
                next: flag(c.IsNextEnabled()),
                previous: flag(c.IsPreviousEnabled()),
                seek: flag(c.IsPlaybackPositionEnabled()),
            },
        })
    }

    fn read_timeline(
        &mut self,
        id: SessionId,
        now_ms: u64,
    ) -> Result<Option<Timeline>, MediaError> {
        let t = self
            .session(id)?
            .GetTimelineProperties()
            .map_err(platform)?;
        let raw = RawTimeline {
            start: t.StartTime().map_err(platform)?.Duration,
            end: t.EndTime().map_err(platform)?.Duration,
            position: t.Position().map_err(platform)?.Duration,
            last_updated: t.LastUpdatedTime().map(|d| d.UniversalTime).unwrap_or(0),
        };
        Ok(playback::normalize_timeline(raw, now_ms))
    }

    fn encode_artwork(&mut self, source: &[u8]) -> Result<Vec<u8>, MediaError> {
        let input = InMemoryRandomAccessStream::new().map_err(platform)?;
        let writer = DataWriter::new().map_err(platform)?;
        writer.WriteBytes(source).map_err(platform)?;
        let buffer = writer.DetachBuffer().map_err(platform)?;
        wait(input.WriteAsync(&buffer), QUERY_TIMEOUT)?;
        input.Seek(0).map_err(platform)?;

        let decoder = wait(BitmapDecoder::CreateAsync(&input), QUERY_TIMEOUT)?;
        let width = decoder.PixelWidth().map_err(platform)?;
        let height = decoder.PixelHeight().map_err(platform)?;
        if width == 0 || height == 0 || width > MAX_DECODE_EDGE || height > MAX_DECODE_EDGE {
            return Err(MediaError::Platform(format!(
                "artwork size {width}x{height} is out of range"
            )));
        }
        let (w, h) = artwork::fit_within(width, height, artwork::MAX_EDGE_PX);
        let transform = BitmapTransform::new().map_err(platform)?;
        transform.SetScaledWidth(w).map_err(platform)?;
        transform.SetScaledHeight(h).map_err(platform)?;
        transform
            .SetInterpolationMode(BitmapInterpolationMode::Fant)
            .map_err(platform)?;
        let pixels = wait(
            decoder.GetPixelDataTransformedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Premultiplied,
                &transform,
                ExifOrientationMode::IgnoreExifOrientation,
                ColorManagementMode::ColorManageToSRgb,
            ),
            QUERY_TIMEOUT,
        )?;
        let data = pixels.DetachPixelData().map_err(platform)?;

        let output = InMemoryRandomAccessStream::new().map_err(platform)?;
        let encoder_id = BitmapEncoder::JpegEncoderId().map_err(platform)?;
        let encoder = wait(
            BitmapEncoder::CreateAsync(encoder_id, &output),
            QUERY_TIMEOUT,
        )?;
        encoder
            .SetPixelData(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Ignore,
                w,
                h,
                96.0,
                96.0,
                &data[..],
            )
            .map_err(platform)?;
        wait(encoder.FlushAsync(), QUERY_TIMEOUT)?;
        let size = output.Size().map_err(platform)?;
        if size == 0 || size > artwork::MAX_ENCODED_BYTES as u64 {
            return Err(MediaError::Platform(format!(
                "encoded artwork size {size} is out of range"
            )));
        }
        read_all(&output.GetInputStreamAt(0).map_err(platform)?, size as u32)
    }

    fn execute(&mut self, id: SessionId, action: NativeAction) -> Result<bool, MediaError> {
        let session = self.session(id)?;
        let op = match action {
            NativeAction::Play => session.TryPlayAsync(),
            NativeAction::Pause => session.TryPauseAsync(),
            NativeAction::TogglePlayPause => session.TryTogglePlayPauseAsync(),
            NativeAction::Next => session.TrySkipNextAsync(),
            NativeAction::Previous => session.TrySkipPreviousAsync(),
        };
        wait(op, CONTROL_TIMEOUT)
    }

    fn seek(&mut self, id: SessionId, position_ms: u64) -> Result<bool, MediaError> {
        let session = self.session(id)?;
        let t = session.GetTimelineProperties().map_err(platform)?;
        let start = t.StartTime().map_err(platform)?.Duration;
        let end = t.EndTime().map_err(platform)?.Duration;
        let duration_ms = u64::try_from(playback::ticks_to_ms(end.saturating_sub(start)))
            .ok()
            .filter(|d| *d > 0)
            .ok_or(MediaError::ControlUnsupported)?;
        let ticks = playback::seek_ticks(position_ms, start, duration_ms);
        wait(
            session.TryChangePlaybackPositionAsync(ticks),
            CONTROL_TIMEOUT,
        )
    }

    fn shutdown(&mut self) {
        for tracked in &mut self.tracked {
            Self::unsubscribe(tracked);
        }
        self.tracked.clear();
        if let Some([sessions, current]) = self.manager_tokens.take() {
            let _ = self.manager.RemoveSessionsChanged(sessions);
            let _ = self.manager.RemoveCurrentSessionChanged(current);
        }
    }
}

impl Drop for WindowsBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}
