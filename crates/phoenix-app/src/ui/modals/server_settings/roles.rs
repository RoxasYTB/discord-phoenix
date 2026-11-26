use gpui::{AnyElement, Entity, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::InputState;
use phoenix_api::{
    models::Snowflake,
    rest::guild_manage::{AdminCall, HOLOGRAPHIC, RoleColors, RoleEdit},
};
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, labels, text},
    with_state,
};
use crate::{
    actions::admin::sf,
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::BtnKind,
};

pub const PERMS: [(&str, &[u32]); 8] = [
    ("gs.perm.s0", &[10, 4, 28, 43, 30, 7, 19, 29, 5]),
    ("gs.perm.s1", &[0, 26, 27, 1, 2, 40]),
    ("gs.perm.s2", &[11, 38, 35, 36, 14, 15, 6, 18, 37, 17, 13, 51, 52, 34, 16, 12, 46, 49]),
    ("gs.perm.s3", &[20, 21, 9, 42, 45, 25, 8, 22, 23, 24, 48]),
    ("gs.perm.s4", &[31, 39, 50]),
    ("gs.perm.s5", &[32]),
    ("gs.perm.s6", &[44, 33]),
    ("gs.perm.s7", &[3]),
];

pub const SOLID: [u32; 20] = [
    0x1ABC9C, 0x2ECC71, 0x3498DB, 0x9B59B6, 0xE91E63, 0xF1C40F, 0xE67E22, 0xE74C3C, 0x95A5A6, 0x607D8B, 0x11806A, 0x1F8B4C, 0x206694, 0x71368A, 0xAD1457, 0xC27C0E, 0xA84300, 0x992D22,
    0x979C9F, 0x546E7A,
];

pub const GRADIENTS: [(&str, u32, u32); 12] = [
    ("fire", 0xAA3B3B, 0xFF4848),
    ("golddust", 0xD4843D, 0xFFDE90),
    ("emerald", 0x369876, 0x71FF9E),
    ("diamond", 0x4CADD0, 0xB2F9FF),
    ("blurple", 0x9E6BFF, 0x9FC1FF),
    ("cupid", 0xFF5DD6, 0xFF9CBF),
    ("rosegold", 0x907575, 0xFFCEC6),
    ("desert", 0x9B8666, 0xFFE1B4),
    ("moss", 0x749472, 0xB3E1B9),
    ("platinum", 0x8799AE, 0xF1F7FF),
    ("plum", 0x826BC2, 0xB7A7F1),
    ("rose", 0x965F7F, 0xFFAADC),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Display,
    Permissions,
    Links,
    Members,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Solid,
    Gradient,
    Holographic,
}

pub fn style_of(c: &RoleColors) -> Style {
    if *c == HOLOGRAPHIC {
        Style::Holographic
    } else if c.secondary.is_some() {
        Style::Gradient
    } else {
        Style::Solid
    }
}

pub fn colors_of(r: &Value) -> RoleColors {
    let col = r.get("colors");
    let pick = |k: &str| col.and_then(|c| c.get(k)).and_then(Value::as_u64).map(|v| v as u32);
    RoleColors { primary: pick("primary_color").unwrap_or(kit::n(r, "color") as u32), secondary: pick("secondary_color"), tertiary: pick("tertiary_color") }
}

pub struct Drag {
    pub id: u64,
    pub over: usize,
}

pub struct Editor {
    pub id: u64,
    pub everyone: bool,
    pub tab: Tab,
    pub name0: String,
    pub name: Option<Entity<InputState>>,
    pub perm_search: Option<Entity<InputState>>,
    pub colors0: RoleColors,
    pub colors: RoleColors,
    pub perms0: u64,
    pub perms: u64,
    pub hoist0: bool,
    pub hoist: bool,
    pub mention0: bool,
    pub mention: bool,
    pub members: Option<Vec<u64>>,
    pub icon: Option<Option<std::path::PathBuf>>,
}

impl Editor {
    pub fn new(r: &Value, everyone: bool) -> Self {
        let colors = colors_of(r);
        let perms = kit::n(r, "permissions");
        let hoist = r.get("hoist").and_then(Value::as_bool).unwrap_or(false);
        let mention = r.get("mentionable").and_then(Value::as_bool).unwrap_or(false);
        Self {
            id: kit::n(r, "id"),
            everyone,
            tab: if everyone { Tab::Permissions } else { Tab::Display },
            name0: kit::s(r, "name"),
            name: None,
            perm_search: None,
            colors0: colors,
            colors,
            perms0: perms,
            perms,
            hoist0: hoist,
            hoist,
            mention0: mention,
            mention,
            members: None,
            icon: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.colors != self.colors0 || self.perms != self.perms0 || self.hoist != self.hoist0 || self.mention != self.mention0 || self.icon.is_some()
    }

    pub fn reset(&mut self) {
        self.colors = self.colors0;
        self.perms = self.perms0;
        self.hoist = self.hoist0;
        self.mention = self.mention0;
        self.name = None;
        self.icon = None;
    }

    pub fn commit(&mut self) {
        self.colors0 = self.colors;
        self.perms0 = self.perms;
        self.hoist0 = self.hoist;
        self.mention0 = self.mention;
        self.icon = None;
    }

    pub fn edit(&self, cx: &gpui::App) -> Option<RoleEdit> {
        let name = self.name.as_ref().map(|i| i.read(cx).value().trim().to_owned()).filter(|n| !n.is_empty() && *n != self.name0 && !self.everyone);
        let e = RoleEdit {
            name,
            colors: (self.colors != self.colors0).then_some(self.colors),
            hoist: (self.hoist != self.hoist0).then_some(self.hoist),
            mentionable: (self.mention != self.mention0).then_some(self.mention),
            permissions: (self.perms != self.perms0).then_some(self.perms),
            icon: self.icon.as_ref().map(|p| p.as_ref().and_then(|p| crate::ui::file_pick::data_uri(p))),
            unicode_emoji: self.icon.as_ref().map(|_| None),
            ..Default::default()
        };
        (e != RoleEdit::default()).then_some(e)
    }
}

pub fn sorted_roles(s: &State) -> Vec<Value> {
    let mut v: Vec<Value> = s.list("roles").iter().filter(|r| kit::n(r, "id") != s.guild.0).cloned().collect();
    v.sort_by_key(|r| (-(r.get("position").and_then(Value::as_i64).unwrap_or(0)), kit::n(r, "id")));
    if let Some(order) = &s.role_order {
        v.sort_by_key(|r| order.iter().position(|id| *id == kit::n(r, "id")).unwrap_or(usize::MAX));
    }
    v
}

pub fn positions(order: &[u64]) -> Vec<(Snowflake, i32)> {
    let n = order.len() as i32;
    order.iter().enumerate().map(|(i, id)| (Snowflake(*id), n - i as i32)).collect()
}

pub fn move_role(order: &[u64], id: u64, to: usize) -> Vec<u64> {
    let mut v: Vec<u64> = order.iter().copied().filter(|x| *x != id).collect();
    v.insert(to.min(v.len()), id);
    v
}

fn my_top(app: &PhoenixApp, s: &State) -> i64 {
    let mine: Vec<u64> = app.members.get(&s.guild).and_then(|m| m.iter().find(|m| m.user == app.me.id)).map(|m| m.roles.iter().map(|r| r.0).collect()).unwrap_or_default();
    s.list("roles").iter().filter(|r| mine.contains(&kit::n(r, "id"))).filter_map(|r| r.get("position").and_then(Value::as_i64)).max().unwrap_or(0)
}

pub fn locked(app: &PhoenixApp, s: &State, r: &Value) -> bool {
    let owner = app.guilds.iter().find(|g| g.id == s.guild).and_then(|g| g.owner) == Some(app.me.id);
    !owner && r.get("position").and_then(Value::as_i64).unwrap_or(0) >= my_top(app, s)
}

fn hsla(v: u32) -> Hsla {
    c(v)
}

pub fn circle(colors: &RoleColors, size: f32) -> AnyElement {
    let d = div().size(z(size)).flex_none().rounded_full();
    if colors.primary == 0 && colors.secondary.is_none() {
        return d.bg(c(0x99AAB5)).into_any_element();
    }
    match colors.secondary {
        Some(sec) => d.bg(gpui::linear_gradient(90., gpui::linear_color_stop(hsla(colors.primary), 0.), gpui::linear_color_stop(hsla(colors.tertiary.unwrap_or(sec)), 1.))).into_any_element(),
        None => d.bg(hsla(colors.primary)).into_any_element(),
    }
}

fn role_icon(guild: u64, r: &Value, size: f32) -> AnyElement {
    let _ = guild;
    if let Some(h) = r.get("icon").and_then(Value::as_str) {
        return gpui::img(SharedString::from(format!("https://cdn.discordapp.com/role-icons/{}/{h}.webp?size=48", kit::s(r, "id")))).size(z(size)).flex_none().into_any_element();
    }
    if let Some(e) = r.get("unicode_emoji").and_then(Value::as_str) {
        return crate::media::emoji::emoji_element(e, size);
    }
    let col = colors_of(r);
    let fill = if col.primary == 0 { c(tok(idx::TEXT_SUBTLE)) } else { hsla(col.primary) };
    gpui::svg().path(crate::icons::path("settings/shield-user")).size(z(size)).flex_none().text_color(fill).into_any_element()
}

pub fn has_icon(r: &Value, e: &Editor) -> bool {
    match &e.icon {
        Some(p) => p.is_some(),
        None => r.get("icon").and_then(Value::as_str).is_some() || r.get("unicode_emoji").and_then(Value::as_str).is_some(),
    }
}

fn draft_icon(guild: u64, r: &Value, e: &Editor, size: f32) -> AnyElement {
    match &e.icon {
        Some(Some(p)) => gpui::img(p.clone()).size(z(size)).flex_none().into_any_element(),
        Some(None) => {
            let mut bare = r.clone();
            if let Some(o) = bare.as_object_mut() {
                o.remove("icon");
                o.remove("unicode_emoji");
            }
            role_icon(guild, &bare, size)
        }
        None => role_icon(guild, r, size),
    }
}

fn pick_role_icon(a: &mut PhoenixApp, cx: &mut Cx) {
    let Some(guild) = with_state(|s| s.guild) else { return };
    if !a.backend.guild_features(guild).iter().any(|f| f == "ROLE_ICONS") {
        crate::ui::boost_layer::open(guild);
        cx.notify();
        return;
    }
    crate::ui::file_pick::pick(&["png", "jpg", "jpeg", "webp", "gif"], cx, |_, path, cx| {
        with_state(|s| {
            if let Some(e) = s.role.as_mut() {
                e.icon = Some(Some(path));
            }
        });
        cx.notify();
    });
}

fn open_editor(id: u64, everyone: bool, members_tab: bool, a: &mut PhoenixApp, cx: &mut Cx) {
    let need = with_state(|s| {
        let r = s.list("roles").iter().find(|r| kit::n(r, "id") == id).cloned()?;
        let mut e = Editor::new(&r, everyone);
        if members_tab {
            e.tab = Tab::Members;
        }
        s.role = Some(e);
        Some(s.guild)
    })
    .flatten();
    if let Some(g) = need
        && !everyone
    {
        a.admin(AdminCall::RoleMemberIds { guild: sf(g), role: Snowflake(id) }, cx, move |_, r, cx| {
            let ids: Vec<u64> = r.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|x| x.as_str().and_then(|s| s.parse().ok())).collect();
            with_state(|s| {
                if let Some(e) = s.role.as_mut().filter(|e| e.id == id) {
                    e.members = Some(ids);
                }
            });
            cx.notify();
        });
    }
    cx.notify();
}

pub fn list(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let q = kit::input_value(s.inputs.get("role_search"), cx).to_lowercase();
    let roles = sorted_roles(s);
    let counts = s.get("counts").cloned().unwrap_or(Value::Null);
    let everyone = s.guild.0;
    let guild = s.guild.0;
    let default_card = div()
        .id("ss-role-everyone")
        .h(z(73.6))
        .pt(z(16.))
        .pb(z(16.))
        .pl(z(16.))
        .pr(z(24.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .cursor_pointer()
        .flex()
        .items_center()
        .child(div().size(z(32.)).flex_none().rounded_full().bg(c(tok(idx::BACKGROUND_BASE_LOWEST))).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("members")).size(z(20.)).text_color(c(tok(idx::TEXT_SUBTLE)))))
        .child(
            div()
                .ml(z(16.))
                .flex_1()
                .flex()
                .flex_col()
                .child(text(crate::t!("gs.roles.default"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE).mb(z(4.)))
                .child(text(crate::t!("gs.roles.default_desc"), 12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE)),
        )
        .child(gpui::svg().path(crate::icons::path("chevron-right")).size(z(24.)).text_color(c(tok(idx::ICON_SUBTLE))))
        .on_click(cx.listener(move |a, _, _, cx| open_editor(everyone, true, false, a, cx)));
    let search = div()
        .mt(z(32.))
        .flex()
        .items_center()
        .gap(z(16.))
        .child(kit::search_box(s.inputs.get("role_search")).flex_1())
        .child(kit::btn_sm("ss-role-create", crate::t!("gs.roles.create"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
            let w = crate::actions::guard::GuardedWrite::create_role(guild, &crate::t!("gs.roles.new_name"));
            a.blocked_write(w, cx);
        })));
    let help = div()
        .mt(z(8.))
        .flex()
        .flex_col()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(crate::t!("gs.roles.help"))
        .child(div().text_color(c(tok(idx::TEXT_LINK))).child(crate::t!("gs.roles.help_link")));
    let shown: Vec<&Value> = roles.iter().filter(|r| q.is_empty() || kit::s(r, "name").to_lowercase().contains(&q)).collect();
    let header = div()
        .ml(z(-32.))
        .mr(z(-8.))
        .pr(z(8.))
        .mb(z(8.))
        .flex()
        .child(div().w(z(32.)).flex_none())
        .child(text(crate::t!("gs.roles.count", n = roles.len()), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).w(z(273.)))
        .child(div().ml(z(16.)).w(z(112.)).flex().justify_end().child(text(crate::t!("gs.roles.members"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT)));
    let drag = s.drag.as_ref().map(|d| (d.id, d.over));
    let order: Vec<u64> = roles.iter().map(|r| kit::n(r, "id")).collect();
    let mut table = div().mt(z(32.)).flex().flex_col().child(header);
    let last = shown.len().saturating_sub(1);
    for (i, r) in shown.iter().enumerate() {
        let id = kit::n(r, "id");
        let lock = locked(app, s, r);
        let managed = r.get("managed").and_then(Value::as_bool).unwrap_or(false);
        let count = counts.get(id.to_string()).and_then(Value::as_u64).unwrap_or(0);
        let group: SharedString = format!("ss-role-{id}").into();
        let order2 = order.clone();
        let row = div()
            .id(("ss-role", id as usize))
            .group(group.clone())
            .relative()
            .h(z(61.))
            .pt(z(1.))
            .ml(z(-32.))
            .mr(z(-8.))
            .pr(z(8.))
            .rounded(z(4.))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
            .child(div().absolute().top_0().left(z(32.)).right(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).group_hover(group.clone(), |s| s.bg(gpui::transparent_black())))
            .when(i == last, |d| d.child(div().absolute().bottom(z(-1.)).left(z(32.)).right(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))))
            .when(drag.is_some_and(|(_, over)| over == i), |d| d.child(div().absolute().top_0().left(z(32.)).right(z(8.)).h(z(2.)).bg(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW)))))
            .child(
                div()
                    .id(("ss-role-drag", id as usize))
                    .w(z(32.))
                    .flex_none()
                    .p(z(8.))
                    .invisible()
                    .when(!lock && !managed, |d| d.group_hover(group.clone(), |s| s.visible()))
                    .child(gpui::svg().path(crate::icons::path("server/drag")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                    .when(!lock, |d| {
                        d.on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                            with_state(|s| s.drag = Some(Drag { id, over: i }));
                            cx.notify();
                        }))
                    }),
            )
            .child(
                div()
                    .w(z(273.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .child(div().mr(z(8.)).child(role_icon(s.guild.0, r, 24.)))
                    .child(text(kit::s(r, "name"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG).truncate().min_w_0())
                    .when(lock, |d| d.child(gpui::svg().path(crate::icons::path("lock")).ml(z(4.)).flex_none().size(z(16.)).text_color(c(tok(idx::TEXT_MUTED))))),
            )
            .child(
                div()
                    .id(("ss-role-count", id as usize))
                    .ml(z(16.))
                    .w(z(112.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(z(4.))
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .hover(|s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                    .child(text(count.to_string(), 16., 20., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT))
                    .child(gpui::svg().path(crate::icons::path("user")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        cx.stop_propagation();
                        open_editor(id, false, true, a, cx)
                    })),
            )
            .child(
                div()
                    .ml(z(16.))
                    .flex_1()
                    .flex()
                    .justify_end()
                    .gap(z(8.))
                    .child(kit::icon_btn(("ss-role-edit", id as usize), if lock { "chat/eye" } else { "pen" }, 40., 20., false))
                    .child(kit::icon_btn(("ss-role-more", id as usize), "more-horizontal", 40., 20., false)),
            )
            .on_mouse_move(cx.listener(move |_, _, _, cx| {
                let changed = with_state(|s| match s.drag.as_mut() {
                    Some(d) if d.over != i => {
                        d.over = i;
                        true
                    }
                    _ => false,
                })
                .unwrap_or(false);
                if changed {
                    cx.notify();
                }
            }))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                with_state(|s| {
                    if let Some(d) = s.drag.take()
                        && d.id != id
                    {
                        let next = move_role(&order2, d.id, d.over);
                        let base: Vec<u64> = {
                            let mut v: Vec<&Value> = s.list("roles").iter().filter(|r| kit::n(r, "id") != s.guild.0).collect();
                            v.sort_by_key(|r| -(r.get("position").and_then(Value::as_i64).unwrap_or(0)));
                            v.iter().map(|r| kit::n(r, "id")).collect()
                        };
                        s.role_order = (next != base).then_some(next);
                    }
                });
                cx.notify();
            }))
            .on_click(cx.listener(move |a, _, _, cx| open_editor(id, false, false, a, cx)));
        table = table.child(row);
    }
    div()
        .flex()
        .flex_col()
        .child(kit::h2(crate::t!("menu.gs.roles")))
        .child(text(crate::t!("gs.roles.desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).mb(z(16.)))
        .child(default_card)
        .child(search)
        .child(help)
        .child(table)
        .into_any_element()
}

fn side(app: &PhoenixApp, s: &State, cur: u64, cx: &mut Cx) -> AnyElement {
    let roles = sorted_roles(s);
    let mut list = div().flex().flex_col();
    let mut all: Vec<Value> = roles;
    if let Some(e) = s.list("roles").iter().find(|r| kit::n(r, "id") == s.guild.0) {
        all.push(e.clone());
    }
    for r in all {
        let id = kit::n(&r, "id");
        let on = id == cur;
        let lock = id != s.guild.0 && locked(app, s, &r);
        let everyone = id == s.guild.0;
        list = list.child(
            div()
                .id(("ss-role-side", id as usize))
                .h(z(34.))
                .mb(z(4.))
                .ml(z(-6.))
                .px(z(10.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(4.))
                .cursor_pointer()
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
                .when(!on, |d| d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
                .child(div().mx(z(4.)).child(circle(&colors_of(&r), 12.)))
                .when(lock, |d| d.child(gpui::svg().path(crate::icons::path("lock")).flex_none().size(z(16.)).text_color(c(tok(idx::TEXT_SUBTLE)))))
                .child(text(kit::s(&r, "name"), 14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG).truncate().min_w_0())
                .on_click(cx.listener(move |a, _, _, cx| {
                    if !on {
                        open_editor(id, everyone, false, a, cx)
                    }
                })),
        );
    }
    div()
        .w(z(232.))
        .flex_none()
        .border_r_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .flex_col()
        .child(
            div()
                .ml(z(16.))
                .mr(z(8.))
                .mb(z(8.))
                .pt(z(60.))
                .px(z(8.))
                .pb(z(16.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("ss-role-back")
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .child(gpui::svg().path(crate::icons::path("nav/back")).size(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT))))
                        .child(text(crate::t!("gs.role.back"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).ml(z(8.)))
                        .on_click(cx.listener(|_, _, _, cx| {
                            with_state(|s| s.role = None);
                            cx.notify();
                        })),
                )
                .child(gpui::svg().path(crate::icons::path("plus")).size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))),
        )
        .child(div().pl(z(40.)).pr(z(16.)).pb(z(96.)).child(list))
        .into_any_element()
}

fn tabs(e: &Editor, cx: &mut Cx) -> AnyElement {
    let count = e.members.as_ref().map_or(0, Vec::len);
    let items: Vec<(Tab, SharedString)> = if e.everyone {
        vec![(Tab::Permissions, crate::t!("gs.role.tab_perms"))]
    } else {
        vec![
            (Tab::Display, crate::t!("gs.role.tab_display")),
            (Tab::Permissions, crate::t!("gs.role.tab_perms")),
            (Tab::Links, crate::t!("gs.role.tab_links")),
            (Tab::Members, crate::t!("gs.role.tab_members", n = count)),
        ]
    };
    let mut bar = div().mt(z(16.)).mb(z(-2.)).flex().border_b_1().border_color(c(tok(idx::BORDER_SUBTLE)));
    for (i, (t, label)) in items.into_iter().enumerate() {
        let on = e.tab == t;
        bar = bar.child(
            div()
                .id(("ss-role-tab", i))
                .relative()
                .when(i > 0, |d| d.ml(z(30.)))
                .mr(z(32.))
                .pb(z(16.))
                .mb(z(-1.))
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(14.8))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(if on { idx::TEXT_BRAND } else { idx::TEXT_SUBTLE })))
                .hover(|s| s.text_color(c(tok(idx::TEXT_STRONG))))
                .child(label)
                .when(on, |d| d.child(div().absolute().bottom(z(-1.)).left_0().right_0().h(z(2.)).rounded_t(z(2.)).bg(c(tok(idx::TEXT_BRAND)))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_state(|s| {
                        if let Some(e) = s.role.as_mut() {
                            e.tab = t;
                        }
                    });
                    cx.notify();
                })),
        );
    }
    bar.into_any_element()
}

fn perm_row(bit: u32, on: bool, cx: &mut Cx) -> AnyElement {
    div()
        .id(("ss-perm", bit as usize))
        .flex()
        .gap(z(24.))
        .cursor_pointer()
        .group(crate::ui::controls::SWITCH_ROW)
        .child(labels(crate::t!(&format!("gs.perm.p{bit}")), Some(crate::t!(&format!("gs.perm.p{bit}_desc")))).flex_1())
        .child(crate::ui::controls::switch(("ss-perm-sw", bit as usize), on))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| {
                if let Some(e) = s.role.as_mut() {
                    e.perms ^= 1u64 << bit;
                }
            });
            cx.notify();
        }))
        .into_any_element()
}

fn permissions(e: &Editor, cx: &mut Cx) -> AnyElement {
    let q = kit::input_value(e.perm_search.as_ref(), cx).to_lowercase();
    let mut col = div()
        .flex()
        .flex_col()
        .child(kit::search_box(e.perm_search.as_ref()))
        .child(
            div()
                .id("ss-perm-clear")
                .mt(z(32.))
                .flex()
                .justify_end()
                .cursor_pointer()
                .child(text(crate::t!("gs.role.clear_perms"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_LINK))
                .on_click(cx.listener(|_, _, _, cx| {
                    with_state(|s| {
                        if let Some(e) = s.role.as_mut() {
                            e.perms = 0;
                        }
                    });
                    cx.notify();
                })),
        );
    for (si, (title, bits)) in PERMS.iter().enumerate() {
        let shown: Vec<u32> = bits
            .iter()
            .copied()
            .filter(|b| q.is_empty() || crate::t!(&format!("gs.perm.p{b}")).to_lowercase().contains(&q) || crate::t!(&format!("gs.perm.p{b}_desc")).to_lowercase().contains(&q))
            .collect();
        if shown.is_empty() {
            continue;
        }
        let mut fs = div().flex().flex_col().when(si > 0, |d| d.mt(z(56.))).when(si == 0, |d| d.mt(z(-18.))).child(text(crate::t!(title), 20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG).mb(z(12.)));
        for (j, b) in shown.iter().enumerate() {
            if j > 0 {
                fs = fs.child(div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))));
            }
            fs = fs.child(perm_row(*b, e.perms & (1u64 << b) != 0, cx));
        }
        col = col.child(fs);
    }
    col.into_any_element()
}

fn style_card(i: usize, style: Style, label: SharedString, e: &Editor, cx: &mut Cx) -> AnyElement {
    let on = style_of(&e.colors) == style;
    let preview = match style {
        Style::Solid => RoleColors { primary: e.colors.primary, secondary: None, tertiary: None },
        Style::Gradient => RoleColors { primary: e.colors.primary, secondary: Some(e.colors.secondary.unwrap_or(0xFF4FB6)), tertiary: None },
        Style::Holographic => HOLOGRAPHIC,
    };
    let name: AnyElement = match preview.secondary {
        Some(sec) => div()
            .relative()
            .child(text("Wumpus", 16., 22., FontWeight::MEDIUM, idx::TEXT_STRONG).text_color(c(preview.primary)))
            .child(div().absolute().top_0().left_0().right_0().bottom_0().opacity(0.5).child(text("Wumpus", 16., 22., FontWeight::MEDIUM, idx::TEXT_STRONG).text_color(c(preview.tertiary.unwrap_or(sec)))))
            .into_any_element(),
        None => text("Wumpus", 16., 22., FontWeight::MEDIUM, idx::TEXT_STRONG).text_color(c(if preview.primary == 0 { 0xDBDEE1 } else { preview.primary })).into_any_element(),
    };
    div()
        .id(("ss-role-style", i))
        .flex_1()
        .min_w_0()
        .rounded(z(8.))
        .border_2()
        .border_color(if on { c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW)) } else { gpui::transparent_black() })
        .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .overflow_hidden()
        .cursor_pointer()
        .flex()
        .flex_col()
        .child(
            div()
                .h(z(86.8))
                .py(z(8.))
                .px(z(4.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .flex()
                .gap(z(8.))
                .child(div().size(z(40.)).flex_none().rounded_full().bg(c(tok(idx::BACKGROUND_BRAND))))
                .child(div().flex_1().min_w_0().flex().flex_col().child(name).child(text(crate::t!("gs.role.preview_msg"), 16., 22., FontWeight::NORMAL, idx::TEXT_DEFAULT))),
        )
        .child(
            div()
                .h(z(29.6))
                .flex()
                .items_center()
                .justify_center()
                .bg(c(tok(if on { idx::BACKGROUND_MOD_SUBTLE } else { idx::BACKGROUND_BASE_LOWER })))
                .child(text(label, 13.3, 16., FontWeight::MEDIUM, if on { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })),
        )
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| {
                if let Some(e) = s.role.as_mut() {
                    e.colors = match style {
                        Style::Solid => RoleColors { primary: e.colors.primary, secondary: None, tertiary: None },
                        Style::Gradient => RoleColors { primary: e.colors.primary, secondary: Some(e.colors.secondary.filter(|_| e.colors != HOLOGRAPHIC).unwrap_or(0xFF4FB6)), tertiary: None },
                        Style::Holographic => HOLOGRAPHIC,
                    };
                }
            });
            cx.notify();
        }))
        .into_any_element()
}

fn swatch(id: (&'static str, usize), bg: gpui::Background, on: bool, set: RoleColors, cx: &mut Cx) -> AnyElement {
    div()
        .id(id)
        .w(z(40.))
        .h(z(20.))
        .rounded(z(8.))
        .bg(bg)
        .cursor_pointer()
        .when(on, |d| d.border_2().border_color(gpui::white()))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| {
                if let Some(e) = s.role.as_mut() {
                    e.colors = set;
                }
            });
            cx.notify();
        }))
        .into_any_element()
}

fn color_picker(e: &Editor, cx: &mut Cx) -> AnyElement {
    let style = style_of(&e.colors);
    let custom_bg: gpui::Background = match style {
        Style::Gradient => gpui::linear_gradient(90., gpui::linear_color_stop(c(e.colors.primary), 0.), gpui::linear_color_stop(c(e.colors.secondary.unwrap_or(e.colors.primary)), 1.)),
        _ => c(if e.colors.primary == 0 { 0x99AAB5 } else { e.colors.primary }).into(),
    };
    let dropper = |col: u32| {
        div()
            .size(z(34.))
            .rounded_full()
            .bg(gpui::white())
            .flex()
            .items_center()
            .justify_center()
            .child(div().size(z(32.)).rounded_full().bg(c(col)).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("server/dropper")).size(z(14.)).text_color(gpui::white())))
    };
    let custom = div()
        .w(z(154.))
        .h(z(50.))
        .flex_none()
        .rounded(z(4.))
        .bg(custom_bg)
        .px(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .child(dropper(e.colors.primary))
        .when(style == Style::Gradient, |d| d.child(dropper(e.colors.secondary.unwrap_or(e.colors.primary))));
    let mut rows = div().flex().flex_col().gap(z(10.));
    match style {
        Style::Gradient => {
            for chunk in GRADIENTS.chunks(6).enumerate() {
                let (ri, list) = chunk;
                let mut row = div().flex().gap(z(8.));
                for (j, (_, a, b)) in list.iter().enumerate() {
                    let set = RoleColors { primary: *a, secondary: Some(*b), tertiary: None };
                    row = row.child(swatch(("ss-grad", ri * 6 + j), gpui::linear_gradient(180., gpui::linear_color_stop(c(*a), 0.), gpui::linear_color_stop(c(*b), 1.)), e.colors == set, set, cx));
                }
                rows = rows.child(row);
            }
        }
        _ => {
            for (ri, list) in SOLID.chunks(10).enumerate() {
                let mut row = div().flex().gap(z(8.));
                for (j, col) in list.iter().take(6).enumerate() {
                    let set = RoleColors { primary: *col, secondary: None, tertiary: None };
                    row = row.child(swatch(("ss-solid", ri * 10 + j), c(*col).into(), e.colors == set, set, cx));
                }
                rows = rows.child(row);
            }
        }
    }
    div().flex().gap(z(10.)).when(style != Style::Holographic, |d| d.child(custom).child(rows)).into_any_element()
}

fn required(label: SharedString) -> gpui::Div {
    div().flex().gap(z(4.)).child(text(label, 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(text("*", 16., 20., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL))
}

fn display(app: &PhoenixApp, s: &State, e: &Editor, cx: &mut Cx) -> AnyElement {
    let _ = app;
    let role = s.list("roles").iter().find(|r| kit::n(r, "id") == e.id).cloned().unwrap_or(Value::Null);
    let hoist = div()
        .id("ss-role-hoist")
        .flex()
        .gap(z(24.))
        .group(crate::ui::controls::SWITCH_ROW)
        .cursor_pointer()
        .child(labels(crate::t!("gs.role.hoist"), None).flex_1())
        .child(crate::ui::controls::switch("ss-role-hoist-sw", e.hoist))
        .on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| {
                if let Some(e) = s.role.as_mut() {
                    e.hoist = !e.hoist;
                }
            });
            cx.notify();
        }));
    let mention = div()
        .id("ss-role-mention")
        .flex()
        .gap(z(24.))
        .group(crate::ui::controls::SWITCH_ROW)
        .cursor_pointer()
        .child(labels(crate::t!("gs.role.mention"), Some(crate::t!("gs.role.mention_desc"))).flex_1())
        .child(crate::ui::controls::switch("ss-role-mention-sw", e.mention))
        .on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| {
                if let Some(e) = s.role.as_mut() {
                    e.mention = !e.mention;
                }
            });
            cx.notify();
        }));
    let line = || div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)));
    div()
        .flex()
        .flex_col()
        .child(div().mt(z(16.)).flex().flex_col().child(required(crate::t!("gs.role.name")).mb(z(8.))).children(e.name.as_ref().map(super::super::input_box)))
        .child(line())
        .child(text(crate::t!("gs.role.style"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG).mb(z(8.)))
        .child(
            div()
                .pb(z(8.))
                .flex()
                .gap(z(8.))
                .child(style_card(0, Style::Solid, crate::t!("gs.role.style_solid"), e, cx))
                .child(style_card(1, Style::Gradient, crate::t!("gs.role.style_gradient"), e, cx))
                .child(style_card(2, Style::Holographic, crate::t!("gs.role.style_holo"), e, cx)),
        )
        .child(div().mt(z(24.)).flex().flex_col().child(required(crate::t!("gs.role.color")).mb(z(4.))).child(text(crate::t!("gs.role.color_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).mb(z(8.))).child(color_picker(e, cx)))
        .child(
            div()
                .mt(z(16.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(text(crate::t!("gs.role.icon"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG))
                        .child(
                            div()
                                .px(z(6.))
                                .rounded(z(8.))
                                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                                .flex()
                                .items_center()
                                .gap(z(4.))
                                .child(gpui::svg().path(crate::icons::path("boost")).size(z(14.)).text_color(c(0xFF73FA)))
                                .child(text(crate::t!("gs.role.unlocked"), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT)),
                        ),
                )
                .child(text(crate::t!("gs.role.icon_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).mt(z(8.)))
                .child(
                    div()
                        .mt(z(16.))
                        .flex()
                        .items_center()
                        .gap(z(16.))
                        .child(div().size(z(80.)).flex_none().rounded(z(8.)).bg(c(tok(idx::BACKGROUND_BASE_LOWEST))).flex().items_center().justify_center().child(draft_icon(s.guild.0, &role, e, 40.)))
                        .child(crate::ui::controls::button("ss-role-icon-pick", crate::t!("gs.role.icon_pick"), BtnKind::Primary).on_click(cx.listener(|a, _, _, cx| pick_role_icon(a, cx))))
                        .when(has_icon(&role, e), |d| {
                            d.child(kit::critical_secondary("ss-role-icon-remove", crate::t!("gs.role.icon_remove")).h(z(40.)).px(z(16.)).on_click(cx.listener(|_, _, _, cx| {
                                with_state(|s| {
                                    if let Some(e) = s.role.as_mut() {
                                        e.icon = Some(None);
                                    }
                                });
                                cx.notify();
                            })))
                        }),
                ),
        )
        .child(theme_preview(s.guild.0, &role, e))
        .child(line())
        .child(hoist)
        .child(line())
        .child(mention)
        .child(line())
        .child(
            div()
                .flex()
                .flex_col()
                .child(labels(crate::t!("gs.role.view_as"), Some(crate::t!("gs.role.view_as_desc"))))
                .child(div().mt(z(8.)).flex().child(kit::btn_sm("ss-role-view-as", crate::t!("gs.role.view_as"), BtnKind::Secondary).on_click({
                    let (guild, role) = (s.guild, e.id);
                    cx.listener(move |a, _, _, cx| crate::ui::view_as::start(a, guild, crate::model::Id(role), cx))
                }))),
        )
        .into_any_element()
}

pub static PREVIEW_THEMES: [(&[u32; crate::theme::generated::COLOR_COUNT], bool); 4] =
    [(&crate::theme::generated::ASH, false), (&crate::theme::generated::DARK, false), (&crate::theme::generated::ONYX, false), (&crate::theme::generated::LIGHT, true)];

fn rgba(v: u32) -> Hsla {
    gpui::Hsla::from(gpui::rgba(v))
}

pub fn preview_stops(colors: &RoleColors) -> Option<[u32; 3]> {
    colors.secondary.map(|s| [colors.primary, s, colors.tertiary.unwrap_or(colors.primary)])
}

fn theme_preview(guild: u64, role: &Value, e: &Editor) -> AnyElement {
    let stops = preview_stops(&e.colors);
    let now = chrono::Local::now().format("%H:%M").to_string();
    let mut out = div().mt(z(16.)).rounded(z(8.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).overflow_hidden().flex().flex_col();
    for (n, (table, light)) in PREVIEW_THEMES.iter().enumerate() {
        let col = |i: usize| rgba(table[i]);
        let name: AnyElement = match stops {
            Some(st) => div().child(crate::fonts::name_text_with("Wumpus".into(), None, 16., Some(st))).into_any_element(),
            None => div()
                .text_size(z(16.))
                .line_height(z(22.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if e.colors.primary == 0 { col(idx::TEXT_STRONG) } else { c(e.colors.primary) })
                .child("Wumpus")
                .into_any_element(),
        };
        let row = div()
            .py(z(16.))
            .child(
                div()
                    .relative()
                    .pl(z(80.))
                    .pr(z(30.))
                    .py(z(2.))
                    .min_h(z(48.))
                    .child(div().absolute().left(z(20.)).top(z(4.)).size(z(40.)).rounded_full().overflow_hidden().child(gpui::img(SharedString::from("https://cdn.discordapp.com/embed/avatars/0.png")).size(z(40.))))
                    .child(
                        div()
                            .h(z(22.))
                            .flex()
                            .items_center()
                            .child(name)
                            .child(div().ml(z(4.)).child(role_icon(guild, role, 20.)))
                            .child(div().ml(z(4.)).text_size(z(12.)).line_height(z(22.)).font_weight(FontWeight::MEDIUM).text_color(col(idx::TEXT_MUTED)).child(now.clone())),
                    )
                    .child(div().text_size(z(16.)).line_height(z(22.)).text_color(col(idx::TEXT_DEFAULT)).child(crate::t!("gs.role.preview_message"))),
            );
        let mut wrap = div().id(("ss-role-preview", n)).bg(col(idx::BACKGROUND_BASE_LOW)).flex().flex_col().child(row);
        if *light {
            wrap = wrap.child(
                div()
                    .px(z(16.))
                    .py(z(8.))
                    .bg(col(idx::BACKGROUND_MOD_SUBTLE))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(col(idx::TEXT_SUBTLE))
                    .child(crate::t!("gs.role.light_notice")),
            );
        }
        out = out.child(wrap);
    }
    out.into_any_element()
}

fn links() -> AnyElement {
    div()
        .mt(z(24.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(text(crate::t!("gs.role.links_notice"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).p(z(8.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_INFO))))
        .child(text(crate::t!("gs.role.links_title"), 14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG).mt(z(16.)))
        .child(text(crate::t!("gs.role.links_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE))
        .child(div().mt(z(16.)).flex().justify_center().child(crate::ui::controls::button("ss-role-link-add", crate::t!("gs.role.links_add"), BtnKind::Secondary)))
        .into_any_element()
}

fn members(app: &PhoenixApp, e: &Editor, cx: &mut Cx) -> AnyElement {
    let rows: Vec<AnyElement> = e
        .members
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, id)| {
            let u = app.users.get(&crate::model::Id(*id));
            let name: SharedString = u.map(|u| u.display_name.clone()).unwrap_or_else(|| id.to_string().into());
            let tag: SharedString = u.map(|u| u.name.clone()).unwrap_or_default();
            div()
                .id(("ss-role-member", i))
                .h(z(40.))
                .py(z(4.))
                .pl(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .child(crate::ui::avatar(u, 24.))
                .child(text(name, 14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG).ml(z(8.)))
                .child(text(tag, 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).ml(z(4.)))
                .child(div().flex_1())
                .child(div().mx(z(8.)).child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
                .into_any_element()
        })
        .collect();
    let _ = cx;
    div()
        .mt(z(16.))
        .flex()
        .flex_col()
        .child(div().flex().items_center().gap(z(16.)).child(kit::search_box(None).flex_1()).child(kit::btn_sm("ss-role-add-members", crate::t!("gs.role.add_members"), BtnKind::Primary)))
        .child(div().mt(z(24.)).flex().flex_col().children(rows))
        .into_any_element()
}

pub fn editor(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let Some(e) = s.role.as_ref() else {
        return div().into_any_element();
    };
    let name = if e.everyone { "@everyone".to_owned() } else { e.name0.clone() };
    let body = match e.tab {
        Tab::Display => display(app, s, e, cx),
        Tab::Permissions => permissions(e, cx),
        Tab::Links => links(),
        Tab::Members => members(app, e, cx),
    };
    let header = div()
        .pt(z(60.))
        .px(z(8.))
        .pb(z(16.))
        .mx(z(-8.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(text(crate::t!("gs.role.edit_title", name = name), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate().min_w_0())
                .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        )
        .child(tabs(e, cx));
    let right = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .ml(z(16.))
        .flex()
        .flex_col()
        .child(div().flex_none().child(header))
        .child(div().id("ss-role-scroll").flex_1().min_h_0().overflow_y_scroll().pb(z(60.)).child(body));
    div()
        .h_full()
        .w(z(740.))
        .max_w_full()
        .pr(z(40.))
        .flex()
        .child(div().id("ss-role-side-scroll").flex_none().h_full().overflow_y_scroll().child(side(app, s, e.id, cx)))
        .child(right)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_themes_follow_the_client_order() {
        assert_eq!(PREVIEW_THEMES.len(), 4);
        assert!(PREVIEW_THEMES[3].1 && !PREVIEW_THEMES[0].1);
        assert_eq!(crate::theme::generated::ASH[idx::BACKGROUND_BASE_LOW], 0x3637_3EFF);
        assert_eq!(crate::theme::generated::DARK[idx::BACKGROUND_BASE_LOW], 0x2020_24FF);
        assert_eq!(preview_stops(&RoleColors { primary: 1, secondary: Some(2), tertiary: None }), Some([1, 2, 1]));
        assert_eq!(preview_stops(&RoleColors { primary: 1, secondary: None, tertiary: None }), None);
    }
    use serde_json::json;

    #[test]
    fn reorder_and_positions() {
        let order = vec![1, 2, 3, 4];
        assert_eq!(move_role(&order, 4, 0), vec![4, 1, 2, 3]);
        assert_eq!(move_role(&order, 1, 9), vec![2, 3, 4, 1]);
        assert_eq!(positions(&[7, 8]), vec![(Snowflake(7), 2), (Snowflake(8), 1)]);
    }

    #[test]
    fn styles_from_colors() {
        let r = json!({ "id": "4", "color": 1, "colors": { "primary_color": 11_127_295, "secondary_color": 16_759_788, "tertiary_color": 16_761_760 } });
        assert_eq!(style_of(&colors_of(&r)), Style::Holographic);
        let r = json!({ "id": "4", "color": 5, "colors": { "primary_color": 5, "secondary_color": 6, "tertiary_color": null } });
        assert_eq!(style_of(&colors_of(&r)), Style::Gradient);
        assert_eq!(style_of(&colors_of(&json!({ "id": "4", "color": 7 }))), Style::Solid);
    }

    #[test]
    fn editor_only_sends_changes() {
        let r = json!({ "id": "4", "name": "Mods", "permissions": "8", "color": 0, "hoist": false, "mentionable": true });
        let mut e = Editor::new(&r, false);
        assert!(!e.dirty());
        e.perms |= 1 << 4;
        e.hoist = true;
        assert!(e.dirty());
        e.reset();
        assert!(!e.dirty());
        assert_eq!(PERMS.iter().map(|p| p.1.len()).sum::<usize>(), 51);
    }
}
