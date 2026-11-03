//! Every piece of interface text lives here, so it can be translated in one place.
//! The interface is English by default.

use crate::state::{Desktop, Workspace};
use gtk::glib::DateTime;

const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

fn weekday(dt: &DateTime) -> &'static str {
    WEEKDAYS[(dt.day_of_week().clamp(1, 7) - 1) as usize]
}

fn month(dt: &DateTime) -> &'static str {
    MONTHS[(dt.month().clamp(1, 12) - 1) as usize]
}

/// "Monday, November 2": written out, like a running head, not abbreviated.
pub fn bar_date(dt: &DateTime) -> String {
    format!("{}, {} {}", weekday(dt), month(dt), dt.day_of_month())
}

/// "14:30"
pub fn clock(dt: &DateTime) -> String {
    format!("{:02}:{:02}", dt.hour(), dt.minute())
}

/// "Monday · 2026"
pub fn today_eyebrow(dt: &DateTime) -> String {
    format!("{} · {}", weekday(dt), dt.year())
}

/// "November 2"
pub fn today_date(dt: &DateTime) -> String {
    format!("{} {}", month(dt), dt.day_of_month())
}

pub const RIGHT_NOW: &str = "Right now";
pub const TODAY_TITLE: &str = "Today";
pub const OPEN_TODAY: &str = "Open Today";
pub const NOTIFICATIONS: &str = "Notifications";
pub const NOTHING_NEW: &str = "Nothing new.";
pub const CLEAR_ALL: &str = "Clear all";
pub const START_FOCUS: &str = "Start focus";
pub const END_FOCUS: &str = "End focus";
pub const DISMISS: &str = "Dismiss";
pub const NOW_PLAYING: &str = "Now playing";
pub const PREVIOUS: &str = "Previous";
pub const PAUSE: &str = "Pause";
pub const PLAY: &str = "Play";
pub const NEXT: &str = "Next";

/// Roman numerals, numbered like chapters: 4 -> "IV". Zero and very large numbers stay digits.
pub fn roman(n: u64) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    const STEPS: [(u64, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let (mut left, mut out) = (n, String::new());
    for (value, letters) in STEPS {
        while left >= value {
            out.push_str(letters);
            left -= value;
        }
    }
    out
}

/// The name shown in the bar: the workspace's own name, or its number as a chapter numeral.
pub fn workspace_label(ws: &Workspace) -> String {
    ws.name.clone().unwrap_or_else(|| roman(ws.idx))
}

/// "Workspace: Writing, 1 of 3" for screen readers (plain digits: "II" would be read as letters).
pub fn workspace_a11y(ws: &Workspace, position: usize, total: usize) -> String {
    let name = ws.name.clone().unwrap_or_else(|| ws.idx.to_string());
    format!("Workspace: {name}, {position} of {total}. Open the overview")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "2 unread notifications"
pub fn unread(n: usize) -> String {
    plural(n, "unread notification", "unread notifications")
}

/// "Focus until 16:00"
pub fn focus_until(until: &DateTime) -> String {
    format!("Focus until {}", clock(until))
}

/// "3 held"
pub fn held(n: usize) -> String {
    format!("{n} held")
}

/// "now", "5 min ago", "2 h ago", then the time of day.
pub fn time_ago(now: i64, then: i64) -> String {
    let secs = (now - then).max(0);
    match secs {
        0..=59 => "now".to_owned(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=43_199 => format!("{} h ago", secs / 3600),
        _ => DateTime::from_unix_local(then).map(|dt| clock(&dt)).unwrap_or_default(),
    }
}

/// The windows half of Today's "Right now".
pub fn desktop_sentence(desktop: &Desktop) -> String {
    if !desktop.connected {
        return "Waiting for niri.".to_owned();
    }
    let windows = desktop.window_count();
    if windows == 0 {
        return "No windows open. A clean desk.".to_owned();
    }
    let spaces = desktop.occupied_workspace_count().max(1);
    let mut text = if spaces == 1 {
        format!("{} open on one workspace.", plural(windows, "window", "windows"))
    } else {
        format!("{} open across {} workspaces.", plural(windows, "window", "windows"), spaces)
    };
    if let Some(title) = desktop.focused_window().and_then(|w| w.title.as_deref()).filter(|t| !t.is_empty()) {
        text.push_str(&format!(" You’re in “{title}”."));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::roman;

    #[test]
    fn chapter_numerals() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(14), "XIV");
        assert_eq!(roman(2026), "MMXXVI");
        assert_eq!(roman(0), "0");
        assert_eq!(roman(4000), "4000");
    }
}
