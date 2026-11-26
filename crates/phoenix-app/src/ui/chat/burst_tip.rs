use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, anchored, canvas, deferred, div, point, prelude::*, px,
};

use crate::actions::{ActionReply, ActionRequest};
use crate::app::PhoenixApp;
use crate::model::{Id, Message, Reaction};
use crate::theme::{c, generated::idx, tok, z};

pub const SHOW_AFTER: Duration = Duration::from_millis(750);
pub const HIDE_AFTER: Duration = Duration::from_millis(200);
const MAX_W: f32 = 288.;
const GAP: f32 = 8.;
const ROOM: f32 = 120.;

type ReactorKey = (u64, u64, String);

thread_local! {
    static BOUNDS: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    static USERS: RefCell<HashMap<ReactorKey, Vec<Id>>> = RefCell::new(HashMap::new());
    static LOADING: RefCell<HashSet<ReactorKey>> = RefCell::new(HashSet::new());
    static SHOWN: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

fn reactor_key(channel: Id, msg: Id, r: &Reaction) -> ReactorKey {
    (channel.0, msg.0, r.emoji_id.map_or_else(|| r.emoji.to_string(), |i| i.0.to_string()))
}

pub fn tip_key(pill: u64) -> u64 {
    crate::ui::states::key(("reaction-burst-tip", pill))
}

pub fn text(names: &[String], total: u32, emoji: &str) -> Option<SharedString> {
    let n = total.saturating_sub(names.len() as u32);
    let ns = n.to_string();
    let plural = |base: &str| if n == 1 { format!("{base}_one") } else { base.to_owned() };
    let s = match names {
        [] => return None,
        [a] if n == 0 => crate::i18n::t("burst.tip_1", &[("a", a.as_str()), ("emoji", emoji)]),
        [a] => crate::i18n::t(&plural("burst.tip_1n"), &[("a", a.as_str()), ("n", ns.as_str()), ("emoji", emoji)]),
        [a, b] if n == 0 => crate::i18n::t("burst.tip_2", &[("a", a.as_str()), ("b", b.as_str()), ("emoji", emoji)]),
        [a, b] => crate::i18n::t(&plural("burst.tip_2n"), &[("a", a.as_str()), ("b", b.as_str()), ("n", ns.as_str()), ("emoji", emoji)]),
        [a, b, cc, ..] if n == 0 => crate::i18n::t("burst.tip_3", &[("a", a.as_str()), ("b", b.as_str()), ("c", cc.as_str()), ("emoji", emoji)]),
        [a, b, cc, ..] => crate::i18n::t(
            &plural("burst.tip_3n"),
            &[("a", a.as_str()), ("b", b.as_str()), ("c", cc.as_str()), ("n", ns.as_str()), ("emoji", emoji)],
        ),
    };
    Some(s)
}

pub fn visible(pill: Option<(bool, std::time::Instant)>, tip: Option<(bool, std::time::Instant)>, shown: bool) -> bool {
    let ready = pill.is_some_and(|(on, at)| on && at.elapsed() >= SHOW_AFTER);
    if ready {
        return true;
    }
    if !shown {
        return false;
    }
    let on = |s: Option<(bool, std::time::Instant)>| s.is_some_and(|(o, _)| o);
    let left = |s: Option<(bool, std::time::Instant)>| s.is_some_and(|(o, at)| !o && at.elapsed() < HIDE_AFTER);
    on(pill) || on(tip) || left(pill) || left(tip)
}

pub fn refresh_after(cx: &mut App, d: Duration) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(d + Duration::from_millis(10)).await;
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

pub fn on_pill_hover(entity: &Entity<PhoenixApp>, channel: Id, id: Id, r: &Reaction, on: bool, cx: &mut App) {
    if !on {
        refresh_after(cx, HIDE_AFTER);
        return;
    }
    refresh_after(cx, SHOW_AFTER);
    entity.update(cx, |a, cx| load_names(a, channel, id, r, cx));
}

pub fn load_names(a: &mut PhoenixApp, channel: Id, id: Id, r: &Reaction, cx: &mut gpui::Context<PhoenixApp>) {
    let key = reactor_key(channel, id, r);
    if USERS.with(|u| u.borrow().contains_key(&key)) || LOADING.with(|l| l.borrow().contains(&key)) {
        return;
    }
    LOADING.with(|l| l.borrow_mut().insert(key.clone()));
    let emoji = r.clone();
    a.action(ActionRequest::Reactors { channel, msg: id, emoji, burst: true }, cx, move |a, res, _| {
        LOADING.with(|l| l.borrow_mut().remove(&key));
        let ids = match res {
            Ok(ActionReply::Users(us)) => {
                let ids = us.iter().map(|u| u.id).collect();
                for u in us {
                    a.users.entry(u.id).or_insert(u);
                }
                ids
            }
            _ => Vec::new(),
        };
        USERS.with(|u| u.borrow_mut().insert(key, ids));
    });
}

fn names(app: &PhoenixApp, msg: &Message, r: &Reaction) -> Vec<String> {
    let guild = app.channels.get(&msg.channel).and_then(|c| c.guild);
    let ids = USERS.with(|u| u.borrow().get(&reactor_key(msg.channel, msg.id, r)).cloned()).unwrap_or_default();
    ids.into_iter()
        .filter(|u| !app.nav.is_blocked(*u) && !app.nav.is_ignored(*u))
        .filter_map(|u| app.member_nick_in(guild, u).map(|n| n.to_string()).or_else(|| app.users.get(&u).map(|x| x.display_name.to_string())))
        .take(3)
        .collect()
}

pub fn probe(pill: u64) -> AnyElement {
    canvas(
        move |b, _, _| {
            BOUNDS.with(|m| m.borrow_mut().insert(pill, b));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

fn body_text(s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_DEFAULT))).child(s.into())
}

fn nitro_icon() -> gpui::Svg {
    gpui::svg().path(crate::icons::path("nitro")).size(z(24.)).flex_none().mr(z(5.)).text_color(c(tok(idx::TEXT_DEFAULT)))
}

pub fn layer(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &Reaction, pill: u64) -> Option<AnyElement> {
    let tip = tip_key(pill);
    let shown = SHOWN.with(|s| s.borrow().contains(&pill));
    let show = visible(crate::ui::states::state(pill), crate::ui::states::state(tip), shown);
    let label = crate::ui::modals::reactions::tab_label(r);
    let line = show.then(|| text(&names(app, msg, r), r.burst_count, &label)).flatten();
    SHOWN.with(|s| {
        let mut s = s.borrow_mut();
        if line.is_some() {
            s.insert(pill);
        } else {
            s.remove(&pill);
        }
    });
    let line = line?;
    let b = BOUNDS.with(|m| m.borrow().get(&pill).copied())?;
    let premium = app.backend.has_premium();
    let (ch, id) = (msg.channel, msg.id);
    let e_open = entity.clone();
    let e_add = entity.clone();
    let e_unlock = entity.clone();
    let rc = r.clone();
    let list = msg.reactions.clone();
    let selected = crate::ui::modals::reactions::entries(&list)
        .iter()
        .position(|en| en.burst && list.get(en.index).is_some_and(|x| x.emoji == r.emoji && x.emoji_id == r.emoji_id))
        .unwrap_or(0);
    let inner = div()
        .id(("burst-tip-inner", pill))
        .cursor_pointer()
        .flex()
        .items_center()
        .child(div().size(z(32.)).flex_none().child(crate::media::emoji::emoji_element(&r.emoji, 32.)))
        .child(body_text(format!("\u{200E}{line}")).ml(z(12.)).flex_1().min_w_0())
        .on_click(move |_, _, cx| {
            SHOWN.with(|s| s.borrow_mut().remove(&pill));
            e_open.update(cx, |a, cx| a.open_reactions(ch, id, selected, cx));
        });
    let prompt = div().mt(z(12.)).pt(z(8.)).border_t_1().border_color(c(tok(idx::BORDER_SUBTLE))).w_full().flex().items_center();
    let footer = if r.me_burst {
        prompt.when(premium, |d| d.child(nitro_icon())).child(body_text(crate::t!("burst.tip_sent")).ml(z(12.))).into_any_element()
    } else if premium {
        prompt
            .id(("burst-tip-add", pill))
            .cursor_pointer()
            .child(nitro_icon())
            .child(body_text(crate::t!("burst.tip_add")).ml(z(12.)))
            .on_click(move |_, _, cx| e_add.update(cx, |a, cx| a.toggle_burst_reaction(ch, id, &rc, cx)))
            .into_any_element()
    } else {
        prompt
            .child(
                div().w_full().flex().flex_col().child(body_text(crate::t!("burst.tip_upsell"))).child(
                    div().mt(z(8.)).w_full().flex().child(
                        crate::ui::controls::button_icon(("burst-tip-unlock", pill), "nitro", crate::t!("burst.tip_unlock"), crate::ui::controls::BtnKind::Expressive)
                            .flex_1()
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                e_unlock.update(cx, crate::ui::modals::burst_upsell::open_nitro);
                            }),
                    ),
                ),
            )
            .into_any_element()
    };
    let card = div()
        .id(("burst-tip", pill))
        .max_w(z(MAX_W))
        .p(z(16.))
        .rounded(z(crate::theme::generated::global::RADIUS_XS))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .shadow(crate::ui::inbox::popout_shadow())
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .occlude()
        .on_hover(move |on, window, cx| {
            if crate::ui::states::set_hover(tip, *on) {
                window.refresh();
            }
            if !*on {
                refresh_after(cx, HIDE_AFTER);
            }
        })
        .child(inner)
        .child(footer);
    let top = f32::from(b.origin.y);
    let mid = f32::from(b.origin.x + b.size.width / 2.);
    let span = f32::from(z(MAX_W + 32.));
    let below = top < f32::from(z(ROOM));
    let frame = div().w(px(span)).flex().flex_col().items_center().child(card);
    let (corner, y) = if below { (gpui::Anchor::TopLeft, f32::from(b.origin.y + b.size.height) + f32::from(z(GAP))) } else { (gpui::Anchor::BottomLeft, top - f32::from(z(GAP))) };
    let el = deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .anchor(corner)
            .snap_to_window_with_margin(z(8.))
            .position(point(px(mid - span / 2.), px(y)))
            .child(frame),
    )
    .with_priority(3);
    Some(div().absolute().top_0().left_0().child(el).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn text_follows_the_client_cases() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let t = crate::i18n::t;
        assert_eq!(text(&[], 3, ":fire:"), None);
        assert_eq!(text(&n(&["Ann"]), 1, ":fire:"), Some(t("burst.tip_1", &[("a", "Ann"), ("emoji", ":fire:")])));
        assert_eq!(text(&n(&["Ann"]), 2, ":fire:"), Some(t("burst.tip_1n_one", &[("a", "Ann"), ("n", "1"), ("emoji", ":fire:")])));
        assert_eq!(
            text(&n(&["Ann", "Bob", "Cy"]), 7, ":fire:"),
            Some(t("burst.tip_3n", &[("a", "Ann"), ("b", "Bob"), ("c", "Cy"), ("n", "4"), ("emoji", ":fire:")]))
        );
        assert_eq!(text(&n(&["Ann", "Bob"]), 2, ":x:"), Some(t("burst.tip_2", &[("a", "Ann"), ("b", "Bob"), ("emoji", ":x:")])));
    }

    #[test]
    fn tooltip_waits_then_lingers() {
        let now = Instant::now();
        let ago = |ms: u64| now - Duration::from_millis(ms);
        assert!(!visible(Some((true, ago(100))), None, false));
        assert!(visible(Some((true, ago(800))), None, false));
        assert!(visible(Some((false, ago(50))), None, true));
        assert!(!visible(Some((false, ago(400))), None, true));
        assert!(visible(Some((false, ago(400))), Some((true, ago(300))), true));
        assert!(!visible(Some((false, ago(50))), None, false));
    }
}
