//! niri IPC over its Unix socket, on the GTK main loop (no extra threads).
//!
//! Requests and events are JSON, one per line. See niri's `niri-ipc` crate for the schema.

use gtk::{gio, glib, prelude::*};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

fn socket_path() -> Option<PathBuf> {
    std::env::var_os("NIRI_SOCKET").map(PathBuf::from)
}

async fn connect() -> Result<gio::SocketConnection, String> {
    let path = socket_path().ok_or("NIRI_SOCKET is not set")?;
    let address = gio::UnixSocketAddress::new(&path);
    gio::SocketClient::new()
        .connect_future(&address)
        .await
        .map_err(|e| format!("cannot connect to {}: {e}", path.display()))
}

fn send(conn: &gio::SocketConnection, request: &Value) -> Result<(), String> {
    let mut line = request.to_string();
    line.push('\n');
    conn.output_stream()
        .write_all(line.as_bytes(), gio::Cancellable::NONE)
        .map(|_| ())
        .map_err(|e| format!("cannot write to niri: {e}"))
}

/// Follow niri's event stream for as long as the shell runs, reconnecting when niri restarts.
pub fn follow_events(on_event: impl Fn(Value) + 'static) {
    glib::spawn_future_local(async move {
        loop {
            if let Err(e) = stream(&on_event).await {
                eprintln!("liubai-shell: niri events: {e}");
            }
            glib::timeout_future(Duration::from_secs(2)).await;
        }
    });
}

async fn stream(on_event: &impl Fn(Value)) -> Result<(), String> {
    let conn = connect().await?;
    send(&conn, &json!("EventStream"))?;
    let input = gio::DataInputStream::new(&conn.input_stream());
    loop {
        let line = input
            .read_line_utf8_future(glib::Priority::DEFAULT)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let Some(line) = line else {
            return Err("niri closed the connection".to_owned());
        };
        match serde_json::from_str::<Value>(&line) {
            // The first line answers the request itself: {"Ok":"Handled"}.
            Ok(v) if v.get("Ok").is_some() => {}
            Ok(v) if v.get("Err").is_some() => return Err(format!("niri refused: {v}")),
            Ok(v) => on_event(v),
            Err(e) => eprintln!("liubai-shell: unreadable event: {e}"),
        }
    }
}

/// Ask niri to do something, e.g. `action(json!({"ToggleOverview": {}}))`.
pub fn action(action: Value) {
    glib::spawn_future_local(async move {
        let result = async {
            let conn = connect().await?;
            send(&conn, &json!({ "Action": action }))?;
            // Wait for the reply so the connection is not dropped mid-request.
            let input = gio::DataInputStream::new(&conn.input_stream());
            input
                .read_line_utf8_future(glib::Priority::DEFAULT)
                .await
                .map_err(|e| format!("no reply: {e}"))?;
            Ok::<(), String>(())
        }
        .await;
        if let Err(e) = result {
            eprintln!("liubai-shell: niri action: {e}");
        }
    });
}

pub fn toggle_overview() {
    action(json!({ "ToggleOverview": {} }));
}

pub fn focus_workspace_up() {
    action(json!({ "FocusWorkspaceUp": {} }));
}

pub fn focus_workspace_down() {
    action(json!({ "FocusWorkspaceDown": {} }));
}
