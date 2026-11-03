//! Search: one sheet near the top of the screen for apps and the shell's own
//! actions. Built when opened, destroyed when closed.

use crate::matching::{self, Candidate, Kind};
use gtk::{gdk, gio, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::{Cell, RefCell}, rc::Rc};

pub const LIMIT: usize = 7;

pub enum Event {
    Close,
    Open(Candidate),
}

pub type Handler = Rc<dyn Fn(Event)>;

pub struct Search {
    window: gtk::Window,
    entry: gtk::Entry,
    list: gtk::Box,
    candidates: Vec<Candidate>,
    shown: RefCell<Vec<Candidate>>,
    selected: Cell<usize>,
    handler: Handler,
}

/// Installed apps that want to be shown, as search candidates.
/// (Desktop-file keywords need gio-unix; not worth a dependency yet.)
pub fn apps() -> Vec<Candidate> {
    gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.should_show())
        .filter_map(|app| {
            Some(Candidate {
                key: app.id()?.to_string(),
                name: app.display_name().to_string(),
                detail: app.description().map(|d| d.to_string()).unwrap_or_default(),
                keywords: Vec::new(),
                kind: Kind::App,
            })
        })
        .collect()
}

/// Start an app by its desktop id.
pub fn launch(id: &str) {
    let Some(app) = gio::AppInfo::all().into_iter().find(|a| a.id().as_deref() == Some(id)) else {
        eprintln!("recto-shell: no app called {id}");
        return;
    };
    let context = gdk::Display::default().map(|d| d.app_launch_context());
    if let Err(e) = app.launch(&[], context.as_ref()) {
        eprintln!("recto-shell: cannot open {id}: {e}");
    }
}

fn later(handler: &Handler, event: Event) {
    let handler = handler.clone();
    glib::idle_add_local_once(move || handler(event));
}

impl Search {
    pub fn open(
        app: &gtk::Application,
        monitor: Option<&gdk::Monitor>,
        candidates: Vec<Candidate>,
        handler: Handler,
    ) -> Rc<Search> {
        let window = gtk::Window::builder().application(app).title("Search").build();
        window.add_css_class("recto-search");
        window.set_resizable(false); // follow the content as it shrinks, too
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        if let Some(monitor) = monitor {
            window.set_monitor(Some(monitor));
            // A fifth of the way down; .search adds 28 of shadow room above the sheet.
            window.set_margin(Edge::Top, (monitor.geometry().height() / 5 - 28).max(0));
        }
        window.set_anchor(Edge::Top, true);
        window.set_namespace(Some("recto-search"));
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sheet.add_css_class("search");
        sheet.set_size_request(640, -1);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        header.add_css_class("search-header");
        let label = gtk::Label::new(Some("Search"));
        label.add_css_class("search-label");
        let entry = gtk::Entry::builder()
            .placeholder_text("Apps and actions")
            .hexpand(true)
            .build();
        entry.add_css_class("search-entry");
        entry.update_property(&[gtk::accessible::Property::Label("Search apps and actions")]);
        header.append(&label);
        header.append(&entry);
        sheet.append(&header);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("search-results");
        sheet.append(&list);

        let footer = gtk::Label::builder()
            .label("Enter to open · ↑↓ to move · Esc to close")
            .xalign(0.0)
            .build();
        footer.add_css_class("search-footer");
        sheet.append(&footer);

        window.set_child(Some(&sheet));
        let search = Rc::new(Search {
            window,
            entry,
            list,
            candidates,
            shown: RefCell::default(),
            selected: Cell::new(0),
            handler,
        });

        let weak = Rc::downgrade(&search);
        search.entry.connect_changed(move |_| {
            if let Some(s) = weak.upgrade() {
                s.selected.set(0);
                s.refilter();
            }
        });
        let weak = Rc::downgrade(&search);
        search.entry.connect_activate(move |_| {
            if let Some(s) = weak.upgrade() {
                s.open_selected();
            }
        });

        // Arrows and Escape are handled before the entry sees them.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&search);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(s) = weak.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            match key {
                gdk::Key::Escape => later(&s.handler, Event::Close),
                gdk::Key::Down => s.move_selection(1),
                gdk::Key::Up => s.move_selection(-1),
                gdk::Key::n if ctrl => s.move_selection(1),
                gdk::Key::p if ctrl => s.move_selection(-1),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        search.window.add_controller(keys);

        search.refilter();
        search.window.present();
        search.entry.grab_focus();
        search
    }

    fn move_selection(&self, step: i32) {
        let count = self.shown.borrow().len() as i32;
        if count == 0 {
            return;
        }
        let next = (self.selected.get() as i32 + step).rem_euclid(count);
        self.selected.set(next as usize);
        self.render();
    }

    fn refilter(&self) {
        let query = self.entry.text();
        let ranked: Vec<Candidate> = matching::rank(&query, &self.candidates, LIMIT).into_iter().cloned().collect();
        *self.shown.borrow_mut() = ranked;
        self.render();
    }

    fn render(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let shown = self.shown.borrow();
        if shown.is_empty() {
            let none = gtk::Label::builder().label("Nothing matches.").xalign(0.0).build();
            none.add_css_class("search-empty");
            self.list.append(&none);
            return;
        }
        for (i, candidate) in shown.iter().enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("search-dot");
            dot.set_valign(gtk::Align::Center);
            row.append(&dot);
            let name = gtk::Label::new(Some(&candidate.name));
            name.add_css_class("search-name");
            row.append(&name);
            let detail = gtk::Label::builder()
                .label(&candidate.detail)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(30)
                .build();
            detail.add_css_class("search-detail");
            row.append(&detail);
            let kind = gtk::Label::new(Some(match candidate.kind {
                Kind::App => "App",
                Kind::Action => "Action",
            }));
            kind.add_css_class("search-kind");
            row.append(&kind);

            let button = gtk::Button::builder().child(&row).focusable(false).build();
            button.add_css_class("search-row");
            if i == self.selected.get() {
                button.add_css_class("selected");
            }
            let (handler, chosen) = (self.handler.clone(), candidate.clone());
            button.connect_clicked(move |_| later(&handler, Event::Open(chosen.clone())));
            self.list.append(&button);
        }
    }

    fn open_selected(&self) {
        let chosen = self.shown.borrow().get(self.selected.get()).cloned();
        if let Some(chosen) = chosen {
            later(&self.handler, Event::Open(chosen));
        }
    }

    /// Gives up the keyboard at once (so an app it opened gets it), then leaves.
    pub fn close(&self) {
        crate::motion::leave(&self.window);
    }
}
