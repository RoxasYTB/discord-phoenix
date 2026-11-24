//! System message lines (joins, pins, boosts, thread created, calls...):
//! icon in the avatar gutter, muted text with the actor's name in white,
//! timestamp at the end.

use gpui::{AnyElement, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::icons::ChatIcon;
use crate::{
    app::PhoenixApp,
    model::{Message, MessageKind},
    richtext::RenderedMessage,
    theme::{Palette, c, z},
};

/// Stable pick among the join phrasings (by message id) so a row never
/// changes text between renders.
fn join_text(id: u64) -> (SharedString, SharedString) {
    let n = (id >> 22) as usize % 6;
    (crate::i18n::t(&format!("sys.join{n}.before"), &[]), crate::i18n::t(&format!("sys.join{n}.after"), &[]))
}

pub fn is_system(kind: MessageKind) -> bool {
    !matches!(kind, MessageKind::Default | MessageKind::Reply)
}

pub fn render(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage) -> Option<AnyElement> {
    if !is_system(msg.kind) {
        return None;
    }
    // Types we don't know with no text: the client draws nothing for them, not a bare
    // "⋯ name time" row (seen in real DMs).
    if msg.kind == MessageKind::OtherSystem && msg.content.is_empty() {
        return Some(match r.day_divider.clone() {
            Some(label) => super::day_divider(label).into_any_element(),
            None => div().into_any_element(),
        });
    }
    // actorHook: the member's server nick and role colour, like a message author.
    if let MessageKind::PollResult(poll) = msg.kind {
        let line = poll_result(app, entity, msg, r, poll);
        return Some(match r.day_divider.clone() {
            Some(label) => div().child(super::day_divider(label)).child(line).into_any_element(),
            None => line,
        });
    }
    let name: SharedString = msg
        .author_nick
        .clone()
        .or_else(|| app.member_nick(msg.author))
        .or_else(|| app.users.get(&msg.author).map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("sys.someone"));
    let name_color = app.author_color(msg.author).map(c).unwrap_or_else(|| c(Palette::TEXT_HEADER));
    // Group DM add/remove name the other member ("X a ajouté Y au groupe."); leaving
    // the group on one's own keeps the single-name line.
    let target = |t: Option<crate::model::Id>| t.filter(|t| *t != msg.author);
    let other: Option<(SharedString, SharedString)> = match msg.kind {
        MessageKind::RecipientAdd(t) | MessageKind::RecipientRemove(t) => target(t).map(|t| {
            let n = app.users.get(&t).map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("sys.someone"));
            let tail = if matches!(msg.kind, MessageKind::RecipientAdd(_)) { "sys.recipient_add_tail" } else { "sys.recipient_kick_tail" };
            (n, crate::i18n::t(tail, &[]))
        }),
        _ => None,
    };
    let (icon, color, before, after): (ChatIcon, u32, SharedString, SharedString) = match msg.kind {
        MessageKind::Join => {
            let (b, a) = join_text(msg.id.0);
            (ChatIcon::Join, Palette::GREEN, b, a)
        }
        MessageKind::Pin => (ChatIcon::Join, Palette::INTERACTIVE, "".into(), crate::t!("sys.pin")),
        MessageKind::Boost(0) => (ChatIcon::Boost, 0xFF73FA, "".into(), crate::t!("sys.boost")),
        MessageKind::Boost(t) => (
            ChatIcon::Boost,
            0xFF73FA,
            "".into(),
            crate::t!("sys.boost_level", t = t),
        ),
        MessageKind::ThreadCreated => (
            ChatIcon::Thread,
            Palette::INTERACTIVE,
            "".into(),
            // cZ9Uf2: "{actor} a commencé un fil : **{name}**. Voir tous les {fils}."; the
            // name and "fils" are links drawn below.
            crate::t!("sys.thread_prefix"),
        ),
        MessageKind::RecipientAdd(t) => match target(t) {
            Some(_) => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.recipient_add_named")),
            None => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.recipient_add")),
        },
        MessageKind::RecipientRemove(t) => match target(t) {
            Some(_) => (ChatIcon::Join, Palette::RED, "".into(), crate::t!("sys.recipient_kick_named")),
            None => (ChatIcon::Join, Palette::RED, "".into(), crate::t!("sys.recipient_remove")),
        },
        MessageKind::Call => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.call")),
        MessageKind::ChannelNameChange => (
            ChatIcon::Edit,
            Palette::INTERACTIVE,
            "".into(),
            crate::t!("sys.rename", c = msg.content),
        ),
        // Stage system lines (real stage chat showed bare "⋯ name time" rows).
        MessageKind::StageStart => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.stage_start", c = msg.content)),
        MessageKind::StageEnd => (ChatIcon::Join, Palette::RED, "".into(), crate::t!("sys.stage_end", c = msg.content)),
        MessageKind::StageSpeaker => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.stage_speaker")),
        MessageKind::StageTopic => (ChatIcon::Edit, Palette::INTERACTIVE, "".into(), crate::t!("sys.stage_topic", c = msg.content)),
        _ => (
            ChatIcon::More,
            Palette::INTERACTIVE,
            "".into(),
            if msg.content.is_empty() { "".into() } else { format!(": {}", msg.content).into() },
        ),
    };
    let thread = msg.thread.clone().filter(|_| msg.kind == MessageKind::ThreadCreated);
    let line = div()
        .id(("sysmsg", msg.id.0))
        .relative()
        // systemMessage_ row: content at x+80; iconContainer_bLMKT5 72x22 at x+8, centred icon
        // (live dump ref-cache message-content-*.json, 2026-11-24).
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 17. } else { 0. }))
        .min_h(z(22.))
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(z(16.))
        .text_color(c(Palette::TEXT_MUTED))
        .hover(|s| s.bg(c(Palette::BG_MSG_HOVER)))
        .child(
            div()
                .absolute()
                .left(z(36.)) // 8 + (72 - 16) / 2
                .top(z(7.)) // container_bLMKT5 padding-top 2 + (22 - 16) / 2 + row py 2
                .child(super::icons::icon(icon, 16., c(color))),
        )
        .child(before)
        .child({
            let author = app.users.get(&msg.author);
            crate::fonts::styled_name(
                div().font_weight(FontWeight::MEDIUM).text_color(name_color).cursor_pointer().hover(|s| s.underline()),
                author,
                false,
            )
            .child(match crate::ui::role_gradient::for_author(app, msg.author) {
                Some(stops) => crate::fonts::name_text_with(name, author, 16., Some(stops)),
                None => crate::fonts::name_text(name, author, 16.),
            })
        })
        .child(after)
        // content_bLMKT5 a: medium, --interactive-text-active (the <strong> keeps the
        // anchor's 500 weight).
        .when_some(thread.clone(), |d, t| {
            let (e1, e2) = (entity.clone(), entity.clone());
            let link = |id: (&'static str, u64), text: SharedString| {
                div()
                    .id(id)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_TEXT_ACTIVE)))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(text)
            };
            let t2 = t.clone();
            d.child(link(("sys-thread-name", msg.id.0), t.name.clone()).on_click(move |_, w, cx| {
                let t = t2.clone();
                e1.update(cx, |a, cx| a.open_thread(t, w, cx));
            }))
            .child(crate::t!("sys.thread_see_all"))
            .child(link(("sys-thread-all", msg.id.0), crate::t!("sys.thread_all_link")).on_click(move |_, _, cx| {
                e2.update(cx, |a, cx| a.toggle_threads_popout(cx));
            }))
            .child(".")
        })
        .when_some(other, |d, (n, tail)| {
            d.child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(Palette::TEXT_HEADER))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(n),
            )
            .child(tail)
        })
        .child(
            div()
                .ml(z(6.))
                .text_size(z(12.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(r.time_full.clone()),
        );
    let cta = if msg.kind == MessageKind::Join { welcome_cta(app, entity, msg) } else { None };
    let line: AnyElement = match (cta, thread) {
        (Some(b), _) => div().child(line).child(b).into_any_element(),
        // The thread card under the line (systemMessageContainer_: margin-top 0) and the
        // .hasThread_.isSystemMessage_ spine from 1.75rem down.
        (None, Some(t)) => div()
            .relative()
            .child(line)
            .child(div().pl(z(80.)).pr(z(30.)).pb(z(2.)).flex().child(super::message_ext::thread_chip(app, entity, &t, true)))
            .child(super::message_ext::thread_spine(28. + if r.group_start { 17. } else { 0. }))
            .into_any_element(),
        (None, None) => line.into_any_element(),
    };
    Some(match r.day_divider.clone() {
        Some(label) => div().child(super::day_divider(label)).child(line).into_any_element(),
        None => line,
    })
}

/// Greeting stickers the client cycles through for "Wave to say hi!"
/// (id, is APNG); the rest are Lottie.
const WAVE_STICKERS: [(u64, bool); 6] = [
    (749054660769218631, false),
    (751606379340365864, false),
    (754108890559283200, false),
    (781291131828699156, true),
    (816087792291282944, false),
    (819128604311027752, false),
];

/// Client pick: (our snowflake timestamp + message timestamp) % 6.
fn wave_sticker(me: u64, msg: u64) -> (u64, bool) {
    WAVE_STICKERS[((me >> 22) + (msg >> 22)) as usize % WAVE_STICKERS.len()]
}

/// welcomeCTA_eZQ1Gc (margin-bottom 2) under a join line: secondary button
/// with the greeting sticker as its icon. Hidden for bots, ourselves and
/// channels where we can't send.
fn welcome_cta(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> Option<AnyElement> {
    let bot = app.users.get(&msg.author).is_some_and(|u| u.bot);
    let blocked = app.nav.extras.get(&msg.channel).is_some_and(|x| x.send_blocked);
    if bot || blocked || msg.author == app.me.id {
        return None;
    }
    let (sticker, apng) = wave_sticker(app.me.id.0, msg.id.0);
    let label = crate::t!("sys.wave");
    // The greeting sticker as a 20px still (Lottie ones rasterized from their JSON).
    let icon = if apng {
        gpui::img(SharedString::from(format!("https://media.discordapp.net/stickers/{sticker}.png?size=40")))
    } else {
        crate::media::lottie::image(crate::media::lottie::url(sticker), 40, true)
    };
    let button = crate::ui::controls::button(("wave-cta", msg.id.0), SharedString::default(), crate::ui::controls::BtnKind::Secondary)
        .h(z(32.))
        .px(z(12.))
        .text_size(z(14.))
        .gap(z(4.))
        .child(icon.flex_none().size(z(20.)))
        .child(label);
    let (e, channel) = (entity.clone(), msg.channel);
    Some(
        // flex row: the button keeps its content width (as a block child it stretched to
        // the whole message column)
        div()
            .flex()
            .pl(z(80.))
            .pt(z(4.))
            .mb(z(2.)) // welcomeCTA_eZQ1Gc margin-bottom 2px
            .child(button.on_click(move |_, _, cx| {
                e.update(cx, |a, cx| crate::ui::pickers::detach(a.backend.send_sticker(channel, crate::model::Id(sticker)), cx));
            }))
            .into_any_element(),
    )
}

/// Field `name` of the POLL_RESULT embed.
fn poll_field<'a>(msg: &'a Message, name: &str) -> Option<&'a str> {
    msg.embeds.first()?.fields.iter().find(|f| f.name.as_ref() == name).map(|f| f.value.as_ref())
}

/// `s.EJ(question, 50)`: cut to 49 characters plus "…" when longer than 50.
pub fn truncate_question(q: &str) -> String {
    if q.chars().count() > 50 { q.chars().take(49).collect::<String>() + "\u{2026}" } else { q.to_string() }
}

/// POLL_RESULT (live sandbox 2026-11-24): system line VJcK41 "Le sondage « **question** » de
/// **author** est clos." (question cut at 50, a link to the poll; author in nick + role
/// colour), PollsIcon 16px --interactive-icon-default in the gutter, then the embed card
/// container_D5ZFqQ (margin 8 0 2, padding 8 16 8 8, radius 8, --background-surface-high,
/// 1px --border-subtle, gap 8): the winner's emoji, "Réponse B" body-sm/semibold + check,
/// "Réponse gagnante • 100%" body-xs/medium (a+lSZ/ "Il n'y a pas eu de vainqueur" without
/// votes), and a secondary sm "Voir le sondage" that jumps to the poll.
fn poll_result(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, poll: Option<crate::model::Id>) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let tk = |i: usize| c(tok(i));
    let name: SharedString = msg
        .author_nick
        .clone()
        .or_else(|| app.member_nick(msg.author))
        .or_else(|| app.users.get(&msg.author).map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("sys.someone"));
    let name_color = app.author_color(msg.author).map(c).unwrap_or_else(|| tk(idx::INTERACTIVE_TEXT_ACTIVE));
    let question = truncate_question(poll_field(msg, "poll_question_text").unwrap_or(""));
    let votes: u32 = poll_field(msg, "victor_answer_votes").and_then(|v| v.parse().ok()).unwrap_or(0);
    let total: u32 = poll_field(msg, "total_votes").and_then(|v| v.parse().ok()).unwrap_or(0);
    let victor = poll_field(msg, "victor_answer_text").map(str::to_string);
    let emoji = poll_field(msg, "victor_answer_emoji_name").map(str::to_string);
    let emoji_id = poll_field(msg, "victor_answer_emoji_id").map(str::to_string);
    let (ch, e1, e2) = (msg.channel, entity.clone(), entity.clone());
    let link = |id: (&'static str, u64), text: SharedString, color: gpui::Hsla| {
        div().id(id).font_weight(FontWeight::MEDIUM).text_color(color).cursor_pointer().hover(|s| s.underline()).child(text)
    };
    let line = div()
        .id(("sysmsg", msg.id.0))
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 17. } else { 0. }))
        .min_h(z(22.))
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .text_color(c(Palette::TEXT_MUTED))
        .child(
            div().absolute().left(z(36.)).top(z(7.)).child(gpui::svg().path(crate::icons::path("attach/poll")).size(z(16.)).text_color(tk(idx::INTERACTIVE_ICON_DEFAULT))),
        )
        .children({
            // fr: "Le sondage « Q » de A est clos." / en: "A's poll Q has closed."
            let q = link(("sys-poll-q", msg.id.0), question.into(), tk(idx::INTERACTIVE_TEXT_ACTIVE))
                .on_click(move |_, _, cx| {
                    if let Some(p) = poll {
                        e1.update(cx, |a, cx| a.jump_to_message(ch, p, cx));
                    }
                })
                .into_any_element();
            let author_user = app.users.get(&msg.author);
            let author = crate::fonts::styled_name(
                div().id(("sys-poll-author", msg.id.0)).font_weight(FontWeight::MEDIUM).text_color(name_color).cursor_pointer().hover(|s| s.underline()),
                author_user,
                false,
            )
            .child(match crate::ui::role_gradient::for_author(app, msg.author) {
                Some(stops) => crate::fonts::name_text_with(name, author_user, 16., Some(stops)),
                None => crate::fonts::name_text(name, author_user, 16.),
            })
            .into_any_element();
            let t = |k: &str| crate::i18n::t(k, &[]).into_any_element();
            if crate::i18n::locale() == "en" {
                vec![author, t("sys.poll_closed_b"), q, t("sys.poll_closed_c")]
            } else {
                vec![t("sys.poll_closed_a"), q, t("sys.poll_closed_b"), author, t("sys.poll_closed_c")]
            }
        })
        .child(div().ml(z(6.)).text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(r.time_full.clone()));
    let pct = if total == 0 { 0 } else { (votes as f64 * 100. / total as f64).round() as u32 };
    let emoji_el = emoji.map(|n| {
        let src: SharedString = match emoji_id {
            Some(id) => format!("https://cdn.discordapp.com/emojis/{id}.webp?size=48").into(),
            None => crate::media::emoji::twemoji_url(&n).unwrap_or_default(),
        };
        div().size(z(40.)).flex_none().flex().items_center().justify_center().child(gpui::img(src).size(z(32.)))
    });
    let description = if victor.is_some() && votes > 0 {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(victor.unwrap_or_default())
                    // victorIcon_D5ZFqQ: 16px circle check (white disc, --control-connected ring)
                    .child(div().size(z(16.)).flex().items_center().justify_center().child(
                        div().size(z(15.)).rounded_full().bg(tk(idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT)).flex().items_center().justify_center().child(
                            gpui::svg().path(crate::icons::path("check")).size(z(10.)).text_color(gpui::white()),
                        ),
                    )),
            )
            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("sys.poll_winner", pct = pct)))
    } else {
        div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("sys.poll_no_winner"))
    };
    let card = div()
        .ml(z(80.))
        .mt(z(8.))
        .mb(z(2.))
        .max_w(z(432.))
        .pt(z(8.))
        .pb(z(8.))
        .pl(z(8.))
        .pr(z(16.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(z(8.))
        .children(emoji_el)
        .child(div().flex_1().min_w_0().child(description))
        .child(crate::ui::controls::button_sm(("sys-poll-view", msg.id.0), crate::t!("sys.poll_view"), crate::ui::controls::BtnKind::Secondary).on_click(move |_, _, cx| {
            if let Some(p) = poll {
                e2.update(cx, |a, cx| a.jump_to_message(ch, p, cx));
            }
        }));
    div().flex().flex_col().pb(z(2.)).child(line).child(div().flex().child(card)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_result_title_is_cut_like_the_client() {
        // Live 2026-11-24: "[Test Phoenix] envoyé depuis le client Rust (Phoe…".
        let q = "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — sondage de test : quelle réponse ?";
        assert_eq!(super::truncate_question(q), "[Test Phoenix] envoyé depuis le client Rust (Phoe\u{2026}");
        assert_eq!(super::truncate_question("court"), "court");
    }

    #[test]
    fn join_text_is_stable() {
        assert_eq!(join_text(1 << 22), join_text(1 << 22));
        assert!(is_system(MessageKind::Join));
        assert!(!is_system(MessageKind::Reply));
        assert_eq!(wave_sticker(0, 0).0, 749054660769218631);
        assert_eq!(wave_sticker(3 << 22, 0), (781291131828699156, true));
    }
}
