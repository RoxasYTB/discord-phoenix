use std::rc::Rc;

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, deferred, div, prelude::*};
use phoenix_api::models::Snowflake;
use phoenix_api::rest::guild_manage::AdminCall;

use super::{Modal, close_action};
use crate::{
    actions::{close_modal, open_modal, with_modal},
    app::PhoenixApp,
    model::{ChannelKind, Id, ImageRef},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub const HEADER_ART_DARK: &str = "https://discord.com/assets/ef95e909088f721c.svg";
pub const HEADER_ART_LIGHT: &str = "https://discord.com/assets/9b2c01218d4ab2ff.svg";
const MANAGE_WEBHOOKS: u64 = 1 << 29;
const TOO_MANY_WEBHOOKS: &str = "30007";

pub struct State {
    pub channel: Id,
    pub guild: Option<Id>,
    pub target: Option<Id>,
    pub open: Option<u8>,
    pub error: Option<SharedString>,
    pub submitting: bool,
}

pub struct Destination {
    pub guild: Id,
    pub name: SharedString,
    pub icon: ImageRef,
    pub channels: Vec<(Id, SharedString)>,
}

pub fn open(channel: Id, window: &mut Window) {
    open_modal(Modal::Follow(State { channel, guild: None, target: None, open: None, error: None, submitting: false }), Some(window));
}

pub fn destinations(app: &PhoenixApp) -> Vec<Destination> {
    app.guilds
        .iter()
        .filter(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g.id), MANAGE_WEBHOOKS))
        .filter_map(|g| {
            let channels: Vec<(Id, SharedString)> = app
                .guild_channels
                .get(&g.id)
                .into_iter()
                .flatten()
                .filter_map(|id| app.channels.get(id))
                .filter(|ch| ch.kind == ChannelKind::Text)
                .map(|ch| (ch.id, ch.name.clone()))
                .collect();
            (!channels.is_empty()).then(|| Destination { guild: g.id, name: g.name.clone(), icon: g.icon.clone(), channels })
        })
        .collect()
}

pub fn default_channel(channels: &[(Id, SharedString)]) -> Option<Id> {
    channels.iter().find(|(_, n)| n.as_ref() == "general").or(channels.first()).map(|(id, _)| *id)
}

pub fn follow_call(source: Id, target: Id) -> AdminCall {
    AdminCall::FollowChannel { channel: Snowflake(source.0), webhook_channel: Snowflake(target.0) }
}

fn edit(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::Follow(s)) = m {
            f(s);
        }
    });
}

type Lead = Rc<dyn Fn(usize) -> Option<AnyElement>>;
type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[allow(clippy::too_many_arguments)]
fn select(id: &'static str, label: SharedString, options: Vec<SharedString>, selected: Option<usize>, open: bool, disabled: bool, lead: Lead, key: u8, pick: Pick, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let value = selected.and_then(|k| options.get(k).cloned());
    let shown = value.clone().unwrap_or_else(|| crate::t!("select.placeholder"));
    let mut row = div().flex_1().min_w_0().pl(z(12.)).flex().items_center().gap(z(8.));
    if let Some(l) = selected.and_then(&*lead) {
        row = row.child(l);
    }
    row = row.child(
        div()
            .min_w_0()
            .truncate()
            .text_size(z(16.))
            .line_height(z(20.))
            .text_color(c(tok(if value.is_some() { idx::TEXT_STRONG } else { idx::TEXT_MUTED })))
            .child(shown),
    );
    let field = div()
        .id(SharedString::from(format!("{id}-box")))
        .h(z(40.))
        .w_full()
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .when(disabled, |d| d.opacity(0.5))
        .when(!disabled, |d| d.cursor_pointer().hover(move |s| if open { s } else { s.border_color(c(tok(idx::INPUT_BORDER_HOVER))) }))
        .child(row)
        .child(
            div().flex_none().w(z(36.)).h_full().pr(z(4.)).flex().items_center().justify_center().child(
                gpui::svg()
                    .path(crate::icons::path("settings/chevron-down"))
                    .size(z(20.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
            ),
        )
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |_, _, _, cx| {
                edit(|s| s.open = if s.open == Some(key) { None } else { Some(key) });
                cx.notify();
            }))
        });
    let mut wrap = div().relative().w_full().child(field);
    if open && !disabled {
        let mut list = div()
            .id(SharedString::from(format!("{id}-list")))
            .occlude()
            .max_h(z(320.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(2.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
            .flex()
            .flex_col();
        for (k, l) in options.into_iter().enumerate() {
            let pick = pick.clone();
            let on = Some(k) == selected;
            list = list.child(
                div()
                    .id((id, k))
                    .flex_none()
                    .h(z(40.))
                    .px(z(8.))
                    .gap(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .cursor_pointer()
                    .children(lead(k))
                    .child(div().flex_1().min_w_0().truncate().child(l))
                    .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(20.)).text_color(c(tok(idx::TEXT_BRAND)))))
                    .on_click(move |_, w, cx| pick(k, w, cx)),
            );
        }
        wrap = wrap.child(deferred(div().absolute().top(z(44.)).left_0().w_full().child(list)).with_priority(30));
    }
    div().flex().flex_col().child(super::label(label)).child(wrap).into_any_element()
}

fn text(t: SharedString, color: usize) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(color))).child(t)
}

fn header(app: &PhoenixApp, s: &State) -> AnyElement {
    let ch = app.channels.get(&s.channel);
    let name = ch.map(|c| c.name.clone()).unwrap_or_default();
    let guild = ch.and_then(|c| c.guild).and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let icon = guild.map(|g| crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 50.));
    let art = if crate::theme::is_light() { HEADER_ART_LIGHT } else { HEADER_ART_DARK };
    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .p(z(16.))
        .rounded_t(z(4.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .child(gpui::img(SharedString::from(art)).absolute().top_0().left_0().size_full().object_fit(gpui::ObjectFit::Cover))
        .children(icon)
        .child(
            div().relative().ml(z(16.)).max_w(z(320.)).min_w_0().overflow_hidden().rounded(z(4.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).child(
                div()
                    .flex()
                    .items_center()
                    .p(z(8.))
                    .bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                    .child(gpui::svg().path(crate::icons::path("megaphone")).flex_none().size(z(20.)).ml(z(2.)).mr(z(8.)).text_color(c(tok(idx::TEXT_MUTED))))
                    .child(div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(name)),
            ),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let dests = destinations(app);
    let nsfw = app.nav.extras.get(&s.channel).is_some_and(|e| e.nsfw);
    let none = dests.is_empty();
    let gsel = s.guild.and_then(|g| dests.iter().position(|d| d.guild == g));
    let chans: Vec<(Id, SharedString)> = gsel.map(|k| dests[k].channels.clone()).unwrap_or_default();
    let csel = s.target.and_then(|t| chans.iter().position(|(id, _)| *id == t));
    let guild_ids: Vec<(Id, Vec<(Id, SharedString)>)> = dests.iter().map(|d| (d.guild, d.channels.clone())).collect();
    let icons: Vec<(ImageRef, SharedString)> = dests.iter().map(|d| (d.icon.clone(), d.name.clone())).collect();
    let glead: Lead = Rc::new(move |k| icons.get(k).map(|(i, n)| crate::ui::invite::guild_icon(Some(i), n, 20.)));
    let gpick: Pick = Rc::new(move |k, w, _| {
        if let Some((g, chs)) = guild_ids.get(k) {
            let (g, first) = (*g, default_channel(chs));
            edit(|s| {
                if s.guild != Some(g) {
                    s.guild = Some(g);
                    s.target = first;
                }
                s.open = None;
            });
        }
        w.refresh();
    });
    let chan_ids: Vec<Id> = chans.iter().map(|(id, _)| *id).collect();
    let cpick: Pick = Rc::new(move |k, w, _| {
        let t = chan_ids.get(k).copied();
        edit(|s| {
            s.target = t;
            s.open = None;
        });
        w.refresh();
    });
    let clead: Lead = Rc::new(|_| Some(gpui::svg().path(crate::icons::path("hash")).flex_none().size(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).into_any_element()));
    let names: Vec<SharedString> = dests.iter().map(|d| d.name.clone()).collect();
    let cnames: Vec<SharedString> = chans.iter().map(|(_, n)| n.clone()).collect();
    let hint = match &s.error {
        Some(e) => text(e.clone(), idx::TEXT_FEEDBACK_CRITICAL),
        None if none => text(crate::t!("follow.no_permission"), idx::TEXT_FEEDBACK_CRITICAL),
        None => text(crate::t!("follow.permission_hint"), idx::TEXT_MUTED),
    };
    let form = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(select("follow-guild", crate::t!("follow.send_to"), names, gsel, s.open == Some(0), none, glead, 0, gpick, cx))
        .child(select("follow-channel", crate::t!("follow.select_channel"), cnames.clone(), csel, s.open == Some(1), cnames.is_empty(), clead, 1, cpick, cx))
        .child(hint.mb(z(16.)));
    let body = div()
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("follow.title")))
        .child(text(crate::t!("follow.description"), idx::TEXT_MUTED).mt(z(8.)).when(!nsfw, |d| d.mb(z(24.))))
        .when(nsfw, |d| d.child(text(crate::t!("follow.nsfw"), idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)).mb(z(24.))));
    let ready = s.guild.is_some() && s.target.is_some() && !s.submitting;
    let close = close_action(cx);
    let close2 = close.clone();
    let footer = div()
        .flex()
        .gap(z(8.))
        .justify_end()
        .child(button("follow-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| close(w, cx)))
        .child(button("follow-go", crate::t!("forum.follow"), BtnKind::Primary).when(!ready, |b| b.opacity(0.5)).on_click(cx.listener(move |a, _, _, cx| {
            if ready {
                submit(a, cx);
            }
        })));
    let el = div()
        .w(z(440.))
        .max_h(z(720.))
        .rounded(z(12.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.24), offset: gpui::point(z(0.), z(12.)), blur_radius: z(24.), spread_radius: z(0.), inset: false }])
        .flex()
        .flex_col()
        .child(header(app, s))
        .child(div().pt(z(32.)).px(z(24.)).flex().flex_col().font_weight(FontWeight::MEDIUM).child(body).child(form))
        .child(div().px(z(24.)).pb(z(24.)).child(footer));
    crate::ui::modal::frame("follow-backdrop", close2, el)
}

pub fn submit(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut call = None;
    edit(|s| {
        if let Some(t) = s.target {
            s.submitting = true;
            s.error = None;
            call = Some(follow_call(s.channel, t));
        }
    });
    let Some(call) = call else { return };
    a.admin(call, cx, |_, r, cx| {
        match r {
            Ok(_) => close_modal(),
            Err(e) => edit(|s| {
                s.submitting = false;
                s.error = Some(if e.contains(TOO_MANY_WEBHOOKS) { crate::t!("follow.too_many_webhooks") } else { crate::t!("follow.failed") });
            }),
        }
        cx.notify();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_posts_the_destination_channel() {
        let r = phoenix_api::rest::guild_manage::build(&follow_call(Id(5), Id(9)));
        assert_eq!((r.method, r.path.as_str(), r.body), (reqwest::Method::POST, "/channels/5/followers", Some(serde_json::json!({ "webhook_channel_id": "9" }))));
    }

    #[test]
    fn default_destination_prefers_general() {
        let chans = vec![(Id(1), SharedString::from("news")), (Id(2), SharedString::from("general"))];
        assert_eq!(default_channel(&chans), Some(Id(2)));
        assert_eq!(default_channel(&chans[..1]), Some(Id(1)));
        assert_eq!(default_channel(&[]), None);
    }
}
