//! Channel sidebar (268px, `--custom-guild-sidebar-width`): guild header,
//! channel tree / DM list, and the floating panels card (voice + user).

/// `--radius-sm`: channel and DM rows in the refreshed client.
const ROW_RADIUS: f32 = 8.;

use crate::ui::tooltip::Tipped as _;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    Transformation, div, list, percentage, prelude::*, px,
};

use crate::{
    app::{PhoenixApp, SidebarRow},
    ui::home::data::HomeView,
    assets::Icon,
    model::{ChannelKind, Id},
    theme::{Palette, c, generated::idx, m, space, tok, z},
    ui::{badge, icon},
};

/// Colour of a generated CSS token.
fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn channel_icon(kind: ChannelKind) -> Icon {
    match kind {
        ChannelKind::Text | ChannelKind::Category => Icon::Hash,
        ChannelKind::Voice => Icon::Speaker,
        ChannelKind::Announcement => Icon::Megaphone,
        ChannelKind::Forum => Icon::Forum,
        ChannelKind::Stage => Icon::Stage,
    }
}

/// Sidebar rows for the current selection: DMs, or the guild channel tree
/// with collapsed categories (persisted), hidden muted channels, threads
/// under their parent and live voice members.
pub fn build_rows(app: &mut PhoenixApp) {
    // Keep the scroll position anchored on the same row across rebuilds (unread,
    // voice or typing updates), like the live client; item 0 is the banner/spacer.
    let anchor = app.sidebar_list.logical_scroll_top();
    let anchor_row = anchor.item_ix.checked_sub(1).and_then(|i| app.sidebar_rows.get(i).copied());
    let old_guild = app.sidebar_rows_guild;
    app.sidebar_rows_guild = app.selected_guild;
    app.sidebar_rows.clear();
    match app.selected_guild {
        None => {
            let reqs = &app.nav.message_requests;
            let rows: Vec<SidebarRow> =
                app.dms.iter().filter(|d| !reqs.contains(d)).map(|id| SidebarRow::Channel(*id)).collect();
            let mut rows = rows;
            crate::actions::pins::apply(&mut rows);
            app.sidebar_rows = rows;
        }
        Some(g) => {
            let hide_muted = app.nav.hide_muted.contains(&g);
            let ids = app.guild_channels.get(&g).cloned().unwrap_or_default();
            let mut current_cat: Option<Id> = None;
            let mut rows = Vec::with_capacity(ids.len());
            for id in ids {
                let Some(ch) = app.channels.get(&id) else { continue };
                if ch.kind == ChannelKind::Category {
                    current_cat = Some(id);
                    rows.push(SidebarRow::Category(id));
                    continue;
                }
                let selected = Some(id) == app.selected_channel
                    || app.selected_channel.and_then(|s| app.nav.thread_parent(s)) == Some(id);
                let collapsed = current_cat.is_some_and(|c| app.collapsed.contains(&c));
                // Collapsed categories still show the selected / unread channels.
                if collapsed && !selected && !(ch.unread && !ch.muted) && ch.voice_users.is_empty() {
                    continue;
                }
                if hide_muted && ch.muted && !selected {
                    continue;
                }
                rows.push(SidebarRow::Channel(id));
                for t in app.nav.threads.get(&id).into_iter().flatten() {
                    if !collapsed || Some(t.id) == app.selected_channel {
                        rows.push(SidebarRow::Thread(t.id));
                    }
                }
                // Live client (channel list, 2026-11-23): voice users are listed by their
                // shown name (guild nick, else display name), case-insensitively.
                // Stage: only speakers get rows; `suppress` users go to the audience strip.
                let stage = ch.kind == ChannelKind::Stage;
                let mut vu: Vec<(String, Id)> = ch
                    .voice_users
                    .iter()
                    .filter(|u| !stage || !crate::ui::voice_panel::flags(**u).suppress)
                    .map(|u| (voice_user_name(app, *u).to_lowercase(), *u))
                    .collect();
                vu.sort();
                // "Hide Names": one row holding the avatar strip (rendered by the first user).
                let take = if crate::ui::voice_row::names_hidden(id) { 1 } else { usize::MAX };
                rows.extend(vu.into_iter().take(take).map(|(_, u)| SidebarRow::VoiceUser { user: u }));
                if stage && ch.voice_users.iter().any(|u| crate::ui::voice_panel::flags(*u).suppress) {
                    rows.push(SidebarRow::StageAudience(id));
                }
            }
            // Drop (expanded) categories left empty by hidden muted channels.
            if hide_muted {
                let keep: Vec<SidebarRow> = rows
                    .iter()
                    .enumerate()
                    .filter(|(i, r)| match r {
                        SidebarRow::Category(cat) => {
                            app.collapsed.contains(cat) || matches!(rows.get(i + 1), Some(SidebarRow::Channel(_)))
                        }
                        _ => true,
                    })
                    .map(|(_, r)| *r)
                    .collect();
                rows = keep;
            }
            crate::actions::pins::apply(&mut rows);
            app.sidebar_rows = rows;
        }
    }
    app.sidebar_list.reset(app.sidebar_rows.len() + 1);
    if old_guild == app.selected_guild {
        let new_ix = match anchor_row {
            Some(r) => app.sidebar_rows.iter().position(|x| *x == r).map(|i| i + 1),
            None => Some(0),
        };
        if let Some(item_ix) = new_ix {
            app.sidebar_list.scroll_to(gpui::ListOffset { item_ix, offset_in_item: anchor.offset_in_item });
        }
    }
}

/// Name a voice user row shows: guild nick, else display name.
pub fn voice_user_name(app: &PhoenixApp, user: Id) -> SharedString {
    app.member_nick(user)
        .or_else(|| app.users.get(&user).map(|u| u.display_name.clone()))
        .unwrap_or_default()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let entity = cx.entity();
    // Re-render while the top of the list is in view so the banner follows the scroll.
    let weak = entity.downgrade();
    app.sidebar_list.set_scroll_handler(move |ev: &gpui::ListScrollEvent, _w, cx: &mut App| {
        if ev.visible_range.start <= 1 {
            let _ = weak.update(cx, |_, cx| cx.notify());
        }
    });
    div()
        .w(z(crate::ui::sidebar_resize::width() + m::sidebar_extra_w()))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_TERTIARY)) // the client's sidebar_ is transparent over bg_ (the rail colour) in every theme (CDP 2026-11-24)
        .rounded_tl(z(m::FRAME_RADIUS))
        .overflow_hidden()
        .relative()
        .child(header(app, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(list(app.sidebar_list.clone(), move |ix, _w, cx| row(&entity, ix, cx)).size_full())
                .children(crate::ui::channel_unread_bar::render(app)),
        )
        .when_some(app.selected_guild.filter(|_| app.nav.guild_menu), |d, g| {
            d.child(crate::ui::guild_menu::render(app, g, m::HEADER_H + 4., cx))
        })
        // Frame border drawn over the content, not as layout border: the live rows sit
        // at sidebarList_.x + 10 (the 1px border does not push them right).
        .child(
            div()
                .absolute()
                .inset_0()
                .border_l_1()
                .border_t_1()
                .border_color(c(Palette::APP_FRAME_BORDER))
                .rounded_tl(z(m::FRAME_RADIUS)),
        )
        .child(crate::ui::sidebar_resize::handle(cx))
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    match app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        None => div()
            .h(px(m::HEADER_H + 1.)) // + the frame border, now drawn over the content
            .flex_none()
            .px(z(10.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .child(
                // dump -channels-me+ #588: `button_F7qvhY sm secondary fullWidth`: ~31px,
                // radius 8, `--control-secondary-background-default`, 1px `--border-muted`,
                // centered `body-sm/medium` (14/18) `--text-strong`.
                div()
                    .w_full()
                    .h(z(31.))
                    .rounded(z(8.))
                    .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                    .border_1()
                    .border_color(t(idx::BORDER_MUTED))
                    .px(z(11.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t(idx::TEXT_STRONG))
                    .child(div().min_w_0().text_ellipsis().child(crate::t!("dm.find"))),
            )
            .id("dm-search")
            .cursor_pointer()
            .on_click(cx.listener(|a, _, w, cx| a.open_switcher(w, cx)))
            .into_any_element(),
        Some(g) => {
            let hash = app.nav.banners.get(&g.id).cloned();
            // The refreshed client only shows a banner when the guild has one.
            let full_h = ((crate::ui::sidebar_resize::width() + m::sidebar_extra_w()) * 9. / 21.).round();
            // Live client (`animatedContainer_XH4OpX`, 2026-11-23): the banner scrolls up with
            // the list and is gone (opacity 0) once only the header bar is left.
            let top = app.sidebar_list.logical_scroll_top();
            let scrolled = if top.item_ix > 0 { f32::MAX } else { f32::from(top.offset_in_item) };
            let banner_h = full_h - scrolled.min(full_h);
            let banner = hash
                .as_ref()
                .filter(|_| banner_h > m::HEADER_H)
                .map(|_| g.banner_color.unwrap_or(Palette::BG_SECONDARY));
            let menu_open = app.nav.guild_menu;
            let gid = g.id.0;
            div()
                .id("guild-banner")
                .group("guild-banner")
                .flex_none()
                .relative()
                .when_some(banner, |d, col| {
                    d.h(px(banner_h))
                        .bg(c(col))
                        .overflow_hidden()
                        .when_some(hash.clone(), |d, hash| {
                            let url = |ext: &str| -> SharedString {
                                format!("https://cdn.discordapp.com/banners/{gid}/{hash}.{ext}?size=480").into()
                            };
                            let animated = hash.starts_with("a_");
                            // Explicit width: with only left/right, taffy sizes the image from its aspect
                            // ratio (16:9 at 144 px tall = 256 px), leaving the right of the banner empty.
                            d.child(gpui::img(url("png")).absolute().bottom_0().left_0().right_0().w_full().h(px(full_h)).object_fit(gpui::ObjectFit::Cover))
                                .when(animated, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .opacity(0.)
                                            .group_hover("guild-banner", |s| s.opacity(1.))
                                            .child(gpui::img(url("gif")).id("banner-frames").size_full().object_fit(gpui::ObjectFit::Cover)),
                                    )
                                })
                        })
                        .child(
                            div().absolute().top_0().left_0().right_0().h(z(56.)).bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(gpui::black().opacity(0.4), 0.),
                                gpui::linear_color_stop(gpui::transparent_black(), 1.),
                            )),
                        )
                        .child(
                            div().absolute().bottom_0().left_0().right_0().h(z(40.)).bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(gpui::transparent_black(), 0.),
                                gpui::linear_color_stop(c(Palette::BG_TERTIARY).opacity(0.8), 1.),
                            )),
                        )
                })
                .child({
                    // `header_XH4OpX` (dump -channels-N-N+ #588): `--custom-channel-header-height`,
                    // padding `--space-xs` (10px cozy), 1px `--border-subtle` bottom.
                    // `guildDropdown_XH4OpX` (#590): 32px, padding 0 10, radius 8, gap 2;
                    // name `body-md/semibold` (16/20) + 16px chevron. `inviteButton_XH4OpX`
                    // (#609): 32px, radius 8, 20px glyph.
                    let fg = if banner.is_some() { gpui::white() } else { t(idx::TEXT_STRONG) };
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(m::HEADER_H + 1.))
                        .p(z(10.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(z(4.))
                        .when(banner.is_none(), |d| d.border_b_1().border_color(t(idx::BORDER_SUBTLE)))
                        .child(
                            div()
                                .id("guild-header")
                                .h(z(32.))
                                .min_w_0()
                                .px(z(10.))
                                .flex()
                                .items_center()
                                .gap(z(2.))
                                .rounded(z(8.))
                                .cursor_pointer()
                                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .when(menu_open, |d| d.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .on_click(cx.listener(|a, _, _, cx| {
                                    a.nav.guild_menu = !a.nav.guild_menu;
                                    cx.notify();
                                }))
                                .child(
                                    // vanilla-guild-text #616 guildBadgeAndName_: gap 10; #617/#619
                                    // guildIconV2Container_ 20px holding an 18px guildBadge_ (Discovery).
                                    div()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(z(10.))
                                        .when(g.discoverable, |d| {
                                            d.child(
                                                // GuildBadge DISCOVERABLE → ExperimentalWebBadgeIcon 18px,
                                                // two tones: --badge-inverted-background / --badge-inverted-text.
                                                div().w(z(20.)).flex_none().flex().justify_center().child(
                                                    div()
                                                        .relative()
                                                        .size(z(18.))
                                                        .child(
                                                            gpui::svg()
                                                                .path(crate::icons::path("discovery-badge"))
                                                                .absolute()
                                                                .size(z(18.))
                                                                .text_color(t(idx::BADGE_INVERTED_BACKGROUND)),
                                                        )
                                                        .child(
                                                            gpui::svg()
                                                                .path(crate::icons::path("discovery-badge-fg"))
                                                                .absolute()
                                                                .size(z(18.))
                                                                .text_color(t(idx::BADGE_INVERTED_TEXT)),
                                                        ),
                                                ),
                                            )
                                        })
                                        .child(
                                    div()
                                        .min_w_0()
                                        .text_size(z(16.))
                                        .line_height(z(20.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(fg)
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(g.name.clone()),
                                        ),
                                )
                                .child(icon(if menu_open { Icon::Close } else { Icon::Chevron }, 16. * crate::theme::zoom(), fg)),
                        )
                        .child(
                            div()
                                .id("guild-invite")
                                .tooltip(crate::ui::tooltip::tip(crate::t!("menu.invite")))
                                .size(z(32.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(z(8.))
                                .cursor_pointer()
                                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .child(
                                    gpui::svg()
                                        .path(crate::icons::path("invite"))
                                        .size(z(20.))
                                        .flex_none()
                                        .text_color(fg),
                                ),
                        )
                })
                .when(banner.is_none(), |d| d.h(px(m::HEADER_H + 1.)))
                .into_any_element()
        }
    }
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    if ix == 0 {
        // Top spacer / DM header rows.
        return if app.selected_guild.is_none() {
            dm_top(entity, cx).into_any_element()
        } else {
            guild_top(app, entity.clone(), cx).into_any_element()
        };
    }
    let Some(r) = app.sidebar_rows.get(ix - 1).copied() else {
        return div().into_any_element();
    };
    match r {
        SidebarRow::Category(id) => {
            // `containerDefault_yOdhm_` padding-top `--custom-category-channel-space-before-category`;
            // `wrapperCommon_yOdhm_`: 24px, padding-inline `--space-md` / `--space-md - --space-xs`,
            // `--channels-default` (hover `--interactive-text-hover`); name then a 12px
            // chevron (`mainContent` gap 4px, rotated -90deg when collapsed); 16px add
            // button shown on hover only (`addButton_yOdhm_`).
            let ch = &app.channels[&id];
            let collapsed = app.collapsed.contains(&id);
            let e = entity.clone();
            div()
                .id(("cat", id.0))
                .group("cat")
                // dump -channels-N-N+ #655/#656: li padding-top 16, row 24px,
                // `.enlarge-target-size .wrapper_yOdhm_` padding-inline 20 / 6 (cozy).
                .pt(z(space::category_before()))
                .pl(z(20.))
                // padding-inline-end 6 + the scroller's 8px stable gutter (live
                // `wrapper_yOdhm_` 327px wide in a 335px scroller, like the channel rows).
                .pr(z(6. + 8.))
                .h(z(24. + space::category_before()))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .text_color(t(idx::CHANNELS_DEFAULT))
                .hover(|s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_category_saved(id, cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Channel(id), ev.position, w)
                })
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .child(crate::ui::emoji_text::label(ch.name.clone(), 14.)),
                        )
                        .child(
                            icon(Icon::Chevron, 12., t(idx::CHANNELS_DEFAULT))
                                .group_hover("cat", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                                .with_transformation(Transformation::rotate(percentage(if collapsed {
                                    -0.25
                                } else {
                                    0.
                                }))),
                        ),
                )
                .child(
                    // `addButton_yOdhm_ forceVisible_yOdhm_` (dump #662): 24px box, padding 4,
                    // 16px glyph, always shown.
                    div()
                        .size(z(24.))
                        .flex_none()
                        .p(z(4.))
                        .id(("cat-add", id.0))
                        .tooltip(crate::ui::tooltip::tip(crate::t!("menu.create_channel")))
                        .group("cat-add")
                        // `addButtonIcon_yOdhm_:hover` --interactive-text-hover;
                        // `:active` --interactive-text-active + translateY(.5px).
                        .active(|s| s.pt(z(4.5)))
                        .child(
                            icon(Icon::Plus, 16. * crate::theme::zoom(), t(idx::CHANNELS_DEFAULT))
                                .group_hover("cat-add", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))),
                        ),
                )
                .into_any_element()
        }
        SidebarRow::Channel(id) => channel_row(app, entity, id),
        SidebarRow::Thread(id) => thread_row(app, entity, id),
        SidebarRow::StageAudience(id) => crate::ui::voice_row::stage_audience(app, id),
        SidebarRow::VoiceUser { user } => {
            // `list_YUoOxm listDefault_YUoOxm` (live channel list): padding-bottom 10 after
            // a channel's last voice user.
            let hidden = app.channels.values().find(|c| c.voice_users.contains(&user) && crate::ui::voice_row::names_hidden(c.id));
            if let Some(ch) = hidden {
                let mut vu: Vec<(String, Id)> =
                    ch.voice_users.iter().map(|u| (voice_user_name(app, *u).to_lowercase(), *u)).collect();
                vu.sort();
                let users: Vec<Id> = vu.into_iter().map(|(_, u)| u).collect();
                return crate::ui::voice_row::collapsed_list(app, &users);
            }
            let last = !matches!(app.sidebar_rows.get(ix), Some(SidebarRow::VoiceUser { .. } | SidebarRow::StageAudience(_)));
            let row = crate::ui::voice_panel::voice_user_row(app, entity, user);
            // Stretch the row so its mute/deafen icons sit at the right edge (`icons_YUoOxm`).
            div().w_full().flex().flex_col().when(last, |d| d.pb(z(10.))).child(row).into_any_element()
        }
    }
}

fn dm_top(entity: &Entity<PhoenixApp>, cx: &App) -> impl IntoElement {
    let app = entity.read(cx);
    let home = app.nav.home;
    let requests = app.nav.message_requests.len() as u32;
    let pending = app.nav.count(crate::ui::home::data::RelationshipKind::Incoming) as u32;
    let nav = |id: &'static str, i: Icon, label: SharedString, view: Option<HomeView>, count: u32| {
        let selected = view.is_some_and(|v| v == home);
        let e = entity.clone();
        let stub = label.clone();
        // dump -channels-me+ #597..#609: `channel_zln_tS` li 40px (padding 1/0, margin-left 8),
        // `interactive_zln_tS` 38px radius 8, `link_zln_tS` padding 8 16 8 10; 20px glyph
        // + 12px margin; `body-md/medium` label. Idle `--text-muted`, selected
        // `--background-mod-strong` + `--text-default`.
        div().w_full().pl(z(8.)).pr(z(8.)).py(z(1.)).child(
            div()
                .id(id)
                .h(z(38.))
                .pl(z(10.))
                .pr(z(16.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(if selected { idx::TEXT_DEFAULT } else { idx::TEXT_MUTED }))
                .when(selected, |d| d.bg(t(idx::BACKGROUND_MOD_STRONG)))
                .when(!selected, |d| {
                    // Quests row `withGradient_hjotnx.withQuestHomeHeroContent_hjotnx:hover a`:
                    // `--text-strong` (vanilla-friends-online #646). Its hero image
                    // (#657 `questHomeHeroBackgroundImage_`) is opacity 0 at rest.
                    let hover_fg = if id == "quests" { idx::TEXT_STRONG } else { idx::INTERACTIVE_TEXT_HOVER };
                    d.hover(move |s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(hover_fg)))
                        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE))) // wrapper_TNyK-N:active
                })
                .on_click(move |_, _, cx| {
                    e.update(cx, |a, cx| match view {
                        Some(v) => a.open_home_view(v, cx),
                        None => a.stub(&stub, cx),
                    })
                })
                .child(icon(i, 20. * crate::theme::zoom(), t(if selected { idx::TEXT_STRONG } else { idx::TEXT_MUTED })))
                .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                .when(count > 0, |d| d.child(badge(count).bg(c(Palette::NOTIFICATION)))),
        )
    };
    let e = entity.clone();
    div()
        .pt(z(8.)) // dump -channels-me+ #595: 8px spacer
        .w_full() // full sidebar width so the divider and the header "+" reach the right edge (vanilla-dm #648/#649)
        .flex()
        .flex_col()
        .child(nav("friends", Icon::Friends, crate::t!("friends.title"), Some(HomeView::Friends), pending))
        // vanilla (dump vanilla-friends-online): Friends, Nitro, Shop, Quests.
        // "Message Requests" only shows while requests are pending.
        .when(requests > 0, |d| {
            d.child(nav("msg-requests", Icon::Inbox, crate::t!("friends.message_requests"), Some(HomeView::MessageRequests), requests))
        })
        .child(nav("nitro", Icon::Nitro, crate::t!("dm.nitro"), Some(HomeView::Nitro), 0))
        .child(nav("shop", Icon::Gift, crate::t!("dm.shop"), Some(HomeView::Shop), 0))
        // Quests (`/quest-home`): QuestsIcon (vanilla-friends-online #653).
        .child(nav("quests", Icon::Quests, crate::t!("dm.quests"), None, 0))
        // `sectionDivider_u88IQl` (#648): 1px, margin 12/8, `--border-subtle`.
        .child(div().mx(z(8.)).my(z(12.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
        .child(
            // `privateChannelsHeaderContainer_Bce46W` (vanilla-home-friends #652): 24px, padding
            // 0 6 4 20, `body-sm/medium` 14/18 `--channels-default`, :hover `--interactive-text-hover`.
            // `headerActions_` (#654) gap 4: `moreButton_` (opacity 0 until the header is
            // hovered) then "Create DM"; `iconWrapper_ybUK7Z clickable_` 24px (padding 4) with a
            // 16px glyph, `--icon-muted`, :hover `--icon-subtle`.
            div()
                .id("dm-header")
                .group("dm-header")
                .h(z(24.))
                .pl(z(20.))
                .pr(z(6.))
                .pb(z(4.))
                .flex()
                .justify_between()
                .items_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::CHANNELS_DEFAULT))
                .hover(|s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                // headerText_Bce46W margin-right 10
                .child(div().flex_1().min_w_0().mr(z(10.)).text_ellipsis().child(crate::t!("dm.header")))
                .child({
                    let btn = |id: &'static str, glyph: gpui::Svg| {
                        div()
                            .id(id)
                            .group(id)
                            .size(z(24.))
                            .p(z(4.))
                            .flex()
                            .cursor_pointer()
                            .child(glyph.size(z(16.)).text_color(t(idx::ICON_MUTED)).group_hover(id, |s| s.text_color(t(idx::ICON_SUBTLE))))
                    };
                    let e2 = e.clone();
                    div()
                        .flex()
                        .gap(z(4.))
                        .child(
                            btn("dm-more", gpui::svg().path(crate::icons::path("more-horizontal")))
                                .opacity(0.)
                                .group_hover("dm-header", |s| s.opacity(1.))
                                .tip(crate::t!("header.more"))
                                .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.stub("More", cx))),
                        )
                        .child(
                            btn("new-dm", icon(Icon::Plus, 16., t(idx::ICON_MUTED)))
                                .tip(crate::t!("groupdm.create"))
                                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_group_dm_picker(cx))),
                        )
                }),
        )
}

/// Icon-provider name of the *LockIcon variant for `kind`.
pub(crate) fn lock_icon_name(kind: ChannelKind) -> Option<&'static str> {
    Some(match kind {
        ChannelKind::Text => "text-lock",
        ChannelKind::Voice => "voice-lock",
        ChannelKind::Announcement => "announcement-lock",
        ChannelKind::Forum => "forum-lock",
        ChannelKind::Stage => "stage-lock",
        ChannelKind::Category => return None,
    })
}

/// Channel glyph with Discord's small corner badges (lock for private
/// channels, "!" for age-restricted ones).
/// `hover`: row group whose hover turns the glyph `--interactive-text-hover` (live
/// ref-cache/voice-channel-row-hover.json: `icon_TNyK-N` oklab(.895) like the name).
fn decorated_icon(kind: ChannelKind, color: gpui::Hsla, locked: bool, nsfw: bool, bg: gpui::Hsla, hover: Option<SharedString>) -> AnyElement {
    let hov = move |g: gpui::Svg| match hover.clone() {
        Some(h) => g.group_hover(h, |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))),
        None => g,
    };
    div()
        .relative()
        .size(z(20.))
        .flex_none()
        // Vanilla icon picker: NSFW wins over lock; locked channels swap to the
        // dedicated *LockIcon glyph (live aria "Salon privé (verrouillé)").
        .map(|d| match lock_icon_name(kind).filter(|_| locked && !nsfw) {
            Some(name) => d.child(hov(gpui::svg().path(crate::icons::path(name)).size(z(20.)).flex_none().text_color(color))),
            None => d.child(hov(icon(channel_icon(kind), 20., color))),
        })
        .when(nsfw, |d| {
            d.child(
                div()
                    .absolute()
                    .right(z(-3.))
                    .top(z(-3.))
                    .size(z(11.))
                    .rounded_full()
                    .bg(bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(9.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child("!"),
            )
        })
        .into_any_element()
}

/// "00 │ 06" user-limit pill of a limited voice channel (`wrapper_QQRrAh`).
fn user_limit_pill(users: u32, limit: u32) -> AnyElement {
    let cell = |txt: String| {
        div()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(12.))
            .line_height(z(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
            .child(txt)
    };
    let slant = gpui::canvas(
        |_, _, _| {},
        |b, _, window, _| {
            // total_:after: border-top 20px transparent + border-inline-end 6px
            // --background-mod-strong → a right triangle left of `total_`.
            let (x, y, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.height));
            let w = f32::from(b.size.width);
            let mut pb = gpui::PathBuilder::fill();
            pb.move_to(gpui::point(gpui::px(x + w), gpui::px(y)));
            pb.line_to(gpui::point(gpui::px(x + w), gpui::px(y + h)));
            pb.line_to(gpui::point(gpui::px(x), gpui::px(y + h)));
            pb.close();
            if let Ok(p) = pb.build() {
                window.paint_path(p, t(idx::BACKGROUND_MOD_STRONG));
            }
        },
    )
    .absolute()
    .top_0()
    .left(z(-6.))
    .w(z(6.))
    .h_full();
    div()
        .flex_none()
        .ml(z(12.))
        .pr(z(4.))
        .child(
            div()
                .h(z(20.))
                .flex()
                .overflow_hidden()
                .rounded(z(crate::theme::generated::global::RADIUS_MD))
                .border_1()
                .border_color(t(idx::BORDER_NORMAL))
                .bg(t(idx::BACKGROUND_BASE_LOW))
                // `users_QQRrAh` width 28; `.extraLong_` (3+ digits) width auto.
                .child(cell(format!("{users:02}")).when(users < 100 && limit < 100, |d| d.w(z(28.))).px(z(4.)))
                .child(
                    cell(format!("{limit:02}"))
                        .relative()
                        .min_w(z(20.))
                        .pl(z(2.))
                        .pr(z(4.))
                        // GPUI's overflow_hidden doesn't clip to the parent's radius.
                        .rounded_r(z(crate::theme::generated::global::RADIUS_MD - 1.))
                        .bg(t(idx::BACKGROUND_MOD_STRONG))
                        .child(slant),
                ),
        )
        .into_any_element()
}

fn channel_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id) -> AnyElement {
    let Some(ch) = app.channels.get(&id) else { return div().into_any_element() };
    let selected = app.selected_channel == Some(id);
    let is_dm = ch.guild.is_none();
    let connected = !is_dm && app.voice_channel == Some(id);
    let extra = app.nav.extras.get(&id);
    let is_group = extra.is_some_and(|e| e.recipients > 1 || e.group_icon.is_some());
    let unread = ch.unread && !ch.muted;
    let guild_dim = !is_dm && ch.muted && !selected;
    // `--channels-default` at rest, `--interactive-text-active` when selected
    // or unread, `--interactive-muted` when muted.
    // `name_TNyK-N`: `--channels-default`; `modeSelected`/`modeUnreadImportant`:
    // `--interactive-text-active`; `modeMuted`: `--text-muted` at 50 % opacity.
    // `modeConnected_TNyK-N .name_`: `--interactive-text-active` too.
    let fg = if (selected || connected) && !is_dm {
        // `.modeConnected_/.modeSelected_ .name_TNyK-N {color: var(--text-strong)}`
        tok(idx::TEXT_STRONG)
    } else if selected || connected || (ch.unread && !ch.muted) {
        tok(idx::INTERACTIVE_TEXT_ACTIVE)
    } else if ch.muted {
        tok(idx::TEXT_MUTED)
    } else {
        tok(idx::CHANNELS_DEFAULT)
    };
    let e = entity.clone();
    let lead: AnyElement = if is_dm && is_group {
        let ex = extra.cloned().unwrap_or_default();
        match ex.group_icon {
            Some(crate::model::ImageRef::Url(u)) => {
                gpui::img(u).size(z(32.)).rounded_full().flex_none().into_any_element()
            }
            // vanilla-group-dm #800: generated diagonal face pile, 32px.
            _ if !ex.members.is_empty() => crate::ui::group_dm::group_avatar(
                app,
                id,
                32. * crate::theme::zoom(),
                if selected { t(idx::INTERACTIVE_BACKGROUND_SELECTED) } else { c(Palette::BG_TERTIARY) },
            ),
            _ => div()
                .size(z(32.))
                .flex_none()
                .rounded_full()
                .bg(c(crate::theme::DEFAULT_AVATAR[(id.0 % 5) as usize]))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(Icon::Members, 18., gpui::white()))
                .into_any_element(),
        }
    } else if is_dm {
        let u = ch.voice_users.first().and_then(|u| app.users.get(u));
        let p = ch.voice_users.first().map(|u| app.presence_of(*u)).unwrap_or_default();
        // the status cut shows the row's hover / selected background like the mask
        let g = SharedString::from(format!("chrow{}", id.0));
        let deco = crate::ui::avatar_deco::source(u, None);
        crate::ui::avatar_with_status_deco(
            u,
            32.,
            p,
            c(Palette::BG_TERTIARY),
            move |d| {
                if selected {
                    d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED))
                } else {
                    d.group_hover(g, |s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)))
                }
            },
            deco,
        )
        .into_any_element()
    } else {
        // `icon_TNyK-N` `--channel-icon`; muted `--icon-muted`; unread `--icon-subtle`.
        // `iconLive_Wx6SnQ`: the channel we're connected to tints its icon
        // --text-feedback-positive (live: oklab(.633 -.114 .059), same as the timer).
        let col = t(if app.voice_channel == Some(id) {
            idx::TEXT_FEEDBACK_POSITIVE
        } else if selected {
            // `.modeConnected_/.modeSelected_ .icon_TNyK-N {color: var(--icon-strong)}`
            // (f4b2f93f8f92b799.css, after the `--channel-icon` rule, so it wins).
            idx::ICON_STRONG
        } else if ch.muted {
            idx::ICON_MUTED
        } else if unread {
            idx::ICON_SUBTLE
        } else {
            idx::CHANNEL_ICON
        });
        decorated_icon(
            ch.kind,
            col,
            extra.is_some_and(|e| e.locked),
            extra.is_some_and(|e| e.nsfw),
            if selected { t(idx::INTERACTIVE_BACKGROUND_SELECTED) } else { c(Palette::BG_TERTIARY) },
            (!selected && app.voice_channel != Some(id)).then(|| SharedString::from(format!("chrow{}", id.0))),
        )
    };
    let name: SharedString = extra.and_then(|e| e.group_name.clone()).filter(|_| is_dm).unwrap_or_else(|| ch.name.clone());
    let members: Option<SharedString> =
        (is_dm && is_group).then(|| crate::t!("invite.members", n = extra.map(|e| e.recipients + 1).unwrap_or(1)));
    let dm_tag = (is_dm && !is_group)
        .then(|| ch.voice_users.first().and_then(|u| app.users.get(u)).and_then(|u| u.tag.clone()))
        .flatten();
    let group = SharedString::from(format!("chrow{}", id.0));
    // `iconsContainer_zln_tS` (vanilla-home-friends #680/#695): padding-inline-start 4,
    // min-width 32 when nameplated. `closeButton_zln_tS` is display:none until the row is
    // hovered, then 24px (padding 3, margin-right 6), opacity .7 (1 on its own hover or
    // when plated); `innerCloseButton_` padding 1, radius 50%, 16px `closeIcon_`.
    // Plated: `--custom-nameplate-neutral` / `--custom-nameplate-neutral-hovered`
    // (web bundle module 351952 `e3c95a79030889c3.js`: dark rgba(0,0,0,.22/.33),
    // light rgba(255,255,255,.22/.33)) and a white .9 glyph (#681..#683).
    let plated = is_dm
        && !is_group
        && ch.voice_users.first().and_then(|u| app.users.get(u)).is_some_and(|u| u.nameplate.is_some());
    let close = is_dm.then(|| {
        let e = entity.clone();
        let fg = if plated { gpui::hsla(0., 0., 1., 0.9) } else { c(fg) };
        let (plate_bg, plate_hover) = if crate::theme::is_light() {
            (gpui::hsla(0., 0., 1., 0.22), gpui::hsla(0., 0., 1., 0.33))
        } else {
            (gpui::hsla(0., 0., 0., 0.22), gpui::hsla(0., 0., 0., 0.33))
        };
        div()
            .flex()
            .flex_none()
            .justify_end()
            .items_center()
            .pl(z(4.))
            .when(plated, |d| d.min_w(z(32.)))
            .child(
                div()
                    .id(("dm-close", id.0))
                    .hidden()
                    .group_hover(group.clone(), |s| s.flex())
                    .size(z(24.))
                    .p(z(3.))
                    .mr(z(6.))
                    .when(!plated, |d| d.opacity(0.7).hover(|s| s.opacity(1.)))
                    // aria-label "Close DM" / "Leave Group" (#681 / #806)
                    .tip(if is_group { crate::t!("menu.leave_group") } else { crate::t!("menu.close_dm") })
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        e.update(cx, |a, cx| a.close_dm(id, cx));
                    })
                    .child(
                        div()
                            .p(z(1.))
                            .rounded_full()
                            .when(plated, |d| d.bg(plate_bg).hover(|s| s.bg(plate_hover)))
                            .child(icon(Icon::Close, 16., fg)),
                    ),
            )
    });
    // `children_TNyK-N` (vanilla-guild-text #691): margin -4 -4 -4 0, then 24px
    // `iconItem_Wx6SnQ` boxes (padding 4, 16px `actionIcon`): "Invite" and, with
    // Manage Channels, "Edit Channel". Shown on hover and on the selected row.
    let can_edit = ch.guild.is_some_and(|g| {
        crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 4) // MANAGE_CHANNELS
    });
    let action = |i: Icon, path: Option<&'static str>| {
        // Live CDP: on the selected channel row, action icons (invite, edit) use --interactive-text-active
        // (~rgb(220,220,223)) rather than --interactive-text-default (~rgb(150,151,158)).
        let col = t(if selected { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT });
        // iconItem_Wx6SnQ:hover .actionIcon_Wx6SnQ color --interactive-text-hover
        let hov = |s: gpui::StyleRefinement| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER));
        // actionIcon_Wx6SnQ:active color --interactive-text-active, translateY(.5px)
        let act = |s: gpui::StyleRefinement| s.text_color(t(idx::INTERACTIVE_TEXT_ACTIVE)).mt(z(0.5));
        div().id(gpui::ElementId::Name(format!("ch-act-{}", path.unwrap_or("gear")).into())).size(z(24.)).p(z(4.)).flex_none().group("ch-action").child(match path {
            Some(p) => gpui::svg().path(crate::icons::path(p)).size(z(16.)).text_color(col).group_hover("ch-action", hov).id("g").group_active("ch-action", act).into_any_element(),
            None => icon(i, 16., col).group_hover("ch-action", hov).id("g").group_active("ch-action", act).into_any_element(),
        })
    };
    let is_voice = matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage);
    // Voice rows (vanilla-voice-sidebar #885/#889): "Ouvrir la discussion" (ChatIcon)
    // then "Inviter dans le salon vocal", same 24px `iconItem_` boxes.
    let chat_entity = entity.clone();
    let invite_hover = (!is_dm).then(|| {
        div()
            .flex()
            .flex_none()
            .my(z(-4.))
            .mr(z(-4.))
            // Hidden icons take no room (live: the voice timer / name keep the full width).
            .when(!selected, |d| d.hidden().group_hover(group.clone(), |s| s.flex()))
            .when(is_voice, |d| {
                d.child(
                    div()
                        .id(("vc-chat", id.0))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            chat_entity.update(cx, |a, cx| a.open_channel_chat(id, cx));
                        })
                        .tooltip(crate::ui::tooltip::tip(crate::t!("channel.open_chat")))
                        .child(action(Icon::Members, Some("chat"))),
                )
            })
            .child(action(Icon::Members, Some("invite")).tooltip(crate::ui::tooltip::tip(crate::t!("perm.create_invite"))))
            .when(can_edit, |d| d.child(action(Icon::Gear, None).tooltip(crate::ui::tooltip::tip(crate::t!("menu.edit_channel")))))
    });
    // `channelInfo_Wx6SnQ` (vanilla-voice-sidebar #1003..#1006): margin-inline-start 12,
    // padding-inline-end --space-4; `tabularNumbers_` `body-xs/normal` `fontCode_` 12/16,
    // `--text-feedback-positive`: time since the channel's `voice_start_time`.
    let timer = (matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) && !ch.voice_users.is_empty())
        .then(|| crate::voice::start_time::get(id.0))
        .flatten()
        .map(|st| {
            div()
                .flex_none()
                .ml(z(12.))
                .pr(z(4.))
                .font_family(crate::theme::FONT_MONO())
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(t(idx::TEXT_FEEDBACK_POSITIVE))
                .child(crate::voice::start_time::format(crate::voice::start_time::now_s() - st))
        });
    // `wrapper_QQRrAh` user-limit pill (live "Travaux Groupe #1": "00 │ 06"), shown on
    // limited voice channels in place of the timer: h 20, 1px --border-normal,
    // --radius-md, --background-base-low; `users_` 28px padding 0 --space-4,
    // `total_` 20px padding-inline 2 4 --background-mod-strong with a 6px slanted
    // left edge (`total_:after` border triangle); body-xs/medium 12/16 tabular.
    let limit_pill = (timer.is_none() && ch.kind == ChannelKind::Voice)
        .then(|| app.backend.channel_user_limit(id))
        .flatten()
        .map(|limit| user_limit_pill(ch.voice_users.len() as u32, limit));
    div()
        .id(("ch", id.0))
        .relative()
        // `wrapper_TNyK-N` margin-inline-start `--space-xs` (10px cozy, dump -channels-N-N+
        // #628); DM `channel_zln_tS` margin 8 (dump -channels-me+ #656). Right: scrollbar gutter.
        .pl(z(if is_dm { 8. } else { 10. }))
        .pr(z(8.))
        .py(z(1.))
        .child(
            // `unread_TNyK-N`: 4x8, `0 --radius-xs --radius-xs 0`, inset
            // `-(--space-xs) + 1px` from the link, vertically centred.
            div()
                .absolute()
                .left(z(1.))
                .top(z(if is_dm { 21. } else { 15. }))
                .w(z(4.))
                .h(z(8.))
                .rounded_r(z(4.))
                .when(unread && !selected, |d| d.bg(t(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(
            div()
                .id(("chrow", id.0))
                .group(group)
                // `link_TNyK-N` 36px, padding 4/10 (dump #629); DM `link_zln_tS` 48px, pl 10 (me #658).
                // Connected voice row: `link_` 52px (linkTop 28 + linkBottom 16 + padding 4/4).
                .h(z(if is_dm { 48. } else if connected { space::channels_name_lh() + 24. } else { space::channels_name_lh() + 8. }))
                .pl(z(10.))
                .pr(z(if is_dm { 0. } else { 10. })) // DM `link_zln_tS` padding 0 0 0 10 (#663)
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .gap(z(if is_dm { 12. } else { 10. })) // linkTop_TNyK-N gap --space-xs (10 cozy); DM avatar margin 12
                .cursor_pointer()
                .text_color(c(fg))
                .when(is_dm && ch.muted && !selected, |d| d.opacity(0.5))
                .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!selected, |d| {
                    d.hover(|s| {
                        let s = s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER));
                        if is_dm { s.opacity(1.) } else { s }
                    })
                    // wrapper_TNyK-N:active .link_TNyK-N background
                    .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                })
                // Hover warms the message cache so the click paints at once (docs/perf.md).
                .on_hover({
                    let e = e.clone();
                    move |hovered: &bool, _, cx| {
                        if *hovered {
                            e.read(cx).backend.prefetch_messages(id);
                        }
                    }
                })
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_channel(id, cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Channel(id), ev.position, w)
                })
                // DM `interactive_zln_tS` > `container_AFi54G` (vanilla-home-friends #661):
                // nameplate of the 1:1 recipient behind the row, `.channel` opacity.
                .relative()
                .when_some(
                    (is_dm && !is_group)
                        .then(|| ch.voice_users.first().and_then(|u| app.users.get(u)))
                        .flatten()
                        // Only the selected row and the hovered one draw it (vanilla-home-friends:
                        // no `container_AFi54G` under idle rows); hovered `.channel_.hover_` opacity .4.
                        // Mask start (module 351952, `content: ti` = the `layout_TBNqol` ref):
                        // its width - 5, i.e. avatar 32 + margin 12 + name/tag width - 5.
                        .and_then(|u| {
                            let fade = crate::ui::nameplate::Fade::Measured {
                                place: crate::ui::nameplate::Place::Channel,
                                id: id.0,
                                offset: 32. + 12. - 5.,
                                fallback: 220.,
                            };
                            crate::ui::nameplate::layer_with_fade(u, 48., crate::ui::nameplate::Place::Channel, !selected, selected, fade)
                        }),
                    |d, l| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .when(!selected, |w| w.opacity(0.).group_hover(SharedString::from(format!("chrow{}", id.0)), |s| s.opacity(1.)))
                                .child(l),
                        )
                    },
                )
                .child(
                    // `.modeMuted_TNyK-N .icon_/.name_ {opacity:.5}`: only the icon and the name
                    // dim (also on hover); the row background stays opaque.
                    if guild_dim {
                        div().flex_none().opacity(0.5).child(top_line(lead, connected)).into_any_element()
                    } else {
                        top_line(lead, connected)
                    },
                )
                .child(
                    div()
                        .when(guild_dim, |d| d.opacity(0.5))
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(z(16.))
                                .line_height(z(if is_dm { 20. } else { space::channels_name_lh() })) // --channels-name-line-height
                                .font_weight(FontWeight::MEDIUM) // --font-weight-semibold-1x-light-theme (500)
                                .flex()
                                .items_center()
                                .child(
                                    // Name + tag, as wide as their content: the DM nameplate
                                    // mask reads this width (`content_TBNqol`).
                                    div()
                                        .relative()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .child(
                                            // 1:1 DM: `withDisplayNameStyles_zln_tS` > `container_or4Hj- dnsFont_`
                                            // (vanilla-dm #675/#734): the recipient's name style font.
                                            crate::fonts::styled_name(
                                                div().min_w_0().text_ellipsis(),
                                                (is_dm && !is_group).then(|| ch.voice_users.first().and_then(|u| app.users.get(u))).flatten(),
                                                false,
                                            )
                                            .child(crate::ui::emoji_text::label(name, 16.)),
                                        )
                                        // 1:1 DM: server tag chiplet after the name, chipletContainerInner_ margin-left 4px
                                        // (vanilla-dm #677).
                                        .when_some(dm_tag, |d, t| d.child(crate::ui::members::server_tag(&t).ml(z(4.))))
                                        .when(plated, |d| {
                                            d.child(crate::ui::nameplate::width_probe(crate::ui::nameplate::Place::Channel, id.0))
                                        }),
                                ),
                        )
                        .when(connected, |d| {
                            // `linkBottom_TNyK-N` (live connected voice row): margin-left 30 from
                            // the icon (so under the name), `subtitle_` body-xs/medium 12/16
                            // `--channels-default` "Set a channel status" + 12px pencil, margin 4.
                            d.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .text_size(z(12.))
                                    .line_height(z(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t(idx::CHANNELS_DEFAULT))
                                    .child(div().min_w_0().text_ellipsis().child(crate::t!("voice.set_status")))
                                    .child(
                                        gpui::svg()
                                            .path(crate::icons::path("pen"))
                                            .ml(z(4.))
                                            .size(z(12.))
                                            .flex_none()
                                            .text_color(t(idx::CHANNELS_DEFAULT)),
                                    ),
                            )
                        })
                        .when_some(members, |d, m| {
                            d.child(
                                div()
                                    .text_size(z(12.))
                                    .line_height(z(14.))
                                    .text_color(c(Palette::TEXT_MUTED))
                                    .child(m),
                            )
                        }),
                )
                .when(ch.mentions > 0, |d| d.child(badge(ch.mentions).bg(c(Palette::NOTIFICATION))))
                .when(ch.mentions == 0, |d| d.children(invite_hover.map(|h| top_line(h, connected))))
                .children(timer.map(|t| top_line(t, connected)))
                .children(limit_pill)
                .children(close)
                // Tab focus ring (3d994642c869c81b.js FocusRing offset:{top:2,bottom:2,right:4}).
                .map(|d| crate::ui::focus_ring::ring_inset_edges(d, crate::ui::states::key(("chrow", id.0)), ROW_RADIUS, 0., 2., 4., 2.)),
        )
        .into_any_element()
}

/// Connected voice row: `linkTop_TNyK-N` (28px, 4px below the link's top) holds the
/// icon, name, hover actions and timer, with the status line under it.
fn top_line(e: impl IntoElement, connected: bool) -> AnyElement {
    if connected {
        div().self_start().mt(z(4.)).h(z(space::channels_name_lh())).flex().items_center().flex_none().child(e).into_any_element()
    } else {
        e.into_any_element()
    }
}

/// Thread sub-row: curved connector from the parent channel, then the name.
fn thread_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id) -> AnyElement {
    let Some(th) = app.nav.find_thread(id) else { return div().into_any_element() };
    let selected = app.selected_channel == Some(id);
    let live = app.channels.get(&id);
    let unread = live.is_some_and(|c| c.unread);
    let mentions = live.map(|c| c.mentions).unwrap_or(0);
    let e = entity.clone();
    div()
        .id(("thread", id.0))
        .relative()
        .pl(z(space::xs())) // --space-xs
        .pr(z(8.))
        .py(z(1.)) // `wrapper_TNyK-N` padding 1px 0 (live thread row 38px)
        .child(
            // `unread_TNyK-N`: 4x8 pill on the left edge when the thread has unread messages.
            div()
                .absolute()
                .left(z(1.))
                .top(z(15.))
                .w(z(4.))
                .h(z(8.))
                .rounded_r(z(4.))
                .when(unread && !selected, |d| d.bg(t(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(
            div()
                // `spine_reDhww`: `--channels-spine-offset-left`, top 0, `--spine-default`;
                // live box 10x20 at the row's top, curving into the row's middle.
                .absolute()
                .left(z(space::spine_left())) // --channels-spine-offset-left
                .top_0()
                .w(z(10.))
                .h(z(19.))
                .border_l_1()
                .border_b_1()
                .rounded_bl(z(8.))
                .border_color(t(idx::SPINE_DEFAULT)),
        )
        .child(
            div()
                .id(("threadrow", id.0))
                // `typeThread_TNyK-N.wrapper`: margin `28px + --space-xs` from the list
                // (8px of it is the row inset); link padding 4px / `--space-xs`.
                .ml(z(space::spine_left()))
                .h(z(space::channels_name_lh() + 8.))
                .px(z(10.))
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(z(14.)) // `typeThread_ .name_` body-sm/medium (live 14px)
                .line_height(z(space::channels_name_lh())) // --channels-name-line-height
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(if selected || unread { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNELS_DEFAULT }))
                .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!selected, |d| {
                    d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE))) // wrapper_TNyK-N:active
                })
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_thread(id, cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Thread(id), ev.position, w)
                })
                .child(div().flex_1().min_w_0().text_ellipsis().child(crate::ui::emoji_text::label(th.name.clone(), 14.)))
                .when(mentions > 0, |d| d.child(badge(mentions).bg(c(Palette::NOTIFICATION)))),
        )
        .into_any_element()
}

/// The bottom-left area under both the guild rail and the channel list (as in
/// the refreshed client): the rail and sidebar backgrounds continue behind a
/// floating panels card (`--background-base-low`, 1px border, 8px radius, 8px
/// from the edges) holding the voice panel (when connected) above the user row.
pub fn panels(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .flex_none()
        // Rail + channel list width, so a long custom status ellipsizes instead
        // of widening the card past the sidebar.
        .w(z(m::SERVER_BAR_W + crate::ui::sidebar_resize::width()))
        // The live scrollers (`sidebarList_`, `guilds_`) run down to the card's top edge,
        // behind its 10px margin: overlap them by that margin and leave it transparent.
        .mt(z(-10.))
        .relative()
        .child(div().absolute().top(z(10.)).bottom_0().left_0().w(z(m::server_bar_w())).bg(c(Palette::BG_TERTIARY)))
        .child(
            div()
                .absolute()
                .top(z(10.))
                .bottom_0()
                .right_0()
                .left(z(m::server_bar_w()))
                .border_l_1()
                .border_color(c(Palette::APP_FRAME_BORDER))
                .bg(c(Palette::BG_SECONDARY)),
        )
        .child(panels_card(app, cx))
}

fn panels_card(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .flex_none()
        .m(z(10.)) // dump -channels-N-N+ #976 `panels_pog62O` at x=10 / bottom 10 (cozy --space-xs)
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_MUTED)) // panels_pog62O
        .bg(t(idx::BACKGROUND_BASE_LOW))
        .overflow_hidden()
        .flex()
        .flex_col()
        .when(app.voice_channel.is_some(), |d| d.child(crate::ui::voice_panel::voice_panel(app, cx)))
        .child(user_panel(app, cx))
}

fn user_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    use crate::ui::voice_panel::{Audio, audio_button, panel_button, panel_glyph};
    let status = match app.presence {
        crate::model::Presence::Online => crate::t!("status.online"),
        crate::model::Presence::Idle => crate::t!("status.idle"),
        crate::model::Presence::Dnd => crate::t!("status.dnd"),
        crate::model::Presence::Offline => crate::t!("status.invisible"),
    };
    let muted = app.mic_muted || app.deafened;
    // `container_g6VwXp` (dump -channels-N-N+ #979): 56px, padding 14px, gap
    // `--space-xs` (10px cozy).
    div()
        .relative()
        .overflow_hidden()
        .h(z(m::USER_PANEL_H))
        .flex_none()
        .px(z(14.))
        .flex()
        .items_center()
        .gap(z(10.))
        .when_some(
            crate::ui::nameplate::layer(&app.me, m::USER_PANEL_H, crate::ui::nameplate::Place::Account, false, false),
            |d, l| d.child(l),
        )
        .child(
            // `accountPopoutButtonWrapper_g6VwXp`: -4px inline-start margin,
            // 5px 4px padding, `--radius-xs`, 8px gap, hover `--background-mod-normal`.
            div()
                .id("me")
                .flex_1()
                .min_w(z(112.))
                .ml(z(-6.)) // dump #980: margin-inline-start -6, padding 5/6
                .py(z(5.))
                .px(z(6.))
                .flex()
                .items_center()
                .gap(z(8.))
                .rounded(z(4.))
                .cursor_pointer()
                .group("user-panel-avatar")
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .on_click(cx.listener(|a, _, w, cx| a.toggle_status_picker(w, cx)))
                // The decoration plays while the panel is hovered (ui::avatar_deco).
                .on_hover(cx.listener(|_, h: &bool, _, cx| {
                    if crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(3, 0), *h) {
                        cx.notify();
                    }
                }))
                .overflow_hidden()
                .child(crate::ui::avatar_with_status_deco(
                    Some(&app.me),
                    32.,
                    app.presence,
                    c(Palette::BG_SECONDARY_ALT),
                    |d| d.group_hover("user-panel-avatar", |s| s.bg(t(idx::BACKGROUND_MOD_NORMAL))),
                    crate::ui::avatar_deco::source(Some(&app.me), Some(crate::ui::avatar_deco::hover_key(3, 0))),
                ))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .mt(z(-1.)) // panelTitleContainer_g6VwXp
                                .mb(z(2.))
                                // dump #985 `body-md/medium` 16/22, `--text-strong`
                                .text_size(z(16.))
                                .line_height(z(22.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t(idx::TEXT_STRONG))
                                .text_ellipsis()
                                // vanilla-guild-text #997 `container_or4Hj- dnsFont_`: own name style font.
                                .child(
                                    crate::fonts::styled_name(div().text_ellipsis(), Some(&app.me), false)
                                        .child(crate::ui::emoji_text::label(app.me.display_name.clone(), 14.)),
                                ),
                        )
                        .child(
                            div()
                                // dump #989 `body-xs/medium` 12/13, `--interactive-text-default`
                                .text_size(z(12.))
                                .line_height(z(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
                                .text_ellipsis()
                                .child(crate::ui::emoji_text::status_label(
                                    // vanilla-guild-text user panel: the custom status replaces "Online".
                                    // Emoji + text from the own custom-status activity while it matches.
                                    {
                                        let act = app.people.activities.get(&app.me.id).and_then(|v| {
                                            v.iter().find(|a| a.kind == crate::presence::ActivityKind::Custom)
                                        });
                                        match (&app.people.custom_status, act) {
                                            (Some(c), Some(a)) if a.state.as_ref() == Some(c) => a.headline(),
                                            (Some(c), _) => c.clone(),
                                            (None, Some(a)) if app.people.custom_status.is_none() && a.state.is_none() => a.headline(),
                                            _ => status,
                                        }
                                    },
                                    14. * crate::theme::zoom() / 1.375,
                                )),
                        ),
                ),
        )
        .child(
            // `buttons_g6VwXp`: `--space-8` gap.
            div()
                .flex()
                .flex_none()
                .gap(z(8.))
                .child(audio_button(
                    Audio::Mic,
                    if muted { Icon::MicOff } else { Icon::Mic },
                    muted,
                    if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                    |a, cx| {
                        a.mic_muted = !a.mic_muted;
                        crate::voice::sync_self_state(a);
                        cx.notify();
                    },
                    cx,
                ))
                .child(audio_button(
                    Audio::Deafen,
                    if app.deafened { Icon::HeadphonesOff } else { Icon::Headphones },
                    app.deafened,
                    if app.deafened { crate::t!("voice.undeafen") } else { crate::t!("voice.deafen") },
                    |a, cx| {
                        a.deafened = !a.deafened;
                        crate::voice::sync_self_state(a);
                        cx.notify();
                    },
                    cx,
                ))
                .child(
                    panel_button("settings", panel_glyph(Icon::Gear, false), false)
                        .tip(crate::t!("settings.user_settings"))
                        .on_click(cx.listener(|a, _, _, cx| crate::ui::settings::open(a, cx))),
                ),
        )
}

/// `maxBoostCount` of `GuildPowerupsProgressBarUI` (client module 616740 `tg`): the
/// tier-3 threshold `M2T[TIER_3]` = 14 (0 when the guild has `PREMIUM_TIER_3_OVERRIDE`)
/// plus `boostPrice` of every power-up with no `includedInLevel` that is enabled
/// (constants module 568065 `sy`/`YV`, static, not fetched): GUILD_TAGS 3,
/// ENHANCED_ROLE_COLORS 3, badge packs PETS 3 / FLEX 5 / PLANT 3 / CREEPY_CRAWLIES 2,
/// GAME_SERVERS 3 (experiment, on for the user); GUILD_THEME is disabled.
/// Live check: vanilla-guild-text #661 "18/36 Boosts" = 14 + 22.
fn boost_goal_max(features: &[String]) -> u32 {
    const TIER_3: u32 = 14;
    const PERKS: u32 = 3 + 3 + 3 + 5 + 3 + 2 + 3;
    let base = if features.iter().any(|f| f == "PREMIUM_TIER_3_OVERRIDE") { 0 } else { TIER_3 };
    base + PERKS
}

/// Boost goal bar (`GUILD_PREMIUM_PROGRESS_BAR` action row, vanilla-guild-text
/// #653..#662), shown when `premiumProgressBarEnabled && premiumSubscriberCount > 0`.
fn boost_goal(count: u32, enabled: bool, max: u32) -> Option<gpui::AnyElement> {
    if !enabled || count == 0 {
        return None;
    }
    let pct = (count as f32 / max as f32).min(1.);
    let pink = t(idx::GUILD_BOOSTING_PINK_REFRESH);
    let text = |s: SharedString| {
        // `text_BFMMcK` `body-xs/semibold` 12/16 600 `--text-default`, ellipsis.
        div()
            .text_size(z(12.))
            .line_height(z(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t(idx::TEXT_DEFAULT))
            .text_ellipsis()
            .child(s)
    };
    let count_text: SharedString = if count >= max {
        crate::t!("guild.boost_goal.count_done", n = count)
    } else {
        crate::t!("guild.boost_goal.count", n = count, max = max)
    };
    Some(
        // `container_BFMMcK` + `containerWithMargin_`: margin 0 0 12 8, padding
        // --space-xxs --space-xxs --space-sm (4 4 16), border-bottom 1px --border-subtle.
        // Right: the scroller's 8px stable gutter (live box 88..407 in a 416px list).
        div().w_full().pl(z(8.)).pr(z(8.)).mb(z(12.)).child(
        div()
            .id("boost-goal")
            .group("boost-goal")
            .w_full()
            .pt(z(4.))
            .px(z(4.))
            .pb(z(16.))
            .border_b_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .cursor_pointer()
            .child(
                // `contentContainer_` > `progressContainer_`: 26px, --radius-round,
                // `--background-gradient-highest, --background-base-low`.
                div()
                    .relative()
                    .h(z(26.))
                    .rounded_full()
                    .bg(t(idx::BACKGROUND_BASE_LOW))
                    .child(
                        // `progress_BFMMcK`: absolute inset 2px, width `calc(d% - 4px)`,
                        // linear-gradient(270deg, pink 40% → pink 10%), --shadow-low +
                        // inset 0 0 4.6px pink 20%.
                        div().absolute().top_0().bottom_0().left_0().w(gpui::relative(pct)).p(z(2.)).child(
                        div()
                            .size_full()
                            .rounded_full()
                            .bg(gpui::linear_gradient(
                                270.,
                                gpui::linear_color_stop(pink.opacity(0.4), 0.),
                                gpui::linear_color_stop(pink.opacity(0.1), 1.),
                            ))
                            .group_hover("boost-goal", |s| {
                                // `container_:hover .progress_`: 70 % → 25 %.
                                s.bg(gpui::linear_gradient(
                                    270.,
                                    gpui::linear_color_stop(pink.opacity(0.7), 0.),
                                    gpui::linear_color_stop(pink.opacity(0.25), 1.),
                                ))
                            })
                            .shadow(crate::ui::inbox::low_shadow()),
                        ),
                    )
                    .child(
                        // `textContainer_`: absolute, inset 0 --space-8, row,
                        // space-between, gap --space-8; `textContentContainer_` gap 2.
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(z(8.))
                            .right(z(8.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(z(8.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(2.))
                                    .min_w_0()
                                    .child(text(crate::t!("guild.boost_goal.title")))
                                    .when(count >= max, |d| d.child(text("\u{1F389}".into()))),
                            )
                            .child(
                                // `boostCountText_`: opacity .7, 1 on container hover.
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(2.))
                                    .flex_none()
                                    .opacity(0.7)
                                    .group_hover("boost-goal", |s| s.opacity(1.))
                                    .child(text(count_text))
                                    .child(
                                        gpui::svg()
                                            .path(crate::icons::path("chevron-right"))
                                            .size(z(12.))
                                            .flex_none()
                                            .text_color(t(idx::TEXT_DEFAULT)),
                                    ),
                            ),
                    ),
            ),
        )
        .into_any_element(),
    )
}

/// Rows above the first channel of a guild (vanilla-guild-text #663..#680): the
/// same `wrapper_TNyK-N`/`link_TNyK-N` rows as channels (36px, padding 4 10, gap 10,
/// 20px `icon_TNyK-N`, 16/28 medium `--channels-default`) for "Events", "Members"
/// (with moderation rights) and "Server Boosts", then `sectionDivider_ysO7Gy`
/// (1px `--border-subtle`, inset 20 / 11) and a 12px gap.
fn guild_top(app: &PhoenixApp, entity: Entity<PhoenixApp>, cx: &App) -> impl IntoElement {
    let ev_entity = entity.clone();
    let perms = app.selected_guild.map(|g| app.backend.guild_permissions(g)).unwrap_or(0);
    let mods = crate::ui::inbox::has_perm(perms, (1 << 1) | (1 << 2) | (1 << 5)); // KICK | BAN | MANAGE_GUILD
    let features = app.selected_guild.map(|g| app.backend.guild_features(g)).unwrap_or_default();
    let has = |f: &str| features.iter().any(|x| x == f);
    let browse: Option<SharedString> = if has("GUILD_ONBOARDING") {
        Some(crate::t!("guild.top.channels_roles"))
    } else if has("COMMUNITY") {
        Some(crate::t!("guild.top.browse"))
    } else {
        None
    };
    let guide = has("COMMUNITY") && crate::ui::guide::has_guide(&features);
    let (guide_on, page_on) = (crate::ui::guide::is_open(app, cx), crate::ui::community::is_open(app, cx));
    // vanilla-server-guide #644: modeSelected_ row = --interactive-background-selected,
    // name and icon --interactive-text-active.
    let item = |id: &'static str, glyph: &'static str, label: SharedString, on: bool| {
        div().pl(z(10.)).pr(z(8.)).py(z(1.)).child(
            div()
                .id(id)
                .h(z(36.))
                .px(z(10.))
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .gap(z(10.))
                .cursor_pointer()
                .text_color(t(if on { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNELS_DEFAULT }))
                .when(on, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!on, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER))))
                .child(gpui::svg().path(crate::icons::path(glyph)).size(z(20.)).flex_none().text_color(t(if on { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNEL_ICON })))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(z(16.))
                        .line_height(z(28.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .child(label),
                ),
        )
    };
    div()
        .w_full()
        // vanilla-inbox-0 #623: 12px spacer div above the first row.
        .pt(z(12.))
        .flex()
        .flex_col()
        .children(app.selected_guild.and_then(|g| {
            let (n, on) = app.backend.guild_boosts(g)?;
            boost_goal(n, on, boost_goal_max(&app.backend.guild_features(g)))
        }))
        // vanilla-server-guide #643: "Server Guide" (SignPostIcon) first.
        .when(guide, |d| {
            let ent = entity.clone();
            d.child(
                div()
                    .id("g-guide-open")
                    .on_click(move |_, _, cx| {
                        ent.update(cx, |a, cx| {
                            if let Some(g) = a.selected_guild {
                                crate::ui::guide::open(a, g, cx);
                            }
                        })
                    })
                    .child(item("g-guide", "signpost", crate::t!("guild.top.guide"), guide_on)),
            )
        })
        .child(
            div()
                .id("g-events-open")
                .on_click(move |_, _, cx| {
                    ev_entity.update(cx, |a, cx| {
                        if let Some(g) = a.selected_guild {
                            crate::ui::events::open(a, g, cx);
                        }
                    })
                })
                .child(item("g-events", "calendar", crate::t!("guild.top.events"), false)),
        )
        // vanilla-inbox-0 #629..#633: "Channels & Roles" row (ChannelListMagnifyingGlassIcon)
        // for onboarding guilds, "Browse Channels" for other community guilds.
        .when_some(browse, |d, label| {
            let ent = entity.clone();
            d.child(
                div()
                    .id("g-browse-open")
                    .on_click(move |_, _, cx| {
                        ent.update(cx, |a, cx| {
                            if let Some(g) = a.selected_guild {
                                crate::ui::community::open(a, g, cx);
                            }
                        })
                    })
                    .child(item("g-browse", "channel-browse", label, page_on)),
            )
        })
        .when(mods, |d| d.child(item("g-members", "members", crate::t!("guild.top.members"), false)))
        .child(item("g-boosts", "boost", crate::t!("guild.top.boosts"), false))
        // vanilla-inbox-0 #645 sectionDivider_: margin-top 15, inline 20 / 11, 1px.
        .child(div().mt(z(15.)).ml(z(20.)).mr(z(20.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE))) // sectionDivider_ysO7Gy 100..396 (11 + the 9px gutter from the list edge)
        .child(div().h(z(12.)))
}
