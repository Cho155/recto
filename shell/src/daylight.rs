//! Paper by day, Ink by night: the "Auto" theme mode.
//!
//! recto-theme keeps the mode in <data>/recto/mode and the switch times in
//! <data>/recto/shared/shell/auto (rendered from the tokens). The shell checks once
//! a minute and asks recto-theme to switch when the time comes.

use gtk::glib;
use std::path::PathBuf;

fn data() -> PathBuf {
    glib::user_data_dir().join("recto")
}

/// "paper", "ink" or "auto". Missing file means no automatic switching.
pub fn mode() -> Option<String> {
    let text = std::fs::read_to_string(data().join("mode")).ok()?;
    let mode = text.trim().to_owned();
    ["paper", "ink", "auto"].contains(&mode.as_str()).then_some(mode)
}

/// The mode, or the current theme when no mode was ever chosen.
pub fn effective_mode() -> Option<String> {
    mode().or_else(current_theme)
}

/// The theme in use, from the `current` link.
pub fn current_theme() -> Option<String> {
    let target = std::fs::read_link(data().join("current")).ok()?;
    target.file_name()?.to_str().map(str::to_owned)
}

fn parse_clock(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// (paper_from, ink_from) in minutes after midnight.
pub fn parse_times(text: &str) -> Option<(u32, u32)> {
    let mut paper = None;
    let mut ink = None;
    for line in text.lines() {
        match line.split_once('=') {
            Some(("paper_from", v)) => paper = parse_clock(v),
            Some(("ink_from", v)) => ink = parse_clock(v),
            _ => {}
        }
    }
    Some((paper?, ink?))
}

pub fn times() -> (u32, u32) {
    std::fs::read_to_string(data().join("shared/shell/auto"))
        .ok()
        .and_then(|t| parse_times(&t))
        .unwrap_or((7 * 60, 19 * 60))
}

/// Which theme the clock asks for at `minute` (minutes after midnight).
pub fn theme_at(minute: u32, (paper_from, ink_from): (u32, u32)) -> &'static str {
    let paper = if paper_from <= ink_from {
        minute >= paper_from && minute < ink_from
    } else {
        // Paper spans midnight (an unusual choice, but allowed).
        minute >= paper_from || minute < ink_from
    };
    if paper { "paper" } else { "ink" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_and_night() {
        let t = (7 * 60, 19 * 60);
        assert_eq!(theme_at(6 * 60 + 59, t), "ink");
        assert_eq!(theme_at(7 * 60, t), "paper");
        assert_eq!(theme_at(18 * 60 + 59, t), "paper");
        assert_eq!(theme_at(19 * 60, t), "ink");
    }

    #[test]
    fn paper_across_midnight() {
        let t = (22 * 60, 6 * 60);
        assert_eq!(theme_at(23 * 60, t), "paper");
        assert_eq!(theme_at(3 * 60, t), "paper");
        assert_eq!(theme_at(12 * 60, t), "ink");
    }

    #[test]
    fn reads_times() {
        assert_eq!(parse_times("paper_from=07:30\nink_from=18:45\n"), Some((450, 1125)));
        assert_eq!(parse_times("paper_from=25:00\nink_from=18:45"), None);
    }
}
