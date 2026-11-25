use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, svg,
};
use serde_json::{Value, json};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{Palette, c, ca, z},
    ui::home::data::{NavReply, NavRequest},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnboardingOption {
    pub id: String,
    pub title: String,
    pub description: String,
    pub channel_ids: Vec<String>,
    pub role_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnboardingPrompt {
    pub id: String,
    pub title: String,
    pub single_select: bool,
    pub required: bool,
    pub options: Vec<OnboardingOption>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnboardingData {
    pub guild_id: String,
    pub enabled: bool,
    pub default_channel_ids: Vec<String>,
    pub prompts: Vec<OnboardingPrompt>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberVerification {
    pub version: String,
    pub description: String,
    pub rules: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationGate {
    None,
    Email,
    Wait5Min,
    Wait10Min,
    Phone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveScreen {
    Onboarding(Id),
    Rules(Id),
}

#[derive(Default)]
struct JoinFlowState {
    lurking: HashSet<Id>,
    nsfw_accepted_channels: HashSet<Id>,
    nsfw_accepted_guilds: HashSet<Id>,
    verification_levels: HashMap<Id, u8>,
    active_screen: Option<ActiveScreen>,
    onboarding: HashMap<Id, OnboardingData>,
    onboarding_step: usize,
    onboarding_selected: HashMap<String, HashSet<String>>,
    verification: HashMap<Id, MemberVerification>,
    rules_accepted: bool,
    error: Option<SharedString>,
    busy: bool,
}

thread_local! {
    static STATE: RefCell<JoinFlowState> = RefCell::new(JoinFlowState::default());
}

pub fn mark_lurking(guild: Id) {
    STATE.with(|s| {
        s.borrow_mut().lurking.insert(guild);
    });
}

pub fn clear_lurking(guild: Id) {
    STATE.with(|s| {
        s.borrow_mut().lurking.remove(&guild);
    });
}

pub fn is_lurking(guild: Id) -> bool {
    STATE.with(|s| s.borrow().lurking.contains(&guild))
}

pub fn set_verification_level(guild: Id, level: u8) {
    STATE.with(|s| {
        s.borrow_mut().verification_levels.insert(guild, level);
    });
}

pub fn verification_gate(level: u8) -> VerificationGate {
    match level {
        1 => VerificationGate::Email,
        2 => VerificationGate::Wait5Min,
        3 => VerificationGate::Wait10Min,
        4 => VerificationGate::Phone,
        _ => VerificationGate::None,
    }
}

pub fn is_nsfw_accepted(channel: Id, guild: Option<Id>) -> bool {
    STATE.with(|s| {
        let st = s.borrow();
        st.nsfw_accepted_channels.contains(&channel)
            || guild.is_some_and(|g| st.nsfw_accepted_guilds.contains(&g))
    })
}

pub fn accept_nsfw_channel(channel: Id) {
    STATE.with(|s| {
        s.borrow_mut().nsfw_accepted_channels.insert(channel);
    });
}

pub fn accept_nsfw_guild(guild: Id) {
    STATE.with(|s| {
        s.borrow_mut().nsfw_accepted_guilds.insert(guild);
    });
}

pub fn parse_onboarding(v: &Value) -> OnboardingData {
    let guild_id = v.get("guild_id").and_then(Value::as_str).unwrap_or_default().to_owned();
    let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let default_channel_ids = v
        .get("default_channel_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let prompts = v
        .get("prompts")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|p| OnboardingPrompt {
                    id: p.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    title: p.get("title").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    single_select: p.get("single_select").and_then(Value::as_bool).unwrap_or(false),
                    required: p.get("required").and_then(Value::as_bool).unwrap_or(false),
                    options: p
                        .get("options")
                        .and_then(Value::as_array)
                        .map(|opts| {
                            opts.iter()
                                .map(|o| OnboardingOption {
                                    id: o.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
                                    title: o.get("title").and_then(Value::as_str).unwrap_or_default().to_owned(),
                                    description: o
                                        .get("description")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_owned(),
                                    channel_ids: o
                                        .get("channel_ids")
                                        .and_then(Value::as_array)
                                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                                        .unwrap_or_default(),
                                    role_ids: o
                                        .get("role_ids")
                                        .and_then(Value::as_array)
                                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                                        .unwrap_or_default(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    OnboardingData { guild_id, enabled, default_channel_ids, prompts }
}

pub fn onboarding_response_body(option_ids: &[String], prompt_ids: &[String], now_secs: u64) -> Value {
    let mut responses_seen = serde_json::Map::new();
    for id in option_ids {
        responses_seen.insert(id.clone(), json!(now_secs));
    }
    let mut prompts_seen = serde_json::Map::new();
    for id in prompt_ids {
        prompts_seen.insert(id.clone(), json!(now_secs));
    }
    json!({
        "onboarding_responses": option_ids,
        "onboarding_prompts_seen": prompts_seen,
        "onboarding_responses_seen": responses_seen,
    })
}

pub fn parse_member_verification(v: &Value) -> MemberVerification {
    let version = v.get("version").and_then(Value::as_str).unwrap_or_default().to_owned();
    let description = v.get("description").and_then(Value::as_str).unwrap_or_default().to_owned();
    let rules = v
        .get("form_fields")
        .and_then(Value::as_array)
        .and_then(|fields| {
            fields.iter().find(|f| f.get("field_type").and_then(Value::as_str) == Some("TERMS"))
        })
        .and_then(|terms| terms.get("values").and_then(Value::as_array))
        .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    MemberVerification { version, description, rules }
}

pub fn member_verification_submit_body(version: &str, rules: &[String]) -> Value {
    json!({
        "version": version,
        "form_fields": [
            {
                "field_type": "TERMS",
                "label": "Read and agree to the server rules",
                "required": true,
                "values": rules,
                "response": true,
            }
        ]
    })
}

pub fn open_onboarding(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.active_screen = Some(ActiveScreen::Onboarding(guild));
        st.onboarding_step = 0;
        st.onboarding_selected.clear();
        st.error = None;
    });
    app.nav_request(NavRequest::FetchOnboarding(guild), cx, move |_, res, cx| {
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            match res {
                Ok(NavReply::Json(v)) => {
                    st.onboarding.insert(guild, parse_onboarding(&v));
                }
                Ok(_) => {}
                Err(e) => st.error = Some(e.0.into()),
            }
        });
        cx.notify();
    });
    cx.notify();
}

pub fn open_rules(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.active_screen = Some(ActiveScreen::Rules(guild));
        st.rules_accepted = false;
        st.error = None;
    });
    app.nav_request(NavRequest::FetchMemberVerification(guild), cx, move |_, res, cx| {
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            match res {
                Ok(NavReply::Json(v)) => {
                    st.verification.insert(guild, parse_member_verification(&v));
                }
                Ok(_) => {}
                Err(e) => st.error = Some(e.0.into()),
            }
        });
        cx.notify();
    });
    cx.notify();
}

pub fn close_screen(cx: &mut Context<PhoenixApp>) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.active_screen = None;
        st.busy = false;
        st.error = None;
    });
    cx.notify();
}

pub fn lurker_banner(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let gid = app.selected_guild?;
    if !is_lurking(gid) {
        return None;
    }
    let name = app.guilds.iter().find(|g| g.id == gid).map(|g| g.name.clone()).unwrap_or_default();
    Some(
        div()
            .id("lurker-preview-banner")
            .flex_none()
            .px(z(16.))
            .py(z(8.))
            .bg(c(Palette::BLURPLE))
            .flex()
            .items_center()
            .justify_between()
            .gap(z(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .child(crate::t!("join_flow.preview_banner", name = name)),
            )
            .child(
                div()
                    .id("lurker-join-btn")
                    .px(z(14.))
                    .py(z(6.))
                    .rounded(z(4.))
                    .bg(gpui::white())
                    .text_color(c(Palette::BLURPLE))
                    .text_size(z(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        a.nav_request(NavRequest::JoinGuild(gid), cx, move |_, res, cx| {
                            if res.is_ok() {
                                clear_lurking(gid);
                                cx.notify();
                            }
                        });
                    }))
                    .child(crate::t!("join_flow.join_btn")),
            )
            .into_any_element(),
    )
}

pub fn verification_banner(app: &PhoenixApp, _cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let gid = app.selected_guild?;
    let level = STATE.with(|s| s.borrow().verification_levels.get(&gid).copied().unwrap_or(0));
    let gate = verification_gate(level);
    if gate == VerificationGate::None {
        return None;
    }
    let msg = match gate {
        VerificationGate::Email => crate::t!("join_flow.verify_email"),
        VerificationGate::Wait5Min => crate::t!("join_flow.verify_5m"),
        VerificationGate::Wait10Min => crate::t!("join_flow.verify_10m"),
        VerificationGate::Phone => crate::t!("join_flow.verify_phone"),
        VerificationGate::None => return None,
    };
    Some(
        div()
            .id("verification-level-banner")
            .flex_none()
            .px(z(16.))
            .py(z(8.))
            .bg(c(Palette::BG_SECONDARY))
            .border_b_1()
            .border_color(ca(0xFFFFFF14))
            .flex()
            .items_center()
            .gap(z(8.))
            .child(svg().path(crate::icons::path("warning")).size(z(16.)).text_color(c(Palette::YELLOW)))
            .child(div().text_size(z(13.)).text_color(c(Palette::TEXT_NORMAL)).child(msg))
            .into_any_element(),
    )
}

pub fn nsfw_gate(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let ch_id = app.selected_channel?;
    let is_nsfw = app.nav.extras.get(&ch_id).is_some_and(|e| e.nsfw);
    if !is_nsfw || is_nsfw_accepted(ch_id, app.selected_guild) {
        return None;
    }
    Some(
        div()
            .id("nsfw-age-gate")
            .absolute()
            .inset_0()
            .bg(c(Palette::BG_PRIMARY))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p(z(24.))
            .child(
                div()
                    .w(z(420.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(z(12.))
                    .child(
                        div()
                            .size(z(56.))
                            .rounded_full()
                            .bg(ca(0xDA373C26))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path(crate::icons::path("warning")).size(z(28.)).text_color(c(Palette::RED))),
                    )
                    .child(
                        div()
                            .text_size(z(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(Palette::TEXT_HEADER))
                            .text_center()
                            .child(crate::t!("join_flow.nsfw_title")),
                    )
                    .child(
                        div()
                            .text_size(z(14.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .text_center()
                            .child(crate::t!("join_flow.nsfw_desc")),
                    )
                    .child(
                        div()
                            .mt(z(8.))
                            .flex()
                            .items_center()
                            .gap(z(12.))
                            .child(
                                div()
                                    .id("nsfw-nope")
                                    .h(z(38.))
                                    .px(z(20.))
                                    .rounded(z(4.))
                                    .bg(c(Palette::BG_SECONDARY))
                                    .text_color(c(Palette::TEXT_HEADER))
                                    .text_size(z(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
                                    .on_click(cx.listener(|a, _, _, cx| {
                                        a.selected_channel = None;
                                        cx.notify();
                                    }))
                                    .child(crate::t!("join_flow.nsfw_nope")),
                            )
                            .child(
                                div()
                                    .id("nsfw-continue")
                                    .h(z(38.))
                                    .px(z(20.))
                                    .rounded(z(4.))
                                    .bg(c(Palette::RED))
                                    .text_color(gpui::white())
                                    .text_size(z(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        accept_nsfw_channel(ch_id);
                                        cx.notify();
                                    }))
                                    .child(crate::t!("join_flow.nsfw_continue")),
                            ),
                    ),
            )
            .into_any_element(),
    )
}

pub fn render_screen(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let active = STATE.with(|s| s.borrow().active_screen)?;
    match active {
        ActiveScreen::Onboarding(guild) => Some(render_onboarding(app, guild, cx)),
        ActiveScreen::Rules(guild) => Some(render_rules(app, guild, cx)),
    }
}

fn render_onboarding(_app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (data, step, selected, err, busy) = STATE.with(|s| {
        let st = s.borrow();
        (
            st.onboarding.get(&guild).cloned(),
            st.onboarding_step,
            st.onboarding_selected.clone(),
            st.error.clone(),
            st.busy,
        )
    });
    let mut body = div().p(z(24.)).flex().flex_col().gap(z(16.));
    if let Some(d) = data {
        let total = d.prompts.len();
        if step < total {
            let prompt = &d.prompts[step];
            let pid = prompt.id.clone();
            let single = prompt.single_select;
            let chosen = selected.get(&pid).cloned().unwrap_or_default();
            body = body
                .child(
                    div()
                        .text_size(z(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("join_flow.step_of", cur = step + 1, total = total)),
                )
                .child(
                    div()
                        .text_size(z(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(prompt.title.clone()),
                );
            let mut opts = div().flex().flex_col().gap(z(8.));
            for (idx, opt) in prompt.options.iter().enumerate() {
                let oid = opt.id.clone();
                let pid2 = pid.clone();
                let active = chosen.contains(&oid);
                opts = opts.child(
                    div()
                        .id(("onb-opt", idx))
                        .p(z(12.))
                        .rounded(z(8.))
                        .bg(if active { ca(0x5865F233) } else { c(Palette::BG_SECONDARY) })
                        .border_1()
                        .border_color(if active { c(Palette::BLURPLE) } else { ca(0xFFFFFF14) })
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            STATE.with(|s| {
                                let mut st = s.borrow_mut();
                                let set = st.onboarding_selected.entry(pid2.clone()).or_default();
                                if single {
                                    set.clear();
                                    set.insert(oid.clone());
                                } else if !set.remove(&oid) {
                                    set.insert(oid.clone());
                                }
                            });
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_size(z(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(Palette::TEXT_HEADER))
                                .child(opt.title.clone()),
                        )
                        .when(!opt.description.is_empty(), |el| {
                            el.child(
                                div()
                                    .mt(z(2.))
                                    .text_size(z(13.))
                                    .text_color(c(Palette::TEXT_MUTED))
                                    .child(opt.description.clone()),
                            )
                        }),
                );
            }
            body = body.child(opts);
        } else {
            body = body
                .child(
                    div()
                        .text_size(z(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(crate::t!("join_flow.onb_done_title")),
                )
                .child(
                    div()
                        .text_size(z(14.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("join_flow.onb_done_sub")),
                );
        }
        body = body
            .when_some(err, |d, e| d.child(div().text_size(z(12.)).text_color(c(Palette::RED)).child(e)))
            .child(
                div()
                    .mt(z(8.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .id("onb-close")
                            .px(z(12.))
                            .py(z(8.))
                            .cursor_pointer()
                            .text_size(z(14.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .on_click(cx.listener(|_, _, _, cx| close_screen(cx)))
                            .child(crate::t!("common.cancel")),
                    )
                    .child(
                        div()
                            .id("onb-next")
                            .h(z(38.))
                            .px(z(20.))
                            .rounded(z(4.))
                            .bg(c(Palette::BLURPLE))
                            .text_color(gpui::white())
                            .text_size(z(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .flex()
                            .items_center()
                            .when(!busy, |el| el.cursor_pointer())
                            .on_click(cx.listener(move |a, _, _, cx| {
                                if step + 1 < total {
                                    STATE.with(|s| s.borrow_mut().onboarding_step += 1);
                                    cx.notify();
                                } else if step < total {
                                    STATE.with(|s| s.borrow_mut().onboarding_step = total);
                                    cx.notify();
                                } else {
                                    let (options, prompts) = STATE.with(|s| {
                                        let mut st = s.borrow_mut();
                                        st.busy = true;
                                        let opts: Vec<String> =
                                            st.onboarding_selected.values().flatten().cloned().collect();
                                        let prm: Vec<String> = st
                                            .onboarding
                                            .get(&guild)
                                            .map(|o| o.prompts.iter().map(|p| p.id.clone()).collect())
                                            .unwrap_or_default();
                                        (opts, prm)
                                    });
                                    a.nav_request(
                                        NavRequest::SubmitOnboarding { guild, options, prompts },
                                        cx,
                                        |_, res, cx| {
                                            if res.is_ok() {
                                                close_screen(cx);
                                            } else if let Err(e) = res {
                                                STATE.with(|s| {
                                                    let mut st = s.borrow_mut();
                                                    st.busy = false;
                                                    st.error = Some(e.0.into());
                                                });
                                                cx.notify();
                                            }
                                        },
                                    );
                                }
                            }))
                            .child(if step < total {
                                crate::t!("join_flow.next")
                            } else {
                                crate::t!("join_flow.finish")
                            }),
                    ),
            );
    } else {
        body = body.child(
            div().text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("join_flow.loading")),
        );
    }
    crate::ui::invite::modal("onboarding-modal", cx, body)
}

fn render_rules(_app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (data, accepted, err, busy) = STATE.with(|s| {
        let st = s.borrow();
        (st.verification.get(&guild).cloned(), st.rules_accepted, st.error.clone(), st.busy)
    });
    let mut body = div().p(z(24.)).flex().flex_col().gap(z(12.));
    body = body.child(
        div()
            .text_size(z(22.))
            .font_weight(FontWeight::BOLD)
            .text_color(c(Palette::TEXT_HEADER))
            .child(crate::t!("join_flow.rules_title")),
    );
    if let Some(v) = data {
        if !v.description.is_empty() {
            body = body.child(div().text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(v.description.clone()));
        }
        let mut list = div().flex().flex_col().gap(z(8.));
        for (i, r) in v.rules.iter().enumerate() {
            list = list.child(
                div()
                    .p(z(10.))
                    .rounded(z(6.))
                    .bg(c(Palette::BG_SECONDARY))
                    .text_size(z(14.))
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(format!("{}. {r}", i + 1)),
            );
        }
        body = body
            .child(list)
            .child(
                div()
                    .id("rules-accept-check")
                    .mt(z(8.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| {
                        STATE.with(|s| {
                            let mut st = s.borrow_mut();
                            st.rules_accepted = !st.rules_accepted;
                        });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(z(18.))
                            .rounded(z(4.))
                            .border_1()
                            .border_color(c(Palette::BLURPLE))
                            .when(accepted, |d| d.bg(c(Palette::BLURPLE))),
                    )
                    .child(
                        div()
                            .text_size(z(14.))
                            .text_color(c(Palette::TEXT_HEADER))
                            .child(crate::t!("join_flow.rules_agree")),
                    ),
            )
            .when_some(err, |d, e| d.child(div().text_size(z(12.)).text_color(c(Palette::RED)).child(e)))
            .child(
                div()
                    .mt(z(8.))
                    .flex()
                    .justify_end()
                    .gap(z(8.))
                    .child(
                        div()
                            .id("rules-submit")
                            .h(z(38.))
                            .px(z(20.))
                            .rounded(z(4.))
                            .bg(c(Palette::BLURPLE))
                            .text_color(gpui::white())
                            .text_size(z(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .flex()
                            .items_center()
                            .when(accepted && !busy, |d| d.cursor_pointer())
                            .when(!accepted || busy, |d| d.opacity(0.5))
                            .on_click(cx.listener(move |a, _, _, cx| {
                                if !accepted || busy {
                                    return;
                                }
                                STATE.with(|s| s.borrow_mut().busy = true);
                                a.nav_request(
                                    NavRequest::SubmitMemberVerification {
                                        guild,
                                        version: v.version.clone(),
                                        rules: v.rules.clone(),
                                    },
                                    cx,
                                    |_, res, cx| {
                                        if res.is_ok() {
                                            close_screen(cx);
                                        } else if let Err(e) = res {
                                            STATE.with(|s| {
                                                let mut st = s.borrow_mut();
                                                st.busy = false;
                                                st.error = Some(e.0.into());
                                            });
                                            cx.notify();
                                        }
                                    },
                                );
                            }))
                            .child(crate::t!("join_flow.rules_submit")),
                    ),
            );
    } else {
        body = body.child(
            div().text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("join_flow.loading")),
        );
    }
    crate::ui::invite::modal("rules-modal", cx, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_onboarding_and_builds_response() {
        let raw = json!({
            "guild_id": "123",
            "enabled": true,
            "default_channel_ids": ["10", "20"],
            "prompts": [{
                "id": "p1",
                "title": "Pick your roles",
                "single_select": false,
                "required": true,
                "options": [{
                    "id": "o1",
                    "title": "Rust",
                    "description": "Rustaceans",
                    "channel_ids": ["30"],
                    "role_ids": ["40"]
                }]
            }]
        });
        let data = parse_onboarding(&raw);
        assert!(data.enabled);
        assert_eq!(data.prompts.len(), 1);
        assert_eq!(data.prompts[0].options[0].role_ids, vec!["40"]);

        let body = onboarding_response_body(&["o1".into()], &["p1".into()], 1700000000);
        assert_eq!(body["onboarding_responses"], json!(["o1"]));
        assert_eq!(body["onboarding_prompts_seen"]["p1"], json!(1700000000));
        assert_eq!(body["onboarding_responses_seen"]["o1"], json!(1700000000));
    }

    #[test]
    fn parses_member_verification_and_builds_submit() {
        let raw = json!({
            "version": "2026-01-01T00:00:00Z",
            "description": "Welcome!",
            "form_fields": [{
                "field_type": "TERMS",
                "values": ["Be nice", "No spam"]
            }]
        });
        let mv = parse_member_verification(&raw);
        assert_eq!(mv.version, "2026-01-01T00:00:00Z");
        assert_eq!(mv.rules, vec!["Be nice", "No spam"]);

        let body = member_verification_submit_body(&mv.version, &mv.rules);
        assert_eq!(body["version"], "2026-01-01T00:00:00Z");
        assert_eq!(body["form_fields"][0]["response"], json!(true));
        assert_eq!(body["form_fields"][0]["values"], json!(["Be nice", "No spam"]));
    }

    #[test]
    fn verification_levels_and_nsfw_and_lurker_state() {
        assert_eq!(verification_gate(0), VerificationGate::None);
        assert_eq!(verification_gate(1), VerificationGate::Email);
        assert_eq!(verification_gate(2), VerificationGate::Wait5Min);
        assert_eq!(verification_gate(3), VerificationGate::Wait10Min);
        assert_eq!(verification_gate(4), VerificationGate::Phone);

        let g = Id(999);
        let ch = Id(888);
        assert!(!is_lurking(g));
        mark_lurking(g);
        assert!(is_lurking(g));
        clear_lurking(g);
        assert!(!is_lurking(g));

        assert!(!is_nsfw_accepted(ch, Some(g)));
        accept_nsfw_channel(ch);
        assert!(is_nsfw_accepted(ch, Some(g)));
        let ch2 = Id(889);
        accept_nsfw_guild(g);
        assert!(is_nsfw_accepted(ch2, Some(g)));
    }
}
