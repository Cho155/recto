//! System status the bar may mention: battery, network, sound. Plain data.
//! The bar keeps a quiet battery figure; the rest is said only when it needs attention.

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
    /// `None` until the default output has been read once.
    pub muted: Option<bool>,
    /// Something is recording from a microphone. `None` without pactl.
    pub mic_in_use: Option<bool>,
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
    if status.muted == Some(true) {
        out.push(Alert { text: "Muted".into(), urgent: false });
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
        assert!(alerts(&Status { online: Some(true), muted: Some(false), ..Status::default() }).is_empty());
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
    fn order_is_offline_muted() {
        let status = Status { online: Some(false), muted: Some(true), ..battery(5.0, false, None) };
        let texts: Vec<String> = alerts(&status).into_iter().map(|a| a.text).collect();
        assert_eq!(texts, vec!["Offline", "Muted"]);
    }

    #[test]
    fn microphone_in_use_is_mentioned_last() {
        let status = Status { muted: Some(true), mic_in_use: Some(true), ..Status::default() };
        let texts: Vec<String> = alerts(&status).into_iter().map(|a| a.text).collect();
        assert_eq!(texts, vec!["Muted", "Mic in use"]);
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
