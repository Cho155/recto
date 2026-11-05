//! System status the bar may mention: battery, network, Bluetooth, sound. Plain data.
//! The bar keeps quiet marks for these; words appear only when something needs attention.

use crate::glyph::Glyph;

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub percent: f64,
    pub charging: bool,
    pub full: bool,
    /// Seconds until empty while discharging, when UPower knows.
    pub seconds_left: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    /// `None` when there is no battery (or no UPower).
    pub battery: Option<Battery>,
    /// `None` when NetworkManager is not running.
    pub online: Option<bool>,
    pub network_name: Option<String>,
    /// Connected by cable: say so, rather than the connection's generated name.
    pub network_wired: bool,
    /// Something is recording from a microphone. `None` without pactl.
    pub mic_in_use: Option<bool>,
    /// `None` without NetworkManager or a Wi‑Fi device.
    pub wifi: Option<crate::wifi::Summary>,
    /// `None` without BlueZ or an adapter.
    pub bluetooth: Option<BluetoothState>,
    /// The default output. `None` until read, or without a sound server.
    pub volume: Option<Volume>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BluetoothState {
    pub powered: bool,
    /// Names of connected devices.
    pub connected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Volume {
    pub percent: u32,
    pub muted: bool,
}

/// How a mark on the bar is set: off (faint), quiet (secondary), active (text).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tone {
    Off,
    Quiet,
    Active,
}

/// One of the bar's small marks: what to draw, how strongly, and what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub glyph: Glyph,
    pub tone: Tone,
    /// Tooltip and screen reader: "Wi‑Fi: Home".
    pub spoken: String,
}

pub fn network_mark(status: &Status) -> Option<Mark> {
    let cable = status.network_wired && status.online == Some(true);
    let mark = |glyph, tone, spoken: &str| Some(Mark { glyph, tone, spoken: spoken.to_owned() });
    match &status.wifi {
        Some(w) if !w.enabled && cable => mark(Glyph::Wired { off: false }, Tone::Quiet, "Connected by cable; Wi‑Fi is off"),
        Some(w) if !w.enabled => mark(Glyph::Wifi { level: 0, off: true }, Tone::Off, "Wi‑Fi is off"),
        Some(w) => match &w.ssid {
            Some(ssid) => mark(Glyph::Wifi { level: crate::wifi::level(w.strength), off: false }, Tone::Quiet, &format!("Wi‑Fi: {ssid}")),
            None if cable => mark(Glyph::Wired { off: false }, Tone::Quiet, "Connected by cable"),
            None => mark(Glyph::Wifi { level: 0, off: false }, Tone::Quiet, "Wi‑Fi: not connected"),
        },
        None if cable => mark(Glyph::Wired { off: false }, Tone::Quiet, "Connected by cable"),
        None if status.online.is_some() => mark(Glyph::Wired { off: true }, Tone::Off, "Not connected"),
        None => None,
    }
}

pub fn bluetooth_mark(status: &Status) -> Option<Mark> {
    let bt = status.bluetooth.as_ref()?;
    let (tone, spoken) = if !bt.powered {
        (Tone::Off, "Bluetooth is off".to_owned())
    } else if bt.connected.is_empty() {
        (Tone::Quiet, "Bluetooth is on".to_owned())
    } else {
        (Tone::Active, format!("Bluetooth: {}", bt.connected.join(", ")))
    };
    Some(Mark { glyph: Glyph::Bluetooth { off: !bt.powered }, tone, spoken })
}

pub fn sound_mark(status: &Status) -> Option<Mark> {
    let v = status.volume.as_ref()?;
    if v.muted {
        return Some(Mark { glyph: Glyph::Speaker { waves: 0, muted: true }, tone: Tone::Quiet, spoken: "Sound is muted".into() });
    }
    let waves = match v.percent {
        0 => 0,
        1..50 => 1,
        _ => 2,
    };
    Some(Mark { glyph: Glyph::Speaker { waves, muted: false }, tone: Tone::Quiet, spoken: format!("Volume {}%", v.percent) })
}


#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub text: String,
    pub urgent: bool,
}

pub const LOW_BATTERY: f64 = 20.0;
pub const CRITICAL_BATTERY: f64 = 10.0;

/// "25 min", "1 h 20 min", "5 h"
pub fn duration(seconds: i64) -> String {
    let minutes = (seconds.max(0) + 30) / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Charge {
    Plenty,
    Low,
    Critical,
}

/// The bar's battery figure: always there on a machine with a battery.
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    /// "96%"; when running low, "15% · about 25 min".
    pub text: String,
    /// On mains power (charging, or full): shown with a small bolt.
    pub plugged: bool,
    pub charge: Charge,
    /// The tooltip and what a screen reader says: "Battery 96%, plugged in".
    pub spoken: String,
}

pub fn battery_figure(b: &Battery) -> Figure {
    let plugged = b.charging || b.full;
    let percent = format!("{:.0}%", b.percent);
    let left = b.seconds_left.filter(|s| *s > 0 && !plugged).map(duration);
    let charge = if plugged || b.percent > LOW_BATTERY {
        Charge::Plenty
    } else if b.percent > CRITICAL_BATTERY {
        Charge::Low
    } else {
        Charge::Critical
    };
    let text = match (&left, charge) {
        (Some(left), Charge::Low | Charge::Critical) => format!("{percent} · about {left}"),
        _ => percent.clone(),
    };
    let spoken = if b.full {
        format!("Battery {percent}, plugged in")
    } else if b.charging {
        format!("Battery {percent}, charging")
    } else if let Some(left) = &left {
        format!("Battery {percent}, about {left} left")
    } else {
        format!("Battery {percent}")
    };
    Figure { text, plugged, charge, spoken }
}

/// What the bar should say right now, most urgent first. Empty most of the time.
/// (A low battery shows in the battery figure itself.)
pub fn alerts(status: &Status) -> Vec<Alert> {
    let mut out = Vec::new();
    if status.online == Some(false) {
        out.push(Alert { text: "Offline".into(), urgent: false });
    }
    if status.mic_in_use == Some(true) {
        out.push(Alert { text: "Mic in use".into(), urgent: false });
    }
    out
}

/// The battery and network half of Today's "Right now". Empty when nothing is known.
pub fn right_now(status: &Status) -> String {
    let mut parts = Vec::new();
    if let Some(b) = &status.battery {
        parts.push(if b.full {
            "Battery full.".to_owned()
        } else if b.charging {
            format!("Battery at {:.0}%, charging.", b.percent)
        } else if let Some(secs) = b.seconds_left.filter(|s| *s > 0) {
            format!("Battery at {:.0}%, about {} left.", b.percent, duration(secs))
        } else {
            format!("Battery at {:.0}%.", b.percent)
        });
    }
    match (status.online, status.network_name.as_deref()) {
        (Some(true), _) if status.network_wired => parts.push("Connected by cable.".to_owned()),
        (Some(true), Some(name)) if !name.is_empty() => parts.push(format!("Connected to {name}.")),
        (Some(true), _) => parts.push("Connected.".to_owned()),
        (Some(false), _) => parts.push("Offline.".to_owned()),
        (None, _) => {}
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(percent: f64, charging: bool, left: Option<i64>) -> Status {
        Status {
            battery: Some(Battery { percent, charging, full: false, seconds_left: left }),
            ..Status::default()
        }
    }

    fn figure(percent: f64, charging: bool, left: Option<i64>) -> Figure {
        battery_figure(battery(percent, charging, left).battery.as_ref().unwrap())
    }

    #[test]
    fn quiet_when_all_is_well() {
        assert!(alerts(&battery(80.0, false, Some(18_000))).is_empty());
        assert!(alerts(&Status { online: Some(true), ..Status::default() }).is_empty());
    }

    #[test]
    fn battery_figure_is_quiet_until_low() {
        let f = figure(82.0, false, Some(18_000));
        assert_eq!((f.text.as_str(), f.plugged, f.charge), ("82%", false, Charge::Plenty));
        assert_eq!(f.spoken, "Battery 82%, about 5 h left");
        let f = figure(15.0, false, Some(1_500));
        assert_eq!((f.text.as_str(), f.charge), ("15% · about 25 min", Charge::Low));
        let f = figure(8.0, false, None);
        assert_eq!((f.text.as_str(), f.charge), ("8%", Charge::Critical));
    }

    #[test]
    fn plugged_in_is_never_low() {
        let f = figure(8.0, true, Some(1_500));
        assert_eq!((f.text.as_str(), f.plugged, f.charge), ("8%", true, Charge::Plenty));
        assert_eq!(f.spoken, "Battery 8%, charging");
        let full = battery_figure(&Battery { percent: 96.0, charging: false, full: true, seconds_left: None });
        assert_eq!((full.text.as_str(), full.plugged, full.spoken.as_str()), ("96%", true, "Battery 96%, plugged in"));
    }

    #[test]
    fn low_battery_is_not_an_alert_too() {
        assert!(alerts(&battery(5.0, false, None)).is_empty());
    }

    #[test]
    fn offline_then_microphone() {
        let status = Status { online: Some(false), mic_in_use: Some(true), ..battery(5.0, false, None) };
        let texts: Vec<String> = alerts(&status).into_iter().map(|a| a.text).collect();
        assert_eq!(texts, vec!["Offline", "Mic in use"]);
    }

    #[test]
    fn network_mark_follows_wifi_and_cable() {
        use crate::wifi::Summary;
        let wifi = |enabled, ssid: Option<&str>, strength| Some(Summary { enabled, ssid: ssid.map(Into::into), strength });
        let on = Status { online: Some(true), wifi: wifi(true, Some("Home"), 80), ..Status::default() };
        let m = network_mark(&on).unwrap();
        assert_eq!((m.glyph, m.tone, m.spoken.as_str()), (Glyph::Wifi { level: 3, off: false }, Tone::Quiet, "Wi‑Fi: Home"));
        let off = Status { online: Some(false), wifi: wifi(false, None, 0), ..Status::default() };
        assert_eq!(network_mark(&off).unwrap().tone, Tone::Off);
        let cable = Status { online: Some(true), network_wired: true, wifi: wifi(true, None, 0), ..Status::default() };
        assert_eq!(network_mark(&cable).unwrap().glyph, Glyph::Wired { off: false });
        assert!(network_mark(&Status::default()).is_none());
    }

    #[test]
    fn bluetooth_and_sound_marks() {
        let bt = |powered, connected: &[&str]| Status {
            bluetooth: Some(BluetoothState { powered, connected: connected.iter().map(|s| s.to_string()).collect() }),
            ..Status::default()
        };
        assert_eq!(bluetooth_mark(&bt(false, &[])).unwrap().glyph, Glyph::Bluetooth { off: true });
        let m = bluetooth_mark(&bt(true, &["WH-1000XM4"])).unwrap();
        assert_eq!((m.tone, m.spoken.as_str()), (Tone::Active, "Bluetooth: WH-1000XM4"));
        assert!(bluetooth_mark(&Status::default()).is_none());

        let vol = |percent, muted| Status { volume: Some(Volume { percent, muted }), ..Status::default() };
        assert_eq!(sound_mark(&vol(45, false)).unwrap().glyph, Glyph::Speaker { waves: 1, muted: false });
        assert_eq!(sound_mark(&vol(80, false)).unwrap().spoken, "Volume 80%");
        assert_eq!(sound_mark(&vol(80, true)).unwrap().glyph, Glyph::Speaker { waves: 0, muted: true });
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(1_500), "25 min");
        assert_eq!(duration(4_800), "1 h 20 min");
        assert_eq!(duration(18_000), "5 h");
    }

    #[test]
    fn right_now_sentence() {
        let s = Status { online: Some(true), network_name: Some("Home Wi-Fi".into()), ..battery(82.0, false, Some(18_000)) };
        assert_eq!(right_now(&s), "Battery at 82%, about 5 h left. Connected to Home Wi-Fi.");
        assert_eq!(right_now(&Status::default()), "");
        let cable = Status { online: Some(true), network_name: Some("Wired connection 1".into()), network_wired: true, ..Status::default() };
        assert_eq!(right_now(&cable), "Connected by cable.");
    }
}
