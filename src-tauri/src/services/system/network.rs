//! Network connectivity: mapping Windows' connectivity model and debounced
//! "lost"/"restored" alerts.
//!
//! Connectivity comes from `ConnectionProfile::GetNetworkConnectivityLevel`,
//! i.e. Windows' own Network Connectivity Status Indicator. An enabled or
//! associated adapter is *not* treated as internet access: only
//! `InternetAccess` is.

// Parts of this module are only reached from the Windows backend.
#![cfg_attr(not(windows), allow(dead_code))]

use super::models::{
    Availability, ConnectionKind, Connectivity, NetworkConnection, NetworkState, SystemAlert,
};

/// `NetworkConnectivityLevel` → [`Connectivity`].
pub fn map_level(level: i32) -> Connectivity {
    match level {
        3 => Connectivity::Internet,
        2 => Connectivity::Constrained,
        1 => Connectivity::Local,
        _ => Connectivity::None,
    }
}

/// IANA `ifType` values (RFC 1213 / IANAifType-MIB).
const IF_ETHERNET: u32 = 6;
const IF_WIFI: u32 = 71;
const IF_WWANPP: u32 = 243;
const IF_WWANPP2: u32 = 244;

/// Connection type from the profile's flags and its adapter's IANA type.
pub fn connection_kind(iana_type: Option<u32>, is_wlan: bool, is_wwan: bool) -> ConnectionKind {
    match (is_wlan, is_wwan, iana_type) {
        (true, _, _) | (_, _, Some(IF_WIFI)) => ConnectionKind::Wifi,
        (_, true, _) | (_, _, Some(IF_WWANPP | IF_WWANPP2)) => ConnectionKind::Cellular,
        (_, _, Some(IF_ETHERNET)) => ConnectionKind::Ethernet,
        _ => ConnectionKind::Other,
    }
}

/// Profile names are user/network supplied: printable, bounded, or absent.
pub fn clean_profile_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect::<String>()
        .trim()
        .to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Builds the normalized state. `connection` is only kept while there is
/// some connectivity.
pub fn state(connectivity: Connectivity, connection: Option<NetworkConnection>) -> NetworkState {
    NetworkState {
        status: Availability::Ready,
        connectivity,
        connection: if connectivity == Connectivity::None {
            None
        } else {
            connection
        },
    }
}

fn is_online(state: &NetworkState) -> bool {
    matches!(
        state.connectivity,
        Connectivity::Internet | Connectivity::Constrained
    )
}

/// How long connectivity must stay lost before "Network disconnected" is
/// shown. Wi-Fi roaming and adapter resets often drop for a second or two.
pub const LOSS_CONFIRM_MS: u64 = 3_000;

/// Debounced connectivity alerts.
#[derive(Debug, Default)]
pub struct NetworkAlerts {
    online: Option<bool>,
    /// Lost at this time; announced only if still lost at the deadline.
    lost_since: Option<u64>,
}

impl NetworkAlerts {
    /// Feeds a new state read at `now_ms`.
    pub fn observe(&mut self, next: &NetworkState, now_ms: u64) -> Option<SystemAlert> {
        if next.status != Availability::Ready {
            return None;
        }
        let online = is_online(next);
        let previous = self.online.replace(online);
        match (previous, online) {
            (Some(true), false) => {
                self.lost_since = Some(now_ms);
                None
            }
            (Some(false), true) => {
                if self.lost_since.take().is_some() {
                    // A short blip: nothing was announced, nothing to restore.
                    None
                } else {
                    next.connection
                        .as_ref()
                        .map(|c| SystemAlert::NetworkRestored {
                            kind: c.kind,
                            name: c.name.clone(),
                        })
                }
            }
            _ => None,
        }
    }

    /// When [`Self::poll`] should run next, if a loss awaits confirmation.
    pub fn deadline(&self) -> Option<u64> {
        self.lost_since.map(|t| t + LOSS_CONFIRM_MS)
    }

    /// Announces a loss that lasted [`LOSS_CONFIRM_MS`]. Call with a freshly
    /// observed state.
    pub fn poll(&mut self, now_ms: u64) -> Option<SystemAlert> {
        let since = self.lost_since?;
        if now_ms < since + LOSS_CONFIRM_MS {
            return None;
        }
        self.lost_since = None;
        (self.online == Some(false)).then_some(SystemAlert::NetworkLost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wifi(level: i32) -> NetworkState {
        state(
            map_level(level),
            Some(NetworkConnection {
                kind: ConnectionKind::Wifi,
                name: Some("Home".into()),
            }),
        )
    }

    #[test]
    fn maps_connectivity_without_assuming_internet() {
        assert_eq!(map_level(3), Connectivity::Internet);
        assert_eq!(map_level(2), Connectivity::Constrained);
        assert_eq!(map_level(1), Connectivity::Local);
        assert_eq!(map_level(0), Connectivity::None);
        assert_eq!(map_level(42), Connectivity::None);
        // Associated to Wi-Fi without internet is "local", not online.
        assert_eq!(wifi(1).connectivity, Connectivity::Local);
        assert!(wifi(0).connection.is_none());
    }

    #[test]
    fn classifies_connection_kinds() {
        assert_eq!(
            connection_kind(Some(71), false, false),
            ConnectionKind::Wifi
        );
        assert_eq!(connection_kind(None, true, false), ConnectionKind::Wifi);
        assert_eq!(
            connection_kind(Some(6), false, false),
            ConnectionKind::Ethernet
        );
        assert_eq!(
            connection_kind(Some(243), false, false),
            ConnectionKind::Cellular
        );
        assert_eq!(
            connection_kind(Some(6), false, true),
            ConnectionKind::Cellular
        );
        assert_eq!(
            connection_kind(Some(53), false, false),
            ConnectionKind::Other
        );
        assert_eq!(connection_kind(None, false, false), ConnectionKind::Other);
    }

    #[test]
    fn cleans_profile_names() {
        assert_eq!(clean_profile_name(" Café\u{7} "), Some("Café".into()));
        assert_eq!(clean_profile_name("   "), None);
        assert_eq!(clean_profile_name(&"n".repeat(200)).unwrap().len(), 64);
    }

    #[test]
    fn short_drops_are_not_announced() {
        let mut a = NetworkAlerts::default();
        assert_eq!(a.observe(&wifi(3), 0), None);
        assert_eq!(a.observe(&wifi(0), 1_000), None);
        assert_eq!(a.deadline(), Some(4_000));
        assert_eq!(a.observe(&wifi(3), 2_000), None);
        assert_eq!(a.deadline(), None);
        assert_eq!(a.poll(10_000), None);
    }

    #[test]
    fn confirmed_loss_and_restore_are_announced_once() {
        let mut a = NetworkAlerts::default();
        a.observe(&wifi(3), 0);
        a.observe(&wifi(1), 1_000);
        assert_eq!(a.poll(3_999), None);
        assert_eq!(a.poll(4_000), Some(SystemAlert::NetworkLost));
        assert_eq!(a.poll(9_000), None);
        assert_eq!(a.observe(&wifi(1), 9_500), None);
        assert_eq!(
            a.observe(&wifi(3), 10_000),
            Some(SystemAlert::NetworkRestored {
                kind: ConnectionKind::Wifi,
                name: Some("Home".into())
            })
        );
        assert_eq!(a.observe(&wifi(3), 11_000), None);
    }

    #[test]
    fn startup_state_is_quiet_and_offline_start_announces_connection() {
        let mut a = NetworkAlerts::default();
        assert_eq!(a.observe(&wifi(0), 0), None);
        assert!(a.observe(&wifi(3), 500).is_some());
        let mut b = NetworkAlerts::default();
        assert_eq!(
            b.observe(&NetworkState::with_status(Availability::Unavailable), 0),
            None
        );
        assert_eq!(b.observe(&wifi(3), 0), None);
    }
}
