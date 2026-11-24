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
    from: Option<Id>,
    data: Option<GuildGuide>,
    error: Option<SharedString>,
    previews: std::collections::HashMap<Id, Preview>,
}

#[derive(Clone, Default)]
struct Preview {
    text: Option<String>,
    media: Option<(SharedString, u32, u32)>,
}

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

pub fn has_guide(features: &[String]) -> bool {
    ["COMMUNITY", "GUILD_SERVER_GUIDE", "GUILD_ONBOARDING"].iter().all(|f| features.iter().any(|x| x == f))
}

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

fn discovery_badge(size: f32) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .size(z(size))
        .child(gpui::svg().path(crate::icons::path("discovery-badge")).absolute().size(z(size)).text_color(t(idx::BADGE_INVERTED_BACKGROUND)))
        .child(gpui::svg().path(crate::icons::path("discovery-badge-fg")).absolute().size(z(size)).text_color(t(idx::BADGE_INVERTED_TEXT)))
}

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

fn header(app: &PhoenixApp, g: &Guild, cx: &Context<PhoenixApp>) -> gpui::Div {
    let gid = g.id;
    let art = app.backend.guild_home_header(gid);
    let art = div()
        .ml(z(-16.))
        .mr(z(-16.))
        .relative()
        .rounded(z(16.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
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

fn resource(i: usize, r: &GuideEntry, preview: Option<&Preview>, cx: &Context<PhoenixApp>) -> AnyElement {
    let ch = r.channel;
    let text = Some(r.description.clone()).filter(|d| !d.is_empty()).or_else(|| preview.and_then(|p| p.text.clone()));
    let thumb: Option<AnyElement> = match (&r.icon, preview.and_then(|p| p.media.clone())) {
        (Some(h), _) => Some(
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

fn fit_traits(traits: &[crate::commands::guide::GuideTrait], max_w: f32) -> (usize, usize) {
    if traits.is_empty() {
        return (0, 0);
    }
    const GAP: f32 = 4.;
    const OVERFLOW_W: f32 = 86.;
    let chip_w = |tr: &crate::commands::guide::GuideTrait| -> f32 {
        let emoji = if tr.emoji.token().is_some() { 20. } else { 0. };
        18. + emoji + (tr.label.chars().count() as f32) * 7.2
    };
    let widths: Vec<f32> = traits.iter().map(chip_w).collect();
    let total: f32 = widths.iter().sum::<f32>() + GAP * (widths.len().saturating_sub(1) as f32);
    if total <= max_w {
        return (traits.len(), 0);
    }
    let budget = (max_w - OVERFLOW_W - GAP).max(0.);
    let mut used = 0.;
    let mut shown = 0;
    for (i, &w) in widths.iter().enumerate() {
        let next = if i == 0 { w } else { used + GAP + w };
        if next <= budget || i == 0 {
            used = next;
            shown += 1;
        } else {
            break;
        }
    }
    (shown, traits.len() - shown)
}

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
    let (shown, hidden) = fit_traits(&p.traits, 266.);
    let mut traits = div().flex().items_center().gap(z(4.)).overflow_hidden();
    for tr in p.traits.iter().take(shown) {
        traits = traits.child(
            div()
                .max_w(z(266.))
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
    if hidden > 0 {
        traits = traits.child(
            div()
                .flex_none()
                .py(z(4.))
                .px(z(8.))
                .rounded(z(16.))
                .border_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(t(idx::TEXT_DEFAULT))
                .child(crate::t!("guide.traits_more", n = hidden.to_string())),
        );
    }
    let tag = p.tag.clone().map(|tag| {
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

fn action_row(i: usize, a: &GuideEntry, cx: &Context<PhoenixApp>) -> AnyElement {
    let ch = a.channel;
    div()
        .id(("guide-act", i))
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .hover(|s| s.bg(t(idx::BACKGROUND_SURFACE_HIGHER)))
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, _, cx| {
            if app.channels.contains_key(&ch) {
                app.select_channel(ch, cx);
            }
        }))
        .flex()
        .items_center()
        .gap(z(12.))
        .when_some(a.emoji.token(), |d, tk| d.child(div().flex_none().child(crate::media::emoji::emoji_element(&tk, 20.))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(crate::ui::emoji_text::label(a.title.clone().into(), 16.)),
                )
                .when(!a.description.is_empty(), |d| {
                    d.child(div().mt(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_MUTED)).child(a.description.clone()))
                }),
        )
        .child(
            gpui::svg()
                .path(crate::icons::path(if a.action_type == 1 { "chat" } else { "chevron" }))
                .size(z(16.))
                .flex_none()
                .text_color(t(idx::ICON_MUTED)),
        )
        .into_any_element()
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
            if !d.welcome_message.is_empty() {
                let authors: Vec<String> = d.welcome_authors.iter().filter_map(|id| app.users.get(id).map(|u| u.name.to_string())).collect();
                let mut wcard = div()
                    .mt(z(16.))
                    .p(z(16.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(t(idx::BORDER_SUBTLE))
                    .bg(t(idx::BACKGROUND_SURFACE_HIGH))
                    .flex()
                    .flex_col()
                    .gap(z(8.));
                if !authors.is_empty() {
                    wcard = wcard.child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t(idx::TEXT_STRONG))
                            .child(authors.join(", ")),
                    );
                }
                wcard = wcard.child(crate::ui::profile_bio::bio(&d.welcome_message, false, t(idx::TEXT_DEFAULT)));
                main = main.child(wcard);
            }
            if !d.actions.is_empty() {
                let mut col = div().mt(z(16.)).flex().flex_col().gap(z(8.)).child(
                    div()
                        .text_size(z(20.))
                        .line_height(z(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(crate::t!("guide.getting_started")),
                );
                for (i, a) in d.actions.iter().enumerate() {
                    col = col.child(action_row(i, a, cx));
                }
                main = main.child(col);
            }
            if !d.resources.is_empty() {
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

    #[test]
    fn fit_traits_collapses_overflow_to_one_row() {
        let tr = |label: &str| crate::commands::guide::GuideTrait { label: label.to_owned(), emoji: Default::default() };
        let short = vec![tr("Dev"), tr("Tech")];
        assert_eq!(super::fit_traits(&short, 266.), (2, 0));
        let many = vec![tr("Developpement"), tr("Tech"), tr("Rust"), tr("Linux"), tr("Gaming")];
        let (shown, hidden) = super::fit_traits(&many, 266.);
        assert!(shown >= 1 && hidden > 0 && shown + hidden == many.len());
    }
}
