use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Entity, FontWeight, ImageSource, ParentElement, RenderImage, SharedString,
    Styled, div, img, prelude::*,
};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef},
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button_sm},
        home::data::{InviteKind, InvitePreview, NavReply, NavRequest, RelationshipKind},
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InviteLink {
    pub code: String,
    pub event: Option<Id>,
}

#[derive(Clone)]
enum State {
    Resolving,
    Ready(Rc<InvitePreview>),
    Invalid,
    Failed(Instant, u32),
}

thread_local! {
    static CACHE: RefCell<HashMap<InviteLink, State>> = RefCell::new(HashMap::new());
    static BANNERS: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

const HOSTS: [&str; 3] = [
    "discord.gg/",
    "discord.com/invite/",
    "discordapp.com/invite/",
];
const SUBDOMAINS: [&str; 3] = ["www.", "ptb.", "canary."];

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn strip_code(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("```") {
            match r.find("```") {
                Some(end) => {
                    rest = &r[end + 3..];
                    out.push(' ');
                }
                None => {
                    out.push_str(rest);
                    break;
                }
            }
        } else if let Some(r) = rest.strip_prefix('`') {
            match r.find('`') {
                Some(end) => {
                    rest = &r[end + 1..];
                    out.push(' ');
                }
                None => {
                    out.push_str(rest);
                    break;
                }
            }
        } else {
            let ch = rest.chars().next().unwrap_or(' ');
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    out
}

fn host_boundary(text: &str, at: usize) -> bool {
    let before = &text[..at];
    let before = SUBDOMAINS
        .iter()
        .find_map(|s| before.strip_suffix(s))
        .unwrap_or(before);
    before
        .chars()
        .next_back()
        .is_none_or(|ch| !(ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_' | '@')))
}

pub fn links(content: &str) -> Vec<InviteLink> {
    let text = strip_code(content);
    let mut found: Vec<(usize, InviteLink)> = Vec::new();
    for host in HOSTS {
        let mut from = 0;
        while let Some(off) = text[from..].find(host) {
            let at = from + off;
            from = at + host.len();
            if !host_boundary(&text, at) {
                continue;
            }
            let tail = &text[from..];
            let code: String = tail
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
                .collect();
            if code.is_empty() || code.len() > 32 {
                continue;
            }
            let after = &tail[code.len()..];
            let event = after
                .strip_prefix('?')
                .map(|q| {
                    q.split(|ch: char| ch.is_whitespace() || matches!(ch, '>' | ')' | '#'))
                        .next()
                        .unwrap_or_default()
                })
                .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("event=")))
                .and_then(|v| v.parse::<u64>().ok())
                .map(Id);
            found.push((at, InviteLink { code, event }));
        }
    }
    found.sort_by_key(|(at, _)| *at);
    let mut out: Vec<InviteLink> = Vec::new();
    for (_, l) in found {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

fn ensure(app: &PhoenixApp, entity: &Entity<PhoenixApp>, link: &InviteLink, cx: &App) -> State {
    let current = CACHE.with(|c| c.borrow().get(link).cloned());
    let attempt = match &current {
        Some(State::Failed(t, n)) if t.elapsed() >= retry_delay(*n) => n + 1,
        Some(s) => return s.clone(),
        None => 0,
    };
    CACHE.with(|c| c.borrow_mut().insert(link.clone(), State::Resolving));
    let req = match link.event {
        Some(e) => NavRequest::FetchInviteEvent(link.code.clone(), e),
        None => NavRequest::FetchInvite(link.code.clone()),
    };
    let fut = app.backend.nav(req);
    let key = link.clone();
    let entity = entity.clone();
    cx.spawn(async move |cx| {
        let st = match fut.await {
            Ok(NavReply::Invite(p)) => State::Ready(Rc::new(*p)),
            Ok(_) => State::Invalid,
            Err(e) if e.0.starts_with("HTTP 4") && !e.0.starts_with("HTTP 429") => State::Invalid,
            Err(_) => State::Failed(Instant::now(), attempt),
        };
        let retry = matches!(st, State::Failed(..)).then(|| retry_delay(attempt));
        cx.update(|cx| {
            if let State::Ready(p) = &st
                && let Some(ImageRef::Url(u)) = &p.guild_icon
            {
                let e = entity.clone();
                crate::ui::avatar_color::ensure_with(u, cx, move |cx| {
                    e.update(cx, |a, cx| {
                        a.message_list.remeasure();
                        cx.notify();
                    })
                });
            }
            CACHE.with(|c| c.borrow_mut().insert(key, st));
            entity.update(cx, |a, cx| {
                a.message_list.remeasure();
                cx.notify();
            });
        });
        if let Some(d) = retry {
            cx.background_executor().timer(d).await;
            cx.update(|cx| {
                entity.update(cx, |a, cx| {
                    a.message_list.remeasure();
                    cx.notify();
                })
            });
        }
    })
    .detach();
    State::Resolving
}

fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs((2u64 << attempt.min(5)).min(60))
}

pub fn render(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    author: Id,
    content: &str,
    cx: &App,
) -> Vec<AnyElement> {
    if !content.contains("discord") {
        return Vec::new();
    }
    links(content)
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let own = author == app.me.id;
            let el = match ensure(app, entity, &l, cx) {
                State::Resolving | State::Failed(..) => resolving(),
                State::Invalid => invalid(own),
                State::Ready(p) => card(app, entity, &p, own),
            };
            if i == 0 { el } else { div().mt(z(2.)).child(el).into_any_element() }
        })
        .collect()
}

fn text(size: f32, lh: f32, weight: FontWeight, color: usize) -> gpui::Div {
    div()
        .text_size(z(size))
        .line_height(z(lh))
        .font_weight(weight)
        .text_color(tk(color))
}

fn wrapper() -> gpui::Div {
    div()
        .mt(z(2.))
        .w_full()
        .max_w(z(432.))
        .min_w(z(160.))
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .flex()
        .flex_col()
}

fn title(t: SharedString) -> gpui::Div {
    text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED)
        .mb(z(12.))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(t)
}

fn body() -> gpui::Div {
    div().flex().flex_wrap().items_center().gap(z(16.))
}

fn header_line(icon: Option<AnyElement>, info: gpui::Div) -> gpui::Div {
    div()
        .flex_1()
        .min_w(z(1.))
        .flex()
        .items_center()
        .gap(z(16.))
        .children(icon)
        .child(info)
}

fn info(
    name: AnyElement,
    detail: Option<AnyElement>,
    title_color: usize,
    title_weight: FontWeight,
    gap: f32,
) -> gpui::Div {
    div()
        .flex_1()
        .min_w(z(1.))
        .flex()
        .flex_col()
        .justify_center()
        .child(
            text(16., 20., title_weight, title_color)
                .mb(z(gap))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(name),
        )
        .children(detail)
}

fn resolving() -> AnyElement {
    wrapper()
        .child(title(crate::t!("invite.card.resolving")))
        .child(
            body().child(header_line(
                Some(
                    div()
                        .size(z(48.))
                        .flex_none()
                        .rounded(z(12.))
                        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
                        .into_any_element(),
                ),
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(
                        div()
                            .h(z(16.))
                            .w(z(124.))
                            .rounded(z(3.))
                            .bg(tk(idx::BACKGROUND_MOD_SUBTLE)),
                    )
                    .child(
                        div()
                            .h(z(12.))
                            .w(z(194.))
                            .rounded(z(3.))
                            .bg(tk(idx::BACKGROUND_MOD_SUBTLE)),
                    ),
            )),
        )
        .into_any_element()
}

fn invalid(own: bool) -> AnyElement {
    let (head, detail) = if own {
        (
            crate::t!("invite.card.sent_but"),
            crate::t!("invite.card.try_new"),
        )
    } else {
        (
            crate::t!("invite.card.invited_but"),
            crate::t!("invite.card.ask_new"),
        )
    };
    let icon = div()
        .size(z(48.))
        .flex_none()
        .rounded(z(12.))
        .border_1()
        .border_color(tk(idx::OPACITY_RED_12))
        .bg(tk(idx::OPACITY_RED_12))
        .flex()
        .items_center()
        .justify_center()
        .child(
            gpui::svg()
                .path(crate::icons::path("server/envelope"))
                .size(z(26.))
                .text_color(tk(idx::ICON_FEEDBACK_CRITICAL)),
        );
    wrapper()
        .child(title(head))
        .child(
            body().child(header_line(
                Some(icon.into_any_element()),
                info(
                    crate::t!("invite.card.invalid").into_any_element(),
                    Some(
                        text(14., 16., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT)
                            .child(detail)
                            .into_any_element(),
                    ),
                    idx::TEXT_FEEDBACK_CRITICAL,
                    FontWeight::MEDIUM,
                    4.,
                ),
            )),
        )
        .into_any_element()
}

fn grouped(n: u64) -> String {
    let sep = if crate::i18n::locale().starts_with("fr") {
        "\u{202f}"
    } else {
        ","
    };
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(ch);
    }
    out
}

pub fn members_label(n: u64) -> SharedString {
    if n == 1 {
        crate::t!("invite.card.member_one", n = grouped(n))
    } else {
        crate::t!("invite.card.members", n = grouped(n))
    }
}

pub fn established(guild: Id) -> SharedString {
    use chrono::Datelike;
    let ms = (guild.0 >> 22) as i64 + 1_420_070_400_000;
    let Some(d) = chrono::DateTime::from_timestamp_millis(ms) else {
        return SharedString::default();
    };
    let month = crate::i18n::t(&format!("month_short.{}", d.month()), &[]);
    crate::t!(
        "invite.card.established",
        date = format!("{month} {}", d.year())
    )
}

pub fn group_name(name: Option<&str>, recipients: &[SharedString]) -> SharedString {
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        return n.to_owned().into();
    }
    match recipients {
        [] => crate::t!("invite.card.unnamed"),
        [a] => a.clone(),
        [a, b] => crate::t!("invite.card.names_two", first = a, second = b),
        [a, b, d] => crate::t!("invite.card.names_three", first = a, second = b, third = d),
        [a, b, ..] => crate::t!("invite.card.names_many", first = a, second = b),
    }
}

fn round_icon(src: Option<&ImageRef>, size: f32) -> Option<AnyElement> {
    match src {
        Some(ImageRef::Url(u)) => Some(
            img(ImageSource::from(u.clone()))
                .size(z(size))
                .flex_none()
                .rounded_full()
                .into_any_element(),
        ),
        _ => None,
    }
}

fn guild_icon(p: &InvitePreview, size: f32, radius: f32, acronym_px: f32) -> AnyElement {
    match &p.guild_icon {
        Some(ImageRef::Url(u)) => img(ImageSource::from(u.clone()))
            .size(z(size))
            .flex_none()
            .rounded(z(radius))
            .into_any_element(),
        _ => div()
            .size(z(size))
            .flex_none()
            .rounded(z(radius))
            .bg(tk(idx::BACKGROUND_BASE_LOWEST))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(acronym_px))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::TEXT_STRONG))
            .child(crate::model::acronym(&p.guild_name))
            .into_any_element(),
    }
}

fn dot(online: bool) -> gpui::Div {
    div()
        .size(z(8.))
        .flex_none()
        .rounded_full()
        .bg(tk(if online {
            idx::TEXT_STATUS_ONLINE
        } else {
            idx::TEXT_STATUS_OFFLINE
        }))
}

fn is_voice_or_thread(t: Option<u8>) -> bool {
    matches!(t, Some(2 | 10 | 11 | 12 | 13))
}

fn card(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    p: &Rc<InvitePreview>,
    own: bool,
) -> AnyElement {
    match p.kind {
        InviteKind::GroupDm => group_card(app, entity, p, own),
        InviteKind::Friend => friend_card(app, entity, p, own),
        InviteKind::Guild if p.guild.is_none() => invalid(own),
        InviteKind::Guild
            if p.features.iter().any(|f| f == "HUB") || is_voice_or_thread(p.channel_type) =>
        {
            legacy_guild_card(app, entity, p, own)
        }
        InviteKind::Guild => profile_card(app, entity, p),
    }
}

fn accept_click(
    entity: &Entity<PhoenixApp>,
    code: SharedString,
    guild: Option<Id>,
    channel: Option<Id>,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static {
    let entity = entity.clone();
    move |_, _, cx| {
        let code = code.to_string();
        entity.update(cx, |a, cx| a.accept_embed_invite(code, guild, channel, cx));
    }
}

fn goto_click(
    entity: &Entity<PhoenixApp>,
    guild: Id,
    channel: Option<Id>,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static {
    let entity = entity.clone();
    move |_, _, cx| entity.update(cx, |a, cx| a.go_to_invite(guild, channel, cx))
}

impl PhoenixApp {
    pub fn accept_embed_invite(
        &mut self,
        code: String,
        guild: Option<Id>,
        channel: Option<Id>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.nav_request(NavRequest::AcceptInvite(code), cx, move |this, res, cx| {
            if res.is_ok()
                && let Some(g) = guild
            {
                this.go_to_invite(g, channel, cx);
            }
        });
    }

    pub fn go_to_invite(&mut self, guild: Id, channel: Option<Id>, cx: &mut gpui::Context<Self>) {
        if !self.guilds.iter().any(|g| g.id == guild) {
            return;
        }
        self.parity.inbox.open = false;
        self.select_guild(Some(guild), cx);
        if let Some(ch) = channel.filter(|ch| self.channels.contains_key(ch)) {
            self.select_channel(ch, cx);
        }
    }
}

fn banner_color(p: &InvitePreview, profile_brand: Option<u32>) -> u32 {
    if let Some(b) = profile_brand {
        return b;
    }
    let default = crate::theme::resolve(tok(idx::GUILD_PROFILE_BANNER_BACKGROUND_DEFAULT)) >> 8;
    match &p.guild_icon {
        Some(ImageRef::Url(u)) => crate::ui::avatar_color::get(u).unwrap_or(default),
        _ => default,
    }
}

fn mix(a: u32, b: u32, t: f32) -> [u8; 3] {
    let ch = |v: u32, s: u32| ((v >> s) & 0xff) as f32;
    let m = |s: u32| {
        (ch(a, s) + (ch(b, s) - ch(a, s)) * t)
            .round()
            .clamp(0., 255.) as u8
    };
    [m(16), m(8), m(0)]
}

pub fn radial_banner_pixel(base: u32, light: u32, x: f32, y: f32) -> [u8; 3] {
    let dx = (x - 0.501) / 1.0543;
    let dy = (y - 1.2705) / 1.2705;
    let r = (dx * dx + dy * dy).sqrt();
    let t = ((r - 0.2065) / (0.8516 - 0.2065)).clamp(0., 1.);
    mix(light, base, t)
}

fn baked_banner(base: u32) -> Arc<RenderImage> {
    if let Some(b) = BANNERS.with(|m| m.borrow().get(&base).cloned()) {
        return b;
    }
    let light = crate::ui::settings::profile_editor::lab_shift(base, 18. * 1.75);
    let (w, h) = (300u32 * 2, 72u32 * 2);
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, px) in out.enumerate_pixels_mut() {
        let [r, g, b] = radial_banner_pixel(
            base,
            light,
            (x as f32 + 0.5) / w as f32,
            (y as f32 + 0.5) / h as f32,
        );
        *px = image::Rgba([b, g, r, 255]);
    }
    let arc = Arc::new(RenderImage::new(vec![image::Frame::new(out)]));
    BANNERS.with(|m| m.borrow_mut().insert(base, arc.clone()));
    arc
}

fn parse_hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches('#'), 16)
        .ok()
        .filter(|_| s.trim_start_matches('#').len() == 6)
}

fn profile_card(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    p: &Rc<InvitePreview>,
) -> AnyElement {
    let Some(gid) = p.guild else {
        return invalid(false);
    };
    let prof = p.profile.clone().unwrap_or_default();
    let (online, members) = if p.profile.is_some() {
        (prof.online_count, prof.member_count)
    } else {
        (u64::from(p.online), u64::from(p.members))
    };
    let features: &[String] = if p.profile.is_some() {
        &prof.features
    } else {
        &p.features
    };
    let discoverable = features.iter().any(|f| f == "DISCOVERABLE");
    let custom_banner = prof.custom_banner_hash.clone().filter(|_| discoverable);
    let banner: AnyElement = match custom_banner {
        Some(h) => img(ImageSource::from(SharedString::from(format!(
            "https://cdn.discordapp.com/discovery-splashes/{}/{h}.jpg?size=512",
            gid.0
        ))))
        .size_full()
        .object_fit(gpui::ObjectFit::Cover)
        .into_any_element(),
        None => img(ImageSource::Render(baked_banner(banner_color(
            p,
            prof.brand_color_primary.as_deref().and_then(parse_hex),
        ))))
        .size_full()
        .into_any_element(),
    };
    let tag = prof.tag.clone().map(|tag| {
        div()
            .flex_none()
            .h(z(16.))
            .px(z(4.))
            .rounded(z(4.))
            .bg(tk(idx::BACKGROUND_MOD_STRONG))
            .flex()
            .items_center()
            .gap(z(2.))
            .when_some(prof.badge_hash.clone(), |d, h| {
                d.child(
                    img(SharedString::from(format!(
                        "https://cdn.discordapp.com/guild-tag-badges/{}/{h}.png?size=16",
                        gid.0
                    )))
                    .size(z(14.)),
                )
            })
            .child(text(12., 16., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).child(tag))
    });
    let subtle = |s: SharedString| text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).child(s);
    let count = |on: bool, s: SharedString| {
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .child(dot(on))
            .child(subtle(s))
    };
    let member = app.guilds.iter().any(|g| g.id == gid);
    let gated = features
        .iter()
        .any(|f| f == "MEMBER_VERIFICATION_GATE_ENABLED")
        && features
            .iter()
            .any(|f| f == "MEMBER_VERIFICATION_MANUAL_APPROVAL");
    let cta = if member {
        button_sm(
            ("invite-goto", gid.0),
            crate::t!("invite.card.go_to_server"),
            BtnKind::Active,
        )
        .on_click(goto_click(entity, gid, p.channel_id))
    } else {
        let label = if gated {
            crate::t!("invite.card.apply")
        } else {
            crate::t!("invite.card.join")
        };
        button_sm(("invite-join", gid.0), label, BtnKind::Active).on_click(accept_click(
            entity,
            p.code.clone(),
            Some(gid),
            p.channel_id,
        ))
    };
    let has_body = !prof.description.is_empty() || !prof.traits.is_empty();
    let mut traits = div().flex().flex_wrap().items_center().gap(z(4.));
    for tr in &prof.traits {
        traits = traits.child(
            div()
                .max_w(z(266.))
                .py(z(4.))
                .px(z(8.))
                .rounded(z(16.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .flex()
                .items_center()
                .gap(z(4.))
                .when_some(tr.emoji.token(), |d, t| {
                    d.child(crate::media::emoji::emoji_element(&t, 16.))
                })
                .child(
                    text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT)
                        .min_w_0()
                        .text_ellipsis()
                        .child(SharedString::from(tr.label.clone())),
                ),
        );
    }
    let name = p.guild_name.clone();
    div()
        .mt(z(2.))
        .w(z(302.))
        .flex_none()
        .rounded(z(16.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(z(72.))
                .w_full()
                .flex_none()
                .overflow_hidden()
                .child(banner),
        )
        .child(
            div().mt(z(-34.)).px(z(16.)).child(
                div()
                    .size(z(70.))
                    .rounded(z(22.))
                    .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(guild_icon(p, 64., 19., 14.)),
            ),
        )
        .child(
            div()
                .my(z(8.))
                .px(z(16.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(
                            text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG)
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(crate::ui::emoji_text::label(name, 16.)),
                        )
                        .children(tag),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x(z(8.))
                        .child(count(
                            true,
                            crate::t!("invite.card.online", n = grouped(online)),
                        ))
                        .child(count(false, members_label(members))),
                )
                .child(subtle(established(gid))),
        )
        .map(|d| {
            if has_body {
                d.child(
                    div()
                        .mb(z(16.))
                        .px(z(16.))
                        .flex()
                        .flex_col()
                        .gap(z(16.))
                        .when(!prof.description.is_empty(), |d| {
                            d.child(subtle(prof.description.clone().into()))
                        })
                        .when(!prof.traits.is_empty(), |d| d.child(traits)),
                )
            } else {
                d.child(div().h(z(8.)))
            }
        })
        .child(
            div()
                .w_full()
                .px(z(16.))
                .pb(z(16.))
                .flex()
                .child(cta.flex_1().min_w(z(60.))),
        )
        .into_any_element()
}

fn legacy_guild_card(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    p: &Rc<InvitePreview>,
    own: bool,
) -> AnyElement {
    let Some(gid) = p.guild else {
        return invalid(own);
    };
    let voice = is_voice_or_thread(p.channel_type);
    let stage = p.channel_type == Some(13);
    let hub = p.features.iter().any(|f| f == "HUB");
    let head = match (voice, hub, own, stage) {
        (true, _, true, true) => crate::t!("invite.card.sent_stage"),
        (true, _, true, false) => crate::t!("invite.card.sent_voice"),
        (true, _, false, true) => crate::t!("invite.card.invited_stage"),
        (true, _, false, false) => crate::t!("invite.card.invited_voice"),
        (false, true, true, _) => crate::t!("invite.card.sent_hub"),
        (false, true, false, _) => crate::t!("invite.card.invited_hub"),
        (false, false, true, _) => crate::t!("invite.card.sent_server"),
        (false, false, false, _) => crate::t!("invite.card.invited_server"),
    };
    let member = app.guilds.iter().any(|g| g.id == gid);
    let detail_text = |s: SharedString| {
        text(14., 16., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(s)
    };
    let channel_line = |icon: &'static str| {
        div()
            .flex()
            .items_center()
            .child(
                gpui::svg()
                    .path(crate::icons::path(icon))
                    .size(z(16.))
                    .flex_none()
                    .text_color(tk(idx::TEXT_MUTED)),
            )
            .child(
                detail_text(p.channel_name.clone().unwrap_or_default())
                    .ml(z(4.))
                    .flex_1()
                    .min_w_0(),
            )
            .into_any_element()
    };
    let detail: Option<AnyElement> = if voice {
        Some(channel_line(if stage { "stage" } else { "speaker" }))
    } else if p.members >= 5 || p.online > 0 {
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(z(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(dot(true).mr(z(4.)))
                        .child(detail_text(crate::t!(
                            "invite.card.online",
                            n = grouped(u64::from(p.online))
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(dot(false).mr(z(4.)))
                        .child(detail_text(members_label(u64::from(p.members)))),
                )
                .into_any_element(),
        )
    } else {
        p.channel_name.as_ref().map(|_| channel_line("hash"))
    };
    let label = if voice {
        if stage {
            crate::t!("invite.card.join_stage")
        } else {
            crate::t!("invite.card.join_voice")
        }
    } else if member {
        crate::t!("invite.card.joined")
    } else {
        crate::t!("invite.card.join")
    };
    let btn = button_sm(("invite-legacy", gid.0), label, BtnKind::Active).map(|b| {
        if member {
            b.on_click(goto_click(entity, gid, p.channel_id))
        } else {
            b.on_click(accept_click(
                entity,
                p.code.clone(),
                Some(gid),
                p.channel_id,
            ))
        }
    });
    wrapper()
        .when_some(p.splash.clone(), |d, s| {
            d.child(
                div()
                    .h(z(64.))
                    .mt(z(-16.))
                    .mx(z(-16.))
                    .mb(z(16.))
                    .rounded_t(z(8.))
                    .overflow_hidden()
                    .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
                    .child(
                        img(ImageSource::from(s))
                            .size_full()
                            .object_fit(gpui::ObjectFit::Cover),
                    ),
            )
        })
        .child(title(head))
        .child(
            body()
                .child(header_line(
                    Some(guild_icon(p, 48., 12., 16.)),
                    info(
                        crate::ui::emoji_text::label(p.guild_name.clone(), 16.),
                        detail,
                        idx::TEXT_STRONG,
                        FontWeight::SEMIBOLD,
                        2.,
                    ),
                ))
                .child(if voice {
                    div().w_full().flex().child(btn.flex_1())
                } else {
                    div().flex_none().child(btn)
                }),
        )
        .into_any_element()
}

fn group_card(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    p: &Rc<InvitePreview>,
    own: bool,
) -> AnyElement {
    let known = p
        .channel_id
        .is_some_and(|ch| app.channels.contains_key(&ch));
    let head = if own {
        crate::t!("invite.card.sent_to")
    } else {
        crate::t!("invite.card.invited_to")
    };
    let name = group_name(p.channel_name.as_deref(), &p.recipients);
    let btn = if known {
        button_sm(
            ("invite-gdm", p.channel_id.map_or(0, |c| c.0)),
            crate::t!("invite.card.joined"),
            BtnKind::Secondary,
        )
        .opacity(0.5)
        .cursor_default()
    } else {
        button_sm(
            ("invite-gdm", p.channel_id.map_or(0, |c| c.0)),
            crate::t!("invite.card.join_group"),
            BtnKind::Active,
        )
        .on_click(accept_click(entity, p.code.clone(), None, p.channel_id))
    };
    let detail = text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED)
        .child(members_label(p.recipients.len() as u64))
        .into_any_element();
    wrapper()
        .child(title(head))
        .child(
            body()
                .child(
                    header_line(
                        round_icon(p.channel_icon.as_ref(), 48.),
                        info(
                            name.into_any_element(),
                            Some(detail),
                            idx::TEXT_STRONG,
                            FontWeight::MEDIUM,
                            4.,
                        ),
                    )
                    .w_full(),
                )
                .child(div().w_full().flex().child(btn.flex_1())),
        )
        .into_any_element()
}

fn friend_card(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    p: &Rc<InvitePreview>,
    own: bool,
) -> AnyElement {
    let Some(user) = p.inviter_id else {
        return div().into_any_element();
    };
    let friend = app
        .nav
        .relationship(user)
        .is_some_and(|r| r.kind == RelationshipKind::Friend);
    let head = if own {
        crate::t!("invite.card.sent_friend")
    } else {
        crate::t!("invite.card.got_friend")
    };
    let username = p.inviter_username.clone().unwrap_or_default();
    let btn = if friend {
        let e = entity.clone();
        button_sm(
            ("invite-friend", user.0),
            crate::t!("invite.card.send_message"),
            BtnKind::Secondary,
        )
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_dm_with(user, cx)))
    } else if own {
        button_sm(
            ("invite-friend", user.0),
            crate::t!("invite.card.add_friend"),
            BtnKind::Secondary,
        )
        .opacity(0.5)
        .cursor_default()
    } else {
        button_sm(
            ("invite-friend", user.0),
            crate::t!("invite.card.add_friend"),
            BtnKind::Active,
        )
        .on_click(accept_click(entity, p.code.clone(), None, None))
    };
    let avatar = Some(crate::ui::avatar_ref(
        p.inviter_avatar
            .clone()
            .unwrap_or(ImageRef::Default(((user.0 >> 22) % 6) as u8)),
        48. * crate::theme::zoom(),
    ));
    wrapper()
        .child(title(head))
        .child(
            body()
                .child(header_line(
                    avatar,
                    info(
                        username.clone().into_any_element(),
                        Some(
                            text(14., 16., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT)
                                .child(username)
                                .into_any_element(),
                        ),
                        idx::TEXT_STRONG,
                        FontWeight::SEMIBOLD,
                        2.,
                    ),
                ))
                .child(div().flex_none().child(btn)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(s: &str) -> Vec<(String, Option<u64>)> {
        links(s)
            .into_iter()
            .map(|l| (l.code, l.event.map(|e| e.0)))
            .collect()
    }

    #[test]
    fn finds_invite_links() {
        assert_eq!(
            codes("https://discord.gg/cS8CHeVv"),
            vec![("cS8CHeVv".into(), None)]
        );
        assert_eq!(
            codes("viens <https://discord.com/invite/abc-1?event=42> ou discord.gg/xyz"),
            vec![("abc-1".into(), Some(42)), ("xyz".into(), None)]
        );
        assert_eq!(
            codes("https://ptb.discord.com/invite/p1 et canary.discord.com/invite/p1"),
            vec![("p1".into(), None)]
        );
        assert_eq!(
            codes("https://discordapp.com/invite/old"),
            vec![("old".into(), None)]
        );
        assert!(codes("notdiscord.gg/abc").is_empty());
        assert!(codes("`discord.gg/code` and ```\ndiscord.gg/block\n```").is_empty());
        assert!(codes("https://discord.gg/").is_empty());
        assert_eq!(codes("discord.gg/a discord.gg/a"), vec![("a".into(), None)]);
        assert_eq!(
            codes("https://discord.gg/ev?foo=1&event=7"),
            vec![("ev".into(), Some(7))]
        );
    }

    #[test]
    fn group_names_follow_the_client() {
        let n = |v: &[&str]| {
            group_name(
                None,
                &v.iter()
                    .map(|s| SharedString::from(s.to_string()))
                    .collect::<Vec<_>>(),
            )
            .to_string()
        };
        assert_eq!(n(&[]), "Unnamed");
        assert_eq!(n(&["a"]), "a");
        assert_eq!(n(&["a", "b"]), "a, b");
        assert_eq!(n(&["a", "b", "c"]), "a, b, and c");
        assert_eq!(n(&["a", "b", "c", "d"]), "a, b and others");
        assert_eq!(group_name(Some("Team"), &[]).to_string(), "Team");
    }

    #[test]
    fn member_counts_and_dates() {
        assert_eq!(members_label(1).to_string(), "1 Member");
        assert_eq!(members_label(12).to_string(), "12 Members");
        assert_eq!(members_label(1234).to_string(), "1,234 Members");
        assert_eq!(
            established(Id(1427645329775263756)).to_string(),
            "Est. Oct 2025"
        );
    }

    #[test]
    fn default_banner_gradient_matches_the_client() {
        let base = 0x000000;
        let light = crate::ui::settings::profile_editor::lab_shift(base, 31.5);
        let ch = |v: u32, s: u32| (v >> s) & 0xff;
        assert!((ch(light, 16) as i32 - 74).abs() <= 2, "{light:06x}");
        let bottom = radial_banner_pixel(base, light, 0.5, 1.0);
        assert!(
            (i32::from(bottom[0]) - ch(light, 16) as i32).abs() <= 2,
            "{bottom:?}"
        );
        assert_eq!(radial_banner_pixel(base, light, 0.5, 0.0), [0, 0, 0]);
    }
}
