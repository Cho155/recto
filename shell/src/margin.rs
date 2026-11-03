//! Notification popups, stacked in the right margin. One layer surface holds the
//! whole stack; it exists only while something is on screen.
//!
//! Each card sits in a revealer, its slot: a new card's slot opens and the ones
//! below glide down; a leaving card slides back to the edge while its slot closes.

use crate::{glyph::{self, Glyph}, inbox::{Note, Urgency}, motion, strings};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

pub enum Event {
    /// Clicked the note itself.
    Activate(u32),
    Action(u32, String),
    Dismiss(u32),
    Expire(u32),
}

type Handler = Rc<dyn Fn(Event)>;

struct Card {
    slot: gtk::Revealer,
    note: gtk::Box,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Card {
    fn stop(&self) {
        if let Some(source) = self.timer.borrow_mut().take() {
            source.remove();
        }
    }
}

pub struct Margin {
    window: gtk::Window,
    list: gtk::Box,
    cards: RefCell<HashMap<u32, Card>>,
    handler: Handler,
}

/// Run the handler after the current event, never from inside a widget's own signal.
fn later(handler: &Handler, event: Event) {
    let handler = handler.clone();
    glib::idle_add_local_once(move || handler(event));
}

fn start_timer(timer: &Rc<RefCell<Option<glib::SourceId>>>, handler: &Handler, id: u32, ms: u32) {
    if let Some(old) = timer.borrow_mut().take() {
        old.remove();
    }
    let slot = timer.clone();
    let handler = handler.clone();
    let source = glib::timeout_add_local_once(Duration::from_millis(ms.into()), move || {
        slot.borrow_mut().take(); // this source is finished; never remove it again
        handler(Event::Expire(id));
    });
    *timer.borrow_mut() = Some(source);
}

fn text(label: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder().label(label).xalign(0.0).wrap(true).build();
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    // Wrapping labels ask for their one-line width; cap it so cards stay 320 wide.
    l.set_max_width_chars(36);
    l.add_css_class(class);
    l
}

impl Margin {
    pub fn new(app: &gtk::Application, monitor: Option<&gdk::Monitor>, handler: Handler) -> Margin {
        let window = gtk::Window::builder().application(app).build();
        window.add_css_class("recto-margin");
        window.set_resizable(false); // follow the content as it shrinks, too
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        if let Some(monitor) = monitor {
            window.set_monitor(Some(monitor));
        }
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Right, true);
        window.set_namespace(Some("recto-notifications"));
        window.set_keyboard_mode(KeyboardMode::None);

        // No spacing: the gap below each card is part of its slot, so it closes with it.
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("margin");
        window.set_child(Some(&list));
        Margin { window, list, cards: RefCell::default(), handler }
    }

    /// Show a note, or update it in place when it is already on screen.
    pub fn show(&self, note: &Note, timeout: Option<u32>, now: i64) {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
        card.add_css_class("note");
        card.set_size_request(320, -1);
        if note.urgency == Urgency::Critical {
            card.add_css_class("critical");
        }

        // Urgent notes carry a clay rule down their edge (CSS), not a dot.
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("note-meta");
        let meta = gtk::Label::builder()
            .label(format!("{} · {}", note.app, strings::time_ago(now, note.arrived)))
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        header.append(&meta);
        let dismiss = gtk::Button::builder()
            .child(&glyph::new(Glyph::Close, 12))
            .tooltip_text(strings::DISMISS)
            .build();
        dismiss.add_css_class("note-dismiss");
        dismiss.update_property(&[gtk::accessible::Property::Label(strings::DISMISS)]);
        let (h, id) = (self.handler.clone(), note.id);
        dismiss.connect_clicked(move |_| later(&h, Event::Dismiss(id)));
        header.append(&dismiss);
        card.append(&header);

        if !note.summary.is_empty() {
            card.append(&text(&note.summary, "note-summary"));
        }
        if !note.body.is_empty() {
            let body = text(&note.body, "note-body");
            body.set_lines(3);
            body.set_ellipsize(gtk::pango::EllipsizeMode::End);
            card.append(&body);
        }

        let buttons: Vec<&(String, String)> = note.buttons().collect();
        if !buttons.is_empty() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            row.add_css_class("note-actions");
            for (key, label) in buttons {
                let button = gtk::Button::with_label(label);
                button.add_css_class("note-action");
                let (h, key) = (self.handler.clone(), key.clone());
                button.connect_clicked(move |_| later(&h, Event::Action(id, key.clone())));
                row.append(&button);
            }
            card.append(&row);
        }

        // A click on the note: its default action, or simply dismiss it.
        let click = gtk::GestureClick::new();
        let h = self.handler.clone();
        click.connect_released(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            later(&h, Event::Activate(id));
        });
        card.add_controller(click);

        // Hovering pauses the countdown; leaving gives a few more seconds.
        let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
        if let Some(ms) = timeout {
            start_timer(&timer, &self.handler, id, ms);
            let motion = gtk::EventControllerMotion::new();
            let t = timer.clone();
            motion.connect_enter(move |_, _, _| {
                if let Some(source) = t.borrow_mut().take() {
                    source.remove();
                }
            });
            let (t, h) = (timer.clone(), self.handler.clone());
            motion.connect_leave(move |_| start_timer(&t, &h, id, 3_000));
            card.add_controller(motion);
        }

        let slot = gtk::Revealer::builder()
            // SlideUp keeps the card in place while its slot opens or closes below it.
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(motion::long())
            .child(&card)
            .build();
        let old = self.cards.borrow_mut().remove(&id);
        match old {
            Some(old) => {
                // Updated in place: no motion.
                old.stop();
                slot.set_reveal_child(true);
                self.list.insert_child_after(&slot, Some(&old.slot));
                self.list.remove(&old.slot);
            }
            None => {
                card.add_css_class("arriving");
                self.list.prepend(&slot);
                // Opens at once while the window is not shown yet; glides when it is.
                slot.set_reveal_child(true);
            }
        }
        self.cards.borrow_mut().insert(id, Card { slot, note: card, timer });
        self.window.present();
    }

    /// Take a note off screen. Returns true when nothing is left.
    pub fn remove(&self, id: u32) -> bool {
        if let Some(card) = self.cards.borrow_mut().remove(&id) {
            card.stop();
            card.note.add_css_class("leaving");
            card.slot.set_can_target(false);
            card.slot.set_transition_duration(motion::short());
            card.slot.set_reveal_child(false);
            let (list, slot) = (self.list.clone(), card.slot);
            motion::after(motion::leave_ms(), move || {
                if slot.parent().as_ref() == Some(list.upcast_ref()) {
                    list.remove(&slot);
                }
            });
        }
        self.cards.borrow().is_empty()
    }

    /// Everything leaves; the window goes once the last card has.
    pub fn close(&self) {
        for card in self.cards.borrow_mut().drain().map(|(_, c)| c) {
            card.stop();
        }
        motion::leave(&self.window);
    }
}
