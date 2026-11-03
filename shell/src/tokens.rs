//! The few design tokens the shell needs in code rather than in the stylesheet:
//! motion timings and the stroke of the hand-drawn icons. recto-theme renders
//! them into <data dir>/recto/shared/shell/tokens.json.

use gtk::glib;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    /// Leaving, in ms.
    pub duration_short: u32,
    /// Arriving, in ms.
    pub duration_long: u32,
    /// Line width of the shell's icons, in logical px.
    pub icon_stroke: f64,
}

const FALLBACK: Tokens = Tokens { duration_short: 160, duration_long: 240, icon_stroke: 1.5 };

thread_local! {
    static TOKENS: Cell<Tokens> = const { Cell::new(FALLBACK) };
}

/// Read the rendered tokens. Called with the stylesheet, so a theme reload picks them up.
pub fn load() {
    let path = glib::user_data_dir().join("recto/shared/shell/tokens.json");
    let tokens = std::fs::read_to_string(path).map(|text| parse(&text)).unwrap_or(FALLBACK);
    TOKENS.with(|t| t.set(tokens));
}

pub fn get() -> Tokens {
    TOKENS.with(Cell::get)
}

fn parse(text: &str) -> Tokens {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return FALLBACK };
    let ms = |key: &str, fallback: u32| {
        v[key].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n <= 2_000).unwrap_or(fallback)
    };
    Tokens {
        duration_short: ms("duration_short", FALLBACK.duration_short),
        duration_long: ms("duration_long", FALLBACK.duration_long),
        icon_stroke: v["icon_stroke"].as_f64().filter(|w| (0.5..=4.0).contains(w)).unwrap_or(FALLBACK.icon_stroke),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rendered_tokens() {
        let text = r#"{ "comment": "x", "duration_short": 120, "duration_long": 300, "icon_stroke": 1.25 }"#;
        assert_eq!(parse(text), Tokens { duration_short: 120, duration_long: 300, icon_stroke: 1.25 });
    }

    #[test]
    fn falls_back_on_bad_values() {
        assert_eq!(parse("not json"), FALLBACK);
        let text = r#"{ "duration_short": -1, "duration_long": 99999, "icon_stroke": 12 }"#;
        assert_eq!(parse(text), FALLBACK);
    }
}
