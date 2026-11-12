//! Brightness on Windows.
//!
//! - Built-in panels: WMI namespace `root\WMI`, classes `WmiMonitorBrightness`
//!   (`CurrentBrightness`, 0–100) and `WmiMonitorBrightnessMethods`
//!   (`WmiSetBrightness(Timeout, Brightness)`). Desktops without a built-in
//!   panel have no instances (or report `WBEM_E_NOT_SUPPORTED`).
//! - External monitors: Monitor Configuration API (DDC/CI) through
//!   `GetPhysicalMonitorsFromHMONITOR`, `GetMonitorBrightness`,
//!   `SetMonitorBrightness`. A monitor without DDC/CI simply fails
//!   `GetMonitorBrightness` and is counted as unsupported.
//!
//! No driver is installed and no administrator rights are required. WMI is
//! connected with a per-proxy security blanket (`CoSetProxyBlanket`), because
//! process-wide `CoInitializeSecurity` belongs to the host (WebView2).

use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Devices::Display::{
            DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
            GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
        },
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
        System::{
            Com::{
                CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE,
                RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Variant::VARIANT,
            Wmi::{
                IWbemClassObject, IWbemContext, IWbemLocator, IWbemServices, WbemLocator,
                WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
                WBEM_FLAG_RETURN_WBEM_COMPLETE,
            },
        },
    },
};

use super::{
    super::{
        display::{ddc_to_percent, percent_to_ddc, unsupported_count, DisplayBackend, DisplayId},
        models::{Availability, DisplayInfo, DisplayKind, DisplayState},
        SystemError,
    },
    platform,
};

/// `RPC_C_AUTHN_WINNT` / `RPC_C_AUTHZ_NONE` (Win32 RPC constants).
const RPC_C_AUTHN_WINNT: u32 = 10;
const RPC_C_AUTHZ_NONE: u32 = 0;
/// `IEnumWbemClassObject::Next` timeout (ms).
const WMI_NEXT_TIMEOUT_MS: i32 = 2_000;
const MAX_DISPLAYS: usize = 16;

struct Internal {
    /// `__PATH` of the matching `WmiMonitorBrightnessMethods` instance.
    path: BSTR,
    instance: String,
    brightness: Option<u8>,
}

struct External {
    monitor: PHYSICAL_MONITOR,
    name: String,
    range: (u32, u32),
    brightness: Option<u8>,
}

pub struct WinDisplay {
    wmi: Option<IWbemServices>,
    internal: Vec<Internal>,
    external: Vec<External>,
    /// Every physical monitor handle obtained, destroyed on re-enumeration.
    physical: Vec<PHYSICAL_MONITOR>,
    total: u32,
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> windows::core::BOOL {
    // SAFETY: `data` is the `Vec<HMONITOR>` passed by `monitors` below, which
    // outlives the synchronous EnumDisplayMonitors call.
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    if list.len() < MAX_DISPLAYS {
        list.push(monitor);
    }
    true.into()
}

fn monitors() -> Vec<HMONITOR> {
    let mut list: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only appends to `list` during this call.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut list as *mut Vec<HMONITOR> as isize),
        );
    }
    list
}

fn description(monitor: &PHYSICAL_MONITOR) -> String {
    // PHYSICAL_MONITOR is packed: copy the array out instead of borrowing it.
    let raw = monitor.szPhysicalMonitorDescription;
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    let name = String::from_utf16_lossy(&raw[..len]).trim().to_owned();
    if name.is_empty() || name.eq_ignore_ascii_case("Generic PnP Monitor") {
        "External display".into()
    } else {
        name.chars().take(64).collect()
    }
}

fn ddc_brightness(monitor: &PHYSICAL_MONITOR) -> Option<(u32, u32, u32)> {
    let (mut min, mut current, mut max) = (0u32, 0u32, 0u32);
    // SAFETY: a valid physical monitor handle and caller-owned outputs.
    let handle = monitor.hPhysicalMonitor;
    let ok = unsafe { GetMonitorBrightness(handle, &mut min, &mut current, &mut max) };
    (ok != 0 && max > min).then_some((min, current, max))
}

fn variant_u8(object: &IWbemClassObject, name: &str) -> Option<u8> {
    let mut value = VARIANT::default();
    let name = windows::core::HSTRING::from(name);
    // SAFETY: reads a property into a caller-owned VARIANT.
    unsafe { object.Get(PCWSTR(name.as_ptr()), 0, &mut value, None, None) }.ok()?;
    u32::try_from(&value)
        .ok()
        .and_then(|v| u8::try_from(v.min(100)).ok())
}

fn variant_string(object: &IWbemClassObject, name: &str) -> Option<BSTR> {
    let mut value = VARIANT::default();
    let name = windows::core::HSTRING::from(name);
    // SAFETY: as above.
    unsafe { object.Get(PCWSTR(name.as_ptr()), 0, &mut value, None, None) }.ok()?;
    BSTR::try_from(&value).ok()
}

impl WinDisplay {
    /// Never fails: each interface is detected lazily in `enumerate`.
    pub fn connect() -> Result<Self, SystemError> {
        Ok(Self {
            wmi: None,
            internal: Vec::new(),
            external: Vec::new(),
            physical: Vec::new(),
            total: 0,
        })
    }

    fn wmi(&mut self) -> Option<&IWbemServices> {
        if self.wmi.is_none() {
            // SAFETY: standard WMI connection on this MTA thread.
            let services = unsafe {
                let locator: IWbemLocator =
                    CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
                let services = locator
                    .ConnectServer(
                        &BSTR::from("ROOT\\WMI"),
                        &BSTR::new(),
                        &BSTR::new(),
                        &BSTR::new(),
                        0,
                        &BSTR::new(),
                        None::<&IWbemContext>,
                    )
                    .ok()?;
                CoSetProxyBlanket(
                    &services,
                    RPC_C_AUTHN_WINNT,
                    RPC_C_AUTHZ_NONE,
                    PCWSTR::null(),
                    RPC_C_AUTHN_LEVEL_CALL,
                    RPC_C_IMP_LEVEL_IMPERSONATE,
                    None,
                    EOAC_NONE,
                )
                .ok()?;
                services
            };
            self.wmi = Some(services);
        }
        self.wmi.as_ref()
    }

    /// Runs a WQL query, returning at most [`MAX_DISPLAYS`] objects. An error
    /// (e.g. `WBEM_E_NOT_SUPPORTED` on desktops) yields no objects.
    fn query(&mut self, wql: &str) -> Vec<IWbemClassObject> {
        let Some(services) = self.wmi() else {
            return Vec::new();
        };
        // SAFETY: COM calls on objects owned by this thread.
        let Ok(results) = (unsafe {
            services.ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from(wql),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None::<&IWbemContext>,
            )
        }) else {
            return Vec::new();
        };
        let mut objects = Vec::new();
        while objects.len() < MAX_DISPLAYS {
            let mut row = [None];
            let mut returned = 0u32;
            // SAFETY: one output slot and a caller-owned counter.
            let hr = unsafe { results.Next(WMI_NEXT_TIMEOUT_MS, &mut row, &mut returned) };
            match row[0].take() {
                Some(object) if hr.is_ok() && returned == 1 => objects.push(object),
                _ => break,
            }
        }
        objects
    }

    fn read_internal(&mut self) -> Vec<(String, Option<u8>)> {
        self.query(
            "SELECT InstanceName, CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE",
        )
        .iter()
        .filter_map(|o| {
            let instance = variant_string(o, "InstanceName")?.to_string();
            Some((instance, variant_u8(o, "CurrentBrightness")))
        })
        .collect()
    }

    fn release_physical(&mut self) {
        self.external.clear();
        if !self.physical.is_empty() {
            // SAFETY: destroys handles obtained from GetPhysicalMonitorsFromHMONITOR.
            let _ = unsafe { DestroyPhysicalMonitors(&self.physical) };
            self.physical.clear();
        }
    }

    fn state(&self) -> DisplayState {
        let internal = self.internal.iter().enumerate().map(|(i, d)| DisplayInfo {
            id: DisplayId::Internal(i as u32).wire(),
            name: if self.internal.len() > 1 {
                format!("Built-in display {}", i + 1)
            } else {
                "Built-in display".into()
            },
            kind: DisplayKind::Internal,
            brightness: d.brightness,
        });
        let external = self.external.iter().enumerate().map(|(i, d)| DisplayInfo {
            id: DisplayId::External(i as u32).wire(),
            name: d.name.clone(),
            kind: DisplayKind::External,
            brightness: d.brightness,
        });
        DisplayState {
            status: Availability::Ready,
            displays: internal.chain(external).collect(),
            unsupported_count: unsupported_count(
                self.total,
                self.external.len() as u32,
                self.internal.len() as u32,
            ),
        }
    }
}

impl DisplayBackend for WinDisplay {
    fn enumerate(&mut self) -> Result<DisplayState, SystemError> {
        self.release_physical();
        let mut total = 0u32;
        for hmonitor in monitors() {
            let mut count = 0u32;
            // SAFETY: a monitor handle from EnumDisplayMonitors.
            if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count) }.is_err()
                || count == 0
            {
                total += 1;
                continue;
            }
            let mut list = vec![PHYSICAL_MONITOR::default(); count.min(4) as usize];
            // SAFETY: `list` has room for the reported count (capped).
            if unsafe { GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut list) }.is_err() {
                total += 1;
                continue;
            }
            total += list.len() as u32;
            for monitor in list {
                if let Some((min, current, max)) = ddc_brightness(&monitor) {
                    self.external.push(External {
                        monitor,
                        name: description(&monitor),
                        range: (min, max),
                        brightness: ddc_to_percent(min, current, max),
                    });
                }
                self.physical.push(monitor);
            }
        }
        self.total = total;

        let methods: Vec<(String, BSTR)> = self
            .query(
                "SELECT InstanceName, __PATH FROM WmiMonitorBrightnessMethods WHERE Active = TRUE",
            )
            .iter()
            .filter_map(|o| {
                Some((
                    variant_string(o, "InstanceName")?.to_string(),
                    variant_string(o, "__PATH")?,
                ))
            })
            .collect();
        self.internal = self
            .read_internal()
            .into_iter()
            .filter_map(|(instance, brightness)| {
                let path = methods.iter().find(|(i, _)| *i == instance)?.1.clone();
                Some(Internal {
                    path,
                    instance,
                    brightness,
                })
            })
            .collect();
        Ok(self.state())
    }

    fn refresh(&mut self) -> Result<DisplayState, SystemError> {
        for d in &mut self.external {
            d.brightness = ddc_brightness(&d.monitor)
                .and_then(|(min, current, max)| ddc_to_percent(min, current, max));
        }
        if !self.internal.is_empty() {
            let current = self.read_internal();
            for d in &mut self.internal {
                d.brightness = current
                    .iter()
                    .find(|(i, _)| *i == d.instance)
                    .and_then(|(_, b)| *b);
            }
        }
        Ok(self.state())
    }

    fn set_brightness(&mut self, id: DisplayId, percent: u8) -> Result<(), SystemError> {
        match id {
            DisplayId::External(n) => {
                let d = self.external.get(n as usize).ok_or(SystemError::NotFound)?;
                let (min, max) = d.range;
                // SAFETY: a live physical monitor handle owned by `self`.
                let handle = d.monitor.hPhysicalMonitor;
                let ok = unsafe { SetMonitorBrightness(handle, percent_to_ddc(percent, min, max)) };
                if ok == 0 {
                    return Err(SystemError::Platform(
                        "SetMonitorBrightness failed (DDC/CI)".into(),
                    ));
                }
                Ok(())
            }
            DisplayId::Internal(n) => {
                let path = self
                    .internal
                    .get(n as usize)
                    .ok_or(SystemError::NotFound)?
                    .path
                    .clone();
                let services = self.wmi().ok_or(SystemError::Unavailable)?.clone();
                // SAFETY: builds the documented in-parameters and invokes
                // WmiSetBrightness on the instance path read in `enumerate`.
                unsafe {
                    let mut class = None;
                    services
                        .GetObject(
                            &BSTR::from("WmiMonitorBrightnessMethods"),
                            WBEM_FLAG_RETURN_WBEM_COMPLETE,
                            None::<&IWbemContext>,
                            Some(&mut class),
                            None,
                        )
                        .map_err(|e| platform("GetObject", &e))?;
                    let class: IWbemClassObject = class.ok_or(SystemError::Unsupported)?;
                    let mut input = None;
                    let name = windows::core::HSTRING::from("WmiSetBrightness");
                    class
                        .GetMethod(PCWSTR(name.as_ptr()), 0, &mut input, std::ptr::null_mut())
                        .map_err(|e| platform("GetMethod", &e))?;
                    let input: IWbemClassObject = input
                        .ok_or(SystemError::Unsupported)?
                        .SpawnInstance(0)
                        .map_err(|e| platform("SpawnInstance", &e))?;
                    // CIM uint32 is passed as VT_I4, uint8 as VT_UI1.
                    let timeout = VARIANT::from(0i32);
                    let brightness = VARIANT::from(percent.min(100));
                    for (field, value) in [("Timeout", &timeout), ("Brightness", &brightness)] {
                        let field = windows::core::HSTRING::from(field);
                        input
                            .Put(PCWSTR(field.as_ptr()), 0, value, 0)
                            .map_err(|e| platform("Put", &e))?;
                    }
                    services
                        .ExecMethod(
                            &path,
                            &BSTR::from("WmiSetBrightness"),
                            WBEM_FLAG_RETURN_WBEM_COMPLETE,
                            None::<&IWbemContext>,
                            &input,
                            None,
                            None,
                        )
                        .map_err(|e| platform("WmiSetBrightness", &e))?;
                }
                Ok(())
            }
        }
    }
}

impl Drop for WinDisplay {
    fn drop(&mut self) {
        self.release_physical();
    }
}
