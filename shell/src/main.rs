//! recto-shell: the bar, Today, notifications and the OSD, in one process.
//!
//!   recto-shell                          start (does nothing if already running)
//!   recto-shell toggle-today             open or close Today
//!   recto-shell toggle-search            open or close Search
//!   recto-shell volume up|down|mute      change the volume and show it
//!   recto-shell mic-mute                 mute or unmute the microphone
//!   recto-shell brightness up|down       change the screen brightness
//!   recto-shell focus [minutes|off]      hold notifications for a while
//!   recto-shell reload-style             re-read the stylesheet after a theme switch
//!   recto-shell quit
//!
//! A second invocation hands its command to the running shell over D-Bus
//! (GApplication uniqueness), so niri key bindings can call it directly.

mod bar;
mod clock;
mod contents;
mod controls;
mod daylight;
mod dbusmenu;
mod glyph;
mod ime;
mod inbox;
mod margin;
mod matching;
mod memory;
mod mic;
mod motion;
mod mpris;
mod niri;
mod notify;
mod osd;
mod search;
mod sni;
mod state;
mod status;
mod strings;
mod style;
mod system;
mod today;
mod tokens;
mod tray;

use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

const APP_ID: &str = "org.recto.Shell";
const USAGE: &str = "usage: recto-shell [toggle-today | toggle-search | volume up|down|mute | mic-mute | \
                     brightness up|down | focus [minutes|off] | reload-style | quit]";
const DEFAULT_FOCUS_MINUTES: i64 = 60;

struct Shell {
    app: gtk::Application,
    started: Cell<bool>,
    desktop: RefCell<state::Desktop>,
    inbox: RefCell<inbox::Inbox>,
    status: RefCell<status::Status>,
    bars: RefCell<Vec<bar::Bar>>,
    today: RefCell<Option<today::Today>>,
    margin: RefCell<Option<margin::Margin>>,
    osd: Rc<osd::OsdSlot>,
    server: RefCell<Option<Rc<notify::Server>>>,
    tray: RefCell<Option<Rc<tray::Tray>>>,
    input_method: RefCell<Option<String>>,
    search: RefCell<Option<Rc<search::Search>>>,
    /// The theme Auto mode last asked for, so it asks only once per change.
    auto_pending: RefCell<Option<String>>,
    focus_timer: RefCell<Option<glib::SourceId>>,
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
}

/// Wrap a method so a callback holds only a weak reference to the shell.
fn weak<A: 'static>(
    shell: &Rc<Shell>,
    f: impl Fn(&Rc<Shell>, A) + Clone + 'static,
) -> impl Fn(A) + Clone + 'static {
    let weak: Weak<Shell> = Rc::downgrade(shell);
    move |arg| {
        if let Some(shell) = weak.upgrade() {
            f(&shell, arg);
        }
    }
}

fn unix_now() -> i64 {
    clock::now().to_unix()
}

fn theme_tool() -> String {
    glib::user_data_dir().join("recto/bin/recto-theme").to_string_lossy().into_owned()
}

/// Start a helper and let it run; GIO reaps it when it exits.
fn spawn(argv: &[&str]) {
    let args: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
    if let Err(e) = gio::Subprocess::newv(&args, gio::SubprocessFlags::NONE) {
        eprintln!("recto-shell: cannot run {}: {e}", argv[0]);
    }
}

impl Shell {
    fn start(self: &Rc<Self>) {
        if self.started.replace(true) {
            return;
        }
        // Stay alive even when no window is open (for example, no monitors yet).
        *self.hold.borrow_mut() = Some(self.app.hold());
        style::load();
        self.rebuild_bars();

        if let Some(display) = gdk::Display::default() {
            let rebuild = weak(self, |s, ()| s.rebuild_bars());
            display.monitors().connect_items_changed(move |_, _, _, _| rebuild(()));
        }

        let w = Rc::downgrade(self);
        clock::start(move |now| {
            if let Some(shell) = w.upgrade() {
                shell.tick(now);
            }
        });
        niri::follow_events(weak(self, |s, event: serde_json::Value| s.on_niri(&event)));
        system::watch(weak(self, |s, update: system::Update| s.on_system(update)));

        let server = notify::Server::start(
            {
                let w = Rc::downgrade(self);
                move |incoming| w.upgrade().map_or(0, |s| s.notify(incoming))
            },
            weak(self, |s, id: u32| s.withdraw(id)),
        );
        *self.server.borrow_mut() = Some(server);

        let w = Rc::downgrade(self);
        let tray = tray::Tray::start(move || {
            if let Some(shell) = w.upgrade() {
                shell.on_tray();
            }
        });
        *self.tray.borrow_mut() = Some(tray);
        mic::watch(weak(self, |s, in_use: bool| s.on_mic(in_use)));

        let w = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let muted = controls::read_output_muted().await;
            if let Some(shell) = w.upgrade() {
                shell.status.borrow_mut().muted = muted;
                shell.refresh_notices();
            }
        });
    }

    fn monitors() -> Vec<gdk::Monitor> {
        let Some(display) = gdk::Display::default() else { return Vec::new() };
        let list = display.monitors();
        (0..list.n_items())
            .filter_map(|i| list.item(i).and_downcast::<gdk::Monitor>())
            .collect()
    }

    /// The monitor showing the focused workspace, so popups appear where you are looking.
    fn focused_monitor(&self) -> Option<gdk::Monitor> {
        let output = self.desktop.borrow().focused_workspace()?.output.clone()?;
        Self::monitors()
            .into_iter()
            .find(|m| m.connector().as_deref() == Some(output.as_str()))
    }

    // ── Bar ──────────────────────────────────────────────────────────────

    /// One bar per monitor, rebuilt whenever monitors come or go.
    fn rebuild_bars(self: &Rc<Self>) {
        for bar in self.bars.borrow_mut().drain(..) {
            bar.close();
        }
        let now = clock::now();
        let notices = self.notices();
        let handler: bar::Handler = Rc::new(weak(self, |s, event: bar::Event| match event {
            bar::Event::OpenToday => s.toggle_today(),
            bar::Event::EndFocus => s.set_focus(None),
            bar::Event::ToggleInputMethod => s.toggle_input_method(),
            bar::Event::ToggleTray(anchor) => {
                let tray = s.tray.borrow().clone();
                if let Some(tray) = tray {
                    tray.toggle_panel(&anchor);
                }
            }
        }));
        let bars: Vec<bar::Bar> = Self::monitors()
            .iter()
            .map(|monitor| {
                let bar = bar::Bar::new(&self.app, monitor, handler.clone());
                bar.update_clock(&now);
                bar.update_desktop(&self.desktop.borrow());
                bar.update_notices(&notices);
                bar
            })
            .collect();
        *self.bars.borrow_mut() = bars;
    }

    fn notices(&self) -> bar::Notices {
        let inbox = self.inbox.borrow();
        let focus = inbox.focus_until.and_then(|until| {
            let dt = glib::DateTime::from_unix_local(until).ok()?;
            Some((strings::focus_until(&dt), inbox.held))
        });
        let tray = self.tray.borrow().clone();
        bar::Notices {
            alerts: status::alerts(&self.status.borrow()),
            focus,
            unread: inbox.unread(),
            input_method: self.input_method.borrow().clone(),
            tray_items: tray.map_or(0, |t| t.listed().len()),
        }
    }

    fn refresh_notices(&self) {
        let notices = self.notices();
        for bar in self.bars.borrow().iter() {
            bar.update_notices(&notices);
        }
        self.refresh_today();
    }

    fn refresh_today(&self) {
        if let Some(today) = self.today.borrow().as_ref() {
            let mode = daylight::effective_mode();
            today.update(&self.desktop.borrow(), &clock::now(), &self.inbox.borrow(), &self.status.borrow(), mode.as_deref());
        }
    }

    fn on_niri(&self, event: &serde_json::Value) {
        if self.desktop.borrow_mut().apply(event) {
            let desktop = self.desktop.borrow();
            for bar in self.bars.borrow().iter() {
                bar.update_desktop(&desktop);
            }
            drop(desktop);
            self.refresh_today();
        }
    }

    fn on_system(&self, update: system::Update) {
        if std::env::var_os("RECTO_DEBUG").is_some() {
            eprintln!("recto-shell: system {update:?}");
        }
        let changed = {
            let mut status = self.status.borrow_mut();
            let before = status.clone();
            match update {
                system::Update::Battery(battery) => status.battery = battery,
                system::Update::Network { online, name, wired } => {
                    status.online = online;
                    status.network_name = name;
                    status.network_wired = wired;
                }
            }
            *status != before
        };
        if changed {
            self.refresh_notices();
        }
    }

    fn on_mic(&self, in_use: bool) {
        let changed = self.status.borrow().mic_in_use != Some(in_use);
        self.status.borrow_mut().mic_in_use = Some(in_use);
        if changed {
            self.refresh_notices();
        }
    }

    // ── Tray and input method ────────────────────────────────────────────

    fn on_tray(self: &Rc<Self>) {
        self.refresh_notices();
        self.refresh_input_method();
    }

    /// The input method's tray item changes whenever the input method does; read it again.
    fn refresh_input_method(self: &Rc<Self>) {
        let tray = self.tray.borrow().clone();
        let connection = tray.as_ref().filter(|t| t.has_input_method()).and_then(|t| t.connection());
        let Some(connection) = connection else {
            if self.input_method.borrow_mut().take().is_some() {
                self.refresh_notices();
            }
            return;
        };
        let w = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let label = ime::current(&connection).await;
            let Some(shell) = w.upgrade() else { return };
            if *shell.input_method.borrow() != label {
                *shell.input_method.borrow_mut() = label;
                shell.refresh_notices();
            }
        });
    }

    fn toggle_input_method(self: &Rc<Self>) {
        let connection = self.tray.borrow().as_ref().and_then(|t| t.connection());
        if let Some(connection) = connection {
            ime::toggle(&connection);
            let refresh = weak(self, |s, ()| s.refresh_input_method());
            glib::timeout_add_local_once(Duration::from_millis(200), move || refresh(()));
        }
    }

    fn tick(&self, now: &glib::DateTime) {
        for bar in self.bars.borrow().iter() {
            bar.update_clock(now);
        }
        self.refresh_today();
        self.follow_daylight(now);
    }

    /// In Auto mode, switch between Paper and Ink when the clock says so.
    fn follow_daylight(&self, now: &glib::DateTime) {
        if daylight::mode().as_deref() != Some("auto") {
            return;
        }
        let minute = (now.hour() * 60 + now.minute()) as u32;
        let want = daylight::theme_at(minute, daylight::times());
        let pending = self.auto_pending.borrow().clone();
        if daylight::current_theme().as_deref() != Some(want) && pending.as_deref() != Some(want) {
            *self.auto_pending.borrow_mut() = Some(want.to_owned());
            spawn(&[&theme_tool(), "switch", "auto"]);
        }
    }

    fn set_theme(self: &Rc<Self>, mode: &str) {
        self.auto_pending.borrow_mut().take();
        spawn(&[&theme_tool(), "switch", mode]);
        // recto-theme restyles the shell when it is done; update Today's switch then.
        let refresh = weak(self, |s, ()| s.refresh_today());
        glib::timeout_add_local_once(Duration::from_millis(1_200), move || refresh(()));
    }

    // ── Search ───────────────────────────────────────────────────────────

    fn actions(&self) -> Vec<matching::Candidate> {
        let mode = daylight::effective_mode();
        let focusing = self.inbox.borrow().focus_until.is_some();
        let mut list: Vec<(&str, &str, &str, &[&str])> = Vec::new();
        if mode.as_deref() != Some("paper") {
            list.push(("paper", "Switch to Paper", "Light theme", &["theme", "light", "color", "day"]));
        }
        if mode.as_deref() != Some("ink") {
            list.push(("ink", "Switch to Ink", "Dark theme", &["theme", "dark", "color", "night"]));
        }
        if mode.as_deref() != Some("auto") {
            list.push(("auto", "Follow the time of day", "Paper by day, Ink at night", &["theme", "color", "auto"]));
        }
        if focusing {
            list.push(("end-focus", "End focus", "Show notifications again", &["notifications", "dnd"]));
        } else {
            list.push(("focus", "Start focus", "Hold notifications for an hour", &["do not disturb", "notifications", "dnd"]));
        }
        list.push(("lock", "Lock screen", "", &["lock", "away"]));
        list.into_iter()
            .map(|(key, name, detail, keywords)| matching::Candidate {
                key: key.to_owned(),
                name: name.to_owned(),
                detail: detail.to_owned(),
                keywords: keywords.iter().map(|k| k.to_string()).collect(),
                kind: matching::Kind::Action,
            })
            .collect()
    }

    fn toggle_search(self: &Rc<Self>) {
        let open = self.search.borrow_mut().take();
        if let Some(open) = open {
            open.close();
            return;
        }
        self.close_today();
        let mut candidates = self.actions();
        candidates.extend(search::apps());
        let handler: search::Handler = Rc::new(weak(self, |s, event: search::Event| {
            s.close_search();
            if let search::Event::Open(chosen) = event {
                s.open_candidate(&chosen);
            }
        }));
        let search = search::Search::open(&self.app, self.focused_monitor().as_ref(), candidates, handler);
        *self.search.borrow_mut() = Some(search);
    }

    fn close_search(&self) {
        let open = self.search.borrow_mut().take();
        if let Some(open) = open {
            open.close();
        }
    }

    fn open_candidate(self: &Rc<Self>, chosen: &matching::Candidate) {
        match (chosen.kind, chosen.key.as_str()) {
            (matching::Kind::App, id) => search::launch(id),
            (_, "paper" | "ink" | "auto") => self.set_theme(&chosen.key),
            (_, "focus") => self.set_focus(Some(DEFAULT_FOCUS_MINUTES)),
            (_, "end-focus") => self.set_focus(None),
            (_, "lock") => spawn(&[&theme_tool(), "lock"]),
            _ => {}
        }
    }

    // ── Today ────────────────────────────────────────────────────────────

    fn toggle_today(self: &Rc<Self>) {
        if let Some(open) = self.today.borrow_mut().take() {
            open.close();
            return;
        }
        let handler: today::Handler = Rc::new(weak(self, |s, event: today::Event| match event {
            today::Event::Close => s.close_today(),
            today::Event::ToggleFocus => {
                let focusing = s.inbox.borrow().focus_until.is_some();
                s.set_focus(if focusing { None } else { Some(DEFAULT_FOCUS_MINUTES) });
            }
            today::Event::ClearAll => {
                s.inbox.borrow_mut().clear();
                s.refresh_notices();
            }
            today::Event::SetTheme(mode) => s.set_theme(mode),
        }));
        self.close_search();
        let today = today::Today::open(&self.app, self.focused_monitor().as_ref(), handler);
        *self.today.borrow_mut() = Some(today);
        // Opening Today is reading what is there.
        self.inbox.borrow_mut().mark_all_read();
        self.refresh_notices();
    }

    fn close_today(&self) {
        if let Some(open) = self.today.borrow_mut().take() {
            open.close();
        }
    }

    // ── Notifications ────────────────────────────────────────────────────

    fn notify(self: &Rc<Self>, incoming: notify::Incoming) -> u32 {
        let urgency = inbox::Urgency::from_hint(incoming.urgency);
        let note = inbox::Note {
            id: 0,
            app: if incoming.app.is_empty() { "Notification".into() } else { incoming.app },
            summary: inbox::plain_text(&incoming.summary),
            body: inbox::plain_text(&incoming.body),
            actions: inbox::pair_actions(&incoming.actions),
            urgency,
            arrived: unix_now(),
            read: false,
        };
        let added = self.inbox.borrow_mut().add(note, incoming.replaces);

        for old in &added.evicted {
            self.take_off_screen(*old);
        }
        if added.popup {
            if self.margin.borrow().is_none() {
                let handler = Rc::new(weak(self, |s, event: margin::Event| s.on_margin(event)));
                *self.margin.borrow_mut() =
                    Some(margin::Margin::new(&self.app, self.focused_monitor().as_ref(), handler));
            }
            let inbox = self.inbox.borrow();
            if let (Some(margin), Some(note)) = (self.margin.borrow().as_ref(), inbox.get(added.id)) {
                margin.show(note, inbox::timeout_ms(urgency, incoming.timeout), unix_now());
            }
        } else {
            self.take_off_screen(added.id);
        }
        self.refresh_notices();
        added.id
    }

    fn on_margin(&self, event: margin::Event) {
        let server = self.server.borrow().clone();
        let (id, why) = match event {
            margin::Event::Activate(id) => {
                let has_default = self.inbox.borrow().get(id).is_some_and(|n| n.has_default_action());
                if has_default {
                    if let Some(s) = &server {
                        s.action_invoked(id, "default");
                    }
                }
                self.inbox.borrow_mut().dismiss(id);
                (id, notify::Closed::Dismissed)
            }
            margin::Event::Action(id, key) => {
                if let Some(s) = &server {
                    s.action_invoked(id, &key);
                }
                self.inbox.borrow_mut().dismiss(id);
                (id, notify::Closed::Dismissed)
            }
            margin::Event::Dismiss(id) => {
                self.inbox.borrow_mut().dismiss(id);
                (id, notify::Closed::Dismissed)
            }
            margin::Event::Expire(id) => {
                self.inbox.borrow_mut().expire(id);
                (id, notify::Closed::Expired)
            }
        };
        if let Some(s) = &server {
            s.closed(id, why);
        }
        self.take_off_screen(id);
        self.refresh_notices();
    }

    /// The app withdrew a notification.
    fn withdraw(&self, id: u32) {
        if self.inbox.borrow_mut().withdraw(id) {
            if let Some(s) = self.server.borrow().as_ref() {
                s.closed(id, notify::Closed::ByApp);
            }
            self.take_off_screen(id);
            self.refresh_notices();
        }
    }

    fn take_off_screen(&self, id: u32) {
        let empty = self.margin.borrow().as_ref().is_some_and(|m| m.remove(id));
        if empty {
            if let Some(margin) = self.margin.borrow_mut().take() {
                margin.close();
            }
        }
    }

    /// Start focus for some minutes, or end it with `None`.
    fn set_focus(self: &Rc<Self>, minutes: Option<i64>) {
        if let Some(source) = self.focus_timer.borrow_mut().take() {
            source.remove();
        }
        match minutes {
            Some(m) => {
                let m = m.clamp(1, 24 * 60);
                self.inbox.borrow_mut().start_focus(unix_now() + m * 60);
                let end = weak(self, |s, ()| {
                    s.focus_timer.borrow_mut().take();
                    s.set_focus(None);
                });
                let source = glib::timeout_add_local_once(Duration::from_secs(m as u64 * 60), move || end(()));
                *self.focus_timer.borrow_mut() = Some(source);
            }
            None => self.inbox.borrow_mut().end_focus(),
        }
        self.refresh_notices();
    }

    // ── Volume, microphone, brightness ──────────────────────────────────

    fn control(self: &Rc<Self>, control: controls::Control) {
        let w = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let outcome = controls::apply(control).await;
            let Some(shell) = w.upgrade() else { return };
            shell.osd.show(&shell.app, shell.focused_monitor().as_ref(), &outcome);
            if let Some(muted) = outcome.output_muted {
                shell.status.borrow_mut().muted = Some(muted);
                shell.refresh_notices();
            }
        });
    }

    // ── Commands ─────────────────────────────────────────────────────────

    fn command(self: &Rc<Self>, args: &[String]) -> glib::ExitCode {
        if let Some(control) = controls::Control::parse(args) {
            self.start();
            self.control(control);
            return glib::ExitCode::SUCCESS;
        }
        match args.first().map(String::as_str) {
            None | Some("run") => self.start(),
            Some("toggle-today") => {
                self.start();
                self.toggle_today();
            }
            Some("toggle-search") => {
                self.start();
                self.toggle_search();
            }
            Some("focus") => {
                self.start();
                match args.get(1).map(String::as_str) {
                    Some("off") => self.set_focus(None),
                    Some(n) => match n.parse::<i64>() {
                        Ok(minutes) => self.set_focus(Some(minutes)),
                        Err(_) => {
                            eprintln!("recto-shell: focus takes minutes or 'off'\n{USAGE}");
                            return glib::ExitCode::FAILURE;
                        }
                    },
                    None => self.set_focus(Some(DEFAULT_FOCUS_MINUTES)),
                }
            }
            // Only a running shell has anything to restyle; never start one for this.
            Some("reload-style") => {
                if self.started.get() {
                    style::load();
                }
            }
            Some("quit") => self.app.quit(),
            Some(other) => {
                eprintln!("recto-shell: unknown command '{other}'\n{USAGE}");
                return glib::ExitCode::FAILURE;
            }
        }
        glib::ExitCode::SUCCESS
    }
}

fn main() -> glib::ExitCode {
    // The bar and Today barely redraw, so software drawing is enough, and it keeps
    // the GPU driver out of this process: about 24 MiB less PSS in our measurements
    // (reports/memory/2026-11-02-vm). An explicit GSK_RENDERER still wins.
    if std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "cairo");
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let shell = Rc::new(Shell {
        app: app.clone(),
        started: Cell::new(false),
        desktop: RefCell::default(),
        inbox: RefCell::default(),
        status: RefCell::default(),
        bars: RefCell::default(),
        today: RefCell::default(),
        margin: RefCell::default(),
        osd: Rc::default(),
        server: RefCell::default(),
        tray: RefCell::default(),
        input_method: RefCell::default(),
        search: RefCell::default(),
        auto_pending: RefCell::default(),
        focus_timer: RefCell::default(),
        hold: RefCell::default(),
    });
    // Child processes end with the shell.
    app.connect_shutdown(|_| mic::stop());
    app.connect_command_line(move |_, cmdline| {
        let args: Vec<String> = cmdline
            .arguments()
            .iter()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        shell.command(&args)
    });
    app.run()
}
