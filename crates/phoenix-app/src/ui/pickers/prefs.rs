use std::{collections::HashMap, path::PathBuf};

use serde_json::{Value, json};

use super::{PickItem, emoji, types::GuildEmoji};
use crate::model::Id;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub count: u32,
    pub last: i64,
}

#[derive(Debug, Default)]
pub struct Prefs {
    pub tone: u8,
    pub usage: HashMap<String, Usage>,
    path: Option<PathBuf>,
}

fn config_path() -> Option<PathBuf> {
    crate::paths::config_file("expressions.json")
}

pub fn key_of(item: &PickItem) -> String {
    match item {
        PickItem::Unicode(ix) => format!("u:{}", emoji::glyph(*ix)),
        PickItem::Custom(e) => format!("c:{}:{}:{}", e.id.0, e.name, u8::from(e.animated)),
    }
}

pub fn item_of(key: &str) -> Option<PickItem> {
    if let Some(g) = key.strip_prefix("u:") {
        return emoji::index_of(g).map(PickItem::Unicode);
    }
    let mut it = key.strip_prefix("c:")?.split(':');
    let id: u64 = it.next()?.parse().ok()?;
    let name = it.next()?.to_owned();
    let animated = it.next()? == "1";
    Some(PickItem::Custom(GuildEmoji { id: Id(id), name: name.into(), animated, guild: Id(0), available: true }))
}

impl Prefs {
    pub fn load() -> Self {
        let path = config_path();
        let mut p = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| Self::from_json(&v))
            .unwrap_or_default();
        p.path = path;
        p
    }

    pub fn from_json(v: &Value) -> Self {
        let tone = v["tone"].as_u64().unwrap_or(0).min(5) as u8;
        let usage = v["usage"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, u)| {
                        let usage = Usage {
                            count: u["count"].as_u64().unwrap_or(1) as u32,
                            last: u["last"].as_i64().unwrap_or(0),
                        };
                        (k.clone(), usage)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { tone, usage, path: None }
    }

    pub fn to_json(&self) -> Value {
        let usage: serde_json::Map<String, Value> = self
            .usage
            .iter()
            .map(|(k, u)| (k.clone(), json!({ "count": u.count, "last": u.last })))
            .collect();
        json!({ "tone": self.tone, "usage": usage })
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, self.to_json().to_string());
    }

    pub fn set_tone(&mut self, tone: u8) {
        self.tone = tone.min(5);
        self.save();
    }

    pub fn record(&mut self, item: &PickItem) {
        self.record_at(item, chrono::Utc::now().timestamp());
        if self.usage.len() > 200 {
            let now = chrono::Utc::now().timestamp();
            let mut keys: Vec<(f64, String)> = self.usage.iter().map(|(k, u)| (score(u, now), k.clone())).collect();
            keys.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, k) in keys.into_iter().take(self.usage.len() - 150) {
                self.usage.remove(&k);
            }
        }
        self.save();
    }

    pub fn record_at(&mut self, item: &PickItem, now: i64) {
        let u = self.usage.entry(key_of(item)).or_default();
        u.count += 1;
        u.last = now;
    }

    pub fn frequent(&self, limit: usize, now: i64) -> Vec<PickItem> {
        let mut v: Vec<(f64, &String)> = self.usage.iter().map(|(k, u)| (score(u, now), k)).collect();
        v.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        v.into_iter().filter_map(|(_, k)| item_of(k)).take(limit).collect()
    }
}

fn score(u: &Usage, now: i64) -> f64 {
    let days = (now - u.last).max(0) as f64 / 86_400.;
    let weight = if days < 1. {
        100.
    } else if days < 3. {
        70.
    } else if days < 15. {
        50.
    } else if days < 30. {
        30.
    } else {
        10.
    };
    u.count as f64 * weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frecency_and_roundtrip() {
        let mut p = Prefs::default();
        let fire = PickItem::Unicode(emoji::index_of("🔥").unwrap());
        let joy = PickItem::Unicode(emoji::index_of("😂").unwrap());
        let blob = PickItem::Custom(GuildEmoji {
            id: Id(42),
            name: "blob".into(),
            animated: true,
            guild: Id(1),
            available: true,
        });
        let now = 1_000_000_000;
        p.record_at(&fire, now - 40 * 86_400);
        p.record_at(&fire, now - 40 * 86_400);
        p.record_at(&joy, now);
        p.record_at(&blob, now - 2 * 86_400);
        let f = p.frequent(10, now);
        assert_eq!(f.len(), 3);
        assert_eq!(key_of(&f[0]), "u:😂");
        assert_eq!(key_of(&f[2]), "u:🔥");
        p.tone = 2;
        let back = Prefs::from_json(&p.to_json());
        assert_eq!(back.tone, 2);
        assert_eq!(back.usage, p.usage);
        match item_of("c:42:blob:1").unwrap() {
            PickItem::Custom(e) => assert!(e.animated && e.name == "blob"),
            _ => panic!(),
        }
    }
}
