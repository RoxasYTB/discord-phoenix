use std::rc::Rc;

use gpui::{AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::{Input, InputState};
use phoenix_api::rest::guild_manage::{AdminCall, VIEW_CHANNEL};
use serde_json::{Value, json};

use super::{
    channel_perms_data::{self as data, PermSection},
    channel_settings::{Overwrite, State, with_state},
    settings_form::{description, label, text},
};
use crate::{
    actions::admin::sf,
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, button_sm, switch},
};

pub const ADMINISTRATOR: u64 = 1 << 3;

pub struct AddMembers {
    pub search: Option<Entity<InputState>>,
    pub picked: Vec<(Id, u8)>,
}

pub fn sections(kind: ChannelKind) -> &'static [PermSection] {
    match kind {
        ChannelKind::Text => data::TEXT,
        ChannelKind::Announcement => data::NEWS,
        ChannelKind::Voice => data::VOICE,
        ChannelKind::Stage => data::STAGE,
        ChannelKind::Forum => data::FORUM,
        ChannelKind::Category => data::CAT,
    }
}

pub fn matches(query: &str, label: &str, desc: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || label.to_lowercase().contains(&q) || desc.to_lowercase().contains(&q)
}

fn icon(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(color)
}

fn role_info(s: &State, id: Id) -> Option<(SharedString, Option<u32>, i64, u64)> {
    role_info_in(s.roles.as_deref(), id)
}

pub fn role_info_in(roles: Option<&[Value]>, id: Id) -> Option<(SharedString, Option<u32>, i64, u64)> {
    let r = roles?.iter().find(|r| r.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()) == Some(id.0))?;
    let num = |k: &str| r.get(k).and_then(|x| x.as_str().and_then(|s| s.parse().ok()).or_else(|| x.as_u64())).unwrap_or(0);
    let color =
        r.pointer("/colors/primary_color").and_then(Value::as_u64).or_else(|| r.get("color").and_then(Value::as_u64)).filter(|c| *c != 0).map(|c| c as u32);
    Some((
        r.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
        color,
        r.get("position").and_then(Value::as_i64).unwrap_or(0),
        num("permissions"),
    ))
}

fn role_color(color: Option<u32>) -> gpui::Hsla {
    match color {
        Some(v) => crate::theme::role_color(Some(v)),
        None => c(tok(idx::TEXT_SUBTLE)),
    }
}

pub fn target_name(app: &PhoenixApp, s: &State, o: &Overwrite) -> SharedString {
    if Some(o.id) == s.guild {
        return "@everyone".into();
    }
    if o.kind == 0 {
        role_info(s, o.id)
            .map(|r| r.0)
            .or_else(|| s.guild.and_then(|g| app.roles.get(&g)).and_then(|rs| rs.iter().find(|r| r.id == o.id)).map(|r| r.name.clone()))
            .unwrap_or_else(|| o.id.0.to_string().into())
    } else {
        app.users.get(&o.id).map(|u| u.display_name.clone()).unwrap_or_else(|| o.id.0.to_string().into())
    }
}

pub fn sidebar_order(app: &PhoenixApp, s: &State) -> Vec<Overwrite> {
    let pos = |o: &Overwrite| {
        role_info(s, o.id)
            .map(|r| r.2)
            .or_else(|| s.guild.and_then(|g| app.roles.get(&g)).and_then(|rs| rs.iter().find(|r| r.id == o.id)).map(|r| r.position as i64))
            .unwrap_or(0)
    };
    let mut roles: Vec<Overwrite> = s.overwrites.iter().filter(|o| o.kind == 0 && Some(o.id) != s.guild).cloned().collect();
    roles.sort_by_key(|o| -pos(o));
    let mut out = roles;
    out.push(s.everyone().cloned().unwrap_or(Overwrite { id: s.guild.unwrap_or(Id(0)), kind: 0, allow: 0, deny: 0 }));
    out.extend(s.overwrites.iter().filter(|o| o.kind == 1).cloned());
    out
}

fn sync_card(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let synced = s.synced()?;
    let cat = s.parent.and_then(|p| app.channels.get(&p)).map(|c| c.name.to_string()).unwrap_or_default();
    let key = if synced { "cs.synced_with" } else { "cs.not_synced_with" };
    let msg = crate::t!(key, category = "\u{1}");
    let (before, after) = msg.split_once('\u{1}').map(|(a, b)| (a.to_owned(), b.to_owned())).unwrap_or((msg.to_string(), String::new()));
    let line = div().flex_1().min_w_0().mx(z(10.)).text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(
        gpui::StyledText::new(format!("{before}{cat}{after}")).with_highlights([(
            before.len()..before.len() + cat.len(),
            gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() },
        )]),
    );
    let channel = s.channel;
    Some(
        div()
            .mt(z(16.))
            .p(z(16.))
            .rounded(z(12.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
            .flex()
            .items_center()
            .justify_between()
            .child(icon(
                if synced { "settings/synced" } else { "settings/unsynced" },
                20.,
                c(tok(if synced { idx::ICON_FEEDBACK_POSITIVE } else { idx::ICON_FEEDBACK_WARNING })),
            ))
            .child(line)
            .when(!synced, |d| {
                d.child(button_sm("cs-sync", crate::t!("cs.sync_now"), BtnKind::Secondary).on_click(cx.listener(move |a, _, _, cx| {
                    let mut body = None;
                    with_state(|s| {
                        if let Some(p) = s.parent_overwrites.clone() {
                            body = Some(json!({ "permission_overwrites": p.iter().map(Overwrite::json).collect::<Vec<_>>() }));
                            s.overwrites = p.clone();
                            s.overwrites0 = p;
                        }
                    });
                    if let Some(body) = body {
                        a.admin_toast(AdminCall::PatchChannel { channel: sf(channel), body }, None, cx);
                    }
                    cx.notify();
                })))
            })
            .into_any_element(),
    )
}

#[allow(clippy::too_many_arguments)]
fn access_row(
    id: impl Into<gpui::ElementId>,
    lead: AnyElement,
    name: SharedString,
    sub: Option<SharedString>,
    muted: bool,
    tag: SharedString,
    remove: Option<super::settings_form::Cb>,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let can = remove.is_some();
    div()
        .pb(z(16.))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(lead)
                .child(text(name, 14., 18., if muted { idx::TEXT_MUTED } else { idx::TEXT_DEFAULT }).truncate())
                .children(sub.map(|s| text(s, 12., 16., idx::TEXT_MUTED).max_w(z(250.)).truncate())),
        )
        .child(
            div().flex_none().flex().items_center().gap(z(4.)).child(text(tag, 12., 16., idx::TEXT_MUTED)).child(
                div()
                    .id(id.into())
                    .ml(z(4.))
                    .when(!can, |d| d.opacity(0.5))
                    .when(can, |d| d.cursor_pointer())
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if let Some(r) = &remove {
                            r(cx);
                        }
                        cx.notify();
                    }))
                    .child(icon("settings/circle-x", 16., c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
            ),
        )
        .into_any_element()
}

fn access_list(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut admins: Vec<(Id, SharedString, Option<u32>, i64)> = Vec::new();
    if let Some(rs) = &s.roles {
        for r in rs {
            let id = r.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Id);
            if let Some(id) = id
                && Some(id) != s.guild
                && let Some((n, col, pos, perms)) = role_info(s, id)
                && perms & ADMINISTRATOR != 0
            {
                admins.push((id, n, col, pos));
            }
        }
    }
    admins.sort_by_key(|a| -a.3);
    let mut granted: Vec<(Id, SharedString, Option<u32>, i64)> = s
        .overwrites
        .iter()
        .filter(|o| o.kind == 0 && Some(o.id) != s.guild && o.allow & VIEW_CHANNEL != 0 && !admins.iter().any(|a| a.0 == o.id))
        .map(|o| {
            let r = role_info(s, o.id);
            (o.id, target_name(app, s, o), r.as_ref().and_then(|r| r.1), r.map_or(0, |r| r.2))
        })
        .collect();
    granted.sort_by_key(|a| -a.3);
    let shield = |col: Option<u32>| icon("settings/shield-user", 20., role_color(col)).into_any_element();
    let mut list = div()
        .id("cs-access")
        .max_h(z(400.))
        .overflow_y_scroll()
        .px(z(16.))
        .pb(z(8.))
        .flex()
        .flex_col()
        .child(div().pb(z(16.)).child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))))
        .child(text(crate::t!("cs.roles"), 14., 18., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD).pb(z(16.)));
    for (n, (_, name, col, _)) in admins.into_iter().enumerate() {
        list = list.child(access_row(("cs-acc-a", n), shield(col), name, None, true, crate::t!("cs.administrator"), None, cx));
    }
    for (n, (id, name, col, _)) in granted.into_iter().enumerate() {
        let rm: Rc<dyn Fn(&mut App)> = Rc::new(move |_| {
            with_state(|s| {
                if let Some(o) = s.overwrites.iter_mut().find(|o| o.id == id) {
                    o.allow &= !VIEW_CHANNEL;
                }
            })
        });
        list = list.child(access_row(("cs-acc-r", n), shield(col), name, None, false, crate::t!("cs.role"), Some(rm), cx));
    }
    list = list
        .child(div().pb(z(16.)).child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))))
        .child(text(crate::t!("cs.members"), 14., 18., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD).pb(z(16.)));
    let owner = s.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).and_then(|g| g.owner);
    let avatar = |uid: Id| crate::ui::avatar(app.users.get(&uid), 20.);
    if let Some(o) = owner {
        let u = app.users.get(&o);
        let (name, handle) = (u.map(|u| u.display_name.clone()).unwrap_or_default(), u.map(|u| u.name.clone()));
        list = list.child(access_row("cs-acc-owner", avatar(o), name, handle, false, crate::t!("cs.owner"), None, cx));
    }
    for (n, o) in s.overwrites.iter().filter(|o| o.kind == 1 && o.allow & VIEW_CHANNEL != 0 && Some(o.id) != owner).enumerate() {
        let id = o.id;
        let u = app.users.get(&id);
        let rm: Rc<dyn Fn(&mut App)> = Rc::new(move |_| {
            with_state(|s| {
                if let Some(o) = s.overwrites.iter_mut().find(|o| o.id == id) {
                    o.allow &= !VIEW_CHANNEL;
                }
            })
        });
        list =
            list.child(access_row(("cs-acc-m", n), avatar(id), target_name(app, s, o), u.map(|u| u.name.clone()), false, crate::t!("cs.member"), Some(rm), cx));
    }
    list.into_any_element()
}

fn private_card(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let private = s.is_private();
    let cat = s.is_category();
    let voice = matches!(s.kind, ChannelKind::Voice | ChannelKind::Stage);
    let mut card = div()
        .mt(z(16.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(if private { idx::BACKGROUND_MOD_SUBTLE } else { idx::BACKGROUND_MOD_MUTED })))
        .child(
            div()
                .p(z(16.))
                .child(
                    div()
                        .id("cs-private")
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .group(crate::ui::controls::SWITCH_ROW)
                        .on_click(cx.listener(move |_, _, _, cx| {
                            with_state(|s| {
                                let Some(g) = s.guild else { return };
                                let bits = if voice { VIEW_CHANNEL | phoenix_api::rest::guild_manage::CONNECT } else { VIEW_CHANNEL };
                                let o = s.overwrite_mut(g, 0);
                                if o.deny & VIEW_CHANNEL != 0 {
                                    o.deny &= !bits;
                                } else {
                                    o.deny |= VIEW_CHANNEL;
                                    o.allow &= !VIEW_CHANNEL;
                                }
                            });
                            cx.notify();
                        }))
                        .child(icon("lock", 20., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                        .child(
                            text(crate::t!(if cat { "cs.private_category" } else { "cs.private_channel" }), 16., 20., idx::TEXT_DEFAULT)
                                .font_weight(FontWeight::SEMIBOLD)
                                .flex_1()
                                .mx(z(10.)),
                        )
                        .child(switch("cs-private-switch", private)),
                )
                .child(
                    text(crate::t!(if cat { "cs.private_category_desc" } else { "cs.private_channel_desc" }), 12., 16., idx::TEXT_DEFAULT).pt(z(8.)).pl(z(4.)),
                ),
        );
    if private {
        card = card.child(
            div()
                .rounded_b(z(12.))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .child(
                    div()
                        .p(z(16.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            text(crate::t!(if cat { "cs.who_category" } else { "cs.who_channel" }), 14., 18., idx::TEXT_DEFAULT)
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(button_sm("cs-add-members", crate::t!("cs.add_members"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| {
                            with_state(|s| s.add_members = Some(AddMembers { search: None, picked: Vec::new() }));
                            cx.notify();
                        }))),
                )
                .child(access_list(app, s, cx)),
        );
    }
    card.into_any_element()
}

fn tri(n: usize, bit: u64, state: i8, target: Id, kind: u8, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cell = |v: i8| {
        let on = state == v;
        let (glyph, fg, sel_bg, sel_fg) = match v {
            -1 => ("close-small", idx::ICON_FEEDBACK_CRITICAL, idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT, idx::CONTROL_CRITICAL_PRIMARY_TEXT_DEFAULT),
            0 => ("settings/slash", idx::TEXT_DEFAULT, idx::INTERACTIVE_BACKGROUND_SELECTED, idx::INTERACTIVE_ICON_ACTIVE),
            _ => ("check", idx::ICON_FEEDBACK_POSITIVE, idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT, idx::CONTROL_CONNECTED_TEXT_DEFAULT),
        };
        div()
            .id(("cs-tri", n * 3 + (v + 1) as usize))
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(c(tok(if on { sel_bg } else { idx::BACKGROUND_BASE_LOW })))
            .when(!on, |d| d.hover(|d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))))
            .when(v == -1, |d| d.rounded_l(z(8.)))
            .when(v == 1, |d| d.rounded_r(z(8.)))
            .on_click(cx.listener(move |_, _, _, cx| {
                with_state(|s| s.overwrite_mut(target, kind).set(bit, v));
                cx.notify();
            }))
            .child(icon(glyph, 16., c(tok(if on { sel_fg } else { fg }))))
    };
    div()
        .flex_none()
        .w(z(96.))
        .h(z(28.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .flex()
        .child(cell(-1))
        .child(cell(0))
        .child(cell(1))
        .into_any_element()
}

fn advanced(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let order = sidebar_order(app, s);
    let sel = s.selected.or(s.guild).unwrap_or(Id(0));
    let mut side = div().flex().flex_col().child(
        div()
            .id("cs-adv-add")
            .pt(z(10.))
            .px(z(10.))
            .pb(z(8.))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .on_click(cx.listener(|_, _, _, cx| {
                with_state(|s| s.add_members = Some(AddMembers { search: None, picked: Vec::new() }));
                cx.notify();
            }))
            .child(text(crate::t!("cs.roles_members"), 14., 18., idx::TEXT_MUTED).font_weight(FontWeight::MEDIUM))
            .child(icon("plus-medium", 16., c(tok(idx::TEXT_MUTED)))),
    );
    for (n, o) in order.iter().enumerate() {
        let on = o.id == sel;
        let id = o.id;
        let col = if o.kind == 0 && Some(o.id) != s.guild { Some(role_info(s, o.id).and_then(|r| r.1)) } else { None };
        side = side.child(
            div()
                .id(("cs-adv-role", n))
                .h(z(32.))
                .px(z(10.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(if on { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
                .when(!on, |d| d.cursor_pointer().hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).text_color(c(tok(idx::TEXT_STRONG)))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_state(|s| s.selected = Some(id));
                    cx.notify();
                }))
                .children(col.flatten().map(|col| div().flex_none().ml(z(4.)).mr(z(8.)).size(z(12.)).rounded_full().bg(role_color(Some(col)))))
                .when(o.kind == 1, |d| d.child(div().mr(z(8.)).child(crate::ui::avatar(app.users.get(&id), 20.))))
                .child(div().flex_1().min_w_0().truncate().child(target_name(app, s, o))),
        );
    }
    side = side.child(div().mt(z(20.)).mb(z(14.)).mx(z(10.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))).child(text(
        crate::t!("cs.perm_help"),
        14.,
        18.,
        idx::TEXT_LINK,
    ));
    let query = s.perm_search.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let target = order.iter().find(|o| o.id == sel).cloned().unwrap_or(Overwrite { id: sel, kind: 0, allow: 0, deny: 0 });
    let mut body = div().flex_1().min_w_0().pt(z(24.)).pr(z(15.)).pb(z(80.)).flex().flex_col();
    if let Some(i) = &s.perm_search {
        body = body.child(
            div()
                .mb(z(24.))
                .h(z(40.))
                .pl(z(4.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(div().size(z(30.)).flex().items_center().justify_center().child(icon("search", 16., c(tok(idx::TEXT_DEFAULT)))))
                .child(div().flex_1().pr(z(12.)).text_size(z(16.)).child(Input::new(i).appearance(false).bordered(false))),
        );
    }
    let mut n = 0;
    for sec in sections(s.kind) {
        let rows: Vec<_> = sec.rows.iter().filter(|r| matches(&query, &crate::t!(r.label), &crate::t!(r.desc))).collect();
        if rows.is_empty() {
            continue;
        }
        let mut fs =
            div().mb(z(40.)).flex().flex_col().child(text(crate::t!(sec.legend), 20., 24., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM).mb(z(12.)));
        let count = rows.len();
        for (k, r) in rows.into_iter().enumerate() {
            fs = fs.child(
                div()
                    .flex()
                    .items_start()
                    .gap(z(24.))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(label(crate::t!(r.label))).child(description(crate::t!(r.desc))))
                    .child(tri(n, r.bit, target.state(r.bit), target.id, target.kind, cx)),
            );
            n += 1;
            if k + 1 < count {
                fs = fs.child(div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))));
            }
        }
        body = body.child(fs);
    }
    if Some(target.id) != s.guild {
        let t = target.id;
        body = body.child(div().mt(z(40.)).child(
            button("cs-ow-remove", crate::t!("cs.remove_target", name = target_name(app, s, &target).to_string()), BtnKind::Critical).on_click(cx.listener(
                move |_, _, _, cx| {
                    with_state(|s| {
                        s.overwrites.retain(|o| o.id != t);
                        s.selected = None;
                    });
                    cx.notify();
                },
            )),
        ));
    }
    div()
        .flex()
        .flex_col()
        .child(div().mt(z(40.)).mb(z(31.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            div()
                .id("cs-adv-toggle")
                .flex()
                .items_center()
                .cursor_pointer()
                .on_click(cx.listener(|_, _, _, cx| {
                    with_state(|s| s.advanced_open = !s.advanced_open);
                    cx.notify();
                }))
                .child(text(crate::t!("cs.advanced"), 20., 24., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD))
                .child(
                    icon("chevron", 20., c(tok(idx::TEXT_DEFAULT)))
                        .ml(z(14.))
                        .when(!s.advanced_open, |i| i.with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2)))),
                ),
        )
        .when(s.advanced_open, |d| {
            d.child(div().flex().items_start().child(div().flex_none().w(z(232.)).pt(z(24.)).pr(z(15.)).pb(z(80.)).child(side)).child(body))
        })
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if !s.loaded {
        return text(crate::t!("common.loading"), 14., 18., idx::TEXT_SUBTLE).into_any_element();
    }
    let cat = s.is_category();
    div()
        .flex()
        .flex_col()
        .child(text(crate::t!(if cat { "cs.cat_perms_title" } else { "cs.perms_title" }), 20., 24., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD))
        .child(text(crate::t!(if cat { "cs.cat_perms_sub" } else { "cs.perms_sub" }), 14., 18., idx::TEXT_DEFAULT))
        .children(sync_card(app, s, cx))
        .child(private_card(app, s, cx))
        .child(advanced(app, s, cx))
        .into_any_element()
}

pub(super) fn sub_frame(
    id: &'static str,
    width: f32,
    title: SharedString,
    body: AnyElement,
    footer: AnyElement,
    close: crate::ui::modal::Action,
) -> AnyElement {
    let c2 = close.clone();
    let card = div()
        .relative()
        .w(z(width))
        .max_h(z(720.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(div().pt(z(16.)).px(z(24.)).pb(z(8.)).pr(z(60.)).child(text(title, 20., 24., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD)))
        .child(body)
        .child(div().pt(z(16.)).px(z(24.)).pb(z(8.)).flex().gap(z(8.)).child(footer))
        .child(
            div()
                .id(SharedString::from(format!("{id}-x")))
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(move |_, w, cx| close(w, cx))
                .child(icon("close", 20., c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    div()
        .id(SharedString::from(format!("{id}-scrim")))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(c(tok(idx::BACKGROUND_SCRIM)))
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| c2(w, cx))
        .child(
            div().id(SharedString::from(format!("{id}-card"))).occlude().on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(card),
        )
        .into_any_element()
}

pub type Toggle = Rc<dyn Fn(Id, u8, &mut App)>;

#[allow(clippy::too_many_arguments)]
pub fn picker_body(
    app: &PhoenixApp,
    guild: Option<Id>,
    roles: Option<&[Value]>,
    present: &[Id],
    picked: &[(Id, u8)],
    search: Option<&Entity<InputState>>,
    header: AnyElement,
    toggle: Toggle,
    cx: &App,
) -> AnyElement {
    let query = search.map(|i| i.read(cx).value().to_string()).unwrap_or_default().to_lowercase();
    let member_mode = query.starts_with('@');
    let q = query.trim_start_matches('@').to_owned();
    let mut rs: Vec<(Id, SharedString, Option<u32>, i64)> = roles
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = Id(r.get("id")?.as_str()?.parse().ok()?);
            let (n, col, pos, perms) = role_info_in(roles, id)?;
            (Some(id) != guild && perms & ADMINISTRATOR == 0 && !present.contains(&id) && r.get("managed").and_then(Value::as_bool) != Some(true))
                .then_some((id, n, col, pos))
        })
        .filter(|r| !member_mode && r.1.to_lowercase().contains(&q))
        .collect();
    rs.sort_by_key(|r| -r.3);
    let mut list = div().id("cs-am-list").flex_1().min_h_0().max_h(z(358.)).overflow_y_scroll().pt(z(12.)).pb(z(12.)).pl(z(24.)).pr(z(8.)).flex().flex_col();
    let row = |key: (&'static str, usize), lead: AnyElement, name: SharedString, tag: SharedString, id: Id, kind: u8, on: bool| {
        let t = toggle.clone();
        div()
            .id(key)
            .h(z(40.))
            .px(z(6.))
            .py(z(8.))
            .mr(z(4.))
            .rounded(z(4.))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
            .on_click(move |_, _, cx| t(id, kind, cx))
            .child(
                div().flex().items_center().pl(z(8.)).child(crate::ui::controls::checkbox_option("cs-am-check", "", on).w(z(18.))).child(
                    div()
                        .pl(z(8.))
                        .flex()
                        .items_center()
                        .child(lead)
                        .child(text(name, 14., 18., idx::TEXT_DEFAULT).font_weight(FontWeight::MEDIUM).ml(z(8.)).mr(z(4.)).truncate()),
                ),
            )
            .child(text(tag, 12., 16., idx::TEXT_MUTED))
    };
    if !rs.is_empty() {
        list = list.child(text(crate::t!("cs.roles"), 14., 18., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD).pt(z(8.)).h(z(32.)));
        for (n, (id, name, col, _)) in rs.into_iter().enumerate() {
            let on = picked.iter().any(|p| p.0 == id);
            list = list.child(row(
                ("cs-am-role", n),
                icon("settings/shield-user", 20., role_color(col)).into_any_element(),
                name,
                crate::t!("cs.role"),
                id,
                0,
                on,
            ));
        }
    }
    let members: Vec<(Id, SharedString)> = guild
        .and_then(|g| app.members.get(&g))
        .into_iter()
        .flatten()
        .filter(|m| !present.contains(&m.user))
        .filter_map(|m| app.users.get(&m.user).map(|u| (m.user, u.display_name.clone(), u.name.clone())))
        .filter(|(_, n, u)| n.to_lowercase().contains(&q) || u.to_lowercase().contains(&q))
        .map(|(id, n, _)| (id, n))
        .take(50)
        .collect();
    if !members.is_empty() {
        list = list.child(text(crate::t!("cs.members"), 14., 18., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD).pt(z(8.)).h(z(32.)));
        for (n, (id, name)) in members.into_iter().enumerate() {
            let on = picked.iter().any(|p| p.0 == id);
            list = list.child(row(("cs-am-user", n), crate::ui::avatar(app.users.get(&id), 20.), name, crate::t!("cs.member"), id, 1, on));
        }
    }
    div()
        .flex()
        .flex_col()
        .min_h_0()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .flex()
                .flex_col()
                .child(header)
                .children(search.map(|i| {
                    div()
                        .h(z(36.))
                        .px(z(9.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                        .flex()
                        .items_center()
                        .text_size(z(16.))
                        .child(Input::new(i).appearance(false).bordered(false))
                }))
                .child(text(crate::t!("cs.add_members_help"), 12., 16., idx::TEXT_DEFAULT).mt(z(4.))),
        )
        .child(list)
        .into_any_element()
}

pub fn picker_frame(id: &'static str, title: SharedString, body: AnyElement, footer: AnyElement, close: crate::ui::modal::Action) -> AnyElement {
    sub_frame(id, 480., title, body, footer, close)
}

pub fn add_members_modal(app: &PhoenixApp, s: &State, am: &AddMembers, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = {
        let e = e.clone();
        Rc::new(move |_, cx| {
            with_state(|s| s.add_members = None);
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let present: Vec<Id> = s.overwrites.iter().filter(|o| o.allow & VIEW_CHANNEL != 0).map(|o| o.id).collect();
    let toggle: Toggle = {
        let e = e.clone();
        Rc::new(move |id, kind, cx| {
            with_state(|s| {
                if let Some(a) = s.add_members.as_mut() {
                    if let Some(i) = a.picked.iter().position(|p| p.0 == id) {
                        a.picked.remove(i);
                    } else {
                        a.picked.push((id, kind));
                    }
                }
            });
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let header = div()
        .mb(z(8.))
        .flex()
        .justify_center()
        .items_center()
        .gap(z(4.))
        .child(icon(super::channel_settings::channel_icon(s.kind, s.is_private()), 20., c(tok(idx::TEXT_DEFAULT))))
        .child(text(s.name0.clone(), 20., 24., idx::TEXT_DEFAULT))
        .into_any_element();
    let body = picker_body(app, s.guild, s.roles.as_deref(), &present, &am.picked, am.search.as_ref(), header, toggle, cx);
    let done = {
        let e = e.clone();
        move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut App| {
            with_state(|s| {
                let picked = s.add_members.take().map(|a| a.picked).unwrap_or_default();
                for (id, kind) in picked {
                    let o = s.overwrite_mut(id, kind);
                    o.set(VIEW_CHANNEL, 1);
                    s.selected = Some(id);
                }
            });
            e.update(cx, |_, cx| cx.notify());
        }
    };
    let c3 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("cs-am-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| c3(w, cx)))
        .child(button("cs-am-done", crate::t!("cs.done"), BtnKind::Primary).flex_1().on_click(done))
        .into_any_element();
    picker_frame("cs-add-members", crate::t!("cs.add_members"), body, footer, close)
}

pub fn bold_range(body: &str, name: &str) -> Option<std::ops::Range<usize>> {
    body.find(name).filter(|_| !name.is_empty()).map(|i| i..i + name.len())
}

pub fn delete_modal(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        with_state(|s| s.confirm_delete = false);
        e.update(cx, |_, cx| cx.notify());
    });
    let cat = s.is_category();
    let channel = s.channel;
    let body = div()
        .px(z(24.))
        .pt(z(8.))
        .child({
            let body = crate::t!(if cat { "admin.delete_category_body" } else { "admin.delete_channel_body" }, name = s.name0.clone());
            let strong = if cat { s.name0.clone() } else { format!("#{}", s.name0) };
            div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(gpui::StyledText::new(body.clone()).with_highlights(
                bold_range(&body, &strong).map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })),
            ))
        })
        .into_any_element();
    let c2 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("cs-del-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| c2(w, cx)))
        .child(button("cs-del-ok", crate::t!(if cat { "menu.delete_category" } else { "menu.delete_channel" }), BtnKind::Critical).flex_1().on_click(
            cx.listener(move |a, _, _, cx| {
                with_state(|s| s.confirm_delete = false);
                a.delete_channel(channel, cx);
            }),
        ))
        .into_any_element();
    sub_frame("cs-delete", 400., crate::t!(if cat { "admin.delete_category_title" } else { "admin.delete_channel_title" }), body, footer, close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delete_body_bolds_the_channel_name_with_its_hash() {
        let body = "Es-tu sûr de vouloir supprimer #〢💬・phoenix ? Cette action est irréversible.";
        let r = bold_range(body, "#〢💬・phoenix").unwrap();
        assert_eq!(&body[r], "#〢💬・phoenix");
        assert_eq!(bold_range(body, ""), None);
    }

    #[test]
    fn every_kind_has_the_general_section_first() {
        for k in [ChannelKind::Text, ChannelKind::Announcement, ChannelKind::Voice, ChannelKind::Stage, ChannelKind::Forum, ChannelKind::Category] {
            let s = sections(k);
            assert_eq!(s[0].rows[0].bit, VIEW_CHANNEL, "{k:?}");
            assert!(s.iter().all(|sec| !sec.rows.is_empty()));
        }
        assert!(data::VOICE.iter().any(|s| s.rows.iter().any(|r| r.bit == 1 << 20)));
        assert!(!data::TEXT.iter().any(|s| s.rows.iter().any(|r| r.bit == 1 << 20)));
    }

    #[test]
    fn search_matches_label_or_description() {
        assert!(matches("", "a", "b"));
        assert!(matches(" WEB ", "Gérer les webhooks", ""));
        assert!(!matches("zzz", "Voir", "desc"));
    }
}
