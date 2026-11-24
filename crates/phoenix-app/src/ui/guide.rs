//! "Server Guide" page (sidebar row of guilds with COMMUNITY + GUILD_SERVER_GUIDE +
//! GUILD_ONBOARDING; client route `/channels/{g}/@home`). Read-only: loaded with
//! `GET /guilds/{g}/new-member-welcome` and `GET /guilds/{g}/profile`.
//!
//! Values from the live client (dump vanilla-server-guide, 2026-11-24, 1536x788):
//! header bar like the other pages (container_ybUK7Z, 32/20 SignPostIcon,
//! body-md/medium title); scroller homeContainer_ padding 32, gap 32;
//! header_lqGeMc margin-top -16 with the home header art 16px wider on each side
//! (headerArtWrapper_), 25% of its width tall, radius 16; the 100px guild icon
//! (4px --background-base-low border, outer radius 30) at margin -50 0 16; the
//! name row (heading-xxl/semibold 32/40, gap 8, discovery badge 24) with the md
//! secondary "Invite" button pushed right; homeContent_ margin-top 12, gap 24:
//! the main column and the 300px sidebar card.

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    commands::{
        ParityReply, ParityRequest,
        guide::{GuideEntry, GuildGuide, GuildProfile},
    },
    model::{Guild, Id},
    theme::{c, generated::idx, tok, z},
};

#[derive(Default)]
struct Slot(Option<Page>);
impl gpui::Global for Slot {}

#[derive(Clone)]
struct Page {
    guild: Id,
    /// Channel selected when the page opened: selecting another one closes it.
    from: Option<Id>,
    data: Option<GuildGuide>,
    error: Option<SharedString>,
    /// Resource channel → text and first image of its oldest message, shown when
    /// the resource has no description / no custom icon.
    previews: std::collections::HashMap<Id, Preview>,
}

#[derive(Clone, Default)]
struct Preview {
    text: Option<String>,
    /// (url, width, height) of the first image.
    media: Option<(SharedString, u32, u32)>,
}

/// Like the client: the message content, else the first embed description; the
/// first image attachment, else the first embed image or thumbnail.
fn preview_of(msgs: &[crate::model::Message]) -> Preview {
    let Some(m) = msgs.first() else { return Preview::default() };
    let text = Some(m.content.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| m.embeds.iter().find_map(|e| e.description.as_ref().filter(|d| !d.is_empty()).map(|d| d.to_string())));
    let media = m
        .attachments
        .iter()
        .find(|a| a.is_image)
        .map(|a| (a.url.clone(), a.width, a.height))
        .or_else(|| m.embeds.iter().find_map(|e| e.image.as_ref().or(e.thumbnail.as_ref()).map(|i| (i.url.clone(), i.width, i.height))));
    Preview { text, media }
}

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

/// The client shows the "Server Guide" row for these guilds only.
pub fn has_guide(features: &[String]) -> bool {
    ["COMMUNITY", "GUILD_SERVER_GUIDE", "GUILD_ONBOARDING"].iter().all(|f| features.iter().any(|x| x == f))
}

/// The page is shown for the selected guild.
pub fn is_open(app: &PhoenixApp, cx: &App) -> bool {
    cx.try_global::<Slot>().and_then(|s| s.0.as_ref()).is_some_and(|p| Some(p.guild) == app.selected_guild && p.from == app.selected_channel)
}

pub fn close(cx: &mut App) {
    cx.set_global(Slot(None));
}

pub fn open(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    crate::ui::community::close(cx);
    cx.set_global(Slot(Some(Page { guild, from: app.selected_channel, data: None, error: None, previews: Default::default() })));
    app.parity_request(ParityRequest::ServerGuide { guild }, cx, move |a, r, cx| {
        let Some(mut page) = cx.try_global::<Slot>().and_then(|s| s.0.clone()).filter(|p| p.guild == guild) else { return };
        match r {
            Ok(ParityReply::Guide(g)) => {
                // The client loads the oldest messages of a resource channel without a
                // description or icon (`GET /channels/{c}/messages?after={c}&limit=5`).
                for r in g.resources.iter().filter(|r| r.description.is_empty() || r.icon.is_none()) {
                    load_preview(a, guild, r.channel, cx);
                }
                page.data = Some(*g);
            }
            Ok(_) => page.data = Some(GuildGuide::default()),
            Err(e) => {
                page.data = Some(GuildGuide::default());
                page.error = Some(SharedString::from(e.0));
            }
        }
        cx.set_global(Slot(Some(page)));
        cx.notify();
    });
    cx.notify();
}

fn load_preview(app: &PhoenixApp, guild: Id, channel: Id, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.load_messages_at(channel, crate::backend::HistoryAt::After(channel), 5);
    cx.spawn(async move |this, cx| {
        let Ok(msgs) = fut.await else { return };
        let _ = this.update(cx, |_, cx| {
            let Some(mut page) = cx.try_global::<Slot>().and_then(|s| s.0.clone()).filter(|p| p.guild == guild) else { return };
            page.previews.insert(channel, preview_of(&msgs));
            cx.set_global(Slot(Some(page)));
            cx.notify();
        });
    })
    .detach();
}

/// "1 393" (narrow no-break space) in French, "1,393" in English.
fn grouped(n: u64) -> String {
    let sep = if crate::i18n::locale().starts_with("fr") { "\u{202f}" } else { "," };
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

/// "nov. 2024": creation month of the guild, from its snowflake.
fn established(guild: Id) -> SharedString {
    use chrono::Datelike;
    let ms = (guild.0 >> 22) as i64 + 1_420_070_400_000;
    let Some(d) = chrono::DateTime::from_timestamp_millis(ms) else { return SharedString::default() };
    let month = crate::i18n::t(&format!("month_short.{}", d.month()), &[]);
    crate::t!("guide.established", date = format!("{month} {}", d.year()))
}

fn cdn(path: String) -> SharedString {
    SharedString::from(format!("https://cdn.discordapp.com/{path}"))
}

/// Discovery badge (ExperimentalWebBadgeIcon, two tones) at `size`.
fn discovery_badge(size: f32) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .size(z(size))
        .child(gpui::svg().path(crate::icons::path("discovery-badge")).absolute().size(z(size)).text_color(t(idx::BADGE_INVERTED_BACKGROUND)))
        .child(gpui::svg().path(crate::icons::path("discovery-badge-fg")).absolute().size(z(size)).text_color(t(idx::BADGE_INVERTED_TEXT)))
}

/// Guild icon image (or initials) inside a rounded square.
fn icon(g: &Guild, size: f32, radius: f32) -> AnyElement {
    match &g.icon {
        crate::model::ImageRef::Url(u) => gpui::img(gpui::ImageSource::from(u.clone())).size(z(size)).rounded(z(radius)).into_any_element(),
        _ => div()
            .size(z(size))
            .rounded(z(radius))
            .bg(t(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(size * 0.35))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t(idx::TEXT_STRONG))
            .child(g.acronym.clone())
            .into_any_element(),
    }
}

/// Header: art, icon, name, badge and "Invite".
fn header(app: &PhoenixApp, g: &Guild, cx: &Context<PhoenixApp>) -> gpui::Div {
    let gid = g.id;
    let art = app.backend.guild_home_header(gid);
    // headerArt_: background-size cover, padding-top 25% (gpui: aspect ratio 4:1).
    let art = div()
        .ml(z(-16.))
        .mr(z(-16.))
        .relative()
        .rounded(z(16.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        // Stretched by the column, 16px wider on each side through the negative margins.
        .child(div().w_full().pt(gpui::relative(0.25)))
        .when_some(art, |d, h| {
            d.child(
                gpui::img(cdn(format!("home-headers/{}/{h}.png?size=1536", gid.0)))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            )
        });
    let invite = crate::ui::controls::button("guide-invite", crate::t!("guide.invite"), crate::ui::controls::BtnKind::Secondary)
        .on_click(cx.listener(move |a, _, _, cx| a.open_invite_people(gid, cx)));
    div()
        .mt(z(-16.))
        .flex()
        .flex_col()
        .child(art)
        .child(
            // headerIcon_: 100 + 4px border in --background-base-low, radius 30.
            div()
                .mt(z(-50.))
                .mb(z(16.))
                .size(z(108.))
                .p(z(4.))
                .rounded(z(30.))
                .bg(t(idx::BACKGROUND_BASE_LOW))
                .child(icon(g, 100., 26.)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .min_w_0()
                        .text_ellipsis()
                        .text_size(z(32.))
                        .line_height(z(40.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(crate::ui::emoji_text::label(g.name.clone(), 32.)),
                )
                .when(g.discoverable, |d| d.child(discovery_badge(24.)))
                .child(div().flex_1())
                .child(invite),
        )
}

/// One "Resources" row (row_Qo6EL-): padding 16, radius 8, 1px --border-subtle,
/// --background-surface-high (hover --background-surface-higher); title
/// body-md/semibold 16/20 + 8, description body-sm 14/18 --text-default on at most
/// 3 lines (the oldest message of the channel when the description is empty); the
/// custom icon (72x72) or the message's first image (72 on its long side) on the right.
fn resource(i: usize, r: &GuideEntry, preview: Option<&Preview>, cx: &Context<PhoenixApp>) -> AnyElement {
    let ch = r.channel;
    let text = Some(r.description.clone()).filter(|d| !d.is_empty()).or_else(|| preview.and_then(|p| p.text.clone()));
    let thumb: Option<AnyElement> = match (&r.icon, preview.and_then(|p| p.media.clone())) {
        (Some(h), _) => Some(
            // CDN `resource-channels/{channel}/{hash}` (128px), not seen on a real guild yet.
            gpui::img(cdn(format!("resource-channels/{}/{h}.jpg?size=128", ch.0)))
                .size(z(72.))
                .rounded(z(8.))
                .flex_none()
                .into_any_element(),
        ),
        (None, Some((url, w, h))) => {
            let (w, h) = (w.max(1) as f32, h.max(1) as f32);
            let (tw, th) = if h > w { (72. * w / h, 72.) } else { (72., 72. * h / w) };
            Some(gpui::img(url).w(z(tw)).h(z(th)).rounded(z(8.)).flex_none().object_fit(gpui::ObjectFit::Cover).into_any_element())
        }
        _ => None,
    };
    div()
        .id(("guide-res", i))
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .hover(|s| s.bg(t(idx::BACKGROUND_SURFACE_HIGHER)))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| {
            if a.channels.contains_key(&ch) {
                a.select_channel(ch, cx);
            }
        }))
        .flex()
        .items_start()
        .gap(z(12.))
        .when_some(r.emoji.token(), |d, tk| d.child(div().flex_none().child(crate::media::emoji::emoji_element(&tk, 20.))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .mb(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(crate::ui::emoji_text::label(r.title.clone().into(), 16.)),
                )
                .when_some(text, |d, txt| d.child(crate::ui::profile_bio::bio(&txt, true, t(idx::TEXT_DEFAULT)))),
        )
        .children(thumb)
        .into_any_element()
}

/// Sidebar card (container_Xweo-z): 300 wide, radius 16, 1px --border-subtle,
/// --background-surface-highest; 120px discovery splash; 70px icon frame at
/// margin -34 inside padding 16; name heading-sm/semibold + badge + tag chiplet;
/// online / member counts (8px dots, body-sm --text-muted, gap 0 8); creation
/// month; description body-sm and trait chips (padding 4 8, radius 16, 1px
/// --border-subtle, 16px emoji, gap 4).
fn card(g: &Guild, p: &GuildProfile) -> gpui::Div {
    let gid = g.id;
    let muted = |s: SharedString| div().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_MUTED)).child(s);
    let count = |online: bool, s: SharedString| {
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .child(div().size(z(8.)).rounded_full().bg(t(if online { idx::TEXT_STATUS_ONLINE } else { idx::TEXT_STATUS_OFFLINE })))
            .child(muted(s))
    };
    let mut traits = div().flex().flex_wrap().gap(z(4.));
    for tr in &p.traits {
        traits = traits.child(
            div()
                .max_w(z(268.))
                .py(z(4.))
                .px(z(8.))
                .rounded(z(16.))
                .border_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .hover(|s| s.border_color(t(idx::BORDER_STRONG)))
                .flex()
                .items_center()
                .gap(z(4.))
                .when_some(tr.emoji.token(), |d, tk| d.child(crate::media::emoji::emoji_element(&tk, 16.)))
                .child(div().min_w_0().text_ellipsis().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_DEFAULT)).child(tr.label.clone())),
        );
    }
    let tag = p.tag.clone().map(|tag| {
        // chipletContainerInner_ (as after usernames): 14px badge + body-xs/semibold.
        div()
            .flex_none()
            .h(z(16.))
            .px(z(4.))
            .rounded(z(4.))
            .bg(t(idx::BACKGROUND_MOD_STRONG))
            .flex()
            .items_center()
            .gap(z(2.))
            .when_some(p.badge_hash.clone(), |d, h| d.child(gpui::img(cdn(format!("guild-tag-badges/{}/{h}.png?size=16", gid.0))).size(z(14.))))
            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_DEFAULT)).child(tag))
    });
    div()
        .w(z(300.))
        .flex_none()
        .rounded(z(16.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_SURFACE_HIGHEST))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div().relative().h(z(120.)).w_full().flex_none().overflow_hidden().bg(t(idx::BACKGROUND_MOD_STRONG)).when_some(p.custom_banner_hash.clone(), |d, h| {
                d.child(
                    gpui::img(cdn(format!("discovery-splashes/{}/{h}.jpg?size=640", gid.0)))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                )
            }),
        )
        .child(
            // avatarWrapper_: 70px frame in the card colour around the 64px icon
            // (clan-icon squircle mask, measured ~22 / 19px corners on the capture).
            div().mt(z(-34.)).px(z(16.)).child(
                div().size(z(70.)).rounded(z(22.)).bg(t(idx::BACKGROUND_SURFACE_HIGHEST)).flex().items_center().justify_center().child(icon(g, 64., 19.)),
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
                            div()
                                .min_w_0()
                                .text_ellipsis()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t(idx::TEXT_STRONG))
                                .child(crate::ui::emoji_text::label(g.name.clone(), 16.)),
                        )
                        .when(p.features.iter().any(|f| f == "DISCOVERABLE"), |d| d.child(div().w(z(20.)).flex().justify_center().child(discovery_badge(18.))))
                        .children(tag),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x(z(8.))
                        .child(count(true, crate::t!("guide.online", n = grouped(p.online_count))))
                        .child(count(false, crate::t!("guide.members", n = grouped(p.member_count)))),
                )
                .child(muted(established(gid))),
        )
        .child(
            div()
                .mb(z(16.))
                .px(z(16.))
                .flex()
                .flex_col()
                .gap(z(16.))
                .when(!p.description.is_empty(), |d| d.child(muted(p.description.clone().into())))
                .when(!p.traits.is_empty(), |d| d.child(traits)),
        )
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !is_open(app, cx) {
        return None;
    }
    let page = cx.try_global::<Slot>()?.0.clone()?;
    let g = app.guilds.iter().find(|g| g.id == page.guild)?.clone();
    let bar = div()
        .h(z(48.))
        .flex_none()
        .px(z(16.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(div().size(z(32.)).flex().items_center().justify_center().child(
            gpui::svg().path(crate::icons::path("signpost")).size(z(20.)).flex_none().text_color(t(idx::ICON_MUTED)),
        ))
        .child(div().mr(z(8.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_STRONG)).child(crate::t!("guild.top.guide")));
    let mut main = div().flex_1().min_w_0().flex().flex_col();
    match &page.data {
        None => main = main.child(div().mt(z(16.)).text_color(t(idx::TEXT_MUTED)).child(crate::t!("common.loading"))),
        Some(d) => {
            if !d.resources.is_empty() {
                // container_Qo6EL-: margin-top 16, gap 8; heading-lg/semibold 20/24.
                let mut col = div().mt(z(16.)).flex().flex_col().gap(z(8.)).child(
                    div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(crate::t!("guide.resources")),
                );
                for (i, r) in d.resources.iter().enumerate() {
                    col = col.child(resource(i, r, page.previews.get(&r.channel), cx));
                }
                main = main.child(col);
            }
            if let Some(er) = page.error.clone() {
                main = main.child(div().mt(z(16.)).text_size(z(14.)).text_color(t(idx::TEXT_FEEDBACK_CRITICAL)).child(er));
            }
        }
    }
    let side = page.data.as_ref().and_then(|d| d.profile.as_ref()).map(|p| card(&g, p));
    // The scroller keeps a 16px scrollbar gutter (scrollbarGutterStable_): content 1103 of 1119.
    let body = div()
        .p(z(32.))
        .pr(z(48.))
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(header(app, &g, cx))
        .child(div().mt(z(12.)).flex().items_start().gap(z(24.)).child(main).children(side));
    Some(
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(t(idx::BACKGROUND_BASE_LOWER))
            .child(bar)
            .child(div().id("guide-scroll").flex_1().min_h_0().overflow_y_scroll().child(body))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn needs_all_three_features() {
        let f = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(super::has_guide(&f(&["COMMUNITY", "GUILD_ONBOARDING", "GUILD_SERVER_GUIDE", "NEWS"])));
        assert!(!super::has_guide(&f(&["COMMUNITY", "GUILD_ONBOARDING"])));
    }
}
