use std::process::Command;
use std::time::Duration;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct PulseDevice {
    pub name: String,
    pub description: String,
}

fn pactl_json(args: &[&str]) -> Option<Value> {
    let out = Command::new("pactl").args(["-f", "json"]).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

pub fn parse_devices(v: &Value) -> Vec<PulseDevice> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let name = d.get("name")?.as_str()?.to_owned();
                    if name.ends_with(".monitor") || d.get("monitor_of_sink").and_then(Value::as_str).is_some_and(|m| m != "n/a") {
                        return None;
                    }
                    let description = d.get("description").and_then(Value::as_str).unwrap_or(&name).to_owned();
                    Some(PulseDevice { name, description })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_text_descriptions(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(n) = line.strip_prefix("Name: ") {
            name = Some(n.trim().to_owned());
        } else if let Some(d) = line.strip_prefix("Description: ")
            && let Some(n) = name.take()
        {
            out.push((n, d.trim().to_owned()));
        }
    }
    out
}

fn text_descriptions(kind: &str) -> Vec<(String, String)> {
    Command::new("pactl")
        .env("LC_ALL", "C")
        .args(["list", kind])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_text_descriptions(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

pub fn devices(input: bool) -> Option<Vec<PulseDevice>> {
    let kind = if input { "sources" } else { "sinks" };
    let mut v = parse_devices(&pactl_json(&["list", kind])?);
    if v.iter().any(|d| d.description == "(null)" || d.description == d.name) {
        let text = text_descriptions(kind);
        for d in &mut v {
            if (d.description == "(null)" || d.description == d.name)
                && let Some((_, desc)) = text.iter().find(|(n, _)| *n == d.name)
            {
                d.description = desc.clone();
            }
        }
    }
    let def = Command::new("pactl")
        .arg(if input { "get-default-source" } else { "get-default-sink" })
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    if let Some(def) = def
        && let Some(i) = v.iter().position(|d| d.name == def)
    {
        let d = v.remove(i);
        v.insert(0, d);
    }
    Some(v)
}

pub fn node_for(input: bool, description: &str) -> Option<String> {
    devices(input)?.into_iter().find(|d| d.description == description).map(|d| d.name)
}

pub fn route_own_streams(input: bool, node: String) {
    std::thread::spawn(move || {
        let pid = std::process::id().to_string();
        let kind = if input { "source-outputs" } else { "sink-inputs" };
        let cmd = if input { "move-source-output" } else { "move-sink-input" };
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(150));
            let Some(list) = pactl_json(&["list", kind]) else { return };
            let ids: Vec<String> = list
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|s| s.pointer("/properties/application.process.id").and_then(Value::as_str) == Some(pid.as_str()))
                        .filter_map(|s| s.get("index").and_then(Value::as_u64).map(|i| i.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if !ids.is_empty() {
                for id in ids {
                    let _ = Command::new("pactl").args([cmd, &id, &node]).status();
                }
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_descriptions_keep_utf8() {
        let t = "Sink #64\n\tState: RUNNING\n\tName: alsa_output.usb\n\tDescription: Logitech G PRO X Gaming Headset St\u{e9}r\u{e9}o analogique\nSink #67\n\tName: alsa_output.pci\n\tDescription: Audio interne\n";
        assert_eq!(
            parse_text_descriptions(t),
            vec![
                ("alsa_output.usb".to_owned(), "Logitech G PRO X Gaming Headset St\u{e9}r\u{e9}o analogique".to_owned()),
                ("alsa_output.pci".to_owned(), "Audio interne".to_owned()),
            ]
        );
    }

    #[test]
    fn skips_monitors() {
        let v: Value = serde_json::from_str(
            r#"[{"name":"alsa_output.a","description":"Casque"},
                {"name":"alsa_output.a.monitor","description":"Monitor of Casque","monitor_of_sink":"a"},
                {"name":"alsa_input.b","description":"Micro","monitor_of_sink":"n/a"}]"#,
        )
        .unwrap();
        let d = parse_devices(&v);
        assert_eq!(d.iter().map(|d| d.description.as_str()).collect::<Vec<_>>(), ["Casque", "Micro"]);
    }
}
