//! "Mic in use": is anything recording right now?
//!
//! Watched through `pactl subscribe` (PipeWire's PulseAudio layer) in a small child
//! process, rather than linking PipeWire into the shell. The child costs a few MiB
//! (see reports/memory) and needs no build-time dependencies.

use gtk::{gio, glib, prelude::*};
use std::{cell::{Cell, RefCell}, ffi::OsStr, rc::Rc, time::Duration};

thread_local! {
    /// The running `pactl subscribe`, so it can be ended with the shell.
    static CHILD: RefCell<Option<gio::Subprocess>> = const { RefCell::new(None) };
}

/// End the watcher's child process. Without this, a stopped shell would leave
/// `pactl subscribe` behind until its next write fails (found in the test VM).
pub fn stop() {
    if let Some(child) = CHILD.with(|c| c.borrow_mut().take()) {
        child.force_exit();
    }
}

/// Recording streams that are not someone listening: level meters from mixers.
fn is_real_recording(stream: &serde_json::Value) -> bool {
    let props = &stream["properties"];
    let name = props["media.name"].as_str().unwrap_or("");
    !(name == "Peak detect" || props["stream.monitor"].as_str() == Some("true"))
}

/// Reads `pactl --format=json list source-outputs`.
pub fn in_use(json: &str) -> Option<bool> {
    let list: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(list.as_array()?.iter().any(is_real_recording))
}

/// Start pactl with its output piped to us; it ends when the shell does.
fn pactl(args: &[&str]) -> Result<gio::Subprocess, glib::Error> {
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE);
    crate::memory::tie_to_shell(&launcher);
    let argv: Vec<&OsStr> = std::iter::once("pactl").chain(args.iter().copied()).map(OsStr::new).collect();
    launcher.spawn(&argv)
}

async fn check() -> Option<bool> {
    let process = pactl(&["--format=json", "list", "source-outputs"]).ok()?;
    let (out, _) = process.communicate_utf8_future(None).await.ok()?;
    in_use(out.as_deref()?)
}

/// Report whether something is recording, now and whenever it changes.
pub fn watch(on_change: impl Fn(bool) + 'static) {
    let on_change: Rc<dyn Fn(bool)> = Rc::new(on_change);
    glib::spawn_future_local(async move {
        loop {
            if let Err(why) = follow(&on_change).await {
                eprintln!("liubai-shell: microphone watch: {why}");
            }
            // pactl gone (or PipeWire restarting): try again later.
            glib::timeout_future(Duration::from_secs(10)).await;
        }
    });
}

async fn follow(on_change: &Rc<dyn Fn(bool)>) -> Result<(), String> {
    let report = |value: Option<bool>| {
        if let Some(v) = value {
            on_change(v);
        }
    };
    report(check().await);
    let process = pactl(&["subscribe"]).map_err(|e| format!("cannot start pactl: {e}"))?;
    let stdout = process.stdout_pipe().ok_or("pactl has no output")?;
    CHILD.with(|c| *c.borrow_mut() = Some(process.clone()));
    let lines = gio::DataInputStream::new(&stdout);
    // Several events arrive together; check once after they settle.
    let pending = Rc::new(Cell::new(false));
    loop {
        let line = lines
            .read_line_utf8_future(glib::Priority::DEFAULT)
            .await
            .map_err(|e| format!("read failed: {e}"))?
            .ok_or("pactl stopped")?;
        if line.contains("source-output") && !pending.replace(true) {
            let pending = pending.clone();
            let on_change = on_change.clone();
            glib::spawn_future_local(async move {
                glib::timeout_future(Duration::from_millis(150)).await;
                pending.set(false);
                if let Some(v) = check().await {
                    on_change(v);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::in_use;

    #[test]
    fn recording_or_not() {
        assert_eq!(in_use("[]"), Some(false));
        let meter = r#"[{"properties": {"media.name": "Peak detect"}}]"#;
        assert_eq!(in_use(meter), Some(false));
        let call = r#"[{"properties": {"media.name": "Voice call", "application.name": "Meet"}}]"#;
        assert_eq!(in_use(call), Some(true));
        assert_eq!(in_use("not json"), None);
    }
}
