//! The on-screen display for volume, microphone and brightness: one quiet line
//! at the bottom of the screen that leaves after a moment.

use crate::controls::{Outcome, Reading};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

pub const LINGER_MS: u64 = 1_200;

pub struct Osd {
    window: gtk::Window,
    name: gtk::Label,
    level: gtk::ProgressBar,
    value: gtk::Label,
}

impl Osd {
    pub fn new(app: &gtk::Application, monitor: Option<&gdk::Monitor>) -> Osd {
        let window = gtk::Window::builder().application(app).build();
        window.add_css_class("liubai-osd");
        window.set_resizable(false); // follow the content as it shrinks, too
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        if let Some(monitor) = monitor {
            window.set_monitor(Some(monitor));
        }
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 68); // 96 to the panel; .osd adds 28 of shadow room
        window.set_namespace(Some("liubai-osd"));
        window.set_keyboard_mode(KeyboardMode::None);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.add_css_class("osd");
        row.set_size_request(320, 56);
        row.set_accessible_role(gtk::AccessibleRole::Status);
        let name = gtk::Label::new(None);
        name.add_css_class("osd-name");
        let level = gtk::ProgressBar::new();
        level.add_css_class("osd-level");
        level.set_hexpand(true);
        level.set_valign(gtk::Align::Center);
        let value = gtk::Label::new(None);
        value.add_css_class("osd-value");
        value.set_xalign(1.0);
        row.append(&name);
        row.append(&level);
        row.append(&value);
        window.set_child(Some(&row));
        Osd { window, name, level, value }
    }

    pub fn show(&self, outcome: &Outcome) {
        self.name.set_label(outcome.name);
        match &outcome.reading {
            Reading::Level { percent, muted } => {
                self.name.set_visible(true);
                self.level.set_visible(true);
                self.level.set_fraction(f64::from(*percent).min(100.0) / 100.0);
                if *muted {
                    self.level.add_css_class("muted");
                    self.value.set_label("Muted");
                } else {
                    self.level.remove_css_class("muted");
                    self.value.set_label(&format!("{percent}%"));
                }
                self.value.remove_css_class("unavailable");
            }
            Reading::Unavailable(why) => {
                // The sentence names the control itself ("Brightness can’t be changed here").
                self.name.set_visible(false);
                self.level.set_visible(false);
                self.value.set_label(why);
                self.value.add_css_class("unavailable");
            }
        }
        self.window.present();
    }

    pub fn close(&self) {
        crate::motion::leave(&self.window);
    }
}

/// Keeps one OSD alive while keys are pressed, and drops it after it lingers.
#[derive(Default)]
pub struct OsdSlot {
    osd: std::cell::RefCell<Option<Osd>>,
    timer: std::rc::Rc<std::cell::RefCell<Option<glib::SourceId>>>,
}

impl OsdSlot {
    pub fn show(self: &std::rc::Rc<Self>, app: &gtk::Application, monitor: Option<&gdk::Monitor>, outcome: &Outcome) {
        if self.osd.borrow().is_none() {
            *self.osd.borrow_mut() = Some(Osd::new(app, monitor));
        }
        if let Some(osd) = self.osd.borrow().as_ref() {
            osd.show(outcome);
        }
        if let Some(old) = self.timer.borrow_mut().take() {
            old.remove();
        }
        let weak = std::rc::Rc::downgrade(self);
        let source = glib::timeout_add_local_once(std::time::Duration::from_millis(LINGER_MS), move || {
            if let Some(slot) = weak.upgrade() {
                slot.timer.borrow_mut().take();
                if let Some(osd) = slot.osd.borrow_mut().take() {
                    osd.close();
                }
            }
        });
        *self.timer.borrow_mut() = Some(source);
    }
}
