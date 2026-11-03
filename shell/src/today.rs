//! Today: a page for the day, laid over the whole desk below the bar, like
//! turning to a page in a book. Built when opened, destroyed when closed, so it
//! costs no memory the rest of the time. A click on the page outside the text,
//! or Escape, turns back.

use crate::{inbox::Inbox, mpris, state::Desktop, status::{self, Status}, strings};
use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, rc::Rc, time::Duration};

pub enum Event {
    Close,
    ToggleFocus,
    ClearAll,
    /// "paper", "ink" or "auto".
    SetTheme(&'static str),
}

const MODES: [(&str, &str); 3] = [("paper", "Paper"), ("ink", "Ink"), ("auto", "Auto")];

pub type Handler = Rc<dyn Fn(Event)>;

/// "Now playing": a title, an artist, and three words to control the player.
/// Hidden when nothing plays. It talks to the player itself (see mpris.rs).
struct NowPlaying {
    section: gtk::Box,
    title: gtk::Label,
    artist: gtk::Label,
    toggle: gtk::Button,
    player: RefCell<Option<String>>,
}

impl NowPlaying {
    fn new() -> Rc<Self> {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 2);
        section.add_css_class("now-playing");
        section.set_visible(false);
        section.append(&heading(strings::NOW_PLAYING));
        let title = label("", "track");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(40);
        let artist = label("", "artist");
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.set_max_width_chars(40);
        section.append(&title);
        section.append(&artist);

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        controls.add_css_class("controls");
        let toggle = gtk::Button::with_label(strings::PAUSE);
        let np = Rc::new(NowPlaying { section, title, artist, toggle: toggle.clone(), player: RefCell::default() });
        for (i, (text, method)) in [(strings::PREVIOUS, "Previous"), ("", "PlayPause"), (strings::NEXT, "Next")]
            .into_iter()
            .enumerate()
        {
            if i > 0 {
                let sep = gtk::Label::builder().label("·").accessible_role(gtk::AccessibleRole::Presentation).build();
                sep.add_css_class("segment-sep");
                controls.append(&sep);
            }
            let button = if method == "PlayPause" { toggle.clone() } else { gtk::Button::with_label(text) };
            button.add_css_class("word");
            let weak = Rc::downgrade(&np);
            button.connect_clicked(move |_| {
                let Some(np) = weak.upgrade() else { return };
                let Some(player) = np.player.borrow().clone() else { return };
                let weak = Rc::downgrade(&np);
                glib::spawn_future_local(async move {
                    mpris::control(&player, method).await;
                    // Give the player a moment to change track before reading it back.
                    glib::timeout_future(Duration::from_millis(250)).await;
                    if let Some(np) = weak.upgrade() {
                        np.refresh();
                    }
                });
            });
            controls.append(&button);
        }
        np.section.append(&controls);
        np
    }

    fn refresh(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let playing = mpris::current().await;
            if let Some(np) = weak.upgrade() {
                np.show(playing.as_ref());
            }
        });
    }

    fn show(&self, playing: Option<&mpris::Playing>) {
        *self.player.borrow_mut() = playing.map(|p| p.player.clone());
        let Some(p) = playing else {
            self.section.set_visible(false);
            return;
        };
        self.title.set_label(&p.title);
        self.artist.set_label(p.artist.as_deref().unwrap_or(""));
        self.artist.set_visible(p.artist.is_some());
        self.toggle.set_label(if p.playing { strings::PAUSE } else { strings::PLAY });
        self.section.set_visible(true);
    }
}

pub struct Today {
    window: gtk::Window,
    eyebrow: gtk::Label,
    date: gtk::Label,
    focus: gtk::Button,
    modes: Vec<(&'static str, gtk::Button)>,
    now: gtk::Label,
    notes: gtk::Box,
    playing: Rc<NowPlaying>,
    clear: gtk::Button,
}

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder().label(text).xalign(0.0).build();
    l.add_css_class(class);
    l
}

fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .hexpand(true)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    l.add_css_class("section-title");
    l
}

fn later(handler: &Handler, event: Event) {
    let handler = handler.clone();
    glib::idle_add_local_once(move || handler(event));
}

impl Today {
    pub fn open(app: &gtk::Application, monitor: Option<&gdk::Monitor>, handler: Handler) -> Self {
        let window = gtk::Window::builder().application(app).title(strings::TODAY_TITLE).build();
        window.add_css_class("liubai-today");
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        if let Some(monitor) = monitor {
            window.set_monitor(Some(monitor));
        }
        // Every edge: the page covers the desk. The bar's exclusive zone keeps the bar in view.
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
        window.set_namespace(Some("liubai-today"));
        // Modal while open: Escape closes it, and typing never leaks to the window behind.
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        // The text block, centered on the page.
        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sheet.add_css_class("today");
        sheet.set_size_request(880, -1);
        sheet.set_halign(gtk::Align::Center);
        sheet.set_valign(gtk::Align::Center);
        // A click on the open page around the text turns back; clicks on the text
        // (and its buttons) are left alone. Decided by where the click lands, not by
        // claiming it, which would take the click away from the buttons.
        let away = gtk::GestureClick::new();
        let (h, w, text) = (handler.clone(), window.clone(), sheet.clone());
        away.connect_released(move |_, _, x, y| {
            let inside = text
                .compute_bounds(&w)
                .is_some_and(|r| r.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)));
            if !inside {
                later(&h, Event::Close);
            }
        });
        window.add_controller(away);

        // Header: the date, and the focus switch.
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
        titles.set_hexpand(true);
        let eyebrow = label("", "eyebrow");
        let date = label("", "today-date");
        titles.append(&eyebrow);
        titles.append(&date);
        let focus = gtk::Button::with_label(strings::START_FOCUS);
        focus.add_css_class("today-button");
        focus.set_valign(gtk::Align::End);
        let h = handler.clone();
        focus.connect_clicked(move |_| later(&h, Event::ToggleFocus));
        // Paper / Ink / Auto
        let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segments.add_css_class("segments");
        segments.set_valign(gtk::Align::End);
        segments.update_property(&[gtk::accessible::Property::Label("Theme")]);
        let mut modes = Vec::new();
        for (i, (mode, label)) in MODES.into_iter().enumerate() {
            if i > 0 {
                let sep = gtk::Label::builder().label("·").accessible_role(gtk::AccessibleRole::Presentation).build();
                sep.add_css_class("segment-sep");
                segments.append(&sep);
            }
            let button = gtk::Button::with_label(label);
            button.add_css_class("segment");
            let h = handler.clone();
            button.connect_clicked(move |_| later(&h, Event::SetTheme(mode)));
            segments.append(&button);
            modes.push((mode, button));
        }
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.append(&titles);
        header.append(&segments);
        header.append(&focus);
        sheet.append(&header);

        let rule = gtk::Box::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
        rule.add_css_class("rule");
        sheet.append(&rule);

        // Two columns: right now, and notifications.
        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 56);
        columns.set_homogeneous(true);

        let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
        left.append(&heading(strings::RIGHT_NOW));
        let now = label("", "body");
        now.set_wrap(true);
        // Wrapping labels ask for their one-line width; cap it so the sheet keeps its size.
        now.set_max_width_chars(44);
        left.append(&now);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let notes_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        notes_header.append(&heading(strings::NOTIFICATIONS));
        let clear = gtk::Button::with_label(strings::CLEAR_ALL);
        clear.add_css_class("today-link");
        clear.set_valign(gtk::Align::Start);
        let h = handler.clone();
        clear.connect_clicked(move |_| later(&h, Event::ClearAll));
        notes_header.append(&clear);
        right.append(&notes_header);
        let notes = gtk::Box::new(gtk::Orientation::Vertical, 0);
        right.append(&notes);
        let playing = NowPlaying::new();
        right.append(&playing.section);

        columns.append(&left);
        columns.append(&right);
        sheet.append(&columns);

        let keys = gtk::EventControllerKey::new();
        let h = handler.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                later(&h, Event::Close);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);

        window.set_child(Some(&sheet));
        window.present();
        Today { window, eyebrow, date, focus, modes, now, notes, playing, clear }
    }

    pub fn update(&self, desktop: &Desktop, now: &glib::DateTime, inbox: &Inbox, system: &Status, mode: Option<&str>) {
        // Read the player again on every update (at least once a minute while open).
        self.playing.refresh();
        for (name, button) in &self.modes {
            if Some(*name) == mode {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }
        }
        self.eyebrow.set_label(&strings::today_eyebrow(now));
        self.date.set_label(&strings::today_date(now));
        self.focus.set_label(if inbox.focus_until.is_some() { strings::END_FOCUS } else { strings::START_FOCUS });

        let mut sentence = status::right_now(system);
        if !sentence.is_empty() {
            sentence.push(' ');
        }
        sentence.push_str(&strings::desktop_sentence(desktop));
        self.now.set_label(&sentence);

        while let Some(child) = self.notes.first_child() {
            self.notes.remove(&child);
        }
        let recent = inbox.recent(5);
        self.clear.set_visible(!recent.is_empty());
        if recent.is_empty() {
            self.notes.append(&label(strings::NOTHING_NEW, "body-quiet"));
        }
        let at = now.to_unix();
        for note in recent {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
            row.add_css_class("note-row");
            row.append(&label(&format!("{} · {}", note.app, strings::time_ago(at, note.arrived)), "row-meta"));
            let summary = label(if note.summary.is_empty() { &note.body } else { &note.summary }, "row-summary");
            summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
            summary.set_max_width_chars(40);
            row.append(&summary);
            self.notes.append(&row);
        }
    }

    /// Fold the sheet away; the window is destroyed once it has left.
    pub fn close(&self) {
        crate::motion::leave(&self.window);
    }
}
