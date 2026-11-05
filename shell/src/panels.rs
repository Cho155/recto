//! The bar's three small lists: Wi‑Fi, Bluetooth and Sound. Each opens under
//! its mark, like the tray's list and with the same keyboard handling: text
//! rows, state in italic on the right, words ("On · Off") rather than switches.

use crate::{
    bluetooth::Bluetooth,
    glyph::{self, Glyph},
    sound::{Device, Sound},
    wifi::{self, Network},
};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Network,
    Bluetooth,
    Sound,
}

pub enum Event {
    Closed,
    /// Something only the list itself changed (a password field opened): draw it again.
    Redraw,
    WifiPower(bool),
    WifiConnect(Network, Option<String>),
    WifiDisconnect,
    BluetoothPower(bool),
    /// A device's path, and whether to connect (or disconnect).
    BluetoothConnect(String, bool),
    BluetoothPair(String),
    BluetoothSearch,
    Volume { output: bool, name: String, percent: u32 },
    Mute { output: bool, name: String, muted: bool },
    UseDevice { output: bool, name: String },
    /// Open a fuller tool ("Network settings…").
    Launch(&'static str),
}

impl Event {
    /// For RECTO_DEBUG logs; leaves out passwords.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Closed => "closed",
            Event::Redraw => "redraw",
            Event::WifiPower(_) => "wifi-power",
            Event::WifiConnect(..) => "wifi-connect",
            Event::WifiDisconnect => "wifi-disconnect",
            Event::BluetoothPower(_) => "bluetooth-power",
            Event::BluetoothConnect(..) => "bluetooth-connect",
            Event::BluetoothPair(_) => "bluetooth-pair",
            Event::BluetoothSearch => "bluetooth-search",
            Event::Volume { .. } => "volume",
            Event::Mute { .. } => "mute",
            Event::UseDevice { .. } => "use-device",
            Event::Launch(_) => "launch",
        }
    }
}

pub type Handler = Rc<dyn Fn(Event)>;

/// At most this many networks or devices are listed.
const MAX_ROWS: usize = 10;
/// After a slider is moved by hand, readings wait this long before moving it.
const HANDS_OFF_US: i64 = 900_000;

struct Level {
    output: bool,
    name: String,
    scale: gtk::Scale,
    changed: glib::SignalHandlerId,
    value: gtk::Label,
    mute: gtk::Button,
    muted: Rc<Cell<bool>>,
}

pub struct Panel {
    pub kind: Kind,
    popover: gtk::Popover,
    body: gtk::Box,
    handler: Handler,
    /// Wi‑Fi: the network whose password is being typed.
    asking: RefCell<Option<Vec<u8>>>,
    /// Notes beside rows ("Connecting…", or what went wrong), by network name or device path.
    notes: RefCell<HashMap<String, String>>,
    /// A summary of what was drawn last, so the same thing is not drawn twice.
    drawn: RefCell<String>,
    /// Sound: the sliders, changed in place so a drag is never interrupted.
    levels: RefCell<Vec<Level>>,
    touched: Rc<Cell<i64>>,
    focused_once: Cell<bool>,
    /// A row to give the keyboard to on the next draw (after the password field goes).
    refocus: RefCell<Option<String>>,
}

fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder().label(text).xalign(0.0).build();
    l.add_css_class(class);
    l
}

/// A line of quiet italic text: "Wi‑Fi is off."
fn quiet(text: &str) -> gtk::Label {
    let l = label(text, "tray-empty");
    l.set_wrap(true);
    l.set_max_width_chars(30);
    l
}

/// Every widget below `root`, depth first.
fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = Vec::new();
    let mut child = root.first_child();
    while let Some(c) = child {
        out.push(c.clone());
        out.extend(descendants(&c));
        child = c.next_sibling();
    }
    out
}

impl Panel {
    pub fn open(kind: Kind, anchor: &gtk::Widget, handler: Handler) -> Rc<Panel> {
        let popover = crate::tray::popover(anchor, "controls");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("tray-list");
        body.add_css_class("controls-list");
        popover.set_child(Some(&body));
        let h = handler.clone();
        popover.connect_closed(move |_| h(Event::Closed));
        let panel = Rc::new(Panel {
            kind,
            popover,
            body,
            handler,
            asking: RefCell::default(),
            notes: RefCell::default(),
            drawn: RefCell::default(),
            levels: RefCell::default(),
            touched: Rc::new(Cell::new(0)),
            focused_once: Cell::new(false),
            refocus: RefCell::default(),
        });
        panel.popover.popup();
        panel
    }

    pub fn close(&self) {
        self.popover.popdown();
    }

    /// The widget with the keyboard, by name (for RECTO_DEBUG logs).
    pub fn focus_name(&self) -> Option<String> {
        self.popover.root().and_then(|r| r.focus()).map(|w| format!("{} {}", w.type_().name(), w.widget_name()))
    }

    pub fn is_open(&self) -> bool {
        self.popover.is_visible()
    }

    /// A note under the heading, such as why the power could not be changed.
    fn power_note(&self) {
        if let Some(text) = self.notes.borrow().get("power") {
            self.body.append(&quiet(text));
        }
    }

    /// Note something beside a row, or clear it with `None`.
    pub fn note(&self, key: &str, text: Option<&str>) {
        let mut notes = self.notes.borrow_mut();
        match text {
            Some(t) => notes.insert(key.to_owned(), t.to_owned()),
            None => notes.remove(key),
        };
        self.drawn.borrow_mut().clear();
    }

    pub fn stop_asking(&self) {
        self.asking.borrow_mut().take();
        self.drawn.borrow_mut().clear();
    }

    fn send(&self, event: Event) {
        (self.handler)(event);
    }

    /// Start drawing anew, unless `summary` is what is already there. Returns the
    /// name of the row that had the keyboard, to give it back afterwards.
    fn begin(&self, summary: String) -> Option<Option<String>> {
        if *self.drawn.borrow() == summary {
            return None;
        }
        *self.drawn.borrow_mut() = summary;
        let focused = self
            .popover
            .root()
            .and_then(|r| r.focus())
            .filter(|w| w.is_ancestor(&self.body))
            .and_then(|w| {
                std::iter::successors(Some(w), |w| w.parent()).find(|w| !w.widget_name().is_empty() && w.is::<gtk::Button>())
            })
            .map(|w| w.widget_name().to_string());
        let focused = self.refocus.borrow_mut().take().or(focused);
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.levels.borrow_mut().clear();
        Some(focused)
    }

    /// Give the keyboard back to the row it was on, or to the first row on opening.
    fn finish(&self, focused: Option<String>, prefer: Option<gtk::Widget>) {
        if let Some(widget) = prefer {
            widget.grab_focus();
            return;
        }
        let named = focused.and_then(|name| {
            descendants(self.body.upcast_ref()).into_iter().find(|w| w.widget_name() == name.as_str())
        });
        match named {
            Some(w) => {
                w.grab_focus();
            }
            // On opening: the first network, device or slider, once there is one
            // (the list may still be loading). Never a "… settings" link.
            None if !self.focused_once.get() => {
                let first = descendants(self.body.upcast_ref()).into_iter().find(|w| {
                    w.is::<gtk::Scale>() || (w.is::<gtk::Button>() && w.has_css_class("tray-item") && !w.has_css_class("controls-link"))
                });
                if let Some(first) = first {
                    first.grab_focus();
                    self.focused_once.set(true);
                }
            }
            None => {}
        }
    }

    /// "Wi‑Fi                On · Off"
    fn heading(self: &Rc<Self>, title: &str, power: Option<bool>, event: fn(bool) -> Event) -> gtk::Box {
        let row = hbox(0);
        row.add_css_class("controls-heading");
        let name = label(title, "controls-title");
        name.set_hexpand(true);
        row.append(&name);
        if let Some(on) = power {
            let words = hbox(0);
            words.update_property(&[gtk::accessible::Property::Label(&format!("{title} power"))]);
            for (i, (text, value)) in [("On", true), ("Off", false)].into_iter().enumerate() {
                if i > 0 {
                    let sep = gtk::Label::builder().label("·").accessible_role(gtk::AccessibleRole::Presentation).build();
                    sep.add_css_class("segment-sep");
                    words.append(&sep);
                }
                let b = gtk::Button::with_label(text);
                b.add_css_class("segment");
                b.set_widget_name(&format!("power-{text}"));
                if on == value {
                    b.add_css_class("active");
                }
                let w = Rc::downgrade(self);
                b.connect_clicked(move |_| {
                    if let Some(p) = w.upgrade() {
                        if on != value {
                            p.send(event(value));
                        }
                    }
                });
                words.append(&b);
            }
            row.append(&words);
        }
        row
    }

    /// A row of the list: name on the left; note and small marks on the right.
    fn row(key: &str, name: &str, detail: &str, marks: &[Glyph], tooltip: Option<&str>) -> gtk::Button {
        let content = hbox(8);
        let title = gtk::Label::builder().label(name).xalign(0.0).hexpand(true).build();
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(22);
        content.append(&title);
        if !detail.is_empty() {
            let d = gtk::Label::new(Some(detail));
            d.add_css_class("tray-detail");
            d.set_ellipsize(gtk::pango::EllipsizeMode::End);
            d.set_max_width_chars(20);
            content.append(&d);
        }
        for g in marks {
            let area = glyph::new(*g, 13);
            area.add_css_class("controls-mark");
            content.append(&area);
        }
        let button = gtk::Button::builder().child(&content).build();
        button.add_css_class("tray-item");
        button.set_widget_name(key);
        if let Some(t) = tooltip {
            button.set_tooltip_text(Some(t));
        }
        button
    }

    /// A row of quiet italic words that does something: "Find new devices".
    fn word_row(text: &str, name: &str) -> gtk::Button {
        let words = gtk::Label::builder().label(text).xalign(0.0).build();
        let b = gtk::Button::builder().child(&words).build();
        b.add_css_class("tray-item");
        b.add_css_class("controls-link");
        b.set_widget_name(name);
        b
    }

    /// "Network settings…": a quieter row that opens a fuller tool.
    fn link(self: &Rc<Self>, text: &str, program: Option<&'static str>) -> Option<gtk::Button> {
        let program = program?;
        let b = Self::word_row(text, &format!("link-{program}"));
        let w = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.send(Event::Launch(program));
            }
        });
        Some(b)
    }

    fn rule(&self) {
        let r = hbox(0);
        r.add_css_class("menu-separator");
        self.body.append(&r);
    }

    // ── Wi‑Fi ───────────────────────────────────────────────────────────

    /// `wifi`: whether the radio is on; `None` when there is no Wi‑Fi device.
    pub fn show_network(self: &Rc<Self>, wifi: Option<bool>, cable: bool, networks: &[Network], settings: Option<&'static str>) {
        let summary = format!("{wifi:?} {cable} {networks:?} {:?} {:?}", self.asking.borrow(), self.notes.borrow());
        let Some(focused) = self.begin(summary) else { return };
        let enabled = wifi == Some(true);
        let title = if wifi.is_some() { "Wi‑Fi" } else { "Network" };
        self.body.append(&self.heading(title, wifi, Event::WifiPower));
        self.power_note();
        if cable {
            self.body.append(&quiet("Connected by cable."));
        }
        let mut prefer = None;
        if wifi.is_none() {
            if !cable {
                self.body.append(&quiet("Not connected."));
            }
        } else if !enabled {
            self.body.append(&quiet("Wi‑Fi is off."));
        } else if networks.is_empty() {
            self.body.append(&quiet("Looking for networks…"));
        }
        for network in networks.iter().take(if enabled { MAX_ROWS } else { 0 }) {
            let note = self.notes.borrow().get(&network.ssid).cloned();
            let detail = note.unwrap_or_else(|| {
                if network.connected {
                    "Connected".into()
                } else if network.saved.is_some() {
                    "Saved".into()
                } else {
                    String::new()
                }
            });
            let mut marks = Vec::new();
            if network.security != wifi::Security::Open && network.saved.is_none() && !network.connected {
                marks.push(Glyph::Lock);
            }
            marks.push(Glyph::Wifi { level: wifi::level(network.strength), off: false });
            let tooltip = network.connected.then_some("Disconnect");
            let button = Self::row(&format!("net-{}", network.ssid), &network.ssid, &detail, &marks, tooltip);
            let (w, n) = (Rc::downgrade(self), network.clone());
            button.connect_clicked(move |_| {
                let Some(p) = w.upgrade() else { return };
                p.choose_network(&n, settings);
            });
            self.body.append(&button);
            if self.asking.borrow().as_deref() == Some(network.ssid_bytes.as_slice()) {
                prefer = Some(self.password_row(network));
            }
        }
        self.rule();
        if let Some(link) = self.link("Network settings…", settings) {
            self.body.append(&link);
        }
        self.finish(focused, prefer);
    }

    fn choose_network(self: &Rc<Self>, network: &Network, settings: Option<&'static str>) {
        if network.connected {
            self.send(Event::WifiDisconnect);
        } else if network.saved.is_none() && network.security == wifi::Security::Other {
            match settings {
                Some(program) => self.send(Event::Launch(program)),
                None => self.note(&network.ssid, Some("Needs the network settings")),
            }
        } else if network.saved.is_none() && network.security.needs_password() {
            *self.asking.borrow_mut() = Some(network.ssid_bytes.clone());
            self.drawn.borrow_mut().clear();
            self.send(Event::Redraw);
        } else {
            self.send(Event::WifiConnect(network.clone(), None));
        }
    }

    /// The password field under a network that needs one. Returns the field.
    fn password_row(self: &Rc<Self>, network: &Network) -> gtk::Widget {
        let row = hbox(8);
        row.add_css_class("controls-password");
        let entry = gtk::PasswordEntry::builder().show_peek_icon(true).placeholder_text("Password").hexpand(true).build();
        entry.update_property(&[gtk::accessible::Property::Label(&format!("Password for {}", network.ssid))]);
        let go = gtk::Button::with_label("Connect");
        go.add_css_class("segment");
        go.set_widget_name("connect");
        let connect = {
            let (w, n, entry) = (Rc::downgrade(self), network.clone(), entry.clone());
            move || {
                let Some(p) = w.upgrade() else { return };
                let password = entry.text().to_string();
                // WPA passwords are 8 to 63 characters.
                if password.chars().count() < 8 {
                    p.note(&n.ssid, Some("At least 8 characters"));
                    p.send(Event::Redraw);
                    return;
                }
                *p.refocus.borrow_mut() = Some(format!("net-{}", n.ssid));
                p.stop_asking();
                p.send(Event::WifiConnect(n.clone(), Some(password)));
            }
        };
        let c = connect.clone();
        entry.connect_activate(move |_| c());
        go.connect_clicked(move |_| connect());
        row.append(&entry);
        row.append(&go);
        self.body.append(&row);
        entry.upcast()
    }

    // ── Bluetooth ───────────────────────────────────────────────────────

    pub fn show_bluetooth(self: &Rc<Self>, bt: &Bluetooth, settings: Option<&'static str>) {
        let summary = format!("{bt:?} {:?}", self.notes.borrow());
        let Some(focused) = self.begin(summary) else { return };
        self.body.append(&self.heading("Bluetooth", Some(bt.powered), Event::BluetoothPower));
        self.power_note();
        if !bt.powered {
            self.body.append(&quiet("Bluetooth is off."));
        } else {
            let (paired, nearby): (Vec<_>, Vec<_>) = bt.devices.iter().partition(|d| d.paired);
            if paired.is_empty() {
                self.body.append(&quiet("No devices yet."));
            }
            for device in paired.iter().take(MAX_ROWS) {
                let note = self.notes.borrow().get(&device.path).cloned();
                let detail = note.unwrap_or_else(|| match (device.connected, device.battery) {
                    (true, Some(b)) => format!("Connected · {b}%"),
                    (true, None) => "Connected".into(),
                    _ => String::new(),
                });
                let tooltip = if device.connected { "Disconnect" } else { "Connect" };
                let button = Self::row(&device.path, &device.name, &detail, &[], Some(tooltip));
                let (w, path, connected) = (Rc::downgrade(self), device.path.clone(), device.connected);
                button.connect_clicked(move |_| {
                    if let Some(p) = w.upgrade() {
                        p.send(Event::BluetoothConnect(path.clone(), !connected));
                    }
                });
                self.body.append(&button);
            }
            if bt.discovering || !nearby.is_empty() {
                self.body.append(&label("Nearby", "menu-heading"));
                for device in nearby.iter().take(MAX_ROWS) {
                    let note = self.notes.borrow().get(&device.path).cloned().unwrap_or_default();
                    let button = Self::row(&device.path, &device.name, &note, &[], Some("Pair"));
                    let (w, path) = (Rc::downgrade(self), device.path.clone());
                    button.connect_clicked(move |_| {
                        if let Some(p) = w.upgrade() {
                            p.send(Event::BluetoothPair(path.clone()));
                        }
                    });
                    self.body.append(&button);
                }
            }
            if bt.discovering {
                self.body.append(&quiet("Searching…"));
            } else {
                let find = Self::word_row("Find new devices", "find");
                let w = Rc::downgrade(self);
                find.connect_clicked(move |_| {
                    if let Some(p) = w.upgrade() {
                        p.send(Event::BluetoothSearch);
                    }
                });
                self.body.append(&find);
            }
        }
        if let Some(link) = self.link("Bluetooth settings…", settings) {
            self.rule();
            self.body.append(&link);
        }
        self.finish(focused, None);
    }

    // ── Sound ───────────────────────────────────────────────────────────

    pub fn show_sound(self: &Rc<Self>, sound: &Sound, mixer: Option<&'static str>) {
        // Only the layout decides whether to draw anew; levels change in place.
        let names = |list: &[Device]| list.iter().map(|d| format!("{}={}", d.name, d.description)).collect::<Vec<_>>();
        let summary = format!(
            "{:?} {:?} {:?} {:?} {mixer:?}",
            names(&sound.outputs),
            names(&sound.inputs),
            sound.default_output,
            sound.default_input
        );
        let Some(focused) = self.begin(summary) else {
            self.update_levels(sound);
            return;
        };
        for (output, title, devices) in [(true, "Output", &sound.outputs), (false, "Input", &sound.inputs)] {
            let current = if output { sound.output() } else { sound.input() };
            let Some(current) = current else { continue };
            if !output {
                self.rule();
            }
            self.body.append(&label(title, "menu-heading"));
            self.level_row(output, current);
            if devices.len() > 1 {
                for device in devices.iter().take(MAX_ROWS) {
                    let in_use = device.name == current.name;
                    let button = Self::row(&format!("dev-{}", device.name), &device.description, if in_use { "In use" } else { "" }, &[], None);
                    let (w, name) = (Rc::downgrade(self), device.name.clone());
                    button.connect_clicked(move |_| {
                        if let Some(p) = w.upgrade() {
                            p.send(Event::UseDevice { output, name: name.clone() });
                        }
                    });
                    self.body.append(&button);
                }
            } else {
                let caption = label(&current.description, "controls-caption");
                caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
                caption.set_max_width_chars(30);
                self.body.append(&caption);
            }
        }
        if sound.outputs.is_empty() && sound.inputs.is_empty() {
            self.body.append(&quiet("No sound devices."));
        }
        if let Some(link) = self.link("Sound settings…", mixer) {
            self.rule();
            self.body.append(&link);
        }
        self.finish(focused, None);
    }

    fn level_row(self: &Rc<Self>, output: bool, device: &Device) {
        let row = hbox(10);
        row.add_css_class("controls-level-row");
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        scale.set_draw_value(false);
        scale.set_hexpand(true);
        scale.set_value(f64::from(device.percent));
        scale.add_css_class("controls-level");
        scale.set_widget_name(if output { "level-out" } else { "level-in" });
        scale.update_property(&[gtk::accessible::Property::Label(if output { "Output volume" } else { "Input volume" })]);
        let value = label(&format!("{}%", device.percent), "controls-value");
        value.set_width_chars(4);
        value.set_xalign(1.0);
        let muted = Rc::new(Cell::new(device.muted));
        let mute = gtk::Button::with_label(if device.muted { "Muted" } else { "Mute" });
        mute.add_css_class("segment");
        mute.set_widget_name(if output { "mute-out" } else { "mute-in" });
        if device.muted {
            mute.add_css_class("active");
            scale.add_css_class("muted");
        }

        // Send the latest value a moment after the slider stops moving.
        let pending: Rc<Cell<Option<u32>>> = Rc::default();
        let changed = {
            let (w, name, value, touched) = (Rc::downgrade(self), device.name.clone(), value.clone(), self.touched.clone());
            scale.connect_value_changed(move |s| {
                let percent = s.value().round() as u32;
                value.set_label(&format!("{percent}%"));
                touched.set(glib::monotonic_time());
                if pending.replace(Some(percent)).is_some() {
                    return;
                }
                let (w, name, pending) = (w.clone(), name.clone(), pending.clone());
                glib::timeout_add_local_once(Duration::from_millis(60), move || {
                    if let (Some(p), Some(percent)) = (w.upgrade(), pending.take()) {
                        p.send(Event::Volume { output, name, percent });
                    }
                });
            })
        };
        {
            let (w, name, muted) = (Rc::downgrade(self), device.name.clone(), muted.clone());
            mute.connect_clicked(move |_| {
                if let Some(p) = w.upgrade() {
                    p.send(Event::Mute { output, name: name.clone(), muted: !muted.get() });
                }
            });
        }
        row.append(&scale);
        row.append(&value);
        row.append(&mute);
        self.body.append(&row);
        self.levels.borrow_mut().push(Level { output, name: device.name.clone(), scale, changed, value, mute, muted });
    }

    fn update_levels(&self, sound: &Sound) {
        let hands_off = glib::monotonic_time() - self.touched.get() < HANDS_OFF_US;
        for level in self.levels.borrow().iter() {
            let device = if level.output { sound.output() } else { sound.input() };
            let Some(device) = device.filter(|d| d.name == level.name) else { continue };
            if !hands_off {
                level.scale.block_signal(&level.changed);
                level.scale.set_value(f64::from(device.percent));
                level.scale.unblock_signal(&level.changed);
                level.value.set_label(&format!("{}%", device.percent));
            }
            level.muted.set(device.muted);
            level.mute.set_label(if device.muted { "Muted" } else { "Mute" });
            for widget in [level.mute.upcast_ref::<gtk::Widget>(), level.scale.upcast_ref()] {
                let class = if widget.is::<gtk::Scale>() { "muted" } else { "active" };
                if device.muted {
                    widget.add_css_class(class);
                } else {
                    widget.remove_css_class(class);
                }
            }
        }
    }
}
