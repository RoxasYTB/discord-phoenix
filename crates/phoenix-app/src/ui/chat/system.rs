use gpui::{AnyElement, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::icons::ChatIcon;
use crate::{
    app::PhoenixApp,
    model::{Message, MessageKind},
    richtext::RenderedMessage,
    theme::{Palette, c, z},
};

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
    if msg.kind == MessageKind::OtherSystem && msg.content.is_empty() {
        return Some(match r.day_divider.clone() {
            Some(label) => super::day_divider(label).into_any_element(),
            None => div().into_any_element(),
        });
    }
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
    let target = |t: Option<crate::model::Id>| t.filter(|t| *t != msg.author);
    let other: Option<(crate::model::Id, SharedString, SharedString)> = match msg.kind {
        MessageKind::RecipientAdd(t) | MessageKind::RecipientRemove(t) => target(t).map(|t| {
            let n = app.users.get(&t).map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("sys.someone"));
            let tail = if matches!(msg.kind, MessageKind::RecipientAdd(_)) { "sys.recipient_add_tail" } else { "sys.recipient_kick_tail" };
            (t, n, crate::i18n::t(tail, &[]))
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
                .left(z(36.))
                .top(z(7.))
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
            .id(("sys-author", msg.id.0))
            .on_click(super::message_ext::open_user(entity, msg.author, msg.channel))
            .child(match crate::ui::role_gradient::for_author(app, msg.author) {
                Some(stops) => crate::fonts::name_text_with(name, author, 16., Some(stops)),
                None => crate::fonts::name_text(name, author, 16.),
            })
        })
        .child(after)
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
        .when_some(other, |d, (uid, n, tail)| {
            d.child(
                div()
                    .id(("sys-target", msg.id.0))
                    .on_click(super::message_ext::open_user(entity, uid, msg.channel))
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

const WAVE_STICKERS: [(u64, bool); 6] = [
    (749054660769218631, false),
    (751606379340365864, false),
    (754108890559283200, false),
    (781291131828699156, true),
    (816087792291282944, false),
    (819128604311027752, false),
];

fn wave_sticker(me: u64, msg: u64) -> (u64, bool) {
    WAVE_STICKERS[((me >> 22) + (msg >> 22)) as usize % WAVE_STICKERS.len()]
}

fn welcome_cta(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> Option<AnyElement> {
    let bot = app.users.get(&msg.author).is_some_and(|u| u.bot);
    let blocked = app.nav.extras.get(&msg.channel).is_some_and(|x| x.send_blocked);
    if bot || blocked || msg.author == app.me.id {
        return None;
    }
    let (sticker, apng) = wave_sticker(app.me.id.0, msg.id.0);
    let label = crate::t!("sys.wave");
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
        div()
            .flex()
            .pl(z(80.))
            .pt(z(4.))
            .mb(z(2.))
            .child(button.on_click(move |_, _, cx| {
                e.update(cx, |a, cx| crate::ui::pickers::detach(a.backend.send_sticker(channel, crate::model::Id(sticker)), cx));
            }))
            .into_any_element(),
    )
}

fn poll_field<'a>(msg: &'a Message, name: &str) -> Option<&'a str> {
    msg.embeds.first()?.fields.iter().find(|f| f.name.as_ref() == name).map(|f| f.value.as_ref())
}

pub fn truncate_question(q: &str) -> String {
    if q.chars().count() > 50 { q.chars().take(49).collect::<String>() + "\u{2026}" } else { q.to_string() }
}

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
            .on_click(super::message_ext::open_user(entity, msg.author, msg.channel))
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
        div()
            .size(z(40.))
            .flex_none()
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::img(src).size(z(22.)))
    });
    let has_image = emoji_el.is_some();
    let description = if victor.is_some() && votes > 0 {
        div()
            .flex()
            .flex_col()
            .gap(z(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tk(idx::TEXT_DEFAULT))
                            .child(victor.unwrap_or_default()),
                    )
                    .child(crate::ui::components::voted_check(16. * crate::theme::zoom(), true, true)),
            )
            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("sys.poll_winner", pct = pct)))
    } else {
        div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("sys.poll_no_winner"))
    };
    let card = div()
        .ml(z(80.))
        .mt(z(8.))
        .mb(z(2.))
        .w_full()
        .max_w(z(376. + (if has_image { 24. } else { 32. }) + 2.))
        .py(z(8.))
        .pl(z(if has_image { 8. } else { 16. }))
        .pr(z(16.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(z(8.))
        .children(emoji_el)
        .child(div().flex_1().min_w_0().overflow_hidden().child(description))
        .child(crate::ui::controls::button_sm(("sys-poll-view", msg.id.0), crate::t!("sys.poll_view"), crate::ui::controls::BtnKind::Secondary).on_click(move |_, _, cx| {
            if let Some(p) = poll {
                e2.update(cx, |a, cx| a.jump_to_message(ch, p, cx));
            }
        }));
    let mentioned = msg.author == app.me.id;
    let (bg, hover) = if mentioned {
        (Some(idx::MESSAGE_MENTIONED_BACKGROUND_DEFAULT), idx::MESSAGE_MENTIONED_BACKGROUND_HOVER)
    } else {
        (None, idx::MESSAGE_BACKGROUND_HOVER)
    };
    div()
        .id(("sysmsg-poll", msg.id.0))
        .relative()
        .mt(z(if r.group_start { 17. } else { 0. }))
        .py(z(2.))
        .flex()
        .flex_col()
        .when_some(bg, |d, b| d.bg(tk(b)))
        .hover(move |s| s.bg(tk(hover)))
        .when(mentioned, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(idx::ICON_FEEDBACK_WARNING))))
        .child(line)
        .child(div().flex().pr(z(48.)).child(card))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_result_title_is_cut_like_the_client() {
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
