//! Volume, microphone and brightness, changed through wpctl and brightnessctl.
//! The shell makes the change itself and reads the result back, so it never has
//! to keep a PipeWire connection open just to show the OSD.

use gtk::gio;
use std::ffi::OsStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicMute,
    BrightnessUp,
    BrightnessDown,
}

impl Control {
    pub fn parse(words: &[String]) -> Option<Control> {
        match words.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["volume", "up"] => Some(Control::VolumeUp),
            ["volume", "down"] => Some(Control::VolumeDown),
            ["volume", "mute"] => Some(Control::VolumeMute),
            ["mic-mute"] => Some(Control::MicMute),
            ["brightness", "up"] => Some(Control::BrightnessUp),
            ["brightness", "down"] => Some(Control::BrightnessDown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    Level { percent: u32, muted: bool },
    Unavailable(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub name: &'static str,
    pub reading: Reading,
    /// Set for the output device, so the bar can show "Muted".
    pub output_muted: Option<bool>,
}

async fn run(argv: &[&str]) -> Option<String> {
    let args: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    let process = gio::Subprocess::newv(
        &args,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .ok()?;
    let (stdout, _) = process.communicate_utf8_future(None).await.ok()?;
    if !process.is_successful() {
        return None;
    }
    stdout.map(|s| s.to_string())
}

/// "Volume: 0.45" or "Volume: 0.45 [MUTED]"
pub fn parse_wpctl(text: &str) -> Option<(u32, bool)> {
    let rest = text.trim().strip_prefix("Volume:")?.trim();
    let number: f64 = rest.split_whitespace().next()?.parse().ok()?;
    Some(((number * 100.0).round().max(0.0) as u32, rest.contains("[MUTED]")))
}

/// brightnessctl -m: "intel_backlight,backlight,1200,50%,2400"
pub fn parse_brightnessctl(text: &str) -> Option<u32> {
    text.lines().next()?.split(',').nth(3)?.trim_end_matches('%').parse().ok()
}

const SINK: &str = "@DEFAULT_AUDIO_SINK@";
const SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

async fn read_volume(device: &str) -> Option<(u32, bool)> {
    parse_wpctl(&run(&["wpctl", "get-volume", device]).await?)
}

pub async fn read_output_muted() -> Option<bool> {
    read_volume(SINK).await.map(|(_, muted)| muted)
}

pub async fn apply(control: Control) -> Outcome {
    match control {
        Control::VolumeUp | Control::VolumeDown | Control::VolumeMute => {
            let ok = match control {
                Control::VolumeUp => run(&["wpctl", "set-volume", "-l", "1.0", SINK, "5%+"]).await,
                Control::VolumeDown => run(&["wpctl", "set-volume", SINK, "5%-"]).await,
                _ => run(&["wpctl", "set-mute", SINK, "toggle"]).await,
            };
            let reading = match (ok, read_volume(SINK).await) {
                (Some(_), Some((percent, muted))) => Reading::Level { percent, muted },
                _ => Reading::Unavailable("No audio device"),
            };
            let output_muted = match reading {
                Reading::Level { muted, .. } => Some(muted),
                _ => None,
            };
            Outcome { name: "Volume", reading, output_muted }
        }
        Control::MicMute => {
            let ok = run(&["wpctl", "set-mute", SOURCE, "toggle"]).await;
            let reading = match (ok, read_volume(SOURCE).await) {
                (Some(_), Some((percent, muted))) => Reading::Level { percent, muted },
                _ => Reading::Unavailable("No microphone"),
            };
            Outcome { name: "Microphone", reading, output_muted: None }
        }
        Control::BrightnessUp | Control::BrightnessDown => {
            let step = if control == Control::BrightnessUp { "5%+" } else { "5%-" };
            let ok = run(&["brightnessctl", "--class=backlight", "set", step]).await;
            let level = run(&["brightnessctl", "--class=backlight", "-m", "info"]).await;
            let reading = match (ok, level.as_deref().and_then(parse_brightnessctl)) {
                (Some(_), Some(percent)) => Reading::Level { percent, muted: false },
                _ => Reading::Unavailable("Brightness can’t be changed here"),
            };
            Outcome { name: "Brightness", reading, output_muted: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_wpctl() {
        assert_eq!(parse_wpctl("Volume: 0.45\n"), Some((45, false)));
        assert_eq!(parse_wpctl("Volume: 1.00 [MUTED]"), Some((100, true)));
        assert_eq!(parse_wpctl("garbage"), None);
    }

    #[test]
    fn reads_brightnessctl() {
        assert_eq!(parse_brightnessctl("intel_backlight,backlight,1200,50%,2400\n"), Some(50));
        assert_eq!(parse_brightnessctl(""), None);
    }

    #[test]
    fn parses_commands() {
        let words = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(Control::parse(&words("volume up")), Some(Control::VolumeUp));
        assert_eq!(Control::parse(&words("mic-mute")), Some(Control::MicMute));
        assert_eq!(Control::parse(&words("volume")), None);
    }
}
