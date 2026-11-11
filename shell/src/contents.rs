//! The Contents page: every open window, set out like a book's table of contents.
//!
//! Workspaces are chapters, windows are entries, and an entry's page number is its
//! column in niri's scrolling layout. This file is the page's model: plain data with
//! no GTK and no I/O, so it can be tested on its own (like `state`). The view is not
//! written yet; the spec is in docs/CONTENTS.md.

// Nothing calls the model until the view lands. Remove this with the first caller.
#![allow(dead_code)]

use crate::state::{Desktop, Window, Workspace};
use crate::strings;
use serde_json::{json, Value};

/// Where a line of the page leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Window(u64),
    /// A chapter with nothing in it yet: the line reads "A blank page."
    Workspace(u64),
}

impl Target {
    /// The niri action that turns to it.
    pub fn action(self) -> Value {
        match self {
            Target::Window(id) => json!({ "FocusWindow": { "id": id } }),
            Target::Workspace(id) => json!({ "FocusWorkspace": { "reference": { "Id": id } } }),
        }
    }
}

/// One window: "Text Editor  README.md ......... 2".
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub window_id: u64,
    /// The app's name, set upright.
    pub app: String,
    /// What the window holds, set in italic after the name. `None` when the title says nothing more.
    pub title: Option<String>,
    /// The column it sits in. `None` for a floating window: a loose leaf has no page number.
    pub page: Option<u64>,
    /// The window in focus when the page was set.
    pub current: bool,
}

/// One workspace and its windows in reading order.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub workspace_id: u64,
    /// "II"
    pub numeral: String,
    pub name: Option<String>,
    /// The workspace in front of the reader.
    pub current: bool,
    pub entries: Vec<Entry>,
}

impl Chapter {
    /// "II · Reading", or "II" for a workspace without a name.
    pub fn heading(&self) -> String {
        strings::chapter_heading(&self.numeral, self.name.as_deref())
    }

    pub fn is_blank(&self) -> bool {
        self.entries.is_empty()
    }

    /// The places this chapter can take the reader: its entries, or itself when blank.
    fn targets(&self) -> Vec<Target> {
        if self.is_blank() {
            vec![Target::Workspace(self.workspace_id)]
        } else {
            self.entries.iter().map(|e| Target::Window(e.window_id)).collect()
        }
    }
}

/// Set the page for one output (`None`: every workspace niri has).
///
/// `name_of` gives an app's proper name from its app id, normally the Name of its desktop
/// entry; when it has none, the name is worked out from the app id itself.
pub fn chapters(
    desktop: &Desktop,
    output: Option<&str>,
    name_of: impl Fn(&str) -> Option<String>,
) -> Vec<Chapter> {
    // The same workspaces the bar shows: niri's spare empty one only counts while it is in front.
    desktop
        .dots_on(output)
        .into_iter()
        .map(|ws| chapter(desktop, ws, output.is_some(), &name_of))
        .collect()
}

fn chapter(
    desktop: &Desktop,
    ws: &Workspace,
    one_output: bool,
    name_of: &impl Fn(&str) -> Option<String>,
) -> Chapter {
    let mut windows: Vec<&Window> =
        desktop.windows.values().filter(|w| w.workspace_id == Some(ws.id)).collect();
    // Reading order: left to right, then down a column; loose leaves go at the end.
    windows.sort_by_key(|w| (page(w).is_none(), w.column, w.row, w.id));
    Chapter {
        workspace_id: ws.id,
        numeral: strings::roman(ws.idx),
        name: ws.name.clone().filter(|n| !n.trim().is_empty()),
        current: if one_output { ws.is_active } else { ws.is_focused },
        entries: windows.into_iter().map(|w| entry(w, name_of)).collect(),
    }
}

fn page(w: &Window) -> Option<u64> {
    if w.is_floating {
        None
    } else {
        w.column
    }
}

fn entry(w: &Window, name_of: &impl Fn(&str) -> Option<String>) -> Entry {
    let app = w
        .app_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(|id| name_of(id).unwrap_or_else(|| app_label(id)))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| strings::UNTITLED_WINDOW.to_owned());
    Entry {
        window_id: w.id,
        title: subtitle(w.title.as_deref(), &app),
        app,
        page: page(w),
        current: w.is_focused,
    }
}

/// A name for an app that has no desktop entry: "org.gnome.TextEditor" -> "Text Editor",
/// "foot" -> "Foot".
pub fn app_label(app_id: &str) -> String {
    let last = app_id.rsplit('.').find(|part| !part.is_empty()).unwrap_or("");
    let mut out = String::new();
    let mut after_lowercase = false;
    for c in last.chars() {
        if c == '-' || c == '_' {
            out.push(' ');
            after_lowercase = false;
            continue;
        }
        if c.is_uppercase() && after_lowercase {
            out.push(' ');
        }
        if out.is_empty() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        after_lowercase = c.is_lowercase();
    }
    out.trim().to_owned()
}

const SEPARATORS: [&str; 4] = [" — ", " – ", " - ", " | "];

/// What to print after the app's name: the window title without the app's own name on its end
/// ("README.md — Text Editor" -> "README.md"). `None` when nothing is left to say.
fn subtitle(title: Option<&str>, app: &str) -> Option<String> {
    let mut text = title?.trim();
    let app = app.to_lowercase();
    let last_part = SEPARATORS
        .iter()
        .filter_map(|sep| text.rsplit_once(sep))
        .min_by_key(|(_, tail)| tail.len());
    if let Some((head, tail)) = last_part {
        let tail = tail.trim().to_lowercase();
        // "Mozilla Firefox" names Firefox; a tail of a letter or two names nothing.
        let names_app = tail.contains(&app) || (tail.chars().count() >= 3 && app.contains(&tail));
        if !app.is_empty() && names_app {
            text = head.trim();
        }
    }
    (!text.is_empty() && text.to_lowercase() != app).then(|| text.to_owned())
}

/// Every line the reader can go to, top to bottom.
pub fn targets(chapters: &[Chapter]) -> Vec<Target> {
    chapters.iter().flat_map(Chapter::targets).collect()
}

/// Where the mark starts: on the window in focus, else at the top of the chapter in front,
/// else on the first line.
pub fn opening_target(chapters: &[Chapter]) -> Option<Target> {
    chapters
        .iter()
        .flat_map(|c| &c.entries)
        .find(|e| e.current)
        .map(|e| Target::Window(e.window_id))
        .or_else(|| chapters.iter().find(|c| c.current).and_then(|c| c.targets().first().copied()))
        .or_else(|| targets(chapters).first().copied())
}

/// The line `delta` lines from `from`. It stops at the first and last line: a book does not
/// wrap around. A mark that is gone (its window closed) starts again from `opening_target`.
pub fn step(chapters: &[Chapter], from: Option<Target>, delta: i64) -> Option<Target> {
    let all = targets(chapters);
    let Some(at) = from.and_then(|t| all.iter().position(|x| *x == t)) else {
        return opening_target(chapters);
    };
    let last = all.len() as i64 - 1;
    all.get((at as i64 + delta).clamp(0, last) as usize).copied()
}

/// The first line of the chapter `delta` chapters from the one `from` is in.
pub fn step_chapter(chapters: &[Chapter], from: Option<Target>, delta: i64) -> Option<Target> {
    let Some(at) = from.and_then(|t| chapters.iter().position(|c| c.targets().contains(&t))) else {
        return opening_target(chapters);
    };
    let last = chapters.len() as i64 - 1;
    chapters.get((at as i64 + delta).clamp(0, last) as usize)?.targets().first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_names(_: &str) -> Option<String> {
        None
    }

    /// Chapter I "Writing": an editor, a browser, two terminals stacked in column 3, a floating dialog.
    /// Chapter II: a reader. Chapter III: niri's spare empty workspace.
    fn desktop() -> Desktop {
        let mut d = Desktop::default();
        d.apply(&json!({"WorkspacesChanged": {"workspaces": [
            {"id": 1, "idx": 1, "name": "Writing", "output": "A", "is_active": true, "is_focused": true},
            {"id": 2, "idx": 2, "name": null, "output": "A", "is_active": false, "is_focused": false},
            {"id": 3, "idx": 3, "name": null, "output": "A", "is_active": false, "is_focused": false}
        ]}}));
        d.apply(&json!({"WindowsChanged": {"windows": [
            {"id": 14, "title": "Save as", "app_id": "org.gnome.TextEditor", "workspace_id": 1,
             "is_focused": false, "is_floating": true, "layout": {"pos_in_scrolling_layout": null}},
            {"id": 13, "title": "~/Projects/recto", "app_id": "foot", "workspace_id": 1,
             "is_focused": true, "is_floating": false, "layout": {"pos_in_scrolling_layout": [3, 2]}},
            {"id": 12, "title": "cargo test", "app_id": "foot", "workspace_id": 1,
             "is_focused": false, "is_floating": false, "layout": {"pos_in_scrolling_layout": [3, 1]}},
            {"id": 11, "title": "Monaspace — Mozilla Firefox", "app_id": "firefox", "workspace_id": 1,
             "is_focused": false, "is_floating": false, "layout": {"pos_in_scrolling_layout": [2, 1]}},
            {"id": 10, "title": "README.md - Text Editor", "app_id": "org.gnome.TextEditor",
             "workspace_id": 1, "is_focused": false, "is_floating": false,
             "layout": {"pos_in_scrolling_layout": [1, 1]}},
            {"id": 20, "title": "typography.pdf", "app_id": "org.pwmt.zathura", "workspace_id": 2,
             "is_focused": false, "is_floating": false, "layout": {"pos_in_scrolling_layout": [1, 1]}}
        ]}}));
        d
    }

    #[test]
    fn workspaces_become_chapters_and_columns_become_pages() {
        let page = chapters(&desktop(), Some("A"), no_names);
        assert_eq!(page.len(), 2, "the spare empty workspace is left out");
        assert_eq!(page[0].heading(), "I · Writing");
        assert_eq!(page[1].heading(), "II");
        assert!(page[0].current && !page[1].current);

        let lines: Vec<(u64, &str, Option<&str>, Option<u64>)> = page[0]
            .entries
            .iter()
            .map(|e| (e.window_id, e.app.as_str(), e.title.as_deref(), e.page))
            .collect();
        assert_eq!(
            lines,
            vec![
                (10, "Text Editor", Some("README.md"), Some(1)),
                (11, "Firefox", Some("Monaspace"), Some(2)),
                // Two windows in one column share a page, top one first.
                (12, "Foot", Some("cargo test"), Some(3)),
                (13, "Foot", Some("~/Projects/recto"), Some(3)),
                // The floating dialog is a loose leaf: last, with no page number.
                (14, "Text Editor", Some("Save as"), None),
            ]
        );
        assert!(page[0].entries[3].current);
        assert_eq!(page[0].entries.iter().filter(|e| e.current).count(), 1);
    }

    #[test]
    fn a_blank_chapter_shows_only_while_it_is_in_front() {
        let mut d = desktop();
        d.apply(&json!({"WorkspaceActivated": {"id": 3, "focused": true}}));
        let page = chapters(&d, Some("A"), no_names);
        assert_eq!(page.len(), 3);
        assert!(page[2].is_blank() && page[2].current);
        assert_eq!(page[2].heading(), "III");
        assert_eq!(targets(&page).last(), Some(&Target::Workspace(3)));
    }

    #[test]
    fn desktop_entry_names_win_over_guesses() {
        let names = |id: &str| (id == "foot").then(|| "Terminal".to_owned());
        let page = chapters(&desktop(), Some("A"), names);
        assert_eq!(page[0].entries[2].app, "Terminal");
        assert_eq!(page[0].entries[0].app, "Text Editor");
    }

    #[test]
    fn names_from_app_ids() {
        assert_eq!(app_label("foot"), "Foot");
        assert_eq!(app_label("org.gnome.TextEditor"), "Text Editor");
        assert_eq!(app_label("org.pwmt.zathura"), "Zathura");
        assert_eq!(app_label("firefox-esr"), "Firefox esr");
        assert_eq!(app_label(""), "");
    }

    #[test]
    fn titles_drop_the_apps_own_name() {
        assert_eq!(subtitle(Some("README.md — Text Editor"), "Text Editor").as_deref(), Some("README.md"));
        assert_eq!(subtitle(Some("Monaspace - Mozilla Firefox"), "Firefox").as_deref(), Some("Monaspace"));
        // Only the end is dropped, and only when it names the app.
        assert_eq!(subtitle(Some("a - b - Foot"), "Foot").as_deref(), Some("a - b"));
        assert_eq!(subtitle(Some("notes - a"), "Zathura").as_deref(), Some("notes - a"));
        // A title that only repeats the name, or says nothing, is left out.
        assert_eq!(subtitle(Some("Firefox"), "Firefox"), None);
        assert_eq!(subtitle(Some("  "), "Foot"), None);
        assert_eq!(subtitle(None, "Foot"), None);
    }

    #[test]
    fn a_window_without_an_app_id_still_gets_a_line() {
        let mut d = desktop();
        d.apply(&json!({"WindowOpenedOrChanged": {"window":
            {"id": 30, "title": "", "app_id": null, "workspace_id": 2, "is_focused": false,
             "is_floating": false, "layout": {"pos_in_scrolling_layout": [2, 1]}}}}));
        let page = chapters(&d, Some("A"), no_names);
        let line = &page[1].entries[1];
        assert_eq!((line.app.as_str(), line.title.as_deref(), line.page), ("Untitled window", None, Some(2)));
    }

    #[test]
    fn the_mark_starts_on_the_window_in_focus_and_does_not_wrap() {
        let page = chapters(&desktop(), Some("A"), no_names);
        let start = opening_target(&page);
        assert_eq!(start, Some(Target::Window(13)));
        assert_eq!(step(&page, start, 1), Some(Target::Window(14)));
        assert_eq!(step(&page, start, 2), Some(Target::Window(20)));
        assert_eq!(step(&page, start, 99), Some(Target::Window(20)));
        assert_eq!(step(&page, start, -99), Some(Target::Window(10)));
        // A window that closed while the page was open: start again.
        assert_eq!(step(&page, Some(Target::Window(999)), 1), start);
    }

    #[test]
    fn chapter_steps_land_on_a_chapters_first_line() {
        let mut d = desktop();
        d.apply(&json!({"WorkspaceActivated": {"id": 3, "focused": true}}));
        d.apply(&json!({"WindowFocusChanged": {"id": null}}));
        let page = chapters(&d, Some("A"), no_names);
        // Nothing in focus: the mark starts at the top of the chapter in front.
        assert_eq!(opening_target(&page), Some(Target::Workspace(3)));
        assert_eq!(step_chapter(&page, Some(Target::Workspace(3)), -1), Some(Target::Window(20)));
        assert_eq!(step_chapter(&page, Some(Target::Window(20)), -1), Some(Target::Window(10)));
        assert_eq!(step_chapter(&page, Some(Target::Window(12)), 1), Some(Target::Window(20)));
        assert_eq!(step_chapter(&page, Some(Target::Window(20)), 9), Some(Target::Workspace(3)));
    }

    #[test]
    fn an_empty_desk_has_nowhere_to_go() {
        let page = chapters(&Desktop::default(), Some("A"), no_names);
        assert!(page.is_empty());
        assert_eq!(opening_target(&page), None);
        assert_eq!(step(&page, None, 1), None);
        assert_eq!(step_chapter(&page, None, 1), None);
    }

    #[test]
    fn targets_turn_into_niri_actions() {
        assert_eq!(Target::Window(13).action(), json!({"FocusWindow": {"id": 13}}));
        assert_eq!(Target::Workspace(3).action(), json!({"FocusWorkspace": {"reference": {"Id": 3}}}));
    }
}
