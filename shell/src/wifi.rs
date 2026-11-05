//! Wi‑Fi through NetworkManager on the system bus: the radio, the networks in
//! range, the saved ones, connecting and disconnecting. Without NetworkManager
//! or a Wi‑Fi device, everything here reads as `None` and the bar leaves it out.

use crate::bus;
use gtk::{gio, glib, prelude::*};
use std::{collections::HashMap, time::Duration};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";

const DEVICE_TYPE_WIFI: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Open,
    /// WPA/WPA2 personal: a password.
    Psk,
    /// WPA3 personal: a password.
    Sae,
    /// Enterprise or WEP: set up in the network settings, not here.
    Other,
}

impl Security {
    pub fn needs_password(self) -> bool {
        matches!(self, Security::Psk | Security::Sae)
    }
}

/// From an access point's Flags, WpaFlags and RsnFlags.
pub fn security(flags: u32, wpa: u32, rsn: u32) -> Security {
    const PRIVACY: u32 = 0x1;
    const PSK: u32 = 0x100;
    const EAP: u32 = 0x200;
    const SAE: u32 = 0x400;
    const EAP_SUITE_B: u32 = 0x2000;
    let both = wpa | rsn;
    if both & (EAP | EAP_SUITE_B) != 0 {
        Security::Other
    } else if both & PSK != 0 {
        Security::Psk
    } else if rsn & SAE != 0 {
        Security::Sae
    } else if both == 0 && flags & PRIVACY != 0 {
        Security::Other // WEP
    } else {
        Security::Open
    }
}

/// One network by name, however many access points carry it.
#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    pub ssid: String,
    pub ssid_bytes: Vec<u8>,
    /// 0–100, of the strongest access point.
    pub strength: u8,
    pub security: Security,
    /// That access point's object path.
    pub ap: String,
    /// The saved connection for this network, if there is one.
    pub saved: Option<String>,
    pub connected: bool,
}

/// What the bar needs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub enabled: bool,
    pub ssid: Option<String>,
    pub strength: u8,
}

/// Bars to draw for a signal strength: 1 to 3.
pub fn level(strength: u8) -> u8 {
    match strength {
        70.. => 3,
        40.. => 2,
        _ => 1,
    }
}

/// An access point as read from NetworkManager.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub path: String,
    pub ssid: Vec<u8>,
    pub strength: u8,
    pub security: Security,
}

/// Networks to list: hidden ones left out, one row per name, the connected one
/// first, then saved ones, then by strength.
pub fn networks(seen: &[Seen], active_ap: Option<&str>, saved: &HashMap<Vec<u8>, String>) -> Vec<Network> {
    let mut by_name: Vec<Network> = Vec::new();
    for ap in seen.iter().filter(|ap| !ap.ssid.is_empty()) {
        let connected = active_ap == Some(ap.path.as_str());
        match by_name.iter_mut().find(|n| n.ssid_bytes == ap.ssid) {
            Some(n) => {
                if connected || (!n.connected && ap.strength > n.strength) {
                    n.strength = ap.strength;
                    n.ap = ap.path.clone();
                }
                n.connected |= connected;
            }
            None => by_name.push(Network {
                ssid: String::from_utf8_lossy(&ap.ssid).into_owned(),
                ssid_bytes: ap.ssid.clone(),
                strength: ap.strength,
                security: ap.security,
                ap: ap.path.clone(),
                saved: saved.get(&ap.ssid).cloned(),
                connected,
            }),
        }
    }
    by_name.sort_by(|a, b| {
        (b.connected, b.saved.is_some(), b.strength).cmp(&(a.connected, a.saved.is_some(), a.strength))
    });
    by_name
}

// ── Reading ─────────────────────────────────────────────────────────────

async fn get(c: &gio::DBusConnection, path: &str, iface: &str, name: &str) -> Option<glib::Variant> {
    bus::get(c, NM, path, iface, name).await
}

fn path_of(v: &glib::Variant) -> Option<String> {
    v.get::<glib::variant::ObjectPath>().map(|p| p.as_str().to_owned()).filter(|p| p != "/")
}

/// The first Wi‑Fi device, if NetworkManager runs and there is one.
pub async fn device(c: &gio::DBusConnection) -> Option<String> {
    let reply = bus::call(c, NM, NM_PATH, NM, "GetDevices", None, 2_000).await.ok()?;
    let paths = reply.child_value(0).get::<Vec<glib::variant::ObjectPath>>()?;
    for path in paths {
        if get(c, path.as_str(), DEVICE, "DeviceType").await.and_then(|v| v.get::<u32>()) == Some(DEVICE_TYPE_WIFI) {
            return Some(path.as_str().to_owned());
        }
    }
    None
}

pub async fn summary(c: &gio::DBusConnection, device: &str) -> Summary {
    let enabled = get(c, NM_PATH, NM, "WirelessEnabled").await.and_then(|v| v.get::<bool>()).unwrap_or(false);
    let active = get(c, device, WIRELESS, "ActiveAccessPoint").await.as_ref().and_then(path_of);
    let Some(ap) = active.filter(|_| enabled) else {
        return Summary { enabled, ..Summary::default() };
    };
    let props = bus::get_all(c, NM, &ap, AP).await;
    let ssid = props.as_ref().and_then(|p| bus::lookup::<Vec<u8>>(p, "Ssid"));
    let strength = props.as_ref().and_then(|p| bus::lookup::<u8>(p, "Strength")).unwrap_or(0);
    Summary { enabled, ssid: ssid.map(|s| String::from_utf8_lossy(&s).into_owned()), strength }
}

/// Saved Wi‑Fi connections: network name → connection path.
async fn saved(c: &gio::DBusConnection) -> HashMap<Vec<u8>, String> {
    let mut out = HashMap::new();
    let Ok(reply) = bus::call(c, NM, SETTINGS_PATH, SETTINGS, "ListConnections", None, 2_000).await else { return out };
    let Some(paths) = reply.child_value(0).get::<Vec<glib::variant::ObjectPath>>() else { return out };
    for path in paths {
        let Ok(settings) = bus::call(c, NM, path.as_str(), CONNECTION, "GetSettings", None, 2_000).await else { continue };
        // a{sa{sv}}: settings by section; only the Wi‑Fi section matters here.
        let Some(wireless) = bus::entry(&settings.child_value(0), "802-11-wireless") else { continue };
        let wireless = glib::VariantDict::new(Some(&wireless));
        if let Some(ssid) = bus::lookup::<Vec<u8>>(&wireless, "ssid") {
            out.insert(ssid, path.as_str().to_owned());
        }
    }
    out
}

pub async fn list(c: &gio::DBusConnection, device: &str) -> Vec<Network> {
    let Ok(reply) = bus::call(c, NM, device, WIRELESS, "GetAllAccessPoints", None, 2_000).await else { return Vec::new() };
    let paths = reply.child_value(0).get::<Vec<glib::variant::ObjectPath>>().unwrap_or_default();
    let mut seen = Vec::new();
    for path in paths {
        let Some(p) = bus::get_all(c, NM, path.as_str(), AP).await else { continue };
        let flag = |name| bus::lookup::<u32>(&p, name).unwrap_or(0);
        seen.push(Seen {
            path: path.as_str().to_owned(),
            ssid: bus::lookup::<Vec<u8>>(&p, "Ssid").unwrap_or_default(),
            strength: bus::lookup::<u8>(&p, "Strength").unwrap_or(0),
            security: security(flag("Flags"), flag("WpaFlags"), flag("RsnFlags")),
        });
    }
    let active = get(c, device, WIRELESS, "ActiveAccessPoint").await.as_ref().and_then(path_of);
    networks(&seen, active.as_deref(), &saved(c).await)
}

// ── Changing ────────────────────────────────────────────────────────────

pub async fn scan(c: &gio::DBusConnection, device: &str) {
    let options: HashMap<String, glib::Variant> = HashMap::new();
    let _ = bus::call(c, NM, device, WIRELESS, "RequestScan", Some((options,).to_variant()), 5_000).await;
}

pub async fn set_enabled(c: &gio::DBusConnection, on: bool) -> Result<(), String> {
    bus::set(c, NM, NM_PATH, NM, "WirelessEnabled", on.to_variant()).await.map_err(|e| bus::message(&e))
}

pub async fn disconnect(c: &gio::DBusConnection, device: &str) {
    let _ = bus::call(c, NM, device, DEVICE, "Disconnect", None, 10_000).await;
}

/// Connect, and wait until the connection is up or has failed. A network that
/// was new is not kept when it fails, so a mistyped password is not saved.
pub async fn connect(c: &gio::DBusConnection, device: &str, network: &Network, password: Option<&str>) -> Result<(), String> {
    let dev = glib::variant::ObjectPath::try_from(device.to_owned()).map_err(|_| "bad device")?;
    let ap = glib::variant::ObjectPath::try_from(network.ap.clone()).map_err(|_| "bad network")?;
    let (created, active) = match &network.saved {
        Some(saved) => {
            let conn = glib::variant::ObjectPath::try_from(saved.clone()).map_err(|_| "bad connection")?;
            let reply = bus::call(c, NM, NM_PATH, NM, "ActivateConnection", Some((conn, dev, ap).to_variant()), 30_000)
                .await
                .map_err(|e| bus::message(&e))?;
            (None, reply.child_value(0))
        }
        None => {
            let mut settings: HashMap<String, HashMap<String, glib::Variant>> = HashMap::new();
            settings.insert("802-11-wireless".into(), HashMap::from([("ssid".into(), network.ssid_bytes.to_variant())]));
            let key_mgmt = match network.security {
                Security::Psk => Some("wpa-psk"),
                Security::Sae => Some("sae"),
                _ => None,
            };
            if let (Some(key_mgmt), Some(password)) = (key_mgmt, password) {
                settings.insert(
                    "802-11-wireless-security".into(),
                    HashMap::from([("key-mgmt".into(), key_mgmt.to_variant()), ("psk".into(), password.to_variant())]),
                );
            }
            let reply = bus::call(c, NM, NM_PATH, NM, "AddAndActivateConnection", Some((settings, dev, ap).to_variant()), 30_000)
                .await
                .map_err(|e| bus::message(&e))?;
            (path_of(&reply.child_value(0)), reply.child_value(1))
        }
    };
    let active = path_of(&active).ok_or("no connection")?;
    // Active connection states: 1 activating, 2 activated, 3 deactivating, 4 deactivated.
    for _ in 0..90 {
        match get(c, &active, ACTIVE, "State").await.and_then(|v| v.get::<u32>()) {
            Some(2) => return Ok(()),
            Some(1) => glib::timeout_future(Duration::from_millis(500)).await,
            _ => break,
        }
    }
    if let Some(created) = created {
        let _ = bus::call(c, NM, &created, CONNECTION, "Delete", None, 5_000).await;
    }
    Err(if network.security.needs_password() && password.is_some() { "Wrong password?" } else { "Couldn’t connect" }.into())
}

/// Call `on_change` (coalesced) whenever NetworkManager announces something.
pub fn watch(c: &gio::DBusConnection, on_change: impl Fn() + 'static) -> gio::SignalSubscription {
    let changed = bus::coalesce(300, on_change);
    c.subscribe_to_signal(Some(NM), None, None, None, None, gio::DBusSignalFlags::NONE, move |signal| {
        if matches!(signal.signal_name, "PropertiesChanged" | "AccessPointAdded" | "AccessPointRemoved" | "StateChanged") {
            changed();
        }
    })
}

/// The editor to offer under "Network settings…", when installed.
pub fn settings_app() -> Option<&'static str> {
    ["nm-connection-editor"].into_iter().find(|p| bus::installed(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ap(path: &str, ssid: &str, strength: u8, security: Security) -> Seen {
        Seen { path: path.into(), ssid: ssid.as_bytes().to_vec(), strength, security }
    }

    #[test]
    fn security_from_flags() {
        assert_eq!(security(0, 0, 0), Security::Open);
        assert_eq!(security(1, 0, 0x188), Security::Psk);
        assert_eq!(security(1, 0, 0x400), Security::Sae);
        assert_eq!(security(1, 0, 0x588), Security::Psk); // WPA2/WPA3 transition: a password either way
        assert_eq!(security(1, 0, 0x200), Security::Other);
        assert_eq!(security(1, 0, 0), Security::Other);
    }

    #[test]
    fn one_row_per_network_connected_then_saved_then_strongest() {
        let seen = [
            ap("/ap/1", "Harbour", 80, Security::Open),
            ap("/ap/2", "Home", 40, Security::Psk),
            ap("/ap/3", "Home", 65, Security::Psk),
            ap("/ap/4", "", 90, Security::Open),
            ap("/ap/5", "Library", 50, Security::Psk),
        ];
        let saved = HashMap::from([(b"Library".to_vec(), "/conn/7".to_owned())]);
        let list = networks(&seen, Some("/ap/2"), &saved);
        let names: Vec<&str> = list.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, ["Home", "Library", "Harbour"]);
        // The connected access point is the one shown, even if another is stronger.
        assert_eq!((list[0].ap.as_str(), list[0].strength, list[0].connected), ("/ap/2", 40, true));
        assert_eq!(list[1].saved.as_deref(), Some("/conn/7"));
    }

    #[test]
    fn signal_levels() {
        assert_eq!([level(10), level(45), level(90)], [1, 2, 3]);
    }
}
