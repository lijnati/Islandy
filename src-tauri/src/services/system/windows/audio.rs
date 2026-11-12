//! Core Audio backend: default output volume/mute and output devices.
//!
//! - `IMMDeviceEnumerator` finds active render endpoints and the default
//!   console output (the one the Windows volume flyout controls).
//! - `IAudioEndpointVolume` reads/sets the master volume *scalar* and mute.
//! - `IAudioEndpointVolumeCallback` reports volume changes from any source.
//! - `IMMNotificationClient` reports default-device changes, devices being
//!   added/removed/enabled/disabled and renames.
//!
//! Callbacks run on Windows-owned threads and must not call back into the
//! MMDevice API, so they only post an [`AudioEvent`] to the worker. Callback
//! objects are unregistered in [`CoreAudio::shutdown`] (also run on drop),
//! before the enumerator and endpoint are released.
//!
//! Changing the *default* device is not offered: Windows has no documented
//! API for it (the `IPolicyConfig` interface some tools use is undocumented
//! and has changed between releases). The UI links to Sound settings instead.

use std::sync::mpsc::Sender;

use windows::{
    core::{implement, GUID, PCWSTR, PWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::PROPERTYKEY,
        Media::Audio::{
            eConsole, eRender, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE,
        },
        System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ},
    },
};

use super::{
    super::{
        audio::{
            clean_name, device_key, percent_to_scalar, scalar_to_percent, AudioBackend, AudioEvent,
            AudioMessage, DefaultDevice,
        },
        models::AudioDevice,
        SystemError,
    },
    platform,
};

/// Identifies volume changes made by Islandy in notifications (diagnostics).
const EVENT_CONTEXT: GUID = GUID::from_u128(0x6f1c_2a8e_51d4_4a8b_9b3e_1f2c_7d0a_9e41);

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback(Sender<AudioMessage>);

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, _data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        let _ = self.0.send(AudioMessage::Native(AudioEvent::VolumeChanged));
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceCallback(Sender<AudioMessage>);

impl DeviceCallback_Impl {
    fn changed(&self) -> windows::core::Result<()> {
        let _ = self
            .0
            .send(AudioMessage::Native(AudioEvent::DevicesChanged));
        Ok(())
    }
}

impl IMMNotificationClient_Impl for DeviceCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.changed()
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.changed()
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.changed()
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        // Fired once per role; only the console output matters here.
        if flow == eRender && role == eConsole {
            self.changed()
        } else {
            Ok(())
        }
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, key: &PROPERTYKEY) -> windows::core::Result<()> {
        // Many drivers fire this constantly; only renames are relevant.
        if key.fmtid == PKEY_Device_FriendlyName.fmtid && key.pid == PKEY_Device_FriendlyName.pid {
            self.changed()
        } else {
            Ok(())
        }
    }
}

struct Bound {
    endpoint: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

pub struct CoreAudio {
    enumerator: IMMDeviceEnumerator,
    client: Option<IMMNotificationClient>,
    bound: Option<Bound>,
    events: Sender<AudioMessage>,
}

fn device_id(device: &IMMDevice) -> Result<String, SystemError> {
    // SAFETY: GetId returns a CoTaskMemAlloc'd string that we copy and free.
    unsafe {
        let raw: PWSTR = device.GetId().map_err(|e| platform("GetId", &e))?;
        let id = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(id)
    }
}

fn friendly_name(device: &IMMDevice) -> String {
    // SAFETY: plain COM calls on a live device; the PROPVARIANT is owned.
    let name = unsafe {
        device
            .OpenPropertyStore(STGM_READ)
            .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
            .and_then(|value| windows::core::BSTR::try_from(&value))
    };
    clean_name(&name.map(|b| b.to_string()).unwrap_or_default())
}

impl CoreAudio {
    pub fn connect(events: Sender<AudioMessage>) -> Result<Self, SystemError> {
        // SAFETY: COM is initialized (MTA) on this worker thread.
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| platform("MMDeviceEnumerator", &e))?;
        let client: IMMNotificationClient = DeviceCallback(events.clone()).into();
        // SAFETY: the client stays alive (held in `self.client`) until it is
        // unregistered in `shutdown`.
        unsafe { enumerator.RegisterEndpointNotificationCallback(&client) }
            .map_err(|e| platform("RegisterEndpointNotificationCallback", &e))?;
        Ok(Self {
            enumerator,
            client: Some(client),
            bound: None,
            events,
        })
    }

    fn unbind(&mut self) {
        if let Some(bound) = self.bound.take() {
            // SAFETY: unregisters the callback registered in `bind`.
            let _ = unsafe {
                bound
                    .endpoint
                    .UnregisterControlChangeNotify(&bound.callback)
            };
        }
    }

    fn bind(&mut self, device: &IMMDevice) -> Result<(), SystemError> {
        self.unbind();
        // SAFETY: activating a documented interface on a live device; the
        // callback is kept alive in `self.bound` until unregistered.
        unsafe {
            let endpoint: IAudioEndpointVolume = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| platform("Activate(IAudioEndpointVolume)", &e))?;
            let callback: IAudioEndpointVolumeCallback = VolumeCallback(self.events.clone()).into();
            endpoint
                .RegisterControlChangeNotify(&callback)
                .map_err(|e| platform("RegisterControlChangeNotify", &e))?;
            self.bound = Some(Bound { endpoint, callback });
        }
        Ok(())
    }

    fn endpoint(&self) -> Result<&IAudioEndpointVolume, SystemError> {
        self.bound
            .as_ref()
            .map(|b| &b.endpoint)
            .ok_or(SystemError::NotFound)
    }
}

impl AudioBackend for CoreAudio {
    fn refresh_devices(
        &mut self,
    ) -> Result<(Option<DefaultDevice>, Vec<AudioDevice>), SystemError> {
        self.unbind();
        // SAFETY: COM calls on objects owned by this thread.
        let default = match unsafe { self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole) } {
            Ok(device) => Some(device),
            Err(e) => match platform("GetDefaultAudioEndpoint", &e) {
                // No output devices at all.
                SystemError::NotFound => None,
                other => return Err(other),
            },
        };
        let default_id = default.as_ref().map(device_id).transpose()?;
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
        }
        .map_err(|e| platform("EnumAudioEndpoints", &e))?;
        let count = unsafe { collection.GetCount() }.map_err(|e| platform("GetCount", &e))?;
        let mut devices = Vec::with_capacity(count as usize);
        for i in 0..count.min(64) {
            let Ok(device) = (unsafe { collection.Item(i) }) else {
                continue;
            };
            let Ok(id) = device_id(&device) else {
                continue;
            };
            devices.push(AudioDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id: device_key(&id),
                name: friendly_name(&device),
            });
        }
        let Some(device) = default else {
            return Ok((None, devices));
        };
        self.bind(&device)?;
        Ok((
            Some(DefaultDevice {
                id: default_id.unwrap_or_default(),
                name: friendly_name(&device),
            }),
            devices,
        ))
    }

    fn read_volume(&mut self) -> Result<Option<(u8, bool)>, SystemError> {
        let Some(bound) = self.bound.as_ref() else {
            return Ok(None);
        };
        // SAFETY: COM calls on an endpoint owned by this thread.
        unsafe {
            let scalar = bound
                .endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|e| platform("GetMasterVolumeLevelScalar", &e))?;
            let muted = bound
                .endpoint
                .GetMute()
                .map_err(|e| platform("GetMute", &e))?;
            Ok(Some((scalar_to_percent(scalar), muted.as_bool())))
        }
    }

    fn set_volume(&mut self, percent: u8) -> Result<(), SystemError> {
        let endpoint = self.endpoint()?;
        // SAFETY: documented call with a 0.0–1.0 scalar and a context GUID
        // that outlives the call.
        unsafe { endpoint.SetMasterVolumeLevelScalar(percent_to_scalar(percent), &EVENT_CONTEXT) }
            .map_err(|e| platform("SetMasterVolumeLevelScalar", &e))
    }

    fn set_muted(&mut self, muted: bool) -> Result<(), SystemError> {
        let endpoint = self.endpoint()?;
        // SAFETY: as above.
        unsafe { endpoint.SetMute(muted, &EVENT_CONTEXT) }.map_err(|e| platform("SetMute", &e))
    }

    fn shutdown(&mut self) {
        self.unbind();
        if let Some(client) = self.client.take() {
            // SAFETY: unregisters the client registered in `connect`.
            let _ = unsafe {
                self.enumerator
                    .UnregisterEndpointNotificationCallback(&client)
            };
        }
    }
}

impl Drop for CoreAudio {
    fn drop(&mut self) {
        self.shutdown();
    }
}
