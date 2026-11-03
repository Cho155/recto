//! The shell's look comes from a stylesheet that liubai-theme renders from the design tokens.
//! It lives at <data dir>/liubai/current/shell/style.css, so it follows Paper and Ink.

use gtk::{gdk, glib};
use std::{cell::RefCell, path::PathBuf};

thread_local! {
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

pub fn stylesheet() -> PathBuf {
    std::env::var_os("LIUBAI_STYLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::user_data_dir().join("liubai/current/shell/style.css"))
}

/// Load (or reload) the stylesheet for the whole display, and the shell's code tokens with it.
pub fn load() {
    crate::tokens::load();
    let Some(display) = gdk::Display::default() else { return };
    let path = stylesheet();
    if !path.exists() {
        eprintln!("liubai-shell: no stylesheet at {}; run `liubai-theme install`", path.display());
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_path(&path);
    PROVIDER.with(|slot| {
        if let Some(old) = slot.borrow_mut().take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *slot.borrow_mut() = Some(provider);
    });
}
