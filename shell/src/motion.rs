//! Motion: panels open with a short ease-out and leave the way they came.
//! The keyframes live in the stylesheet, rendered from the motion tokens. This
//! module only knows the timings, so that a closing panel can draw its last
//! frames before its window is destroyed.

use crate::tokens;
use gtk::{glib, prelude::*};
use gtk4_layer_shell::{KeyboardMode, LayerShell};
use std::time::Duration;

/// Arriving, in ms of animation.
pub fn long() -> u32 {
    tokens::get().duration_long
}

/// Leaving, in ms of animation.
pub fn short() -> u32 {
    tokens::get().duration_short
}

/// GTK's enable-animations setting; off means no motion at all.
pub fn enabled() -> bool {
    gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
}

/// GTK_SLOWDOWN stretches CSS animations for debugging; stretch our waits to match.
fn slowdown() -> f64 {
    std::env::var("GTK_SLOWDOWN").ok().and_then(|s| s.parse().ok()).filter(|f: &f64| *f > 0.0).unwrap_or(1.0)
}

/// How long a leaving panel needs in real time, with one frame to spare; 0 when animations are off.
pub fn leave_ms() -> u64 {
    if enabled() {
        (f64::from(short()) * slowdown()).round() as u64 + 20
    } else {
        0
    }
}

/// Run `f` after `ms`, or right away for 0.
pub fn after(ms: u64, f: impl FnOnce() + 'static) {
    if ms == 0 {
        f();
    } else {
        glib::timeout_add_local_once(Duration::from_millis(ms), f);
    }
}

/// Close a panel: it gives up the keyboard and stops taking clicks at once,
/// plays its leaving frames (`window.leaving` in the stylesheet), then goes.
pub fn leave(window: &gtk::Window) {
    window.set_keyboard_mode(KeyboardMode::None);
    if let Some(child) = window.child() {
        child.set_can_target(false);
    }
    window.add_css_class("leaving");
    let window = window.clone();
    after(leave_ms(), move || {
        window.destroy();
        glib::idle_add_local_once(crate::memory::give_back);
    });
}
