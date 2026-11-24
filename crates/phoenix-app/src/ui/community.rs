//! "Channels & Roles" page (sidebar row of onboarding guilds; client route
//! `/channels/{g}/customize-community`). Values from the live client (dump
//! vanilla-channels-roles): header container_ybUK7Z (icon 32/20, body-md/medium
//! title); tabBar_xTP3zy margin 32 32 0, gap 24, items padding-bottom 16 with a
//! --text-brand selected tab; scroller padding 32 32 0; pageBody_ padding-right
//! 16; prompt_ cards (1px --border-subtle, radius 8, --background-surface-high,
//! padding 16) with a 2-column promptOptions_ grid (gap 8) of optionButton_
//! (radius 12, padding 12 16, 22px emoji + margin 12, body-md/medium title,
//! body-xs/normal description). Read-only: options are shown, never answered.

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px};

use crate::{
    app::PhoenixApp,
    commands::{OnboardingOption, OnboardingPrompt, ParityReply, ParityRequest},
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
};

#[derive(Default)]
struct Slot(Option<Page>);
impl gpui::Global for Slot {}

struct Page {
    guild: Id,
    /// Channel selected when the page opened: selecting another one closes it.
    from: Option<Id>,
    prompts: Option<Vec<OnboardingPrompt>>,
    error: Option<SharedString>,
    /// "Browse Channels" tab selected.
    browse: bool,
    /// Its "Choose your channels" intro card was dismissed.
    intro_closed: bool,
}

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

/// The page shown for the selected guild, if open.
pub fn is_open(app: &PhoenixApp, cx: &App) -> bool {
    cx.try_global::<Slot>().and_then(|s| s.0.as_ref()).is_some_and(|p| Some(p.guild) == app.selected_guild && p.from == app.selected_channel)
}

pub fn open(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    open_tab(app, guild, false, cx);
}

/// Opens the page on the Customize (`browse == false`) or the Browse Channels tab.
pub fn open_tab(app: &mut PhoenixApp, guild: Id, browse: bool, cx: &mut Context<PhoenixApp>) {
    cx.set_global(Slot(Some(Page { guild, from: app.selected_channel, prompts: None, error: None, browse, intro_closed: false })));
    app.parity_request(ParityRequest::Onboarding { guild }, cx, move |a, r, cx| {
        let from = a.selected_channel;
        let (prompts, error) = match r {
            Ok(ParityReply::Onboarding(p)) => (Some(p), None),
            Ok(_) => (Some(vec![]), None),
            Err(e) => (Some(vec![]), Some(SharedString::from(e.0))),
        };
        let keep = cx.try_global::<Slot>().and_then(|s| s.0.as_ref()).filter(|p| p.guild == guild).map(|p| (p.browse, p.intro_closed));
        if let Some((browse, intro_closed)) = keep {
            cx.set_global(Slot(Some(Page { guild, from, prompts, error, browse, intro_closed })));
        }
        cx.notify();
    });
    cx.notify();
}

fn emoji_token(o: &OnboardingOption) -> Option<String> {
    match (o.emoji_id, o.emoji_name.as_deref()) {
        (Some(id), n) => Some(format!("<{}:{}:{}>", if o.animated { "a" } else { "" }, n.unwrap_or("e"), id.0)),
        (None, Some(n)) if !n.is_empty() => Some(n.to_owned()),
        _ => None,
    }
}

fn option(o: &OnboardingOption) -> gpui::Div {
    // optionButtonWrapper_87WSWY: 1px --border-subtle, radius 12, rgba(151,151,159,.08)
    // (--control-secondary-background-default at 8%: --background-mod-subtle)
    div()
        .rounded(z(12.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
        .cursor_pointer()
        // optionButton_87WSWY: padding 12 16
        .py(z(12.))
        .px(z(16.))
        .flex()
        .items_center()
        .when_some(emoji_token(o), |d, tk| d.child(div().flex_none().mr(z(12.)).child(crate::media::emoji::emoji_element(&tk, 22.))))
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
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(o.title.clone()),
                )
                .when(!o.description.is_empty(), |d| {
                    d.child(div().mt(z(4.)).text_size(z(12.)).line_height(z(16.)).text_color(t(idx::TEXT_DEFAULT)).child(o.description.clone()))
                }),
        )
}

fn prompt(p: &OnboardingPrompt) -> gpui::Div {
    // promptOptions_pHGfKY: 2 equal columns, gap 8
    let mut grid = div().mt(z(16.)).flex().flex_col().gap(z(8.));
    for pair in p.options.chunks(2) {
        let mut row = div().flex().gap(z(8.));
        for o in pair {
            row = row.child(option(o).flex_1().min_w_0());
        }
        if pair.len() == 1 {
            row = row.child(div().flex_1());
        }
        grid = grid.child(row);
    }
    div()
        .mt(z(16.))
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .flex()
        .flex_col()
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(p.title.clone()))
        .child(grid)
        .child(div().mt(z(16.)).text_size(z(12.)).line_height(z(16.)).text_color(t(idx::TEXT_MUTED)).child(if p.single_select {
            crate::t!("community.pick_one")
        } else {
            crate::t!("community.pick_all")
        }))
}

fn update_page(cx: &mut App, f: impl FnOnce(&mut Page)) {
    let next = cx.try_global::<Slot>().and_then(|s| s.0.as_ref()).map(|p| Page {
        guild: p.guild,
        from: p.from,
        prompts: p.prompts.clone(),
        error: p.error.clone(),
        browse: p.browse,
        intro_closed: p.intro_closed,
    });
    if let Some(mut p) = next {
        f(&mut p);
        cx.set_global(Slot(Some(p)));
    }
}

/// Browse Channels groups in sidebar order: channels without a category first,
/// then each category with its channels (`None` = no category header).
fn browse_groups(app: &PhoenixApp, guild: Id) -> Vec<(Option<Id>, Vec<Id>)> {
    let list = app.guild_channels.get(&guild).cloned().unwrap_or_default();
    let kind = |id: Id| app.channels.get(&id).map(|c| c.kind);
    let mut out: Vec<(Option<Id>, Vec<Id>)> = vec![(None, Vec::new())];
    for &id in &list {
        if kind(id) == Some(ChannelKind::Category) {
            out.push((Some(id), Vec::new()));
        }
    }
    for &id in &list {
        if kind(id).is_none_or(|k| k == ChannelKind::Category) {
            continue;
        }
        let parent = app.channels.get(&id).and_then(|c| c.parent);
        let at = parent.and_then(|p| out.iter().position(|(cat, _)| *cat == Some(p))).unwrap_or(0);
        out[at].1.push(id);
    }
    out.retain(|(_, v)| !v.is_empty());
    out
}

/// "Browse Channels" tab (dumps/vanilla-browse-channels, 2026-11-24). Read-only:
/// the follow switches and checkboxes show the READY `OPT_IN_ENABLED` overrides and
/// never change them.
fn browse_body(app: &PhoenixApp, page: &Page, groups: Vec<(Option<Id>, Vec<Id>)>, cx: &Context<PhoenixApp>) -> AnyElement {
    let now = chrono::Utc::now().timestamp();
    let opted = |id: Id| app.backend.channel_opted_in(id);
    // header_fTKmdS: margin 0 24 20 8 inside the 8px scroller padding; the search field
    // (container_0PLWhS md) is 40 high, radius 8, 1px --input-border-default,
    // --input-background-default, 30px magnifier box, body-md placeholder.
    let search = div()
        .mt(z(16.))
        .ml(z(16.))
        .mr(z(32.))
        .mb(z(20.))
        .h(z(40.))
        .flex_none()
        .pl(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::INPUT_BORDER_DEFAULT))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(div().size(z(30.)).flex().items_center().justify_center().child(
            gpui::svg().path(crate::icons::path("search")).size(z(20.)).flex_none().text_color(t(idx::TEXT_DEFAULT)),
        ))
        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(t(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)).child(crate::t!("community.search_channels")));
    // browser_fTKmdS starts 8px in, its content_ adds 8 each side, then a 16px scrollbar gutter:
    // rows span page-left + 16 to page-right - 32 like the search field.
    let mut list = div().pl(z(16.)).pr(z(8.)).pb(z(4.)).flex().flex_col();
    if !page.intro_closed {
        // container_oQmK1h: padding 16, margin-bottom 24, radius 8, heading-lg/semibold
        // title (mb 8), body-md text, tip with a semibold lead (mt 4), 24px close icon.
        list = list.child(
            div()
                .mb(z(24.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_BASE_LOW))
                .flex()
                .items_start()
                .gap(z(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().mb(z(8.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(crate::t!("community.browse_intro_title")))
                        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(t(idx::TEXT_DEFAULT)).child(crate::t!("community.browse_intro")))
                        .child(
                            div()
                                .mt(z(4.))
                                .flex()
                                .flex_wrap()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .text_color(t(idx::TEXT_DEFAULT))
                                .child(div().mr(z(4.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("community.browse_tip_lead")))
                                .child(crate::t!("community.browse_tip")),
                        ),
                )
                .child(
                    div()
                        .id("cr-intro-close")
                        .size(z(24.))
                        .flex_none()
                        .cursor_pointer()
                        .child(gpui::svg().path(crate::icons::path("close")).size(z(24.)).text_color(t(idx::INTERACTIVE_ICON_DEFAULT)))
                        .on_click(cx.listener(|_, _, _, cx| {
                            update_page(cx, |p| p.intro_closed = true);
                            cx.notify();
                        })),
                ),
        );
    }
    for (cat, ids) in groups {
        let followed = cat.and_then(opted).unwrap_or(false);
        if let Some(cat) = cat.and_then(|c| app.channels.get(&c)) {
            // categoryRow_kU9idL: padding 20 0 10, body-lg/medium 20/24 --text-strong;
            // "Follow Category" body-md/medium + 48x24 switch, 24px apart.
            list = list.child(
                div()
                    .pt(z(20.))
                    .pb(z(10.))
                    .flex()
                    .items_center()
                    .child(div().flex_1().min_w_0().text_ellipsis().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_STRONG)).child(crate::ui::emoji_text::label(cat.name.clone(), 20.)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(24.))
                            .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_STRONG)).child(crate::t!("community.follow_category")))
                            .child(crate::ui::controls::switch(("cr-follow", cat.id.0), followed).cursor_default()),
                    ),
            );
        }
        let n = ids.len();
        for (i, id) in ids.into_iter().enumerate() {
            let Some(ch) = app.channels.get(&id) else { continue };
            let on = followed || opted(id).unwrap_or(true);
            let last = app.backend.channel_last_message(id).map(|m| (((m.0 >> 22) + 1_420_070_400_000) / 1000) as i64);
            let mut sub = div().flex().items_center().min_w_0().text_size(z(12.)).line_height(z(16.)).text_color(t(idx::TEXT_DEFAULT));
            if let Some(ts) = last {
                sub = sub.child(div().flex_none().child(crate::t!("community.last_activity", when = crate::i18n::relative_long(ts - now))));
            }
            if let Some(topic) = ch.topic.clone() {
                if last.is_some() {
                    // subtitleSeparator: 4px dot, --border-subtle, margin 0 8
                    sub = sub.child(div().flex_none().mx(z(8.)).size(z(4.)).rounded(z(4.)).bg(t(idx::BORDER_SUBTLE)));
                }
                sub = sub.child(div().flex_1().min_w_0().text_ellipsis().child(crate::ui::emoji_text::label(topic.lines().next().unwrap_or("").to_owned().into(), 12.)));
            }
            // channelRow_kU9idL: padding 10 16, 1px --border-subtle sides and top (bottom on the
            // last), radius 8 8 0 0 / 0 0 8 8; name body-md/medium, --text-muted while the
            // category is followed (row disabled_), icon 20 --channel-icon, gap 10.
            let mut row = div()
                .py(z(10.))
                .px(z(16.))
                .border_l_1()
                .border_r_1()
                .border_t_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .flex()
                .items_center()
                .when(i == 0, |d| d.rounded_t(z(8.)))
                .when(i + 1 == n, |d| d.border_b_1().rounded_b(z(8.)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(z(16.))
                        .flex()
                        .flex_col()
                        .gap(z(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(z(10.))
                                .child(crate::ui::icon(crate::ui::sidebar::channel_icon(ch.kind), 20., t(idx::CHANNEL_ICON)))
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_ellipsis()
                                        .text_size(z(16.))
                                        .line_height(z(20.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t(if followed { idx::TEXT_MUTED } else { idx::TEXT_DEFAULT }))
                                        .child(crate::ui::emoji_text::label(ch.name.clone(), 16.)),
                                ),
                        )
                        .when(last.is_some() || ch.topic.is_some(), |d| d.child(sub)),
                );
            row = row.child(checkbox(on));
            list = list.child(row);
        }
    }
    div()
        .id("community-browse")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(search)
        .child(div().id("community-browse-scroll").flex_1().min_h_0().overflow_y_scroll().pr(z(24.)).child(list))
        .into_any_element()
}

/// checkboxIndicator_vTUVyc (20px, radius 4, 1px border): selected
/// --checkbox-background/border-selected-default with the white check.
fn checkbox(on: bool) -> gpui::Div {
    let (bg, border) = if on {
        (idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT, idx::CHECKBOX_BORDER_SELECTED_DEFAULT)
    } else {
        (idx::CHECKBOX_BACKGROUND_DEFAULT, idx::CHECKBOX_BORDER_DEFAULT)
    };
    div()
        .size(z(20.))
        .flex_none()
        .rounded(z(4.))
        .border_1()
        .border_color(t(border))
        .bg(t(bg))
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(t(idx::CHECKBOX_ICON_ACTIVE))))
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !is_open(app, cx) {
        return None;
    }
    let page = cx.try_global::<Slot>()?.0.as_ref()?;
    let header = div()
        .h(z(48.))
        .flex_none()
        .px(z(16.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(div().size(z(32.)).flex().items_center().justify_center().child(
            gpui::svg().path(crate::icons::path("channel-browse")).size(z(20.)).flex_none().text_color(t(idx::ICON_MUTED)),
        ))
        .child(
            div()
                .mr(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .child(crate::t!("guild.top.channels_roles")),
        );
    let browse = page.browse;
    let channels = browse_groups(app, page.guild);
    let count: usize = channels.iter().map(|(_, v)| v.len()).sum();
    let tab = |id: &'static str, label: SharedString, on: bool, to_browse: bool| {
        div()
            .id(id)
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                update_page(cx, |p| p.browse = to_browse);
                cx.notify();
            }))
            .flex()
            .gap(z(8.))
            .pb(z(16.))
            .mb(px(-1.))
            .text_size(z(16.))
            .line_height(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if on { t(idx::TEXT_BRAND) } else { t(idx::TEXT_MUTED) })
            .when(on, |d| d.border_b_2().border_color(t(idx::TEXT_BRAND)))
            .child(label)
    };
    let tabs = div()
        .mt(z(32.))
        .mx(z(32.))
        .flex_none()
        .flex()
        .gap(z(24.))
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(tab("cr-tab-customize", crate::t!("community.customize"), !browse, false))
        // numberBadge_xTP3zy (vanilla-browse-channels #1022): body-xs/semibold 12/16, round,
        // --badge-secondary-background / -text, 22x16 for two digits.
        .child(tab("cr-tab-browse", crate::t!("community.browse"), browse, true).child(
            div()
                .h(z(16.))
                .px(z(4.))
                .rounded_full()
                .bg(t(idx::BADGE_SECONDARY_BACKGROUND))
                .text_color(t(idx::BADGE_SECONDARY_TEXT))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(count.to_string())),
        ));
    if browse {
        return Some(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .child(header)
                .child(tabs)
                .child(browse_body(app, page, channels, cx))
                .into_any_element(),
        );
    }
    let mut body = div()
        .pr(z(16.))
        .flex()
        .flex_col()
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(crate::t!("community.questions")))
        .child(div().text_size(z(12.)).line_height(z(16.)).text_color(t(idx::TEXT_MUTED)).child(crate::t!("community.questions_sub")));
    match &page.prompts {
        None => body = body.child(div().mt(z(16.)).text_color(t(idx::TEXT_MUTED)).child(crate::t!("common.loading"))),
        Some(list) => {
            for p in list {
                body = body.child(prompt(p));
            }
            if let Some(er) = page.error.clone() {
                body = body.child(div().mt(z(16.)).text_size(z(14.)).text_color(t(idx::TEXT_FEEDBACK_CRITICAL)).child(er));
            }
        }
    }
    Some(
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(t(idx::BACKGROUND_BASE_LOWER))
            .child(header)
            .child(tabs)
            .child(div().id("community-scroll").flex_1().min_h_0().overflow_y_scroll().pt(z(32.)).px(z(32.)).pb(z(32.)).child(body.max_w(z(744.)))) // pageBody_ 727 + padding-right 16 beside the profile column
            .into_any_element(),
    )
}
