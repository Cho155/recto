//! A clock that wakes once a minute, just after the minute turns.

use gtk::glib;
use std::{rc::Rc, time::Duration};

pub fn now() -> glib::DateTime {
    glib::DateTime::now_local().expect("local time is available")
}

/// Call `tick` now and then at every minute boundary.
pub fn start(tick: impl Fn(&glib::DateTime) + 'static) {
    let tick: Rc<dyn Fn(&glib::DateTime)> = Rc::new(tick);
    tick(&now());
    schedule(tick);
}

fn schedule(tick: Rc<dyn Fn(&glib::DateTime)>) {
    let t = now();
    let into_minute_ms = t.second() as u64 * 1000 + t.microsecond() as u64 / 1000;
    // A little past the boundary, so the new minute is already there when we read it.
    let wait = Duration::from_millis(60_000 - into_minute_ms.min(59_999) + 50);
    glib::timeout_add_local_once(wait, move || {
        tick(&now());
        schedule(tick);
    });
}
