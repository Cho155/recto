//! Bluetooth through BlueZ on the system bus: the adapter's power, paired
//! devices, connecting, and finding and pairing new ones. Without BlueZ or an
//! adapter, everything reads as `None` and the bar leaves it out.
//!
//! No pairing agent is registered, so BlueZ pairs without a code ("just works"):
//! headphones, speakers and most mice. Devices that ask for a code, such as
//! keyboards, need a fuller tool.

use crate::bus;
use gtk::{gio, glib, prelude::*};

const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";
const OBJECTS: &str = "org.freedesktop.DBus.ObjectManager";

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub path: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
    /// Percent, for devices that report it.
    pub battery: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bluetooth {
    pub adapter: String,
    pub powered: bool,
    pub discovering: bool,
    /// Paired devices by name, then devices nearby that have a name.
    pub devices: Vec<Device>,
}

impl Bluetooth {
    pub fn connected(&self) -> impl Iterator<Item = &Device> {
        self.devices.iter().filter(|d| d.connected)
    }
}

/// Reads the reply to ObjectManager.GetManagedObjects: `(a{oa{sa{sv}}})`.
pub fn parse(reply: &glib::Variant) -> Option<Bluetooth> {
    let objects = reply.child_value(0);
    let mut adapter: Option<(String, bool, bool)> = None;
    let mut devices = Vec::new();
    for i in 0..objects.n_children() {
        let entry = objects.child_value(i);
        let path = entry.child_value(0).str()?.to_owned();
        let interfaces = entry.child_value(1);
        let mut device: Option<Device> = None;
        let mut battery = None;
        let mut device_adapter = None;
        for j in 0..interfaces.n_children() {
            let iface = interfaces.child_value(j);
            let props = glib::VariantDict::new(Some(&iface.child_value(1)));
            match iface.child_value(0).str() {
                Some(ADAPTER) if adapter.is_none() => {
                    adapter = Some((
                        path.clone(),
                        bus::lookup::<bool>(&props, "Powered").unwrap_or(false),
                        bus::lookup::<bool>(&props, "Discovering").unwrap_or(false),
                    ));
                }
                Some(DEVICE) => {
                    let paired = bus::lookup::<bool>(&props, "Paired").unwrap_or(false);
                    // A device nearby without a name is only an address: not worth a row.
                    let name = if paired {
                        bus::lookup::<String>(&props, "Alias").or_else(|| bus::lookup::<String>(&props, "Name"))
                    } else {
                        bus::lookup::<String>(&props, "Name")
                    };
                    device_adapter = bus::lookup::<glib::variant::ObjectPath>(&props, "Adapter").map(|p| p.as_str().to_owned());
                    device = name.map(|name| Device {
                        path: path.clone(),
                        name,
                        paired,
                        connected: bus::lookup::<bool>(&props, "Connected").unwrap_or(false),
                        battery: None,
                    });
                }
                Some(BATTERY) => battery = bus::lookup::<u8>(&props, "Percentage"),
                _ => {}
            }
        }
        if let Some(mut d) = device {
            d.battery = battery;
            devices.push((device_adapter, d));
        }
    }
    let (adapter, powered, discovering) = adapter?;
    let mut devices: Vec<Device> = devices
        .into_iter()
        .filter(|(owner, _)| owner.as_deref().is_none_or(|o| o == adapter))
        .map(|(_, d)| d)
        .collect();
    // Paired devices first, then by name only: a row must not move under the
    // pointer when its device connects (found in the test VM).
    devices.sort_by(|a, b| b.paired.cmp(&a.paired).then_with(|| a.name.cmp(&b.name)));
    Some(Bluetooth { adapter, powered, discovering, devices })
}

pub async fn read(c: &gio::DBusConnection) -> Option<Bluetooth> {
    let reply = bus::call(c, BLUEZ, "/", OBJECTS, "GetManagedObjects", None, 2_000).await.ok()?;
    parse(&reply)
}

pub async fn set_powered(c: &gio::DBusConnection, adapter: &str, on: bool) -> Result<(), String> {
    bus::set(c, BLUEZ, adapter, ADAPTER, "Powered", on.to_variant()).await.map_err(|e| bus::message(&e))
}

pub async fn set_connected(c: &gio::DBusConnection, device: &str, on: bool) -> Result<(), String> {
    let method = if on { "Connect" } else { "Disconnect" };
    bus::call(c, BLUEZ, device, DEVICE, method, None, 30_000).await.map(|_| ()).map_err(|e| bus::message(&e))
}

/// Pair, trust (so it may reconnect by itself later) and connect.
pub async fn pair(c: &gio::DBusConnection, device: &str) -> Result<(), String> {
    bus::call(c, BLUEZ, device, DEVICE, "Pair", None, 60_000).await.map_err(|e| bus::message(&e))?;
    let _ = bus::set(c, BLUEZ, device, DEVICE, "Trusted", true.to_variant()).await;
    set_connected(c, device, true).await
}

pub async fn set_discovering(c: &gio::DBusConnection, adapter: &str, on: bool) {
    let method = if on { "StartDiscovery" } else { "StopDiscovery" };
    let _ = bus::call(c, BLUEZ, adapter, ADAPTER, method, None, 5_000).await;
}

/// Call `on_change` (coalesced) whenever BlueZ announces something.
pub fn watch(c: &gio::DBusConnection, on_change: impl Fn() + 'static) -> gio::SignalSubscription {
    let changed = bus::coalesce(300, on_change);
    c.subscribe_to_signal(Some(BLUEZ), None, None, None, None, gio::DBusSignalFlags::NONE, move |_| changed())
}

/// The tool to offer under "Bluetooth settings…", when installed.
pub fn settings_app() -> Option<&'static str> {
    ["blueman-manager"].into_iter().find(|p| bus::installed(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objects(text: &str) -> glib::Variant {
        let inner = glib::Variant::parse(Some(glib::VariantTy::new("a{oa{sa{sv}}}").unwrap()), text).unwrap();
        glib::Variant::tuple_from_iter([inner])
    }

    #[test]
    fn adapter_and_devices() {
        let reply = objects(
            "{objectpath '/org/bluez/hci0': {'org.bluez.Adapter1': {'Powered': <true>, 'Discovering': <false>}},
              objectpath '/org/bluez/hci0/dev_1': {'org.bluez.Device1': {'Alias': <'WH-1000XM4'>, 'Paired': <true>,
                  'Connected': <true>, 'Adapter': <objectpath '/org/bluez/hci0'>},
                'org.bluez.Battery1': {'Percentage': <byte 80>}},
              objectpath '/org/bluez/hci0/dev_2': {'org.bluez.Device1': {'Alias': <'Desk Mouse'>, 'Paired': <true>,
                  'Connected': <false>, 'Adapter': <objectpath '/org/bluez/hci0'>}},
              objectpath '/org/bluez/hci0/dev_3': {'org.bluez.Device1': {'Alias': <'4C-87-5D-00-11-22'>, 'Paired': <false>,
                  'Adapter': <objectpath '/org/bluez/hci0'>}},
              objectpath '/org/bluez/hci0/dev_4': {'org.bluez.Device1': {'Name': <'Kitchen Speaker'>, 'Alias': <'Kitchen Speaker'>,
                  'Paired': <false>, 'Adapter': <objectpath '/org/bluez/hci0'>}}}",
        );
        let bt = parse(&reply).unwrap();
        assert!(bt.powered && !bt.discovering);
        let names: Vec<&str> = bt.devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Desk Mouse", "WH-1000XM4", "Kitchen Speaker"]);
        assert_eq!(bt.devices[1].battery, Some(80));
        assert_eq!(bt.connected().count(), 1);
    }

    #[test]
    fn no_adapter_is_none() {
        assert!(parse(&objects("{}")).is_none());
    }
}
