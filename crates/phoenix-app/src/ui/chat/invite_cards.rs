use std::rc::Rc;

use gpui::{AnyElement, Entity, FontWeight, ImageSource, ParentElement, SharedString, Styled, div, img, prelude::*};

use super::invite_embed::{accept_click, body, guild_icon, header_line, info, text, tk, wrapper};
use crate::{
    app::PhoenixApp,
    commands::GuildEvent,
    model::{Channel, ChannelKind, Id, ImageRef},
    theme::{generated::idx, space, z},
    ui::{
        controls::{BtnKind, button, button_icon, button_sm},
        home::data::{InvitePreview, InviteUser},
    },
};

pub fn visible_voice_channel<'a>(app: &'a PhoenixApp, p: &InvitePreview) -> Option<&'a Channel> {
    if p.channel_type != Some(2) {
        return None;
    }
    let ch = app.channels.get(&p.channel_id?)?;
    (ch.kind == ChannelKind::Voice && ch.guild == p.guild).then_some(ch)
}

pub fn voice_users_in_order(ch: &Channel) -> Vec<Id> {
    let streaming = |u: &Id| crate::ui::voice_panel::flags(*u).stream;
    ch.voice_users
        .iter()
        .copied()
        .filter(streaming)
        .chain(ch.voice_users.iter().copied().filter(|u| !streaming(u)))
        .collect()
}

pub fn voice_list_card(app: &PhoenixApp, entity: &Entity<PhoenixApp>, ch: &Channel) -> AnyElement {
    let users = voice_users_in_order(ch);
    let in_channel = app.voice_channel == Some(ch.id);
    let guild_name = ch
        .guild
        .and_then(|g| app.guilds.iter().find(|x| x.id == g))
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let timer = (!users.is_empty())
        .then(|| crate::voice::start_time::get(ch.id.0))
        .flatten()
        .map(|st| {
            div()
                .flex_none()
                .font_family(crate::theme::FONT_MONO())
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(tk(idx::TEXT_FEEDBACK_POSITIVE))
                .child(crate::voice::start_time::format(crate::voice::start_time::now_s() - st))
        });
    let id = ch.id;
    let open = entity.clone();
    let name = div()
        .id(("invite-voice-name", id.0))
        .min_w_0()
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tip(SharedString::from(format!("{guild_name} / {}", ch.name))))
        .on_click(move |_, _, cx| open.update(cx, |a, cx| a.select_channel(id, cx)))
        .child(
            text(16., 20., FontWeight::NORMAL, if in_channel { idx::TEXT_DEFAULT } else { idx::TEXT_MUTED })
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .hover(|s| s.underline())
                .child(ch.name.clone()),
        );
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(z(space::sm()))
        .p(z(space::md()))
        .bg(tk(idx::BACKGROUND_MOD_MUTED))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(space::xs()))
                .child(
                    div().flex_none().pl(z(2.)).child(
                        gpui::svg()
                            .path(crate::icons::path("speaker"))
                            .size(z(20.))
                            .text_color(tk(if users.is_empty() { idx::ICON_SUBTLE } else { idx::TEXT_FEEDBACK_POSITIVE })),
                    ),
                )
                .child(name),
        )
        .children(timer);
    let list: AnyElement = if users.is_empty() {
        div()
            .p(z(space::md()))
            .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED).child(crate::t!("invite.card.voice_nobody")))
            .into_any_element()
    } else {
        div()
            .px(z(space::xxs()))
            .child(
                div()
                    .id(("invite-voice-list", id.0))
                    .max_h(z(125.))
                    .overflow_y_scroll()
                    .py(z(space::xs()))
                    .pl(z(space::xxs()))
                    .flex()
                    .flex_col()
                    .children(users.iter().map(|u| crate::ui::voice_panel::voice_user_row(app, entity, *u))),
            )
            .into_any_element()
    };
    let join = entity.clone();
    let btn = button_icon(("invite-voice-join", id.0), "speaker", crate::t!("invite.card.join_voice"), BtnKind::Active)
        .w_full()
        .on_click(move |_, _, cx| join.update(cx, |a, cx| a.join_voice(Some(id), cx)));
    div()
        .mt(z(2.))
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(
            div()
                .w(z(280.))
                .flex_none()
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(header)
                .child(list)
                .child(div().px(z(space::md())).pb(z(space::md())).child(btn)),
        )
        .child(members_only_footer())
        .into_any_element()
}

fn members_only_footer() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(z(2.))
        .min_w_0()
        .overflow_hidden()
        .child(gpui::svg().path(crate::icons::path("chat/eye-slash")).size(z(14.)).flex_none().text_color(tk(idx::TEXT_MUTED)))
        .child(
            text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(crate::t!("invite.card.voice_members_only")),
        )
}

pub fn stream_card(app: &PhoenixApp, entity: &Entity<PhoenixApp>, p: &Rc<InvitePreview>) -> AnyElement {
    let Some(gid) = p.guild else {
        return div().into_any_element();
    };
    let Some(target) = p.target_user.as_ref() else {
        return div().into_any_element();
    };
    let own = target.id == Some(app.me.id);
    let member = app.guilds.iter().any(|g| g.id == gid);
    let live_here = target.id.is_some_and(|u| {
        crate::ui::voice_panel::flags(u).stream
            && p.channel_id.and_then(|c| app.channels.get(&c)).is_some_and(|c| c.voice_users.contains(&u))
    });
    let key = target.id.zip(p.channel_id).map(|(u, c)| crate::voice::stream::key(Some(gid.0), c.0, u.0));
    let watching = live_here && key.as_ref().is_some_and(|k| app.voice.watching.contains(k));
    let name = target.name.clone();
    let (title, label, kind, disabled) = if member && !live_here {
        (
            if own { crate::t!("invite.card.you_were_live") } else { crate::t!("invite.card.was_live", name = name) },
            crate::t!("invite.card.stream_ended"),
            BtnKind::Active,
            true,
        )
    } else if watching {
        (
            if own { crate::t!("invite.card.you_are_live") } else { crate::t!("invite.card.is_live", name = name) },
            crate::t!("invite.card.watching"),
            BtnKind::Secondary,
            true,
        )
    } else {
        (
            if own { crate::t!("invite.card.you_are_live") } else { crate::t!("invite.card.is_live", name = name) },
            crate::t!("invite.card.watch"),
            BtnKind::Active,
            false,
        )
    };
    let detail: AnyElement = match p.channel_id.and_then(|c| app.channels.get(&c)).filter(|_| app.selected_guild == Some(gid)) {
        Some(ch) => div()
            .flex()
            .items_center()
            .child(gpui::svg().path(crate::icons::path("speaker")).size(z(16.)).flex_none().text_color(tk(idx::TEXT_MUTED)))
            .child(
                text(14., 16., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT)
                    .ml(z(4.))
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(ch.name.clone()),
            )
            .into_any_element(),
        None => text(14., 16., FontWeight::NORMAL, idx::INTERACTIVE_TEXT_DEFAULT)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(crate::t!("invite.card.in_guild", guild = p.guild_name.clone()))
            .into_any_element(),
    };
    let channel = p.channel_id;
    let btn = button_sm(("invite-stream", gid.0), label, kind).map(|b| {
        if disabled {
            b.opacity(0.5).cursor_default()
        } else if member {
            let e = entity.clone();
            b.on_click(move |_, _, cx| {
                if let Some(k) = key.clone() {
                    e.update(cx, |a, cx| crate::ui::voice_row::watch_from_sidebar(a, k, cx));
                }
            })
        } else {
            b.on_click(accept_click(entity, p.code.clone(), Some(gid), channel))
        }
    });
    wrapper()
        .child(super::invite_embed::title(crate::t!("invite.card.stream_title")))
        .child(
            body()
                .child(header_line(
                    Some(guild_icon(p, 48., 12., 16.)),
                    info(title.into_any_element(), Some(detail), idx::TEXT_STRONG, FontWeight::SEMIBOLD, 2.),
                ))
                .child(div().flex_none().child(btn)),
        )
        .into_any_element()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventTime {
    Upcoming,
    StartingWithinHour,
    Started,
    Active,
    Ended,
    Canceled,
}

pub fn event_time(e: &GuildEvent, now: i64) -> EventTime {
    match e.status {
        4 => EventTime::Canceled,
        2 => EventTime::Active,
        3 => EventTime::Ended,
        _ if e.start <= now => EventTime::Started,
        _ if e.start <= now + 3600 => EventTime::StartingWithinHour,
        _ => EventTime::Upcoming,
    }
}

fn calendar_label(ts: i64, now: i64) -> String {
    let (Some(d), Some(n)) = (chrono::DateTime::from_timestamp(ts, 0), chrono::DateTime::from_timestamp(now, 0)) else {
        return String::new();
    };
    let (d, n) = (d.with_timezone(&chrono::Local), n.with_timezone(&chrono::Local));
    let days = (d.date_naive() - n.date_naive()).num_days();
    let time = d.format("%H:%M").to_string();
    match days {
        0 => crate::t!("invite.card.event_today", time = time).to_string(),
        1 => crate::t!("invite.card.event_tomorrow", time = time).to_string(),
        _ => crate::ui::events::when_label_at(ts),
    }
}

pub fn event_status_text(e: &GuildEvent, now: i64) -> String {
    let start = calendar_label(e.start, now);
    let end = e.end.map(|end| {
        let same_day = chrono::DateTime::from_timestamp(end, 0)
            .zip(chrono::DateTime::from_timestamp(e.start, 0))
            .is_some_and(|(a, b)| a.with_timezone(&chrono::Local).date_naive() == b.with_timezone(&chrono::Local).date_naive());
        if same_day {
            chrono::DateTime::from_timestamp(end, 0)
                .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_default()
        } else {
            calendar_label(end, now)
        }
    });
    match event_time(e, now) {
        EventTime::Canceled => crate::t!("invite.card.event_canceled", date = start).to_string(),
        EventTime::StartingWithinHour => {
            let mins = (e.start - now) / 60;
            if mins > 0 {
                crate::t!("invite.card.event_in_minutes", n = mins).to_string()
            } else {
                crate::t!("invite.card.event_soon").to_string()
            }
        }
        EventTime::Started => crate::t!("invite.card.event_soon").to_string(),
        EventTime::Active => {
            let live = if e.location.is_some() {
                crate::t!("invite.card.event_happening")
            } else {
                crate::t!("invite.card.event_live")
            };
            match end {
                Some(end) => crate::t!("invite.card.event_range", start = live, end = end).to_string(),
                None => live.to_string(),
            }
        }
        EventTime::Ended | EventTime::Upcoming => match end {
            Some(end) => crate::t!("invite.card.event_range", start = start, end = end).to_string(),
            None => start,
        },
    }
}

fn event_header(e: &GuildEvent, creator: Option<&InviteUser>) -> gpui::Div {
    let now = chrono::Utc::now().timestamp();
    let time = event_time(e, now);
    let (icon, icon_color, text_color) = match time {
        EventTime::Active => ("calendar", idx::TEXT_FEEDBACK_POSITIVE, if e.end.is_some() { idx::TEXT_DEFAULT } else { idx::TEXT_FEEDBACK_POSITIVE }),
        EventTime::Ended => ("inbox/clock", idx::TEXT_BRAND, idx::TEXT_DEFAULT),
        EventTime::Started | EventTime::StartingWithinHour => ("calendar", idx::TEXT_BRAND, idx::TEXT_BRAND),
        EventTime::Canceled => ("calendar", idx::TEXT_FEEDBACK_CRITICAL, idx::TEXT_DEFAULT),
        EventTime::Upcoming => ("calendar", idx::TEXT_BRAND, idx::TEXT_DEFAULT),
    };
    let count = e.user_count;
    let count_tip = if count == 1 {
        crate::t!("invite.card.event_people_one", n = count)
    } else {
        crate::t!("invite.card.event_people", n = count)
    };
    let status = div()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .min_w_0()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).flex_none().text_color(tk(icon_color)))
                .child(text(14., 18., FontWeight::SEMIBOLD, text_color).child(event_status_text(e, now))),
        )
        .child(div().flex_1())
        .children(creator.map(|c| {
            div()
                .id(("invite-event-creator", e.id.0))
                .flex_none()
                .tooltip(crate::ui::tooltip::tip(crate::t!("invite.card.event_created_by", name = c.name.clone())))
                .child(crate::ui::avatar_ref(
                    c.avatar.clone().unwrap_or(ImageRef::Default(c.id.map_or(0, |u| ((u.0 >> 22) % 6) as u8))),
                    20. * crate::theme::zoom(),
                ))
        }))
        .child(
            div()
                .id(("invite-event-count", e.id.0))
                .flex_none()
                .flex()
                .items_center()
                .gap(z(4.))
                .tooltip(crate::ui::tooltip::tip(count_tip))
                .child(gpui::svg().path(crate::icons::path("members")).size(z(12.)).flex_none().text_color(tk(idx::TEXT_DEFAULT)))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).child(count.to_string())),
        );
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(status)
        .child(text(20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG).child(e.name.clone()))
        .when(!e.description.is_empty(), |d| {
            d.child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).child(e.description.clone()))
        })
}

pub fn event_card(app: &PhoenixApp, entity: &Entity<PhoenixApp>, p: &Rc<InvitePreview>, e: &GuildEvent) -> AnyElement {
    let Some(gid) = p.guild else {
        return div().into_any_element();
    };
    let member = app.guilds.iter().any(|g| g.id == gid);
    let now = chrono::Utc::now().timestamp();
    let time = event_time(e, now);
    let channel = e.channel.or(p.channel_id);
    let ch = channel.and_then(|c| app.channels.get(&c));
    let in_channel = channel.is_some() && app.voice_channel == channel;
    let external = e.location.is_some();
    let place: Option<(&'static str, SharedString)> = match (&e.location, ch) {
        (Some(l), _) => Some(("event/location", l.clone().into())),
        (None, Some(c)) => Some((if c.kind == ChannelKind::Stage { "stage" } else { "speaker" }, c.name.clone())),
        (None, None) => p.channel_name.clone().map(|n| ("speaker", n)),
    };
    let btn = if !member {
        button_sm(("invite-event", e.id.0), crate::t!("invite.card.event_join"), BtnKind::Active)
            .on_click(accept_click(entity, p.code.clone(), Some(gid), channel))
    } else if time == EventTime::Active {
        if external {
            button_sm(("invite-event", e.id.0), crate::t!("invite.card.event_details"), BtnKind::Secondary)
        } else {
            let label = if in_channel { crate::t!("invite.card.event_go_to_channel") } else { crate::t!("invite.card.event_join") };
            let en = entity.clone();
            button_sm(("invite-event", e.id.0), label, BtnKind::Active).on_click(move |_, _, cx| {
                if let Some(c) = channel {
                    en.update(cx, |a, cx| a.go_to_invite(gid, Some(c), cx));
                }
            })
        }
    } else if time == EventTime::Ended {
        button_sm(("invite-event", e.id.0), crate::t!("invite.card.event_over"), BtnKind::Secondary).opacity(0.5).cursor_default()
    } else {
        let on = crate::ui::events::rsvped(e.id);
        let (kind, icon, fg) = if on {
            (BtnKind::Active, "check", idx::CONTROL_CONNECTED_TEXT_DEFAULT)
        } else {
            (BtnKind::Secondary, "bell", idx::CONTROL_SECONDARY_TEXT_DEFAULT)
        };
        let (en, ev) = (entity.clone(), e.id);
        button(("invite-event", e.id.0), crate::t!("invite.card.event_interested"), kind)
            .h(z(32.))
            .min_w(z(60.))
            .px(z(12.))
            .text_size(z(14.))
            .line_height(z(18.))
            .gap(z(4.))
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).flex_none().text_color(tk(fg)))
            .flex_row_reverse()
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                en.update(cx, |a, cx| crate::ui::events::toggle_rsvp(a, gid, ev, cx));
            })
    };
    let guild_name = p.guild_name.clone();
    let details = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(z(16.))
        .child(guild_icon(p, 48., 12., 16.))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div().flex().items_center().gap(z(4.)).min_w_0().child(
                        text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT)
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(crate::ui::emoji_text::label(guild_name, 14.)),
                    ),
                )
                .children(place.map(|(icon, name)| {
                    div()
                        .mt(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .min_w_0()
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .child(gpui::svg().path(crate::icons::path(icon)).size(z(12.)).flex_none().text_color(tk(idx::TEXT_DEFAULT)))
                        .child(
                            text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT)
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(name),
                        )
                })),
        );
    wrapper()
        .when_some(e.image.as_ref(), |d, hash| {
            let url: SharedString = format!("https://cdn.discordapp.com/guild-events/{}/{hash}.png?size=1024", e.id.0).into();
            d.child(
                div()
                    .mt(z(-16.))
                    .mx(z(-16.))
                    .mb(z(16.))
                    .h(z(176.))
                    .rounded_t(z(4.))
                    .overflow_hidden()
                    .child(img(ImageSource::from(url)).size_full().object_fit(gpui::ObjectFit::Cover)),
            )
        })
        .child(event_header(e, p.event_creator.as_ref()))
        .child(div().mt(z(12.)).flex().flex_wrap().items_center().gap(z(16.)).overflow_hidden().child(details).child(div().flex_none().child(btn)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(status: u8, start: i64) -> GuildEvent {
        GuildEvent { status, start, ..Default::default() }
    }

    #[test]
    fn event_time_follows_the_client() {
        let now = 1_000_000;
        assert_eq!(event_time(&ev(4, now + 99_999), now), EventTime::Canceled);
        assert_eq!(event_time(&ev(2, now - 10), now), EventTime::Active);
        assert_eq!(event_time(&ev(3, now - 10), now), EventTime::Ended);
        assert_eq!(event_time(&ev(1, now - 10), now), EventTime::Started);
        assert_eq!(event_time(&ev(1, now + 1800), now), EventTime::StartingWithinHour);
        assert_eq!(event_time(&ev(1, now + 7200), now), EventTime::Upcoming);
        assert_eq!(event_status_text(&ev(1, now + 1800), now), "Starting in 30m");
        assert_eq!(event_status_text(&ev(1, now - 5), now), "Starting soon");
        assert_eq!(event_status_text(&ev(2, now - 5), now), "Live Now");
    }
}
