//! Sound devices: outputs and inputs, their volume and the default ones.
//! Read and changed through `pactl` (PipeWire's PulseAudio layer), like the
//! rest of the shell's sound handling; nothing links against PipeWire.

use gtk::gio;
use std::ffi::OsStr;

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// PulseAudio's name, used to address the device.
    pub name: String,
    /// What people read: "Built-in Audio Analog Stereo".
    pub description: String,
    pub percent: u32,
    pub muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sound {
    pub outputs: Vec<Device>,
    pub inputs: Vec<Device>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
}

impl Sound {
    pub fn output(&self) -> Option<&Device> {
        pick(&self.outputs, self.default_output.as_deref())
    }

    pub fn input(&self) -> Option<&Device> {
        pick(&self.inputs, self.default_input.as_deref())
    }
}

fn pick<'a>(devices: &'a [Device], default: Option<&str>) -> Option<&'a Device> {
    devices.iter().find(|d| Some(d.name.as_str()) == default).or(devices.first())
}

/// 65536 is 100% in PulseAudio's volume scale.
const NORM: f64 = 65536.0;

/// Reads `pactl --format=json list sinks` (or `sources`). Monitors of outputs
/// are left out: they are not microphones.
pub fn parse_devices(json: &str) -> Option<Vec<Device>> {
    let list: serde_json::Value = serde_json::from_str(json).ok()?;
    let devices = list
        .as_array()?
        .iter()
        .filter(|d| {
            let name = d["name"].as_str().unwrap_or("");
            d["properties"]["device.class"].as_str() != Some("monitor") && !name.ends_with(".monitor")
        })
        .filter_map(|d| {
            let channels: Vec<f64> = d["volume"].as_object()?.values().filter_map(|c| c["value"].as_f64()).collect();
            let average = if channels.is_empty() { 0.0 } else { channels.iter().sum::<f64>() / channels.len() as f64 };
            let name = d["name"].as_str()?.to_owned();
            Some(Device {
                description: d["description"].as_str().filter(|s| !s.is_empty()).unwrap_or(&name).to_owned(),
                name,
                percent: (average / NORM * 100.0).round() as u32,
                muted: d["mute"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    Some(devices)
}

/// The default output and input from `pactl --format=json info`.
pub fn parse_defaults(json: &str) -> (Option<String>, Option<String>) {
    let info: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let get = |key: &str| info[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    (get("default_sink_name"), get("default_source_name"))
}

async fn pactl(args: &[&str]) -> Option<String> {
    let argv: Vec<&OsStr> = std::iter::once("pactl").chain(args.iter().copied()).map(OsStr::new).collect();
    let process = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
    let (out, _) = process.communicate_utf8_future(None).await.ok()?;
    process.is_successful().then(|| out.map(|s| s.to_string()).unwrap_or_default())
}

/// Everything at once. `None` when pactl is missing or no sound server answers.
pub async fn read() -> Option<Sound> {
    let info = pactl(&["--format=json", "info"]).await?;
    let outputs = parse_devices(&pactl(&["--format=json", "list", "sinks"]).await?)?;
    let inputs = parse_devices(&pactl(&["--format=json", "list", "sources"]).await?)?;
    let (default_output, default_input) = parse_defaults(&info);
    Some(Sound { outputs, inputs, default_output, default_input })
}

fn kind(output: bool) -> &'static str {
    if output { "sink" } else { "source" }
}

pub async fn set_volume(output: bool, name: &str, percent: u32) {
    let value = format!("{}%", percent.min(100));
    pactl(&[&format!("set-{}-volume", kind(output)), name, &value]).await;
}

pub async fn set_muted(output: bool, name: &str, muted: bool) {
    pactl(&[&format!("set-{}-mute", kind(output)), name, if muted { "1" } else { "0" }]).await;
}

pub async fn set_default(output: bool, name: &str) {
    pactl(&[&format!("set-default-{}", kind(output)), name]).await;
}

/// The mixer to offer under "Sound settings…", when one is installed.
pub fn mixer() -> Option<&'static str> {
    ["pavucontrol", "pwvucontrol"].into_iter().find(|m| crate::bus::installed(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINKS: &str = r#"[
      {"name": "alsa_output.analog-stereo", "description": "Built-in Audio Analog Stereo", "mute": true,
       "volume": {"front-left": {"value": 65536}, "front-right": {"value": 65536}}, "properties": {"device.class": "sound"}},
      {"name": "bluez_output.headphones", "description": "WH-1000XM4", "mute": false,
       "volume": {"front-left": {"value": 29491}, "front-right": {"value": 29491}}, "properties": {}}
    ]"#;

    const SOURCES: &str = r#"[
      {"name": "alsa_output.analog-stereo.monitor", "description": "Monitor of Built-in Audio", "mute": false,
       "volume": {"mono": {"value": 65536}}, "properties": {"device.class": "monitor"}},
      {"name": "alsa_input.analog-stereo", "description": "Built-in Audio Analog Stereo", "mute": false,
       "volume": {"front-left": {"value": 19661}}, "properties": {"device.class": "sound"}}
    ]"#;

    #[test]
    fn devices_with_volume_and_mute() {
        let outputs = parse_devices(SINKS).unwrap();
        assert_eq!(outputs.len(), 2);
        assert_eq!((outputs[0].percent, outputs[0].muted), (100, true));
        assert_eq!((outputs[1].description.as_str(), outputs[1].percent), ("WH-1000XM4", 45));
    }

    #[test]
    fn monitors_are_not_inputs() {
        let inputs = parse_devices(SOURCES).unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!((inputs[0].name.as_str(), inputs[0].percent), ("alsa_input.analog-stereo", 30));
    }

    #[test]
    fn default_device_or_the_first() {
        let (out, input) = parse_defaults(r#"{"default_sink_name": "bluez_output.headphones", "default_source_name": ""}"#);
        let sound = Sound { outputs: parse_devices(SINKS).unwrap(), inputs: parse_devices(SOURCES).unwrap(), default_output: out, default_input: input };
        assert_eq!(sound.output().unwrap().description, "WH-1000XM4");
        assert_eq!(sound.input().unwrap().name, "alsa_input.analog-stereo");
        assert!(parse_devices("not json").is_none());
    }
}
