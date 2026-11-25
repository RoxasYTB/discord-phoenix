use std::collections::BTreeMap;

use reqwest::Method;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::Snowflake;

fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> std::result::Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct ReportButton {
    #[serde(rename = "type", default, deserialize_with = "nullable")]
    pub kind: String,
    #[serde(default)]
    pub target: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportElement {
    #[serde(default, deserialize_with = "nullable")]
    pub name: String,
    #[serde(rename = "type", default, deserialize_with = "nullable")]
    pub kind: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default, deserialize_with = "nullable")]
    pub should_submit_data: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub require_all_options: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub is_localized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckboxOption {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub preselected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLink {
    pub text: String,
    pub url: String,
    pub description: Option<String>,
}

impl ReportElement {
    pub fn checkbox_options(&self) -> Vec<CheckboxOption> {
        let Some(rows) = self.data.as_array() else { return Vec::new() };
        rows.iter()
            .filter_map(|r| {
                let r = r.as_array()?;
                let s = |i: usize| r.get(i).and_then(Value::as_str).map(str::to_owned);
                Some(CheckboxOption {
                    key: s(0)?,
                    label: s(1)?,
                    description: s(2).filter(|d| !d.is_empty()),
                    preselected: r.get(3).and_then(Value::as_bool).unwrap_or(false),
                })
            })
            .collect()
    }

    pub fn link(&self) -> Option<ExternalLink> {
        let d = &self.data;
        let s = |k: &str| d.get(k).and_then(Value::as_str).filter(|v| !v.is_empty()).map(str::to_owned);
        Some(ExternalLink { text: s("link_text")?, url: s("url")?, description: s("link_description") })
    }

    pub fn link_localized(&self) -> bool {
        self.data.get("is_localized").and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn link_header_hidden(&self) -> bool {
        self.data.get("is_header_hidden").and_then(Value::as_bool).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportNode {
    #[serde(default)]
    pub id: u64,
    #[serde(default, deserialize_with = "nullable")]
    pub key: String,
    #[serde(default, deserialize_with = "nullable")]
    pub header: String,
    #[serde(default)]
    pub subheader: Option<String>,
    #[serde(default)]
    pub info: Option<String>,
    #[serde(default)]
    pub button: Option<ReportButton>,
    #[serde(default, deserialize_with = "nullable")]
    pub elements: Vec<ReportElement>,
    #[serde(default, deserialize_with = "nullable")]
    pub children: Vec<(String, u64)>,
    #[serde(default)]
    pub report_type: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub is_multi_select_required: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub is_auto_submit: bool,
}

impl ReportNode {
    pub fn element(&self, kind: &str) -> Option<&ReportElement> {
        self.elements.iter().find(|e| e.kind == kind)
    }

    pub fn has(&self, kind: &str) -> bool {
        self.element(kind).is_some()
    }

    pub fn links(&self) -> impl Iterator<Item = &ReportElement> {
        self.elements.iter().filter(|e| e.kind == "external_link")
    }

    pub fn button_kind(&self) -> Option<&str> {
        self.button.as_ref().map(|b| b.kind.as_str())
    }

    pub fn next_blocked(&self, picks: &[(String, String)]) -> bool {
        self.element("checkbox").is_some_and(|e| {
            (e.should_submit_data && picks.is_empty())
                || (e.require_all_options && e.checkbox_options().iter().any(|o| !picks.iter().any(|(k, _)| *k == o.key)))
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportMenu {
    #[serde(default, deserialize_with = "nullable")]
    pub name: String,
    #[serde(default, deserialize_with = "nullable")]
    pub variant: String,
    #[serde(default, deserialize_with = "nullable")]
    pub version: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub root_node_id: u64,
    #[serde(default)]
    pub success_node_id: u64,
    #[serde(default)]
    pub fail_node_id: u64,
    #[serde(default, deserialize_with = "nullable")]
    pub nodes: BTreeMap<String, ReportNode>,
}

impl ReportMenu {
    pub fn node(&self, id: u64) -> Option<&ReportNode> {
        self.nodes.get(&id.to_string())
    }

    pub fn resolve(&self, target: u64) -> u64 {
        let mut t = target;
        for _ in 0..self.nodes.len() {
            match self.node(t) {
                Some(n) if n.has("skip") && n.button_kind() == Some("next") => match n.button.as_ref().and_then(|b| b.target) {
                    Some(next) => t = next,
                    None => break,
                },
                _ => break,
            }
        }
        t
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportTarget {
    Message { channel: Snowflake, message: Snowflake },
    User { user: Snowflake, guild: Option<Snowflake> },
    Guild { guild: Snowflake },
}

impl ReportTarget {
    pub fn menu_name(self) -> &'static str {
        match self {
            Self::Message { .. } => "message",
            Self::User { .. } => "user",
            Self::Guild { .. } => "guild",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportStep {
    pub node: u64,
    pub label: String,
    pub target: u64,
    pub picks: Vec<(String, String)>,
}

pub fn report_body(menu: &ReportMenu, target: ReportTarget, steps: &[ReportStep]) -> Value {
    let mut elements = Map::new();
    for s in steps {
        if let Some(cb) = menu.node(s.node).and_then(|n| n.element("checkbox")) {
            elements.insert(cb.name.clone(), json!(s.picks.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()));
        }
    }
    let mut v = json!({
        "version": menu.version,
        "variant": menu.variant,
        "language": menu.language.as_deref().filter(|l| !l.is_empty()).unwrap_or("en"),
        "breadcrumbs": steps.iter().map(|s| s.node).collect::<Vec<_>>(),
        "elements": elements,
        "name": target.menu_name(),
    });
    match target {
        ReportTarget::Message { channel, message } => {
            v["channel_id"] = json!(channel);
            v["message_id"] = json!(message);
        }
        ReportTarget::User { user, guild } => {
            v["user_id"] = json!(user);
            if let Some(g) = guild {
                v["guild_id"] = json!(g);
            }
        }
        ReportTarget::Guild { guild } => v["guild_id"] = json!(guild),
    }
    v
}

impl RestClient {
    pub async fn get_report_menu(&self, target: ReportTarget) -> Result<ReportMenu> {
        self.call(Method::GET, &format!("/reporting/menu/{}", target.menu_name()), Body::None).await
    }

    pub async fn submit_report(&self, menu: &ReportMenu, target: ReportTarget, steps: &[ReportStep]) -> Result<()> {
        self.call_empty(Method::POST, &format!("/reporting/{}", target.menu_name()), Body::Json(report_body(menu, target, steps))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MENU: &str = r#"{"name":"message","variant":"8","version":"1.0","postback_url":"/api/reporting/message","root_node_id":7,"success_node_id":72,"fail_node_id":75,
        "nodes":{
          "7":{"id":7,"key":"MESSAGE_WELCOME","header":"Report Message","subheader":"Pick one","info":null,"button":{"type":"cancel","target":null},
               "elements":[{"name":"message_preview","type":"message_preview","data":null,"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true},
                           {"name":"external_link","type":"external_link","data":{"is_localized":true,"link_text":"Report illegal content","url":"https://discord.com/report","link_description":null,"is_header_hidden":true},"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":true,"is_localized":true}],
               "report_type":null,"children":[["I don't like it",99],["Spam",100],["Private info",81],["Something else",82]],"is_multi_select_required":false,"is_auto_submit":false},
          "17":{"id":17,"key":"GENERIC_SUBMIT","header":"Report Summary","subheader":"Review","info":null,"button":{"type":"submit","target":null},"elements":[{"name":"breadcrumbs","type":"breadcrumbs","data":null,"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true}],"report_type":null,"children":[],"is_multi_select_required":false,"is_auto_submit":false},
          "81":{"id":81,"key":"SHARING_PII","header":"What was shared?","subheader":null,"info":null,"button":{"type":"next","target":17},
                "elements":[{"name":"pii_select","type":"checkbox","data":[["face_pic","Face picture"],["ip_address","IP address"],["legal_name","Legal name","",true]],"should_submit_data":true,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true}],
                "report_type":null,"children":[],"is_multi_select_required":false,"is_auto_submit":false},
          "82":{"id":82,"key":"SOMETHING_ELSE","header":"How can we help?","subheader":"","info":null,"button":null,"elements":[],"report_type":null,"children":[["Self harm",162]],"is_multi_select_required":false,"is_auto_submit":false},
          "162":{"id":162,"key":"SELF_HARM_INTERSTITIAL","header":"Get help","subheader":"","info":null,"button":{"type":"next","target":95},"elements":[{"name":"skip","type":"skip","data":null,"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true}],"report_type":null,"children":[],"is_multi_select_required":false,"is_auto_submit":false},
          "95":{"id":95,"key":"SELF_HARM","header":"Tell us more","subheader":null,"info":"If there is imminent danger","button":null,"elements":[],"report_type":null,"children":[],"is_multi_select_required":false,"is_auto_submit":false},
          "99":{"id":99,"key":"MESSAGE_DISLIKE","header":"Thanks","subheader":"Thanks for flagging","info":null,"button":{"type":"done","target":null},"elements":[{"name":"block_users","type":"block_users","data":null,"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true}],"report_type":"sub_dont_like","children":[],"is_multi_select_required":false,"is_auto_submit":true},
          "100":{"id":100,"key":"MESSAGE_SPAM","header":"Report Summary","subheader":null,"info":null,"button":{"type":"submit","target":null},"elements":[],"report_type":"sub_spam","children":[],"is_multi_select_required":false,"is_auto_submit":false},
          "72":{"id":72,"key":"SETTINGS_UPSELLS_SUCCESS","header":"Report Sent","subheader":"Thanks","info":null,"button":{"type":"done","target":null},"elements":[{"name":"success","type":"success","data":null,"should_submit_data":false,"require_all_options":false,"skip_if_unlocalized":false,"is_localized":true}],"report_type":null,"children":[],"is_multi_select_required":false,"is_auto_submit":false}}}"#;

    fn menu() -> ReportMenu {
        serde_json::from_str(MENU).unwrap()
    }

    fn msg() -> ReportTarget {
        ReportTarget::Message { channel: Snowflake(5), message: Snowflake(6) }
    }

    #[test]
    fn parses_the_live_menu_shape() {
        let m = menu();
        assert_eq!(m.root_node_id, 7);
        assert_eq!(m.language, None);
        let root = m.node(7).unwrap();
        assert_eq!(root.children[1], ("Spam".to_owned(), 100));
        assert_eq!(root.button_kind(), Some("cancel"));
        assert!(root.has("message_preview"));
        let link = root.links().next().unwrap();
        assert!(link.link_localized() && link.link_header_hidden());
        assert_eq!(link.link().unwrap().url, "https://discord.com/report");
        assert!(m.node(99).unwrap().is_auto_submit);
        assert_eq!(m.node(82).unwrap().button, None);
    }

    #[test]
    fn checkbox_options_and_next_gate() {
        let m = menu();
        let n = m.node(81).unwrap();
        let opts = n.element("checkbox").unwrap().checkbox_options();
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[1].label, "IP address");
        assert!(opts[2].preselected && opts[2].description.is_none());
        assert!(n.next_blocked(&[]));
        assert!(!n.next_blocked(&[("ip_address".into(), "IP address".into())]));
        assert!(!m.node(100).unwrap().next_blocked(&[]));
    }

    #[test]
    fn skip_nodes_are_jumped_over() {
        let m = menu();
        assert_eq!(m.resolve(162), 95);
        assert_eq!(m.resolve(81), 81);
        assert_eq!(m.resolve(12345), 12345);
    }

    #[test]
    fn message_body_matches_the_client() {
        let m = menu();
        let steps = [
            ReportStep { node: 7, label: "Private info".into(), target: 81, picks: vec![] },
            ReportStep {
                node: 81,
                label: String::new(),
                target: 17,
                picks: vec![("ip_address".into(), "IP address".into()), ("face_pic".into(), "Face picture".into())],
            },
            ReportStep { node: 17, label: String::new(), target: 72, picks: vec![] },
        ];
        let v = report_body(&m, msg(), &steps);
        assert_eq!(
            v,
            json!({"version":"1.0","variant":"8","language":"en","breadcrumbs":[7,81,17],"elements":{"pii_select":["ip_address","face_pic"]},"name":"message","channel_id":"5","message_id":"6"})
        );
    }

    #[test]
    fn auto_submit_body() {
        let m = menu();
        let steps = [
            ReportStep { node: 7, label: "I don't like it".into(), target: 99, picks: vec![] },
            ReportStep { node: 99, label: String::new(), target: 99, picks: vec![] },
        ];
        let v = report_body(&m, msg(), &steps);
        assert_eq!(v["breadcrumbs"], json!([7, 99]));
        assert_eq!(v["elements"], json!({}));
    }

    #[test]
    fn user_and_guild_bodies() {
        let mut m = menu();
        m.language = Some("fr".into());
        let one = [ReportStep { node: 7, ..Default::default() }];
        let v = report_body(&m, ReportTarget::User { user: Snowflake(7), guild: Some(Snowflake(9)) }, &one);
        assert_eq!(v["user_id"], "7");
        assert_eq!(v["guild_id"], "9");
        assert_eq!(v["language"], "fr");
        assert!(v.get("reported_user_id").is_none());
        let v = report_body(&m, ReportTarget::User { user: Snowflake(7), guild: None }, &one);
        assert!(v.get("guild_id").is_none());
        let v = report_body(&m, ReportTarget::Guild { guild: Snowflake(3) }, &one);
        assert_eq!(v["name"], "guild");
        assert_eq!(v["guild_id"], "3");
    }

    #[test]
    fn null_fields_are_tolerated() {
        let m: ReportMenu = serde_json::from_str(r#"{"name":"user","variant":null,"version":"1.0","root_node_id":1,"success_node_id":2,"fail_node_id":3,"nodes":{"1":{"id":1,"key":"K","header":null,"elements":null,"children":null,"button":{"type":"next","target":2}}}}"#).unwrap();
        let n = m.node(1).unwrap();
        assert_eq!(n.header, "");
        assert!(n.children.is_empty() && n.elements.is_empty());
        assert_eq!(m.variant, "");
    }
}
