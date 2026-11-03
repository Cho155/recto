//! The bar: where you are on the left; on the right, only what needs attention,
//! then the time.

use crate::{niri, state::Desktop, status::Alert, strings};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::rc::Rc;

pub enum Event {
    OpenToday,
    EndFocus,
    ToggleInputMethod,
    /// Open or close the tray list under this widget.
    ToggleTray(gtk::Widget),
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
    unread: gtk::Button,
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
                       tray.upcast_ref(), unread.upcast_ref(), clock.upcast_ref()] {
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
            unread,
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

        self.unread.set_visible(notices.unread > 0);
        self.unread.update_property(&[gtk::accessible::Property::Label(&strings::unread(notices.unread))]);
        self.unread.set_tooltip_text(Some(&strings::unread(notices.unread)));
    }

    pub fn update_clock(&self, now: &glib::DateTime) {
        self.date.set_label(&strings::bar_date(now));
        self.time.set_label(&strings::clock(now));
    }

    pub fn close(&self) {
        self.window.destroy();
    }
}
