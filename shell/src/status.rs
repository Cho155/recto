//! System status the bar may mention: battery, network, sound. Plain data.
//! The bar stays quiet unless one of these needs attention.

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

/// What the bar should say right now, most urgent first. Empty most of the time.
pub fn alerts(status: &Status) -> Vec<Alert> {
    let mut out = Vec::new();
    if let Some(b) = &status.battery {
        if !b.charging && !b.full && b.percent <= LOW_BATTERY {
            let mut text = format!("Battery {:.0}%", b.percent);
            if let Some(secs) = b.seconds_left.filter(|s| *s > 0) {
                text.push_str(&format!(" · about {}", duration(secs)));
            }
            out.push(Alert { text, urgent: b.percent <= CRITICAL_BATTERY });
        }
    }
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

    #[test]
    fn quiet_when_all_is_well() {
        assert!(alerts(&battery(80.0, false, Some(18_000))).is_empty());
        assert!(alerts(&Status { online: Some(true), muted: Some(false), ..Status::default() }).is_empty());
    }

    #[test]
    fn low_battery_speaks_up() {
        let a = alerts(&battery(15.0, false, Some(1_500)));
        assert_eq!(a, vec![Alert { text: "Battery 15% · about 25 min".into(), urgent: false }]);
        assert!(alerts(&battery(8.0, false, None))[0].urgent);
        assert!(alerts(&battery(8.0, true, None)).is_empty());
    }

    #[test]
    fn order_is_battery_offline_muted() {
        let status = Status { online: Some(false), muted: Some(true), ..battery(5.0, false, None) };
        let texts: Vec<String> = alerts(&status).into_iter().map(|a| a.text).collect();
        assert_eq!(texts, vec!["Battery 5%", "Offline", "Muted"]);
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
    }
}
