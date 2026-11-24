use std::collections::BTreeMap;

use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::Snowflake;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportButton {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub target: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportNode {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub subheader: Option<String>,
    #[serde(default)]
    pub info: Option<String>,
    #[serde(default)]
    pub button: Option<ReportButton>,
    #[serde(default)]
    pub children: Vec<(String, u64)>,
    #[serde(default)]
    pub report_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReportMenu {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub root_node_id: u64,
    #[serde(default)]
    pub success_node_id: u64,
    #[serde(default)]
    pub fail_node_id: u64,
    #[serde(default)]
    pub nodes: BTreeMap<String, ReportNode>,
}

impl ReportMenu {
    pub fn node(&self, id: u64) -> Option<&ReportNode> {
        self.nodes.get(&id.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportTarget {
    Message { channel: Snowflake, message: Snowflake },
    User { user: Snowflake, guild: Option<Snowflake> },
}

impl ReportTarget {
    pub fn menu_name(self) -> &'static str {
        match self {
            Self::Message { .. } => "message",
            Self::User { .. } => "user",
        }
    }
}

pub fn report_body(menu: &ReportMenu, target: ReportTarget, breadcrumbs: &[u64]) -> Value {
    let mut v = json!({
        "version": menu.version,
        "variant": menu.variant,
        "language": menu.language,
        "breadcrumbs": breadcrumbs,
        "elements": {},
        "name": target.menu_name(),
    });
    match target {
        ReportTarget::Message { channel, message } => {
            v["channel_id"] = json!(channel);
            v["message_id"] = json!(message);
        }
        ReportTarget::User { user, guild } => {
            v["reported_user_id"] = json!(user);
            if let Some(g) = guild {
                v["guild_id"] = json!(g);
            }
        }
    }
    v
}

impl RestClient {
    pub async fn get_report_menu(&self, target: ReportTarget) -> Result<ReportMenu> {
        self.call(Method::GET, &format!("/reporting/menu/{}", target.menu_name()), Body::None).await
    }

    pub async fn submit_report(&self, menu: &ReportMenu, target: ReportTarget, breadcrumbs: &[u64]) -> Result<()> {
        self.call_empty(Method::POST, &format!("/reporting/{}", target.menu_name()), Body::Json(report_body(menu, target, breadcrumbs))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MENU: &str = r#"{"name":"message","variant":"4","version":"1.0","language":"en","root_node_id":1,"success_node_id":9,"fail_node_id":10,
        "nodes":{"1":{"id":1,"key":"start","header":"What's going on?","children":[["Spam",2],["Harassment",3]]},
                 "2":{"id":2,"key":"spam","header":"Spam","button":{"type":"submit","target":9}},
                 "9":{"id":9,"key":"done","header":"Thanks","button":{"type":"done"}}}}"#;

    #[test]
    fn menu_parse() {
        let m: ReportMenu = serde_json::from_str(MENU).unwrap();
        assert_eq!(m.root_node_id, 1);
        let root = m.node(1).unwrap();
        assert_eq!(root.children[1], ("Harassment".to_owned(), 3));
        assert_eq!(m.node(2).unwrap().button.as_ref().unwrap().kind, "submit");
    }

    #[test]
    fn body() {
        let m: ReportMenu = serde_json::from_str(MENU).unwrap();
        let v = report_body(&m, ReportTarget::Message { channel: Snowflake(5), message: Snowflake(6) }, &[1, 2]);
        assert_eq!(v["breadcrumbs"], json!([1, 2]));
        assert_eq!(v["message_id"], "6");
        assert_eq!(v["name"], "message");
        assert_eq!(v["variant"], "4");
        let v = report_body(&m, ReportTarget::User { user: Snowflake(7), guild: None }, &[1]);
        assert_eq!(v["reported_user_id"], "7");
        assert_eq!(v["name"], "user");
    }
}
