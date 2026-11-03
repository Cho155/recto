//! The tray. The shell is both the StatusNotifierWatcher (apps register with it)
//! and the host (it shows them). Items are listed by name, without colored icons,
//! in a small panel under the bar's arrow; an item's menu opens beside it.

use crate::{dbusmenu, sni::{Address, Item}};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";

const XML: &str = r#"
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg direction="in" type="s"/></method>
    <method name="RegisterStatusNotifierHost"><arg direction="in" type="s"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
  </interface>
</node>"#;

struct Entry {
    address: Address,
    item: Item,
    order: u64,
    /// Dropping it unsubscribes from the item's signals.
    _signals: gio::SignalSubscription,
}

/// One row of the tray list.
pub struct Listed {
    pub key: String,
    pub label: String,
    pub detail: String,
    pub has_menu: bool,
}

pub struct Tray {
    connection: RefCell<Option<gio::DBusConnection>>,
    entries: RefCell<HashMap<String, Entry>>,
    /// Per bus name: stops watching it.
    watches: RefCell<HashMap<String, Box<dyn FnOnce()>>>,
    next_order: Cell<u64>,
    on_change: Box<dyn Fn()>,
    panel: RefCell<Option<gtk::Popover>>,
}

fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

fn popover(anchor: &gtk::Widget, class: &str) -> gtk::Popover {
    let p = gtk::Popover::new();
    p.add_css_class(class);
    p.set_has_arrow(false);
    p.set_position(gtk::PositionType::Bottom);
    p.set_parent(anchor);
    // Free the widgets once it closes; nothing of the tray stays around while hidden.
    p.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    p
}

impl Tray {
    pub fn start(on_change: impl Fn() + 'static) -> Rc<Tray> {
        let tray = Rc::new(Tray {
            connection: RefCell::default(),
            entries: RefCell::default(),
            watches: RefCell::default(),
            next_order: Cell::new(0),
            on_change: Box::new(on_change),
            panel: RefCell::default(),
        });
        let weak = Rc::downgrade(&tray);
        let _owner = gio::bus_own_name(
            gio::BusType::Session,
            WATCHER,
            gio::BusNameOwnerFlags::NONE,
            move |connection, _| {
                if let Some(tray) = weak.upgrade() {
                    tray.serve(connection);
                }
            },
            |_, _| {},
            |_, _| eprintln!("liubai-shell: another tray watcher is running; the tray stays empty"),
        );
        tray
    }

    fn serve(self: &Rc<Self>, connection: gio::DBusConnection) {
        let Some(info) = gio::DBusNodeInfo::for_xml(XML).ok().and_then(|n| n.lookup_interface(WATCHER)) else {
            eprintln!("liubai-shell: tray watcher description is invalid");
            return;
        };
        let (on_call, on_prop) = (Rc::downgrade(self), Rc::downgrade(self));
        let registered = connection
            .register_object(WATCHER_PATH, &info)
            .method_call(move |connection, sender, _, _, method, params, invocation| match method {
                "RegisterStatusNotifierItem" => {
                    let service = params.get::<(String,)>().map(|(s,)| s).unwrap_or_default();
                    if let (Some(tray), Some(sender)) = (on_call.upgrade(), sender) {
                        tray.register(&connection, Address::from_registration(&service, sender));
                    }
                    invocation.return_value(None);
                }
                "RegisterStatusNotifierHost" => invocation.return_value(None),
                _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", "No such method"),
            })
            .property(move |_, _, _, _, name| match name {
                "RegisteredStatusNotifierItems" => {
                    on_prop.upgrade().map(|t| t.keys()).unwrap_or_default().to_variant()
                }
                "IsStatusNotifierHostRegistered" => true.to_variant(),
                _ => 0i32.to_variant(),
            })
            .build();
        if let Err(e) = registered {
            eprintln!("liubai-shell: cannot serve the tray: {e}");
            return;
        }
        let _ = connection.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierHostRegistered", None);
        *self.connection.borrow_mut() = Some(connection);
    }

    pub fn connection(&self) -> Option<gio::DBusConnection> {
        self.connection.borrow().clone()
    }

    fn keys(&self) -> Vec<String> {
        let entries = self.entries.borrow();
        let mut list: Vec<(&u64, &String)> = entries.iter().map(|(k, e)| (&e.order, k)).collect();
        list.sort();
        list.into_iter().map(|(_, k)| k.clone()).collect()
    }

    fn register(self: &Rc<Self>, connection: &gio::DBusConnection, address: Address) {
        let key = address.key();
        if self.entries.borrow().contains_key(&key) {
            return;
        }
        let weak = Rc::downgrade(self);
        let k = key.clone();
        let signals = connection.subscribe_to_signal(
            Some(&address.bus),
            Some(ITEM),
            None,
            Some(&address.path),
            None,
            gio::DBusSignalFlags::NONE,
            move |_| {
                if let Some(tray) = weak.upgrade() {
                    tray.refresh(k.clone());
                }
            },
        );
        let order = self.next_order.get();
        self.next_order.set(order + 1);
        self.entries.borrow_mut().insert(
            key.clone(),
            Entry { address: address.clone(), item: Item::default(), order, _signals: signals },
        );

        // Forget the item when its owner leaves the bus.
        if !self.watches.borrow().contains_key(&address.bus) {
            let weak = Rc::downgrade(self);
            let bus = address.bus.clone();
            let id = gio::bus_watch_name_on_connection(
                connection,
                &address.bus,
                gio::BusNameWatcherFlags::NONE,
                |_, _, _| {},
                move |_, _| {
                    let (weak, bus) = (weak.clone(), bus.clone());
                    glib::idle_add_local_once(move || {
                        if let Some(tray) = weak.upgrade() {
                            tray.forget(&bus);
                        }
                    });
                },
            );
            self.watches.borrow_mut().insert(address.bus.clone(), Box::new(move || gio::bus_unwatch_name(id)));
        }
        let _ = connection.emit_signal(
            None,
            WATCHER_PATH,
            WATCHER,
            "StatusNotifierItemRegistered",
            Some(&(key.as_str(),).to_variant()),
        );
        self.refresh(key);
    }

    fn forget(&self, bus: &str) {
        let gone: Vec<String> = self
            .entries
            .borrow()
            .iter()
            .filter(|(_, e)| e.address.bus == bus)
            .map(|(k, _)| k.clone())
            .collect();
        for key in &gone {
            self.entries.borrow_mut().remove(key);
            if let Some(connection) = self.connection() {
                let _ = connection.emit_signal(
                    None,
                    WATCHER_PATH,
                    WATCHER,
                    "StatusNotifierItemUnregistered",
                    Some(&(key.as_str(),).to_variant()),
                );
            }
        }
        let unwatch = self.watches.borrow_mut().remove(bus);
        if let Some(unwatch) = unwatch {
            unwatch();
        }
        if !gone.is_empty() {
            (self.on_change)();
        }
    }

    /// Read an item's properties again (after it registers or signals a change).
    fn refresh(self: &Rc<Self>, key: String) {
        let Some(connection) = self.connection() else { return };
        let Some(address) = self.entries.borrow().get(&key).map(|e| e.address.clone()) else { return };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let reply = connection
                .call_future(
                    Some(&address.bus),
                    &address.path,
                    "org.freedesktop.DBus.Properties",
                    "GetAll",
                    Some(&(ITEM,).to_variant()),
                    Some(glib::VariantTy::new("(a{sv})").expect("valid type")),
                    gio::DBusCallFlags::NONE,
                    2_000,
                )
                .await;
            let Ok(reply) = reply else { return };
            let Some(props) = reply.child_value(0).get::<HashMap<String, glib::Variant>>() else { return };
            let Some(tray) = weak.upgrade() else { return };
            let changed = match tray.entries.borrow_mut().get_mut(&key) {
                Some(entry) => {
                    let item = Item::from_props(&props);
                    let changed = entry.item != item;
                    entry.item = item;
                    changed
                }
                None => false,
            };
            if changed {
                (tray.on_change)();
                tray.refill_panel();
            }
        });
    }

    /// Items to list, in the order they arrived. The input method is shown in the bar instead.
    pub fn listed(&self) -> Vec<Listed> {
        let entries = self.entries.borrow();
        let mut list: Vec<&Entry> = entries
            .values()
            .filter(|e| e.item.shown() && !e.item.is_input_method() && e.item != Item::default())
            .collect();
        list.sort_by_key(|e| e.order);
        list.into_iter()
            .map(|e| Listed {
                key: e.address.key(),
                label: e.item.label(),
                detail: e.item.detail(),
                has_menu: e.item.menu.is_some(),
            })
            .collect()
    }

    pub fn has_input_method(&self) -> bool {
        self.entries.borrow().values().any(|e| e.item.is_input_method())
    }

    fn call(&self, key: &str, interface: &str, method: &str, args: glib::Variant) -> Option<impl std::future::Future<Output = Result<glib::Variant, glib::Error>>> {
        let connection = self.connection()?;
        let entries = self.entries.borrow();
        let entry = entries.get(key)?;
        let path = if interface == MENU { entry.item.menu.clone()? } else { entry.address.path.clone() };
        Some(connection.call_future(
            Some(&entry.address.bus),
            &path,
            interface,
            method,
            Some(&args),
            None,
            gio::DBusCallFlags::NONE,
            2_000,
        ))
    }

    // ── Panel ──────────────────────────────────────────────────────────

    /// Open the tray list under `anchor`, or close it if it is open.
    pub fn toggle_panel(self: &Rc<Self>, anchor: &gtk::Widget) {
        // Take it out first: closing runs a handler that touches `panel` again.
        let open = self.panel.borrow_mut().take();
        if let Some(open) = open {
            open.popdown();
            return;
        }
        let panel = popover(anchor, "tray");
        let weak = Rc::downgrade(self);
        panel.connect_closed(move |_| {
            if let Some(tray) = weak.upgrade() {
                tray.panel.borrow_mut().take();
            }
        });
        *self.panel.borrow_mut() = Some(panel.clone());
        self.refill_panel();
        panel.popup();
    }

    fn close_panel(&self) {
        let open = self.panel.borrow_mut().take();
        if let Some(open) = open {
            open.popdown();
        }
    }

    fn refill_panel(self: &Rc<Self>) {
        let Some(panel) = self.panel.borrow().clone() else { return };
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("tray-list");
        let items = self.listed();
        if items.is_empty() {
            let empty = gtk::Label::new(Some("Nothing in the tray."));
            empty.add_css_class("tray-empty");
            list.append(&empty);
        }
        for listed in items {
            let row = hbox(0);
            row.add_css_class("tray-row");

            let content = hbox(10);
            let name = gtk::Label::builder().label(&listed.label).xalign(0.0).hexpand(true).build();
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(22);
            content.append(&name);
            if !listed.detail.is_empty() {
                let detail = gtk::Label::new(Some(&listed.detail));
                detail.add_css_class("tray-detail");
                detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
                detail.set_max_width_chars(16);
                content.append(&detail);
            }
            let main = gtk::Button::builder().child(&content).hexpand(true).build();
            main.add_css_class("tray-item");
            let (weak, key) = (Rc::downgrade(self), listed.key.clone());
            main.connect_clicked(move |button| {
                if let Some(tray) = weak.upgrade() {
                    tray.activate(&key, button.upcast_ref());
                }
            });
            row.append(&main);

            if listed.has_menu {
                let more = gtk::Button::builder().child(&crate::glyph::new(crate::glyph::Glyph::More, 16)).build();
                more.add_css_class("tray-more");
                more.set_tooltip_text(Some(&format!("Menu for {}", listed.label)));
                more.update_property(&[gtk::accessible::Property::Label(&format!("Menu for {}", listed.label))]);
                let (weak, key) = (Rc::downgrade(self), listed.key.clone());
                more.connect_clicked(move |button| {
                    if let Some(tray) = weak.upgrade() {
                        tray.open_menu(key.clone(), button.clone().upcast());
                    }
                });
                row.append(&more);
            }
            list.append(&row);
        }
        panel.set_child(Some(&list));
    }

    /// Left click: the item's own action, or its menu when it has none.
    fn activate(self: &Rc<Self>, key: &str, anchor: &gtk::Widget) {
        let item_is_menu = self.entries.borrow().get(key).is_some_and(|e| e.item.item_is_menu);
        let has_menu = self.entries.borrow().get(key).is_some_and(|e| e.item.menu.is_some());
        if item_is_menu && has_menu {
            self.open_menu(key.to_owned(), anchor.clone());
            return;
        }
        let Some(call) = self.call(key, ITEM, "Activate", (0i32, 0i32).to_variant()) else { return };
        let (weak, key, anchor) = (Rc::downgrade(self), key.to_owned(), anchor.clone());
        glib::spawn_future_local(async move {
            let failed = call.await.is_err();
            let Some(tray) = weak.upgrade() else { return };
            if failed && has_menu {
                tray.open_menu(key, anchor);
            } else {
                tray.close_panel();
            }
        });
    }

    // ── Item menus ─────────────────────────────────────────────────────

    fn open_menu(self: &Rc<Self>, key: String, anchor: gtk::Widget) {
        let about = self.call(&key, MENU, "AboutToShow", (0i32,).to_variant());
        let layout = self.call(&key, MENU, "GetLayout", (0i32, -1i32, Vec::<String>::new()).to_variant());
        let Some(layout) = layout else {
            // No dbusmenu: let the app show its own menu.
            if let Some(call) = self.call(&key, ITEM, "ContextMenu", (0i32, 0i32).to_variant()) {
                glib::spawn_future_local(async move {
                    let _ = call.await;
                });
            }
            return;
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            if let Some(about) = about {
                let _ = about.await;
            }
            let Ok(reply) = layout.await else { return };
            let Some(root) = dbusmenu::parse(&reply.child_value(1)) else { return };
            if let Some(tray) = weak.upgrade() {
                tray.show_menu(&key, &anchor, dbusmenu::flatten(&root));
            }
        });
    }

    fn show_menu(self: &Rc<Self>, key: &str, anchor: &gtk::Widget, entries: Vec<dbusmenu::Entry>) {
        if anchor.root().is_none() {
            return; // the panel closed while the menu was loading
        }
        let menu = popover(anchor, "tray-menu");
        menu.set_position(gtk::PositionType::Right);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("tray-list");
        for entry in entries {
            match entry {
                dbusmenu::Entry::Separator => {
                    let line = gtk::Box::builder().accessible_role(gtk::AccessibleRole::Separator).build();
                    line.add_css_class("menu-separator");
                    list.append(&line);
                }
                dbusmenu::Entry::Heading { label, depth } => {
                    let heading = gtk::Label::builder().label(&label).xalign(0.0).build();
                    heading.add_css_class("menu-heading");
                    heading.set_margin_start(12 * depth as i32);
                    list.append(&heading);
                }
                dbusmenu::Entry::Item { id, label, enabled, toggle, depth } => {
                    let text = match toggle {
                        Some(true) => format!("✓  {label}"),
                        Some(false) => format!("    {label}"),
                        None => label,
                    };
                    let item = gtk::Button::builder().label(&text).sensitive(enabled).build();
                    item.add_css_class("menu-item");
                    item.set_margin_start(12 * depth as i32);
                    if let Some(child) = item.child().and_downcast::<gtk::Label>() {
                        child.set_xalign(0.0);
                    }
                    let (weak, key, menu) = (Rc::downgrade(self), key.to_owned(), menu.clone());
                    item.connect_clicked(move |_| {
                        menu.popdown();
                        let Some(tray) = weak.upgrade() else { return };
                        // (isvu): id, event, data, timestamp
                        let event = (id, "clicked", 0i32.to_variant(), 0u32).to_variant();
                        if let Some(call) = tray.call(&key, MENU, "Event", event) {
                            glib::spawn_future_local(async move {
                                let _ = call.await;
                            });
                        }
                        tray.close_panel();
                    });
                    list.append(&item);
                }
            }
        }
        menu.set_child(Some(&list));
        menu.popup();
    }
}
