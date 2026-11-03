//! Battery (UPower) and network (NetworkManager), read over the system bus.
//! Both are optional: without the service, its part of the status stays `None`.

use crate::status::Battery;
use gtk::{gio, glib, prelude::*};
use std::cell::RefCell;

#[derive(Debug)]
pub enum Update {
    Battery(Option<Battery>),
    Network { online: Option<bool>, name: Option<String> },
}

thread_local! {
    /// Proxies must outlive their signal handlers; the shell keeps them for its lifetime.
    static KEEP: RefCell<Vec<gio::DBusProxy>> = const { RefCell::new(Vec::new()) };
}

async fn proxy(name: &str, path: &str, interface: &str) -> Option<gio::DBusProxy> {
    let proxy = gio::DBusProxy::for_bus_future(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None,
        name,
        path,
        interface,
    )
    .await
    .ok()?;
    // A proxy is created even when nobody owns the name; treat that as "no service".
    proxy.name_owner()?;
    Some(proxy)
}

fn prop<T: FromVariant>(proxy: &gio::DBusProxy, name: &str) -> Option<T> {
    proxy.cached_property(name)?.get::<T>()
}

fn read_battery(p: &gio::DBusProxy) -> Option<Battery> {
    if !prop::<bool>(p, "IsPresent").unwrap_or(false) {
        return None;
    }
    // UPower states: 1 charging, 2 discharging, 4 fully charged, 5 pending charge.
    let state = prop::<u32>(p, "State").unwrap_or(0);
    Some(Battery {
        percent: prop::<f64>(p, "Percentage").unwrap_or(0.0),
        charging: matches!(state, 1 | 5),
        full: state == 4,
        seconds_left: prop::<i64>(p, "TimeToEmpty").filter(|s| *s > 0),
    })
}

/// NetworkManager states: 10 asleep, 20 disconnected, 30 disconnecting, 40 connecting,
/// 50 local only, 60 site, 70 global.
fn is_online(state: u32) -> Option<bool> {
    match state {
        0 => None,
        10 | 20 => Some(false),
        _ => Some(true),
    }
}

async fn connection_name(path: &str) -> Option<String> {
    let active = proxy(
        "org.freedesktop.NetworkManager",
        path,
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .await?;
    prop::<String>(&active, "Id")
}

async fn read_network(nm: &gio::DBusProxy) -> Update {
    let online = prop::<u32>(nm, "State").and_then(is_online);
    let primary = nm
        .cached_property("PrimaryConnection")
        .and_then(|v| v.get::<glib::variant::ObjectPath>())
        .map(|p| p.as_str().to_owned())
        .filter(|p| p != "/");
    let name = match (online, primary) {
        (Some(true), Some(path)) => connection_name(&path).await,
        _ => None,
    };
    Update::Network { online, name }
}

pub fn watch(on_update: impl Fn(Update) + Clone + 'static) {
    let send = on_update.clone();
    glib::spawn_future_local(async move {
        let Some(upower) = proxy(
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower/devices/DisplayDevice",
            "org.freedesktop.UPower.Device",
        )
        .await
        else {
            return;
        };
        send(Update::Battery(read_battery(&upower)));
        let on_change = send.clone();
        // connect_local: the handler stays on the main thread, so it needn't be Send.
        upower.connect_local("g-properties-changed", false, move |args| {
            if let Some(p) = args.first().and_then(|v| v.get::<gio::DBusProxy>().ok()) {
                on_change(Update::Battery(read_battery(&p)));
            }
            None
        });
        KEEP.with(|k| k.borrow_mut().push(upower));
    });

    let send = on_update;
    glib::spawn_future_local(async move {
        let Some(nm) = proxy(
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        else {
            return;
        };
        send(read_network(&nm).await);
        let on_change = send.clone();
        nm.connect_local("g-properties-changed", false, move |args| {
            // NetworkManager announces many property changes; only two matter here.
            let relevant = args.get(1).and_then(|v| v.get::<glib::Variant>().ok()).is_some_and(|changed| {
                let changed = glib::VariantDict::new(Some(&changed));
                ["State", "PrimaryConnection"].iter().any(|key| changed.contains(key))
            });
            if !relevant {
                return None;
            }
            if let Some(p) = args.first().and_then(|v| v.get::<gio::DBusProxy>().ok()) {
                let on_change = on_change.clone();
                glib::spawn_future_local(async move { on_change(read_network(&p).await) });
            }
            None
        });
        KEEP.with(|k| k.borrow_mut().push(nm));
    });
}

#[cfg(test)]
mod tests {
    use super::is_online;

    #[test]
    fn network_states() {
        assert_eq!(is_online(20), Some(false));
        assert_eq!(is_online(40), Some(true));
        assert_eq!(is_online(70), Some(true));
        assert_eq!(is_online(0), None);
    }
}
