//! Small helpers for the system services the shell talks to (NetworkManager,
//! BlueZ): property reads and writes, calls, and coalescing bursts of signals.

use gtk::{gio, glib, prelude::*};
use std::{cell::Cell, rc::Rc, time::Duration};

const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

pub async fn system() -> Option<gio::DBusConnection> {
    gio::bus_get_future(gio::BusType::System).await.ok()
}

pub async fn call(
    c: &gio::DBusConnection,
    service: &str,
    path: &str,
    interface: &str,
    method: &str,
    args: Option<glib::Variant>,
    timeout_ms: i32,
) -> Result<glib::Variant, glib::Error> {
    c.call_future(Some(service), path, interface, method, args.as_ref(), None, gio::DBusCallFlags::NO_AUTO_START, timeout_ms)
        .await
}

pub async fn get(c: &gio::DBusConnection, service: &str, path: &str, interface: &str, name: &str) -> Option<glib::Variant> {
    let reply = call(c, service, path, PROPERTIES, "Get", Some((interface, name).to_variant()), 2_000).await.ok()?;
    reply.child_value(0).as_variant()
}

pub async fn get_all(c: &gio::DBusConnection, service: &str, path: &str, interface: &str) -> Option<glib::VariantDict> {
    let reply = call(c, service, path, PROPERTIES, "GetAll", Some((interface,).to_variant()), 2_000).await.ok()?;
    Some(glib::VariantDict::new(Some(&reply.child_value(0))))
}

pub async fn set(
    c: &gio::DBusConnection,
    service: &str,
    path: &str,
    interface: &str,
    name: &str,
    value: glib::Variant,
) -> Result<(), glib::Error> {
    call(c, service, path, PROPERTIES, "Set", Some((interface, name, value).to_variant()), 10_000).await.map(|_| ())
}

pub fn lookup<T: glib::variant::FromVariant>(dict: &glib::VariantDict, key: &str) -> Option<T> {
    dict.lookup::<T>(key).ok().flatten()
}

/// The value under `key` in a dictionary of any value type (`a{s…}`).
pub fn entry(dict: &glib::Variant, key: &str) -> Option<glib::Variant> {
    (0..dict.n_children()).map(|i| dict.child_value(i)).find(|e| e.child_value(0).str() == Some(key)).map(|e| e.child_value(1))
}

/// The text of a D-Bus error, without GIO's "GDBus.Error:org.foo.Error:" prefix.
pub fn message(error: &glib::Error) -> String {
    let text = error.message();
    match text.rsplit_once(": ") {
        Some((prefix, rest)) if prefix.starts_with("GDBus.Error") => rest.to_owned(),
        _ => text.to_owned(),
    }
}

/// Call `f` once, `ms` after the first of a burst of calls; services announce
/// a change as many signals in a row.
pub fn coalesce(ms: u64, f: impl Fn() + 'static) -> impl Fn() + Clone + 'static {
    let f = Rc::new(f);
    let pending = Rc::new(Cell::new(false));
    move || {
        if pending.replace(true) {
            return;
        }
        let (f, pending) = (f.clone(), pending.clone());
        glib::timeout_add_local_once(Duration::from_millis(ms), move || {
            pending.set(false);
            f();
        });
    }
}

/// Is `program` installed? For "… settings" links to a fuller tool.
pub fn installed(program: &str) -> bool {
    glib::find_program_in_path(program).is_some()
}
