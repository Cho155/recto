//! The bar: where you are on the left; on the right, only what needs attention,
//! then quiet marks for network, Bluetooth, sound and battery, then the time.

use crate::{
    glyph::{self, Glyph},
    niri,
    panels::Kind,
    state::Desktop,
    status::{self, Alert, Battery, Mark, Tone},
    strings,
};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::rc::Rc;

pub enum Event {
    OpenToday,
    EndFocus,
    ToggleInputMethod,
    /// Open or close the tray list under this widget.
    ToggleTray(gtk::Widget),
    /// Open or close the Wi‑Fi, Bluetooth or Sound list under this widget.
    ToggleList(Kind, gtk::Widget),
}

pub type Handler = Rc<dyn Fn(Event)>;

/// Everything on the right side of the bar apart from the clock.
#[derive(Default)]
pub struct Notices {
    pub alerts: Vec<Alert>,
    /// "Focus until 16:00" and how many notifications it holds.
    pub focus: Option<(String, usize)>,
    pub unread: usize,
    /// Two letters for the input method, when there is more than one.
    pub input_method: Option<String>,
    pub tray_items: usize,
    /// `None` on a machine without a battery.
    pub battery: Option<Battery>,
    /// Network, Bluetooth and sound marks; `None` hides one.
    pub network: Option<Mark>,
    pub bluetooth: Option<Mark>,
    pub sound: Option<Mark>,
}

pub struct Bar {
    window: gtk::Window,
    output: Option<String>,
    view_width: f64,
    handler: Handler,
    workspace: gtk::Button,
    dots: gtk::Box,
    name: gtk::Label,
    strip: gtk::Box,
    notices: gtk::Box,
    divider: gtk::Box,
    input_method: gtk::Button,
    tray: gtk::Button,
    /// Network, Bluetooth and Sound, with the drawing inside each.
    marks: Vec<(Kind, gtk::Button, gtk::DrawingArea)>,
    unread: gtk::Button,
    battery: gtk::Box,
    battery_bolt: gtk::DrawingArea,
    battery_text: gtk::Label,
    date: gtk::Label,
    time: gtk::Label,
}

fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class(class);
    l
}

impl Bar {
    pub fn new(app: &gtk::Application, monitor: &gdk::Monitor, handler: Handler) -> Self {
        let window = gtk::Window::builder().application(app).build();
        window.add_css_class("recto-bar");
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_monitor(Some(monitor));
        for edge in [Edge::Top, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
        window.auto_exclusive_zone_enable();
        window.set_namespace(Some("recto-bar"));
        window.set_keyboard_mode(KeyboardMode::None);

        let root = hbox(0);
        root.add_css_class("bar");

        // Left: workspace dots and name. Clicking opens niri's overview.
        let dots = gtk::Box::new(gtk::Orientation::Vertical, 3);
        dots.add_css_class("dots");
        dots.set_valign(gtk::Align::Center);
        let name = gtk::Label::new(None);
        name.add_css_class("workspace-name");
        let workspace_content = hbox(10);
        workspace_content.append(&dots);
        workspace_content.append(&name);
        let workspace = gtk::Button::builder().child(&workspace_content).build();
        workspace.add_css_class("workspace");
        workspace.connect_clicked(|_| niri::toggle_overview());

        // The scroll: one short line per column, the focused one in clay.
        let strip = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        strip.add_css_class("strip");

        let spacer = hbox(0);
        spacer.set_hexpand(true);

        // Right: notices that come and go, then the unread dot and the clock.
        let notices = hbox(12);
        notices.add_css_class("notices");
        notices.set_valign(gtk::Align::Center);
        let divider = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Presentation)
            .valign(gtk::Align::Center)
            .build();
        divider.add_css_class("divider");

        let input_method = gtk::Button::with_label("");
        input_method.add_css_class("input-method");
        input_method.set_valign(gtk::Align::Center);
        let h = handler.clone();
        input_method.connect_clicked(move |_| h(Event::ToggleInputMethod));

        let tray = gtk::Button::builder()
            .child(&crate::glyph::new(crate::glyph::Glyph::ChevronDown, 14))
            .valign(gtk::Align::Center)
            .build();
        tray.add_css_class("tray-toggle");
        let h = handler.clone();
        tray.connect_clicked(move |button| h(Event::ToggleTray(button.clone().upcast())));

        // Network, Bluetooth, Sound: small marks, each opening its list.
        let marks: Vec<(Kind, gtk::Button, gtk::DrawingArea)> = [
            (Kind::Network, Glyph::Wifi { level: 0, off: false }),
            (Kind::Bluetooth, Glyph::Bluetooth { off: false }),
            (Kind::Sound, Glyph::Speaker { waves: 0, muted: false }),
        ]
        .into_iter()
        .map(|(kind, initial)| {
            let area = glyph::new(initial, 16);
            let button = gtk::Button::builder().child(&area).valign(gtk::Align::Center).visible(false).build();
            button.add_css_class("bar-mark");
            let h = handler.clone();
            button.connect_clicked(move |b| h(Event::ToggleList(kind, b.clone().upcast())));
            (kind, button, area)
        })
        .collect();

        // The battery: a quiet figure, louder when it runs low.
        let battery_bolt = crate::glyph::new(crate::glyph::Glyph::Bolt, 11);
        let battery_text = label("", "battery-figure");
        let battery = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Status)
            .build();
        battery.add_css_class("battery");
        battery.append(&battery_bolt);
        battery.append(&battery_text);

        let dot = hbox(0);
        dot.add_css_class("unread-dot");
        dot.set_valign(gtk::Align::Center);
        dot.set_halign(gtk::Align::Center);
        let unread = gtk::Button::builder().child(&dot).build();
        unread.add_css_class("unread");
        let h = handler.clone();
        unread.connect_clicked(move |_| h(Event::OpenToday));

        let date = label("", "date");
        let time = label("", "time");
        let clock_content = hbox(12);
        clock_content.append(&date);
        clock_content.append(&time);
        let clock = gtk::Button::builder()
            .child(&clock_content)
            .tooltip_text(strings::OPEN_TODAY)
            .build();
        clock.add_css_class("clock");
        let h = handler.clone();
        clock.connect_clicked(move |_| h(Event::OpenToday));

        for widget in [workspace.upcast_ref::<gtk::Widget>(), strip.upcast_ref(), spacer.upcast_ref(),
                       notices.upcast_ref(), divider.upcast_ref(), input_method.upcast_ref(),
                       tray.upcast_ref()] {
            root.append(widget);
        }
        let mark_group = hbox(2);
        mark_group.add_css_class("bar-marks");
        for (_, button, _) in &marks {
            mark_group.append(button);
        }
        for widget in [mark_group.upcast_ref::<gtk::Widget>(), battery.upcast_ref(), unread.upcast_ref(), clock.upcast_ref()] {
            root.append(widget);
        }

        // Scrolling anywhere on the bar moves between workspaces.
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
        );
        scroll.connect_scroll(|_, _, dy| {
            if dy > 0.0 {
                niri::focus_workspace_down();
            } else if dy < 0.0 {
                niri::focus_workspace_up();
            }
            glib::Propagation::Stop
        });
        root.add_controller(scroll);

        window.set_child(Some(&root));
        window.present();

        let bar = Bar {
            window,
            output: monitor.connector().map(|c| c.to_string()),
            view_width: f64::from(monitor.geometry().width()).max(1.0),
            handler,
            workspace,
            dots,
            name,
            strip,
            notices,
            divider,
            input_method,
            tray,
            marks,
            unread,
            battery,
            battery_bolt,
            battery_text,
            date,
            time,
        };
        bar.update_notices(&Notices::default());
        bar
    }

    pub fn update_desktop(&self, desktop: &Desktop) {
        let output = self.output.as_deref();
        let active = desktop.active_on(output);
        let dots = desktop.dots_on(output);

        clear(&self.dots);
        for ws in &dots {
            let dot = hbox(0);
            dot.add_css_class("dot");
            if Some(ws.id) == active.map(|a| a.id) {
                dot.add_css_class("active");
            }
            self.dots.append(&dot);
        }
        // A single dot says nothing.
        self.dots.set_visible(dots.len() > 1);

        clear(&self.strip);
        let Some(ws) = active else {
            self.name.set_label("");
            return;
        };
        self.name.set_label(&strings::workspace_label(ws));
        let position = dots.iter().position(|w| w.id == ws.id).map_or(1, |p| p + 1);
        self.workspace.update_property(&[gtk::accessible::Property::Label(
            &strings::workspace_a11y(ws, position, dots.len().max(position)),
        )]);

        for column in desktop.columns(ws.id, self.view_width) {
            let line = hbox(0);
            line.add_css_class("column");
            if column.focused {
                line.add_css_class("focused");
            } else if column.visible != Some(false) {
                line.add_css_class("visible");
            }
            // Proportional to the column's share of the screen, kept readable.
            let width = (column.width / self.view_width * 48.0).round().clamp(6.0, 40.0) as i32;
            line.set_size_request(width, -1);
            line.set_valign(gtk::Align::Center);
            self.strip.append(&line);
        }
    }

    pub fn update_notices(&self, notices: &Notices) {
        clear(&self.notices);
        let mut first = true;
        let mut separate = |container: &gtk::Box| {
            if !first {
                container.append(&label("·", "notice-sep"));
            }
            first = false;
        };
        for alert in &notices.alerts {
            separate(&self.notices);
            let l = label(&alert.text, "notice");
            if alert.urgent {
                l.add_css_class("urgent");
            }
            self.notices.append(&l);
        }
        if let Some((text, held)) = &notices.focus {
            separate(&self.notices);
            let pill = gtk::Button::with_label(text);
            pill.add_css_class("focus-pill");
            pill.set_tooltip_text(Some(strings::END_FOCUS));
            let h = self.handler.clone();
            pill.connect_clicked(move |_| h(Event::EndFocus));
            self.notices.append(&pill);
            if *held > 0 {
                self.notices.append(&label(&strings::held(*held), "notice-quiet"));
            }
        }
        let any = self.notices.first_child().is_some();
        self.notices.set_visible(any);
        self.divider.set_visible(any);

        match &notices.input_method {
            Some(label) => {
                self.input_method.set_label(label);
                self.input_method.set_visible(true);
                self.input_method.set_tooltip_text(Some("Switch input method"));
                self.input_method.update_property(&[gtk::accessible::Property::Label(&format!("Input method: {label}. Switch"))]);
            }
            None => self.input_method.set_visible(false),
        }
        self.tray.set_visible(notices.tray_items > 0);
        let tray_name = format!("Tray, {} {}", notices.tray_items, if notices.tray_items == 1 { "item" } else { "items" });
        self.tray.set_tooltip_text(Some(&tray_name));
        self.tray.update_property(&[gtk::accessible::Property::Label(&tray_name)]);

        self.update_battery(notices.battery.as_ref());
        for (kind, button, area) in &self.marks {
            let mark = match kind {
                Kind::Network => &notices.network,
                Kind::Bluetooth => &notices.bluetooth,
                Kind::Sound => &notices.sound,
            };
            let Some(mark) = mark else {
                button.set_visible(false);
                continue;
            };
            glyph::set(area, mark.glyph);
            area.queue_draw();
            for (class, tone) in [("off", Tone::Off), ("active", Tone::Active)] {
                if mark.tone == tone {
                    button.add_css_class(class);
                } else {
                    button.remove_css_class(class);
                }
            }
            button.set_tooltip_text(Some(&mark.spoken));
            button.update_property(&[gtk::accessible::Property::Label(&mark.spoken)]);
            button.set_visible(true);
        }

        self.unread.set_visible(notices.unread > 0);
        self.unread.update_property(&[gtk::accessible::Property::Label(&strings::unread(notices.unread))]);
        self.unread.set_tooltip_text(Some(&strings::unread(notices.unread)));
    }

    fn update_battery(&self, battery: Option<&Battery>) {
        let Some(figure) = battery.map(status::battery_figure) else {
            self.battery.set_visible(false);
            return;
        };
        self.battery_text.set_label(&figure.text);
        self.battery_bolt.set_visible(figure.plugged);
        for (class, on) in [("low", figure.charge == status::Charge::Low),
                            ("critical", figure.charge == status::Charge::Critical)] {
            if on {
                self.battery.add_css_class(class);
            } else {
                self.battery.remove_css_class(class);
            }
        }
        self.battery.set_tooltip_text(Some(&figure.spoken));
        self.battery.update_property(&[gtk::accessible::Property::Label(&figure.spoken)]);
        self.battery.set_visible(true);
    }

    /// The output this bar is on, such as "eDP-1".
    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    /// The mark a list of this kind opens under, when it is shown.
    pub fn mark(&self, kind: Kind) -> Option<gtk::Widget> {
        self.marks.iter().find(|(k, b, _)| *k == kind && b.is_visible()).map(|(_, b, _)| b.clone().upcast())
    }

    pub fn update_clock(&self, now: &glib::DateTime) {
        self.date.set_label(&strings::bar_date(now));
        self.time.set_label(&strings::clock(now));
    }

    pub fn close(&self) {
        self.window.destroy();
    }
}
