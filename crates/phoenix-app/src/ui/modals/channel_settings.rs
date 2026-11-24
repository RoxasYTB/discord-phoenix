use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::InputState;
use phoenix_api::rest::guild_manage::{AdminCall, ChannelEdit};
use serde_json::Value;

use super::{
    Modal, input_box,
    settings_layout::{Act, Pick, SideItem, divider, eyebrow, link_btn, overlay, unsaved_bar},
};
use crate::{
    actions::{admin::sf, with_modal},
    app::PhoenixApp,
    model::{Channel, ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, marker_slider, switch},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Overview,
    Permissions,
    Invites,
    Integrations,
}

const SECTIONS: [(Section, &str); 4] =
    [(Section::Overview, "admin.cs_overview"), (Section::Permissions, "admin.cs_permissions"), (Section::Invites, "admin.cs_invites"), (Section::Integrations, "admin.cs_integrations")];

pub const SLOWMODE: [u32; 14] = [0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21_600];

pub fn slowmode_label(s: u32) -> String {
    match s {
        0 => crate::t!("admin.off").to_string(),
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h", s / 3600),
    }
}

pub const PERMS: [(u64, &str); 22] = [
    (1 << 10, "perm.view_channel"),
    (1 << 4, "perm.manage_channel"),
    (1 << 28, "perm.manage_permissions"),
    (1 << 29, "perm.manage_webhooks"),
    (1, "perm.create_invite"),
    (1 << 11, "perm.send_messages"),
    (1 << 38, "perm.send_in_threads"),
    (1 << 35, "perm.create_public_threads"),
    (1 << 36, "perm.create_private_threads"),
    (1 << 14, "perm.embed_links"),
    (1 << 15, "perm.attach_files"),
    (1 << 6, "perm.add_reactions"),
    (1 << 18, "perm.external_emoji"),
    (1 << 17, "perm.mention_everyone"),
    (1 << 13, "perm.manage_messages"),
    (1 << 34, "perm.manage_threads"),
    (1 << 16, "perm.read_history"),
    (1 << 20, "perm.connect"),
    (1 << 21, "perm.speak"),
    (1 << 9, "perm.video"),
    (1 << 22, "perm.mute_members"),
    (1 << 24, "perm.move_members"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Overwrite {
    pub id: Id,
    pub kind: u8,
    pub allow: u64,
    pub deny: u64,
}

impl Overwrite {
    pub fn state(&self, bit: u64) -> i8 {
        if self.allow & bit != 0 {
            1
        } else if self.deny & bit != 0 {
            -1
        } else {
            0
        }
    }

    pub fn set(&mut self, bit: u64, state: i8) {
        self.allow &= !bit;
        self.deny &= !bit;
        match state {
            1 => self.allow |= bit,
            -1 => self.deny |= bit,
            _ => {}
        }
    }
}

pub fn parse_overwrites(v: &Value) -> Vec<Overwrite> {
    let num = |x: Option<&Value>| x.and_then(|x| x.as_str().and_then(|s| s.parse().ok()).or_else(|| x.as_u64())).unwrap_or(0);
    v.get("permission_overwrites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| {
            Some(Overwrite { id: Id(num(o.get("id")).max(1)), kind: o.get("type")?.as_u64()? as u8, allow: num(o.get("allow")), deny: num(o.get("deny")) })
        })
        .collect()
}

pub struct State {
    pub channel: Id,
    pub guild: Option<Id>,
    pub kind: ChannelKind,
    pub section: Section,
    pub name0: String,
    pub topic0: String,
    pub slow0: u32,
    pub nsfw0: bool,
    pub slow: u32,
    pub nsfw: bool,
    pub name: Option<Entity<InputState>>,
    pub topic: Option<Entity<InputState>>,
    pub overwrites0: Vec<Overwrite>,
    pub overwrites: Vec<Overwrite>,
    pub selected: usize,
    pub loaded: bool,
    pub invites: Option<Vec<Value>>,
    pub integrations: Option<Vec<Value>>,
}

impl State {
    pub fn new(ch: &Channel) -> Self {
        let topic = ch.topic.as_ref().map(|t| t.to_string()).unwrap_or_default();
        Self {
            channel: ch.id,
            guild: ch.guild,
            kind: ch.kind,
            section: Section::Overview,
            name0: ch.name.to_string(),
            topic0: topic,
            slow0: 0,
            nsfw0: false,
            slow: 0,
            nsfw: false,
            name: None,
            topic: None,
            overwrites0: Vec::new(),
            overwrites: Vec::new(),
            selected: 0,
            loaded: false,
            invites: None,
            integrations: None,
        }
    }

    pub fn load_channel(&mut self, v: &Value) {
        self.overwrites0 = parse_overwrites(v);
        self.overwrites = self.overwrites0.clone();
        self.slow0 = v.get("rate_limit_per_user").and_then(Value::as_u64).unwrap_or(0) as u32;
        self.nsfw0 = v.get("nsfw").and_then(Value::as_bool).unwrap_or(false);
        self.slow = self.slow0;
        self.nsfw = self.nsfw0;
        if let Some(g) = self.guild {
            if !self.overwrites.iter().any(|o| o.id == g) {
                self.overwrites.insert(0, Overwrite { id: g, kind: 0, allow: 0, deny: 0 });
                self.overwrites0 = self.overwrites.clone();
            }
            self.overwrites.sort_by_key(|o| o.id != g);
            self.overwrites0.sort_by_key(|o| o.id != g);
        }
        self.loaded = true;
    }

    fn edit(&self, cx: &gpui::App) -> ChannelEdit {
        let name = self.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.name0.clone());
        let topic = self.topic.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.topic0.clone());
        ChannelEdit {
            name: (name.trim() != self.name0 && !name.trim().is_empty()).then(|| crate::actions::admin::channel_name(self.kind, &name)),
            topic: (topic != self.topic0).then_some(topic),
            slowmode: (self.slow != self.slow0).then_some(self.slow),
            nsfw: (self.nsfw != self.nsfw0).then_some(self.nsfw),
        }
    }

    fn overwrites_dirty(&self) -> bool {
        self.overwrites != self.overwrites0
    }
}

fn with_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m {
            f(s)
        }
    });
}

pub fn prepare(s: &mut State, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.name.is_none() {
        s.name = Some(mk(SharedString::default(), &s.name0.clone(), false));
    }
    if s.topic.is_none() && s.kind != ChannelKind::Category {
        s.topic = Some(mk(crate::t!("admin.topic_ph"), &s.topic0.clone(), false));
    }
}

fn pick_section(a: &mut PhoenixApp, sec: Section, cx: &mut Context<PhoenixApp>) {
    let mut need = None;
    with_state(|s| {
        s.section = sec;
        need = match sec {
            Section::Invites if s.invites.is_none() => Some(AdminCall::ChannelInvites { channel: sf(s.channel) }),
            Section::Integrations if s.integrations.is_none() => s.guild.map(|g| AdminCall::Integrations { guild: sf(g) }),
            _ => None,
        };
    });
    if let Some(call) = need {
        a.admin(call, cx, move |a, r, cx| {
            let list = match r {
                Ok(v) => v.as_array().cloned().unwrap_or_default(),
                Err(e) => {
                    a.parity_toast(e, cx);
                    Vec::new()
                }
            };
            with_state(|s| match sec {
                Section::Invites => s.invites = Some(list),
                _ => s.integrations = Some(list),
            });
            cx.notify();
        });
    }
}

fn text(t: impl Into<SharedString>, size: f32, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(size + 4.)).text_color(c(tok(color))).child(t.into())
}

fn overview(s: &State, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let mut d = div().flex().flex_col();
    if let Some(i) = &s.name {
        d = d.child(eyebrow(crate::t!(if s.kind == ChannelKind::Category { "admin.category_name" } else { "admin.channel_name" }))).child(input_box(i));
    }
    if let Some(i) = &s.topic {
        d = d.child(divider()).child(eyebrow(crate::t!("admin.channel_topic"))).child(input_box(i));
    }
    if matches!(s.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum | ChannelKind::Voice) {
        let cur = SLOWMODE.iter().position(|x| *x == s.slow).unwrap_or(0);
        let e = cx.entity();
        let on_set: crate::ui::controls::OnSet = Rc::new(move |n, _, cx| {
            with_state(|s| s.slow = SLOWMODE[n.min(SLOWMODE.len() - 1)]);
            e.update(cx, |_, cx| cx.notify());
        });
        d = d
            .child(divider())
            .child(eyebrow(crate::t!("admin.slowmode")))
            .child(marker_slider("cs-slow", SLOWMODE.iter().map(|x| slowmode_label(*x).into()).collect(), cur, Some(0), on_set))
            .child(text(crate::t!("admin.slowmode_desc"), 14., idx::TEXT_SUBTLE).pt(z(8.)));
    }
    if s.kind != ChannelKind::Category {
        d = d.child(divider()).child(
            div()
                .id("cs-nsfw")
                .flex()
                .items_center()
                .gap(z(16.))
                .cursor_pointer()
                .on_click(cx.listener(|_, _, _, cx| {
                    with_state(|s| s.nsfw = !s.nsfw);
                    cx.notify();
                }))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(z(4.))
                        .child(text(crate::t!("admin.nsfw"), 16., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM))
                        .child(text(crate::t!("admin.nsfw_desc"), 14., idx::TEXT_SUBTLE)),
                )
                .group(crate::ui::controls::SWITCH_ROW)
                .child(switch("nsfw-switch", s.nsfw)),
        );
    }
    d
}

fn target_name(app: &PhoenixApp, guild: Option<Id>, o: &Overwrite) -> SharedString {
    if Some(o.id) == guild {
        return "@everyone".into();
    }
    if o.kind == 0 {
        guild.and_then(|g| app.roles.get(&g)).and_then(|rs| rs.iter().find(|r| r.id == o.id)).map(|r| r.name.clone()).unwrap_or_else(|| o.id.0.to_string().into())
    } else {
        app.users.get(&o.id).map(|u| u.name.clone()).unwrap_or_else(|| o.id.0.to_string().into())
    }
}

fn tri(id: (&'static str, usize), state: i8, cx: &mut Context<PhoenixApp>, bit: u64) -> gpui::Div {
    let cell = |n: i8, glyph: &'static str, active_bg: usize| {
        let on = state == n;
        div()
            .id((id.0, id.1 * 3 + (n + 1) as usize))
            .w(z(32.))
            .h(z(24.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(z(14.))
            .font_weight(FontWeight::BOLD)
            .text_color(c(tok(if on { idx::WHITE } else { idx::INTERACTIVE_TEXT_DEFAULT })))
            .when(on, |d| d.bg(c(tok(active_bg))))
            .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
            .on_click(cx.listener(move |_, _, _, cx| {
                with_state(|s| {
                    if let Some(o) = s.overwrites.get_mut(s.selected) {
                        o.set(bit, n)
                    }
                });
                cx.notify();
            }))
            .child(glyph)
    };
    div()
        .flex()
        .rounded(z(4.))
        .overflow_hidden()
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(cell(-1, "✕", idx::STATUS_DANGER))
        .child(cell(0, "/", idx::BACKGROUND_MOD_STRONG))
        .child(cell(1, "✓", idx::STATUS_POSITIVE))
}

fn permissions(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    if !s.loaded {
        return div().child(text(crate::t!("common.loading"), 14., idx::TEXT_SUBTLE));
    }
    let mut list = div().w(z(200.)).flex_none().flex().flex_col().gap(z(2.)).child(eyebrow(crate::t!("admin.roles_members")));
    for (n, o) in s.overwrites.iter().enumerate() {
        let on = n == s.selected;
        list = list.child(
            div()
                .id(("cs-ow", n))
                .px(z(8.))
                .py(z(6.))
                .rounded(z(4.))
                .cursor_pointer()
                .text_size(z(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(if on { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_state(|s| s.selected = n);
                    cx.notify();
                }))
                .truncate()
                .child(target_name(app, s.guild, o)),
        );
    }
    let present: Vec<Id> = s.overwrites.iter().map(|o| o.id).collect();
    let mut roles: Vec<&crate::model::Role> = s.guild.and_then(|g| app.roles.get(&g)).into_iter().flatten().filter(|r| !present.contains(&r.id)).collect();
    roles.sort_by_key(|r| -r.position);
    if !roles.is_empty() {
        list = list.child(div().pt(z(12.)).child(eyebrow(crate::t!("admin.add_role"))));
        for (n, r) in roles.into_iter().take(15).enumerate() {
            let rid = r.id;
            list = list.child(
                link_btn(("cs-add", n), format!("+ {}", r.name).into(), false).on_click(cx.listener(move |_, _, _, cx| {
                    with_state(|s| {
                        s.overwrites.push(Overwrite { id: rid, kind: 0, allow: 0, deny: 0 });
                        s.selected = s.overwrites.len() - 1;
                    });
                    cx.notify();
                })),
            );
        }
    }
    let mut perms = div().flex_1().min_w_0().flex().flex_col();
    if let Some(o) = s.overwrites.get(s.selected) {
        perms = perms.child(eyebrow(crate::t!("admin.perms_for", name = target_name(app, s.guild, o).to_string())));
        let voice = matches!(s.kind, ChannelKind::Voice | ChannelKind::Stage);
        for (n, (bit, key)) in PERMS.iter().enumerate() {
            let voice_bit = [1 << 20, 1 << 21, 1 << 9, 1 << 22, 1 << 24].contains(bit);
            if voice_bit && !voice && s.kind != ChannelKind::Category {
                continue;
            }
            perms = perms.child(
                div()
                    .flex()
                    .items_center()
                    .py(z(10.))
                    .border_b_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .child(div().flex_1().child(text(crate::t!(key), 16., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM)))
                    .child(tri(("cs-tri", n), o.state(*bit), cx, *bit)),
            );
        }
        if Some(o.id) != s.guild {
            let (target, ch, existed) = (o.id, s.channel, s.overwrites0.iter().any(|x| x.id == o.id));
            perms = perms.child(div().pt(z(16.)).child(button("cs-ow-remove", crate::t!("admin.remove_overwrite"), BtnKind::Critical).on_click(cx.listener(
                move |a, _, _, cx| {
                    with_state(|s| {
                        s.overwrites.retain(|x| x.id != target);
                        s.overwrites0.retain(|x| x.id != target);
                        s.selected = 0;
                    });
                    if existed {
                        a.admin_toast(AdminCall::DeleteOverwrite { channel: sf(ch), target: sf(target) }, None, cx);
                    }
                    cx.notify();
                },
            ))));
        }
    }
    div().flex().gap(z(24.)).child(list).child(perms)
}

fn invites(s: &State, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    invite_list(s.invites.as_deref(), "cs-inv", cx, |code| {
        with_state(|s| {
            if let Some(l) = &mut s.invites {
                l.retain(|i| i.get("code").and_then(Value::as_str) != Some(code))
            }
        })
    })
}

pub fn invite_list(list: Option<&[Value]>, id: &'static str, cx: &mut Context<PhoenixApp>, removed: fn(&str)) -> gpui::Div {
    let Some(list) = list else { return div().child(text(crate::t!("common.loading"), 14., idx::TEXT_SUBTLE)) };
    if list.is_empty() {
        return div().child(text(crate::t!("admin.no_invites"), 14., idx::TEXT_SUBTLE));
    }
    let head = div()
        .flex()
        .pb(z(8.))
        .child(div().flex_1().child(eyebrow(crate::t!("admin.inviter"))))
        .child(div().w(z(120.)).child(eyebrow(crate::t!("admin.invite_code"))))
        .child(div().w(z(60.)).child(eyebrow(crate::t!("admin.uses"))))
        .child(div().w(z(90.)).child(eyebrow(crate::t!("admin.expires"))))
        .child(div().w(z(70.)));
    let mut d = div().flex().flex_col().child(head);
    for (n, inv) in list.iter().enumerate() {
        let code = inv.get("code").and_then(Value::as_str).unwrap_or_default().to_owned();
        let who = inv.pointer("/inviter/global_name").and_then(Value::as_str).or_else(|| inv.pointer("/inviter/username").and_then(Value::as_str)).unwrap_or("-").to_owned();
        let uses = inv.get("uses").and_then(Value::as_u64).unwrap_or(0);
        let max = inv.get("max_uses").and_then(Value::as_u64).unwrap_or(0);
        let age = inv.get("max_age").and_then(Value::as_u64).unwrap_or(0);
        let expires = if age == 0 { "∞".to_owned() } else { slowmode_label(age.min(u32::MAX as u64) as u32).replace("168h", "7d") };
        let c2 = code.clone();
        d = d.child(
            div()
                .flex()
                .items_center()
                .py(z(10.))
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(div().flex_1().min_w_0().truncate().child(text(who, 14., idx::TEXT_STRONG)))
                .child(div().w(z(120.)).child(text(code, 14., idx::TEXT_DEFAULT)))
                .child(div().w(z(60.)).child(text(if max == 0 { uses.to_string() } else { format!("{uses}/{max}") }, 14., idx::TEXT_DEFAULT)))
                .child(div().w(z(90.)).child(text(expires, 14., idx::TEXT_DEFAULT)))
                .child(div().w(z(70.)).child(link_btn((id, n), crate::t!("admin.revoke"), true).on_click(cx.listener(move |a, _, _, cx| {
                    removed(&c2);
                    a.admin_toast(AdminCall::DeleteInvite { code: c2.clone() }, None, cx);
                    cx.notify();
                })))),
        );
    }
    d
}

pub fn integrations_list(list: Option<&[Value]>) -> gpui::Div {
    let Some(list) = list else { return div().child(text(crate::t!("common.loading"), 14., idx::TEXT_SUBTLE)) };
    if list.is_empty() {
        return div().child(text(crate::t!("admin.no_integrations"), 14., idx::TEXT_SUBTLE));
    }
    let mut d = div().flex().flex_col().gap(z(8.));
    for i in list {
        let name = i.get("name").and_then(Value::as_str).unwrap_or("-").to_owned();
        let kind = i.get("type").and_then(Value::as_str).unwrap_or("").to_owned();
        d = d.child(
            div()
                .p(z(16.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(text(name, 16., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD))
                .child(text(kind, 14., idx::TEXT_SUBTLE)),
        );
    }
    d
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cat = s.kind == ChannelKind::Category;
    let prefix = if cat { String::new() } else { "# ".to_owned() };
    let mut items = vec![SideItem::Header(format!("{prefix}{}", s.name0).to_uppercase().into())];
    for (n, (sec, key)) in SECTIONS.iter().enumerate() {
        if *sec == Section::Invites && cat {
            continue;
        }
        items.push(SideItem::Item { label: crate::t!(key), index: n, danger: false });
    }
    items.push(SideItem::Separator);
    items.push(SideItem::Item { label: crate::t!(if cat { "menu.delete_category" } else { "menu.delete_channel" }), index: 99, danger: true });
    let channel = s.channel;
    let pick: Pick = Rc::new(move |n, a, cx| {
        if n == 99 {
            a.confirm_delete_channel(channel, cx);
        } else {
            pick_section(a, SECTIONS[n].0, cx);
        }
    });
    let selected = SECTIONS.iter().position(|x| x.0 == s.section).unwrap_or(0);
    let content = match s.section {
        Section::Overview => overview(s, cx),
        Section::Permissions => permissions(app, s, cx),
        Section::Invites => invites(s, cx),
        Section::Integrations => integrations_list(s.integrations.as_deref()),
    };
    let dirty = match s.section {
        Section::Overview => s.edit(cx) != ChannelEdit::default(),
        Section::Permissions => s.overwrites_dirty(),
        _ => false,
    };
    let notice = dirty.then(|| {
        let reset: Act = Rc::new(|_, w, cx| {
            let mut inputs = Vec::new();
            with_state(|s| {
                s.slow = s.slow0;
                s.nsfw = s.nsfw0;
                s.overwrites = s.overwrites0.clone();
                inputs = [(s.name.clone(), s.name0.clone()), (s.topic.clone(), s.topic0.clone())].into_iter().filter_map(|(i, v)| Some((i?, v))).collect();
            });
            for (i, v) in inputs {
                i.update(cx, |st, cx| st.set_value(v, w, cx));
            }
        });
        let save: Act = Rc::new(|a, _, cx| save(a, cx));
        unsaved_bar(reset, save, cx)
    });
    let title = SECTIONS[selected].1;
    overlay("channel-settings", items, selected, pick, crate::t!(title), content.into_any_element(), notice, cx)
}

fn save(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut out = None;
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m {
            let edit = s.edit(cx);
            let changed: Vec<Overwrite> = s.overwrites.iter().filter(|o| !s.overwrites0.contains(o)).cloned().collect();
            let (name, topic) = (edit.name.clone().unwrap_or_else(|| s.name0.clone()), edit.topic.clone().unwrap_or_else(|| s.topic0.clone()));
            if edit != ChannelEdit::default() {
                s.name0 = name.clone();
                s.topic0 = topic.clone();
                s.slow0 = s.slow;
                s.nsfw0 = s.nsfw;
            }
            s.overwrites0 = s.overwrites.clone();
            out = Some((s.channel, edit, changed, name, topic));
        }
    });
    let Some((channel, edit, changed, name, topic)) = out else { return };
    if edit != ChannelEdit::default() {
        a.apply_channel_overview(channel, &name, &topic);
        a.admin_toast(AdminCall::EditChannel { channel: sf(channel), edit }, None, cx);
    }
    for o in changed {
        a.admin_toast(AdminCall::SetOverwrite { channel: sf(channel), target: sf(o.id), kind: o.kind, allow: o.allow, deny: o.deny }, None, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_state() {
        let mut o = Overwrite { id: Id(1), kind: 0, allow: 0, deny: 0 };
        o.set(1024, 1);
        assert_eq!((o.allow, o.deny, o.state(1024)), (1024, 0, 1));
        o.set(1024, -1);
        assert_eq!((o.allow, o.deny, o.state(1024)), (0, 1024, -1));
        o.set(1024, 0);
        assert_eq!(o.state(1024), 0);
    }

    #[test]
    fn parse() {
        let v = serde_json::json!({ "permission_overwrites": [{ "id": "7", "type": 1, "allow": "1024", "deny": "0" }] });
        assert_eq!(parse_overwrites(&v), vec![Overwrite { id: Id(7), kind: 1, allow: 1024, deny: 0 }]);
        assert_eq!(slowmode_label(5), "5s");
        assert_eq!(slowmode_label(120), "2m");
        assert_eq!(slowmode_label(21_600), "6h");
    }

    #[test]
    fn everyone_first() {
        let ch = Channel {
            id: Id(2),
            guild: Some(Id(1)),
            parent: None,
            name: "general".into(),
            topic: None,
            kind: ChannelKind::Text,
            position: 0,
            unread: false,
            muted: false,
            mentions: 0,
            voice_users: Vec::new(),
        };
        let mut s = State::new(&ch);
        s.load_channel(&serde_json::json!({ "rate_limit_per_user": 5, "permission_overwrites": [{ "id": "7", "type": 0, "allow": "0", "deny": "0" }] }));
        assert_eq!(s.overwrites[0].id, Id(1));
        assert_eq!(s.slow, 5);
        assert!(!s.overwrites_dirty());
    }
}
