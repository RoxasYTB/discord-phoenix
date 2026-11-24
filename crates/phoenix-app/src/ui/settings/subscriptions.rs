use parking_lot::RwLock;
use serde_json::Value;

use crate::model::User;

pub const NITRO_APP: &str = "521842831262875670";

pub const OWNER: u64 = 1;

#[derive(Clone, Debug)]
pub struct Member {
    pub user: User,
    pub owner: bool,
    pub me: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Squad {
    pub members: Vec<Member>,
}

impl Squad {
    pub fn owner(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.owner)
    }
    pub fn me_owner(&self) -> bool {
        self.members.iter().any(|m| m.me && m.owner)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Credit {
    pub plan: &'static str,
    pub tier: u8,
    pub months: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Subs {
    pub squad: Option<Squad>,
    pub credits: Vec<Credit>,
}

static DATA: RwLock<Option<Subs>> = RwLock::new(None);

pub fn get() -> Option<Subs> {
    DATA.read().clone()
}

pub fn set(s: Subs) {
    *DATA.write() = Some(s);
}

pub fn fetch(app: &crate::app::PhoenixApp, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    if DATA.read().is_some() {
        return;
    }
    let fut = app.backend.subscriptions();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(s)) = fut.await {
            set(s);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

fn plan(id: &str) -> Option<(&'static str, u8, u32)> {
    Some(match id {
        "978380692553465866" => ("sp.su.plan_basic_month", 0, 1),
        "1024422698568122368" => ("sp.su.plan_basic_year", 0, 12),
        "511651880837840896" => ("sp.su.plan_nitro_month", 2, 1),
        "511651885459963904" => ("sp.su.plan_nitro_year", 2, 12),
        _ => return None,
    })
}

pub fn parse_credits(v: &Value) -> Vec<Credit> {
    let mut out: Vec<Credit> = Vec::new();
    for e in v.as_array().into_iter().flatten() {
        if e.get("consumed").and_then(Value::as_bool) == Some(true) || e.get("deleted").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let Some((key, tier, months)) = e.get("subscription_plan").and_then(|p| p.get("id")).and_then(Value::as_str).and_then(plan) else {
            continue;
        };
        match out.iter_mut().find(|c| c.plan == key) {
            Some(c) => c.months += months,
            None => out.push(Credit { plan: key, tier, months }),
        }
    }
    out
}

pub fn parse_members(v: &Value, me: u64) -> Squad {
    let mut members: Vec<Member> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let u: phoenix_api::models::User = serde_json::from_value(m.get("user")?.clone()).ok()?;
            let owner = m.get("member_type").and_then(Value::as_u64) == Some(OWNER);
            let me = u.id.0 == me;
            Some(Member { user: crate::discord_backend::convert::user(&u), owner, me })
        })
        .collect();
    members.sort_by_key(|m| !m.owner);
    Squad { members }
}

pub fn months_label(n: u32) -> gpui::SharedString {
    if n == 1 { crate::t!("sp.su.month1") } else { crate::t!("sp.su.months", n = n) }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn credits_group_per_plan() {
        let v = json!([
            {"type": 6, "consumed": false, "subscription_plan": {"id": "978380692553465866"}},
            {"type": 11, "consumed": false, "sku_id": "521847234246082599"},
            {"type": 6, "consumed": true, "subscription_plan": {"id": "978380692553465866"}},
            {"type": 6, "subscription_plan": {"id": "511651880837840896"}},
            {"type": 6, "consumed": false, "subscription_plan": {"id": "978380692553465866"}}
        ]);
        let c = super::parse_credits(&v);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].plan, c[0].tier, c[0].months), ("sp.su.plan_basic_month", 0, 2));
        assert_eq!((c[1].tier, c[1].months), (2, 1));
    }

    #[test]
    fn members_owner_first() {
        let u = |id: &str, n: &str| json!({"id": id, "username": n, "discriminator": "0", "avatar": null, "global_name": n});
        let v = json!([
            {"user": u("10", "a"), "member_type": 2},
            {"user": u("11", "b"), "member_type": 1},
            {"user": u("12", "me"), "member_type": 2}
        ]);
        let s = super::parse_members(&v, 12);
        assert_eq!(s.members.len(), 3);
        assert!(s.members[0].owner);
        assert_eq!(s.owner().map(|m| m.user.id.0), Some(11));
        assert!(s.members[2].me);
        assert!(!s.me_owner());
    }
}
