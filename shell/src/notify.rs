//! The freedesktop notification service (org.freedesktop.Notifications).
//! The shell takes the name over from any running daemon, such as mako; when the
//! shell is not running, D-Bus activation can still start that daemon instead.

use gtk::{gio, glib, prelude::*};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

const XML: &str = r#"
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities"><arg direction="out" type="as"/></method>
    <method name="Notify">
      <arg direction="in" type="s"/><arg direction="in" type="u"/><arg direction="in" type="s"/>
      <arg direction="in" type="s"/><arg direction="in" type="s"/><arg direction="in" type="as"/>
      <arg direction="in" type="a{sv}"/><arg direction="in" type="i"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="CloseNotification"><arg direction="in" type="u"/></method>
    <method name="GetServerInformation">
      <arg direction="out" type="s"/><arg direction="out" type="s"/>
      <arg direction="out" type="s"/><arg direction="out" type="s"/>
    </method>
    <signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
    <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
  </interface>
</node>"#;

/// Why a notification closed, as the spec numbers them.
#[derive(Debug, Clone, Copy)]
pub enum Closed {
    Expired = 1,
    Dismissed = 2,
    ByApp = 3,
}

pub struct Incoming {
    pub app: String,
    pub replaces: u32,
    pub summary: String,
    pub body: String,
    pub actions: Vec<String>,
    pub urgency: Option<u8>,
    pub timeout: i32,
}

type Args = (String, u32, String, String, String, Vec<String>, HashMap<String, glib::Variant>, i32);

fn parse(params: &glib::Variant) -> Option<Incoming> {
    let (app, replaces, _icon, summary, body, actions, hints, timeout) = params.get::<Args>()?;
    let urgency = hints.get("urgency").and_then(|v| v.get::<u8>());
    Some(Incoming { app, replaces, summary, body, actions, urgency, timeout })
}

#[derive(Default)]
pub struct Server {
    connection: RefCell<Option<gio::DBusConnection>>,
}

impl Server {
    pub fn start(
        on_notify: impl Fn(Incoming) -> u32 + 'static,
        on_close: impl Fn(u32) + 'static,
    ) -> Rc<Server> {
        let server = Rc::new(Server::default());
        let on_notify = Rc::new(on_notify);
        let on_close = Rc::new(on_close);
        let weak = Rc::downgrade(&server);

        let _owner = gio::bus_own_name(
            gio::BusType::Session,
            NAME,
            gio::BusNameOwnerFlags::ALLOW_REPLACEMENT | gio::BusNameOwnerFlags::REPLACE,
            move |connection, _| {
                let info = gio::DBusNodeInfo::for_xml(XML)
                    .ok()
                    .and_then(|node| node.lookup_interface(NAME));
                let Some(info) = info else {
                    eprintln!("liubai-shell: notification interface description is invalid");
                    return;
                };
                let on_notify = on_notify.clone();
                let on_close = on_close.clone();
                let registered = connection
                    .register_object(PATH, &info)
                    .method_call(move |_, _, _, _, method, params, invocation| match method {
                        "GetCapabilities" => {
                            invocation.return_value(Some(&(vec!["body", "actions"],).to_variant()))
                        }
                        "GetServerInformation" => invocation.return_value(Some(
                            &("Liubai", "Liubai", env!("CARGO_PKG_VERSION"), "1.2").to_variant(),
                        )),
                        "Notify" => match parse(&params) {
                            Some(incoming) => {
                                let id = on_notify(incoming);
                                invocation.return_value(Some(&(id,).to_variant()));
                            }
                            None => invocation.return_dbus_error(
                                "org.freedesktop.DBus.Error.InvalidArgs",
                                "Notify expects (susssasa{sv}i)",
                            ),
                        },
                        "CloseNotification" => {
                            if let Some((id,)) = params.get::<(u32,)>() {
                                on_close(id);
                            }
                            invocation.return_value(None);
                        }
                        _ => invocation.return_dbus_error(
                            "org.freedesktop.DBus.Error.UnknownMethod",
                            "No such method",
                        ),
                    })
                    .build();
                if let Err(e) = registered {
                    eprintln!("liubai-shell: cannot serve notifications: {e}");
                    return;
                }
                if let Some(server) = weak.upgrade() {
                    *server.connection.borrow_mut() = Some(connection);
                }
            },
            |_, _| {},
            |_, _| eprintln!("liubai-shell: another notification service took over"),
        );
        server
    }

    pub fn closed(&self, id: u32, why: Closed) {
        self.emit("NotificationClosed", (id, why as u32).to_variant());
    }

    pub fn action_invoked(&self, id: u32, key: &str) {
        self.emit("ActionInvoked", (id, key).to_variant());
    }

    fn emit(&self, signal: &str, args: glib::Variant) {
        if let Some(connection) = self.connection.borrow().as_ref() {
            if let Err(e) = connection.emit_signal(None, PATH, NAME, signal, Some(&args)) {
                eprintln!("liubai-shell: cannot send {signal}: {e}");
            }
        }
    }
}
