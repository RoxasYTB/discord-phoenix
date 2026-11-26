use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, text},
};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::BtnKind,
};

pub fn tier(s: &State) -> u8 {
    kit::n(s.guild_json(), "premium_tier").min(3) as u8
}

pub fn emoji_slots(tier: u8) -> usize {
    [50, 100, 150, 250][tier.min(3) as usize]
}

pub fn sticker_slots(tier: u8) -> usize {
    [5, 15, 30, 60][tier.min(3) as usize]
}

pub fn sound_slots(tier: u8) -> usize {
    [8, 24, 36, 48][tier.min(3) as usize]
}

fn header(title: SharedString, desc: SharedString, upload: SharedString, id: &'static str, cx: &mut kit::Cx) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(z(20.))
        .child(div().flex().flex_col().gap(z(4.)).child(kit::h2(title)).child(text(desc, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT)))
        .child(div().flex().child(crate::ui::controls::button(id, upload, BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| super::pick_upload("emoji", &crate::ui::file_pick::IMAGES, cx)))))
}

fn uploader(u: Option<&Value>) -> AnyElement {
    let Some(u) = u else {
        return div().into_any_element();
    };
    div()
        .flex()
        .items_center()
        .min_w_0()
        .child(div().mr(z(12.)).child(kit::avatar(kit::user_avatar(u, 40), 20.)))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(text(kit::user_name(u), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).truncate())
                .child(text(kit::s(u, "username"), 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED).mt(z(-2.)).truncate()),
        )
        .into_any_element()
}

fn emoji_table(title: SharedString, list: &[&Value], slots: usize, cx: &mut Cx) -> AnyElement {
    let _ = cx;
    let head = div()
        .py(z(8.))
        .px(z(16.))
        .mb(z(4.))
        .rounded_t(z(12.))
        .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
        .flex()
        .child(text(crate::t!("gs.expr.image"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).w(z(49.3)))
        .child(text(crate::t!("gs.expr.name"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).flex_1())
        .child(text(crate::t!("gs.expr.uploader"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).flex_1())
        .child(div().w(z(62.4)));
    let mut body = div().py(z(4.)).px(z(24.)).flex().flex_col();
    for (i, e) in list.iter().enumerate() {
        let id = kit::s(e, "id");
        let anim = e.get("animated").and_then(Value::as_bool).unwrap_or(false);
        let url: SharedString = format!("https://cdn.discordapp.com/emojis/{id}.{}?size=64", if anim { "gif" } else { "webp" }).into();
        let group: SharedString = format!("ss-emoji-{id}").into();
        body = body.child(
            div()
                .id(("ss-emoji", i))
                .group(group.clone())
                .h(z(64.))
                .flex()
                .items_center()
                .when(i > 0, |d| d.border_t_1().border_color(c(tok(idx::BORDER_SUBTLE))))
                .child(div().w(z(48.1)).flex_none().child(gpui::img(url).size(z(32.)).object_fit(gpui::ObjectFit::Contain)))
                .child(div().flex_1().min_w_0().flex().items_center().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(format!(":{}:", kit::s(e, "name"))))
                .child(div().flex_1().min_w_0().child(uploader(e.get("user"))))
                .child(
                    div()
                        .flex()
                        .gap(z(10.))
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .child(kit::icon_btn(("ss-emoji-edit", i), "pen", 32., 16., false))
                        .child(kit::icon_btn(("ss-emoji-del", i), "close-small", 32., 16., true)),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(kit::legend(title, Some(crate::t!("gs.expr.slots_left", n = slots.saturating_sub(list.len())))))
        .when(!list.is_empty(), |d| d.child(div().mb(z(48.)).rounded(z(12.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).flex().flex_col().child(head).child(body)))
        .into_any_element()
}

pub fn emoji(_app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let mut all: Vec<&Value> = s.list("emojis").iter().collect();
    all.sort_by_key(|e| std::cmp::Reverse(kit::n(e, "id")));
    let stat: Vec<&Value> = all.iter().copied().filter(|e| !e.get("animated").and_then(Value::as_bool).unwrap_or(false)).collect();
    let anim: Vec<&Value> = all.iter().copied().filter(|e| e.get("animated").and_then(Value::as_bool).unwrap_or(false)).collect();
    let slots = emoji_slots(tier(s));
    div()
        .flex()
        .flex_col()
        .gap(z(20.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(24.))
                .pb(z(24.))
                .mb(z(24.))
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(header(crate::t!("menu.gs.emoji"), crate::t!("gs.expr.emoji_desc", n = slots), crate::t!("gs.expr.emoji_upload"), "ss-emoji-upload", cx))
                .child(text(crate::t!("gs.expr.emoji_drop"), 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT)),
        )
        .child(emoji_table(crate::t!("menu.gs.emoji"), &stat, slots, cx))
        .child(emoji_table(crate::t!("gs.expr.animated"), &anim, slots, cx))
        .into_any_element()
}

pub const TIER_BOOSTS: [u64; 3] = [2, 7, 14];
pub const TIER_EXTRA: [usize; 4] = [5, 10, 15, 30];
const BOOST_PINK: u32 = 0xFF73FA;

pub fn sticker_tiers(count: usize) -> [usize; 4] {
    let mut left = count;
    let mut out = [0; 4];
    for (i, cap) in TIER_EXTRA.iter().enumerate() {
        out[i] = left.min(*cap);
        left -= out[i];
    }
    out
}

pub fn lost_level(count: usize, tier: u8) -> Option<u8> {
    (count > sticker_slots(tier)).then(|| (1..=3).find(|l| count <= sticker_slots(*l)).unwrap_or(3))
}

fn sticker_card(i: usize, st: &Value, cx: &mut Cx) -> AnyElement {
    let id = kit::s(st, "id").to_owned();
    let ext = match kit::n(st, "format_type") {
        3 => "json",
        4 => "gif",
        _ => "png",
    };
    let group = SharedString::from(format!("ss-sticker-{i}"));
    let user = st.get("user");
    let action = |n: usize, icon: &'static str, color: usize| {
        div()
            .id(("ss-sticker-act", i * 2 + n))
            .size(z(24.))
            .rounded(z(24.))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(c(tok(color))))
    };
    let sid = id.parse::<u64>().unwrap_or(0);
    div()
        .id(("ss-sticker", i))
        .group(group.clone())
        .relative()
        .w(z(116.))
        .h(z(140.))
        .p(z(12.))
        .rounded(z(8.))
        .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(if ext == "json" {
            div().size(z(72.)).into_any_element()
        } else {
            gpui::img(SharedString::from(format!("https://media.discordapp.net/stickers/{id}.{ext}?size=160"))).size(z(72.)).object_fit(gpui::ObjectFit::Contain).into_any_element()
        })
        .child(text(kit::s(st, "name"), 12., 16., FontWeight::SEMIBOLD, idx::TEXT_STRONG).mt(z(8.)).max_w_full().truncate())
        .children(user.map(|u| {
            div()
                .mt(z(4.))
                .max_w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().mr(z(4.)).child(kit::avatar(kit::user_avatar(u, 32), 16.)))
                .child(text(kit::user_name(u), 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT).truncate().min_w_0())
        }))
        .child(
            div()
                .absolute()
                .top(z(-11.))
                .right(z(-8.))
                .invisible()
                .group_hover(group, |d| d.visible())
                .flex()
                .flex_col()
                .child(action(0, "pen", idx::TEXT_SUBTLE))
                .child(action(1, "close-small", idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)).on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(g) = super::with_state(|s| s.guild) {
                        a.admin_toast(phoenix_api::rest::guild_manage::AdminCall::DeleteSticker { guild: crate::actions::admin::sf(g), sticker: phoenix_api::models::Snowflake(sid) }, None, cx);
                    }
                }))),
        )
        .into_any_element()
}

fn rail(state: u8) -> gpui::Div {
    let dot = match state {
        2 => div().size(z(26.)).rounded_full().bg(c(BOOST_PINK)).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("boost")).size(z(16.)).text_color(gpui::white())),
        1 => div().size(z(16.)).rounded_full().bg(c(BOOST_PINK)),
        _ => div().size(z(16.)).rounded_full().bg(c(tok(idx::BACKGROUND_MOD_STRONG))),
    };
    div()
        .relative()
        .flex_none()
        .w(z(24.))
        .child(div().absolute().left(z(11.)).top_0().bottom_0().w(z(2.)).bg(if state >= 1 { c(BOOST_PINK) } else { c(tok(idx::BACKGROUND_MOD_STRONG)) }))
        .child(div().absolute().top(z(if state == 2 { 21. } else { 26. })).left_0().w(z(24.)).flex().justify_center().child(dot))
}

pub fn stickers(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let list = s.list("stickers").to_vec();
    let t = tier(s);
    let guild = s.guild;
    let partnered = s.guild_json().get("features").and_then(Value::as_array).is_some_and(|f| f.iter().any(|x| matches!(x.as_str(), Some("PARTNERED" | "VERIFIED"))));
    let mut page = div().flex().flex_col().gap(z(16.)).child(kit::h2(crate::t!("menu.gs.stickers")));
    let mut top = div().flex().flex_col();
    if let Some(level) = lost_level(list.len(), t) {
        top = top.child(
            div()
                .mb(z(23.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(gpui::linear_gradient(90., gpui::linear_color_stop(gpui::rgb(0x3442D9), 0.), gpui::linear_color_stop(gpui::rgb(0x9B44F7), 1.)))
                .flex()
                .items_center()
                .child(gpui::svg().path(crate::icons::path("boost")).w(z(25.)).h(z(38.)).text_color(gpui::white()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pl(z(16.))
                        .flex()
                        .flex_col()
                        .child(text(crate::t!("gs.st.lost_title", level = level), 20., 24., FontWeight::SEMIBOLD, idx::WHITE).pb(z(4.)))
                        .child(text(crate::t!("gs.st.lost_desc", level = level), 14., 18., FontWeight::NORMAL, idx::WHITE)),
                )
                .child(div().flex_none().h(z(40.)).px(z(12.)).rounded(z(8.)).bg(gpui::white()).flex().items_center().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(gpui::black()).child(crate::t!("gs.st.boost"))),
        );
    }
    top = top.child(
        div()
            .flex()
            .flex_col()
            .gap(z(8.))
            .child(text(crate::t!("gs.expr.sticker_rules"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
            .child(text(crate::t!("gs.expr.sticker_rules_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT)),
    );
    page = page.child(top).child(div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))));
    let subs = crate::ui::server_pages::fetch(app, s.guild, phoenix_api::rest::guild_server::ServerCall::PremiumSubscriptions { guild: crate::actions::admin::sf(s.guild) }, cx);
    let mut users: Vec<Value> = Vec::new();
    for sub in subs.as_ref().and_then(Value::as_array).into_iter().flatten() {
        if let Some(u) = sub.get("user")
            && !users.iter().any(|x| kit::s(x, "id") == kit::s(u, "id"))
        {
            users.push(u.clone());
        }
    }
    let mut col = div().flex().flex_col();
    if !users.is_empty() {
        let mut avatars = div().flex().items_center();
        for u in users.iter().take(4) {
            avatars = avatars.child(div().mr(z(-8.)).size(z(30.)).rounded_full().border_2().border_color(c(tok(idx::BACKGROUND_BASE_LOW))).overflow_hidden().child(kit::avatar(kit::user_avatar(u, 64), 30.)));
        }
        if users.len() > 4 {
            avatars = avatars.child(div().size(z(30.)).rounded_full().bg(gpui::rgb(0x1E1F22)).flex().items_center().justify_center().text_size(z(12.)).text_color(gpui::white()).child(format!("+{}", users.len() - 4)));
        }
        let n = users.len();
        let bold = crate::t!(if n == 1 { "gs.st.boosters_bold_one" } else { "gs.st.boosters_bold" }, n = n);
        let rest = crate::t!(if n == 1 { "gs.st.boosters_rest_one" } else { "gs.st.boosters_rest" });
        col = col.child(
            div().flex().child(rail(1)).child(
                div()
                    .ml(z(14.))
                    .flex()
                    .flex_col()
                    .child(avatars)
                    .child(div().mt(z(8.)).mb(z(24.)).flex().child(text(bold, 16., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT)).child(text(rest, 16., 16., FontWeight::NORMAL, idx::TEXT_MUTED))),
            ),
        );
    }
    let per = sticker_tiers(list.len());
    let mut start = 0;
    for level in 0..4usize {
        let unlocked = level == 0 || t as usize >= level;
        let used = per[level];
        let stickers_here: Vec<Value> = list.iter().skip(start).take(used).cloned().collect();
        start += used;
        let state = if !unlocked {
            0
        } else if level == t as usize && level > 0 {
            2
        } else {
            1
        };
        let title = if level == 0 { crate::t!("gs.st.free") } else { crate::t!("gs.st.level", n = level) };
        let sub = crate::t!("gs.st.available", avail = TIER_EXTRA[level] - used, n = TIER_EXTRA[level]);
        let right: Option<AnyElement> = if !unlocked {
            Some(text(crate::t!("gs.st.boosts", n = TIER_BOOSTS[level - 1]), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).into_any_element())
        } else if level == t as usize && level < 3 {
            Some(
                kit::btn_sm("ss-sticker-unlock", crate::t!("gs.st.unlock"), BtnKind::Primary)
                    .on_click(cx.listener(|_, _, _, cx| {
                        if let Some(g) = super::with_state(|s| s.guild) {
                            crate::ui::boost_layer::open(g);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else {
            None
        };
        let mut grid = div().px(z(16.)).pb(z(16.)).flex().flex_wrap().gap(z(10.));
        for (k, st) in stickers_here.iter().enumerate() {
            grid = grid.child(sticker_card(start - used + k, st, cx));
        }
        if unlocked && used < TIER_EXTRA[level] && start == list.len() {
            grid = grid.child(
                div()
                    .id(("ss-sticker-add", level))
                    .w(z(116.))
                    .h(z(140.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(c(tok(idx::BORDER_STRONG)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::server_pages::expr_upload::open_sticker(guild, partnered);
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("plus")).size(z(24.)).text_color(c(tok(idx::TEXT_SUBTLE)))),
            );
        }
        let header = div()
            .m(z(1.))
            .mb(z(16.))
            .p(z(16.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
            .flex()
            .items_center()
            .when(level > 0, |d| d.child(gpui::svg().path(crate::icons::path("boost")).flex_none().mr(z(16.)).size(z(32.)).text_color(if unlocked { c(BOOST_PINK) } else { c(tok(idx::TEXT_MUTED)) })))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(text(title, 16., 20., FontWeight::SEMIBOLD, if unlocked { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE }))
                    .when(unlocked, |d| d.child(text(sub, 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT))),
            )
            .children(right);
        let section = div()
            .flex_1()
            .min_w_0()
            .ml(z(14.))
            .when(level < 3, |d| d.mb(z(24.)))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
            .flex()
            .flex_col()
            .child(header)
            .when(!stickers_here.is_empty() || unlocked, |d| d.child(grid));
        col = col.child(div().flex().child(rail(state)).child(section));
    }
    page.child(col).into_any_element()
}

pub fn soundboard(_app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let guild = s.guild;
    let items: Vec<Value> = s.get("sounds").and_then(|v| v.get("items")).and_then(Value::as_array).cloned().unwrap_or_default();
    let t = tier(s);
    let slots = sound_slots(t);
    let head = div()
        .flex()
        .pb(z(8.))
        .child(text(crate::t!("gs.expr.emoji_col"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG).w(z(60.)).mr(z(8.)))
        .child(text(crate::t!("gs.expr.name"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG).w(z(328.)).mr(z(8.)))
        .child(text(crate::t!("gs.expr.uploader"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG).w(z(200.)));
    let mut rows = div().flex().flex_col();
    for (i, snd) in items.iter().enumerate() {
        let emoji: AnyElement = match (kit::id_of(snd, "emoji_id"), snd.get("emoji_name").and_then(Value::as_str)) {
            (Some(id), _) => gpui::img(SharedString::from(format!("https://cdn.discordapp.com/emojis/{id}.webp?size=48"))).size(z(24.)).into_any_element(),
            (None, Some(n)) => crate::media::emoji::emoji_element(n, 24.),
            _ => div().size(z(24.)).into_any_element(),
        };
        let user = snd.get("user").cloned().unwrap_or(Value::Null);
        rows = rows.child(
            div()
                .id(("ss-sound", i))
                .h(z(60.8))
                .flex()
                .items_center()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(div().w(z(60.)).mr(z(8.)).flex().justify_center().child(emoji))
                .child(text(kit::s(snd, "name"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(328.)).mr(z(8.)).truncate())
                .child(div().w(z(200.)).flex().items_center().gap(z(8.)).child(kit::avatar(kit::user_avatar(&user, 40), 20.)).child(text(kit::s(&user, "username"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).truncate()))
                .child(div().flex_1().flex().justify_end().child(kit::icon_btn(("ss-sound-play", i), "play", 32., 16., false))),
        );
    }
    div()
        .flex()
        .flex_col()
        .child(kit::h2(crate::t!("menu.gs.soundboard")))
        .child(text(crate::t!("gs.expr.sound_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).mt(z(16.)))
        .child(div().mt(z(24.)).flex().child(crate::ui::controls::button("ss-sound-upload", crate::t!("gs.expr.sound_upload"), BtnKind::Primary).on_click(cx.listener(move |_, _, _, cx| {
            crate::ui::server_pages::expr_upload::open_sound(guild);
            cx.notify();
        }))))
        .child(text(crate::t!("gs.expr.sound_slots", tier = t, n = slots.saturating_sub(items.len()), total = slots), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).mt(z(50.)).h(z(24.)))
        .child(div().mt(z(32.)).child(head))
        .child(rows)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticker_tiers_fill_free_slots_first() {
        assert_eq!(sticker_tiers(0), [0, 0, 0, 0]);
        assert_eq!(sticker_tiers(7), [5, 2, 0, 0]);
        assert_eq!(sticker_tiers(40), [5, 10, 15, 10]);
        assert_eq!(lost_level(40, 2), Some(3));
        assert_eq!(lost_level(20, 2), None);
        assert_eq!(lost_level(20, 1), Some(2));
    }

    #[test]
    fn quotas_follow_boost_tier() {
        assert_eq!((emoji_slots(0), emoji_slots(2), emoji_slots(9)), (50, 150, 250));
        assert_eq!((sticker_slots(1), sticker_slots(3)), (15, 60));
        assert_eq!((sound_slots(0), sound_slots(2)), (8, 36));
    }
}
