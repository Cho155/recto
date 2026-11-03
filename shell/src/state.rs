//! What the shell knows about the desktop, rebuilt from niri's event stream.
//!
//! Plain data with no GTK and no I/O, so it can be tested on its own.
//! Events are read from `serde_json::Value` rather than typed structs: fields
//! the shell does not use can change between niri versions without breaking it.

use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub id: u64,
    pub idx: u64,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_active: bool,
    pub is_focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
    /// 1-based column in the scrolling layout, if tiled.
    pub column: Option<u64>,
    /// Tile width in logical pixels.
    pub tile_width: f64,
    /// Tile x position relative to the visible view, when niri reports it.
    pub view_x: Option<f64>,
}

/// One column of the scrolling layout, as the bar draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub width: f64,
    pub focused: bool,
    /// `None` when niri does not report positions; the bar then draws every column alike.
    pub visible: Option<bool>,
}

#[derive(Debug, Default)]
pub struct Desktop {
    pub workspaces: BTreeMap<u64, Workspace>,
    pub windows: BTreeMap<u64, Window>,
    /// False until the first event arrives, so the bar can tell "empty" from "unknown".
    pub connected: bool,
}

fn as_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn parse_workspace(v: &Value) -> Option<Workspace> {
    Some(Workspace {
        id: v.get("id")?.as_u64()?,
        idx: v.get("idx").and_then(Value::as_u64).unwrap_or(0),
        name: v.get("name").and_then(as_str),
        output: v.get("output").and_then(as_str),
        is_active: v.get("is_active").and_then(Value::as_bool).unwrap_or(false),
        is_focused: v.get("is_focused").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn pair(v: Option<&Value>) -> Option<(f64, f64)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

fn apply_layout(w: &mut Window, layout: &Value) {
    w.column = layout
        .get("pos_in_scrolling_layout")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_u64);
    if let Some((width, _)) = pair(layout.get("tile_size")) {
        w.tile_width = width;
    }
    w.view_x = pair(layout.get("tile_pos_in_workspace_view")).map(|(x, _)| x);
}

fn parse_window(v: &Value) -> Option<Window> {
    let mut w = Window {
        id: v.get("id")?.as_u64()?,
        title: v.get("title").and_then(as_str),
        app_id: v.get("app_id").and_then(as_str),
        workspace_id: v.get("workspace_id").and_then(Value::as_u64),
        is_focused: v.get("is_focused").and_then(Value::as_bool).unwrap_or(false),
        is_floating: v.get("is_floating").and_then(Value::as_bool).unwrap_or(false),
        column: None,
        tile_width: 0.0,
        view_x: None,
    };
    if let Some(layout) = v.get("layout") {
        apply_layout(&mut w, layout);
    }
    Some(w)
}

impl Desktop {
    /// Apply one niri event. Returns true when something the shell shows may have changed.
    pub fn apply(&mut self, event: &Value) -> bool {
        let Some((kind, body)) = event.as_object().and_then(|o| o.iter().next()) else {
            return false;
        };
        self.connected = true;
        match kind.as_str() {
            "WorkspacesChanged" => {
                self.workspaces = body
                    .get("workspaces")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(parse_workspace).map(|w| (w.id, w)).collect())
                    .unwrap_or_default();
                true
            }
            "WorkspaceActivated" => {
                let Some(id) = body.get("id").and_then(Value::as_u64) else { return false };
                let focused = body.get("focused").and_then(Value::as_bool).unwrap_or(false);
                let output = self.workspaces.get(&id).and_then(|w| w.output.clone());
                for w in self.workspaces.values_mut() {
                    if w.output == output {
                        w.is_active = w.id == id;
                    }
                    if focused {
                        w.is_focused = w.id == id;
                    }
                }
                true
            }
            "WindowsChanged" => {
                self.windows = body
                    .get("windows")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(parse_window).map(|w| (w.id, w)).collect())
                    .unwrap_or_default();
                true
            }
            "WindowOpenedOrChanged" => {
                let Some(w) = body.get("window").and_then(parse_window) else { return false };
                if w.is_focused {
                    for other in self.windows.values_mut() {
                        other.is_focused = false;
                    }
                }
                self.windows.insert(w.id, w);
                true
            }
            "WindowClosed" => {
                let Some(id) = body.get("id").and_then(Value::as_u64) else { return false };
                self.windows.remove(&id).is_some()
            }
            "WindowFocusChanged" => {
                let id = body.get("id").and_then(Value::as_u64);
                for w in self.windows.values_mut() {
                    w.is_focused = Some(w.id) == id;
                }
                true
            }
            "WindowLayoutsChanged" => {
                let Some(changes) = body.get("changes").and_then(Value::as_array) else { return false };
                for change in changes {
                    let Some(entry) = change.as_array() else { continue };
                    let (Some(id), Some(layout)) = (entry.first().and_then(Value::as_u64), entry.get(1))
                    else {
                        continue;
                    };
                    if let Some(w) = self.windows.get_mut(&id) {
                        apply_layout(w, layout);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Workspaces on one output, in niri's order.
    pub fn workspaces_on(&self, output: Option<&str>) -> Vec<&Workspace> {
        let mut list: Vec<&Workspace> = self
            .workspaces
            .values()
            .filter(|w| output.is_none() || w.output.as_deref() == output)
            .collect();
        list.sort_by_key(|w| w.idx);
        list
    }

    /// The workspace shown on an output (or the focused one, when the output is unknown).
    pub fn active_on(&self, output: Option<&str>) -> Option<&Workspace> {
        match output {
            Some(_) => self.workspaces_on(output).into_iter().find(|w| w.is_active),
            None => self.focused_workspace(),
        }
    }

    pub fn focused_workspace(&self) -> Option<&Workspace> {
        self.workspaces.values().find(|w| w.is_focused)
    }

    pub fn focused_window(&self) -> Option<&Window> {
        self.windows.values().find(|w| w.is_focused)
    }

    /// Workspaces worth a dot in the bar. niri always keeps one empty workspace at the end;
    /// it only counts while it is the active one.
    pub fn dots_on(&self, output: Option<&str>) -> Vec<&Workspace> {
        let list = self.workspaces_on(output);
        let occupied = |w: &Workspace| self.windows.values().any(|win| win.workspace_id == Some(w.id));
        let last = list.len().saturating_sub(1);
        list.into_iter()
            .enumerate()
            .filter(|(i, w)| *i != last || w.is_active || occupied(w))
            .map(|(_, w)| w)
            .collect()
    }

    /// Tiled columns of a workspace, left to right.
    pub fn columns(&self, workspace_id: u64, view_width: f64) -> Vec<Column> {
        let mut by_column: BTreeMap<u64, Column> = BTreeMap::new();
        for w in self.windows.values() {
            if w.workspace_id != Some(workspace_id) || w.is_floating {
                continue;
            }
            let Some(index) = w.column else { continue };
            let visible = w.view_x.map(|x| x + w.tile_width > 0.0 && x < view_width);
            let col = by_column.entry(index).or_insert(Column { width: 0.0, focused: false, visible });
            col.width = col.width.max(w.tile_width);
            col.focused |= w.is_focused;
            col.visible = match (col.visible, visible) {
                (Some(a), Some(b)) => Some(a || b),
                (a, b) => a.or(b),
            };
        }
        by_column.into_values().collect()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn occupied_workspace_count(&self) -> usize {
        self.workspaces
            .values()
            .filter(|ws| self.windows.values().any(|w| w.workspace_id == Some(ws.id)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn desktop() -> Desktop {
        let mut d = Desktop::default();
        d.apply(&json!({"WorkspacesChanged": {"workspaces": [
            {"id": 1, "idx": 1, "name": "Writing", "output": "A", "is_active": true, "is_focused": true},
            {"id": 2, "idx": 2, "name": null, "output": "A", "is_active": false, "is_focused": false},
            {"id": 3, "idx": 3, "name": null, "output": "A", "is_active": false, "is_focused": false},
            {"id": 4, "idx": 1, "name": null, "output": "B", "is_active": true, "is_focused": false}
        ]}}));
        d.apply(&json!({"WindowsChanged": {"windows": [
            {"id": 10, "title": "notes", "app_id": "editor", "workspace_id": 1, "is_focused": true,
             "is_floating": false,
             "layout": {"pos_in_scrolling_layout": [1, 1], "tile_size": [800.0, 900.0],
                        "tile_pos_in_workspace_view": [10.0, 40.0]}},
            {"id": 11, "title": "shell", "app_id": "foot", "workspace_id": 1, "is_focused": false,
             "is_floating": false,
             "layout": {"pos_in_scrolling_layout": [2, 1], "tile_size": [500.0, 900.0],
                        "tile_pos_in_workspace_view": [830.0, 40.0]}},
            {"id": 12, "title": "far", "app_id": "foot", "workspace_id": 1, "is_focused": false,
             "is_floating": false,
             "layout": {"pos_in_scrolling_layout": [3, 1], "tile_size": [500.0, 900.0],
                        "tile_pos_in_workspace_view": [1600.0, 40.0]}},
            {"id": 13, "title": "other", "app_id": "foot", "workspace_id": 2, "is_focused": false,
             "is_floating": false, "layout": {"pos_in_scrolling_layout": [1, 1], "tile_size": [500.0, 900.0]}}
        ]}}));
        d
    }

    #[test]
    fn columns_follow_layout_and_viewport() {
        let d = desktop();
        let cols = d.columns(1, 1440.0);
        assert_eq!(cols.len(), 3);
        assert!(cols[0].focused && cols[0].visible == Some(true));
        assert_eq!(cols[1].visible, Some(true));
        assert_eq!(cols[2].visible, Some(false));
        assert_eq!(cols[0].width, 800.0);
    }

    #[test]
    fn columns_without_positions_are_unknown() {
        let d = desktop();
        assert_eq!(d.columns(2, 1440.0)[0].visible, None);
    }

    #[test]
    fn activation_moves_active_and_focus() {
        let mut d = desktop();
        d.apply(&json!({"WorkspaceActivated": {"id": 2, "focused": true}}));
        assert_eq!(d.active_on(Some("A")).map(|w| w.id), Some(2));
        assert_eq!(d.focused_workspace().map(|w| w.id), Some(2));
        // The other output keeps its own active workspace.
        assert_eq!(d.active_on(Some("B")).map(|w| w.id), Some(4));
    }

    #[test]
    fn trailing_empty_workspace_has_no_dot() {
        let d = desktop();
        let ids: Vec<u64> = d.dots_on(Some("A")).iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn focus_and_close_events() {
        let mut d = desktop();
        d.apply(&json!({"WindowFocusChanged": {"id": 11}}));
        assert_eq!(d.focused_window().map(|w| w.id), Some(11));
        assert!(d.apply(&json!({"WindowClosed": {"id": 11}})));
        assert!(d.focused_window().is_none());
        assert_eq!(d.window_count(), 3);
    }

    #[test]
    fn layout_changes_update_columns() {
        let mut d = desktop();
        d.apply(&json!({"WindowLayoutsChanged": {"changes": [
            [12, {"pos_in_scrolling_layout": [3, 1], "tile_size": [500.0, 900.0],
                  "tile_pos_in_workspace_view": [900.0, 40.0]}]
        ]}}));
        assert_eq!(d.columns(1, 1440.0)[2].visible, Some(true));
    }

    #[test]
    fn unknown_events_change_nothing() {
        let mut d = desktop();
        assert!(!d.apply(&json!({"KeyboardLayoutsChanged": {"keyboard_layouts": {}}})));
        assert!(!d.apply(&json!("Handled")));
    }
}
