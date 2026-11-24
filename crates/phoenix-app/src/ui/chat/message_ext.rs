//! Per-message interactive pieces: hover toolbar, reaction pills, inline
//! edit box, thread chip, "NEW" unread divider, right-click handler.

use gpui::{
    AnyElement, App, Div, Entity, FontWeight, IntoElement, MouseDownEvent, ParentElement,
    SharedString, Styled, Window, div, prelude::*,
};
use gpui_component::input::Input;

use super::{
    icons::{ChatIcon, icon},
    state::{EmojiTarget, QUICK_REACTIONS},
};
use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Message, ThreadRef},
    theme::{Palette, c, generated::idx, z},
    ui,
};

use super::tk;

/// Hover bar button: wrapper .button_ height 24px (+ hoverBarButton_ padding 2px),
/// radius 6px, :hover --interactive-background-hover; icon_ 20px.
fn bar_button(id: impl Into<gpui::ElementId>) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(z(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(6.))
        .cursor_pointer()
        .group("bar-btn")
        .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .active(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_ACTIVE))) // button_Z0MHyc:active
}

/// hoverBarButton_ .icon_ transition: transform .4s cubic-bezier(.16,1,.3,1).
const ICON_EASE_MS: u64 = 400;

fn tool_button(id: &'static str, msg: u64, i: SharedString, _size: f32) -> gpui::Stateful<Div> {
    use gpui::AnimationExt;
    let k = ui::states::key(("tb-icon", id, msg));
    let glyph = gpui::svg()
        .path(i)
        .flex_none()
        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
        // button_Z0MHyc:hover colour
        .group_hover("bar-btn", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
        .id("glyph")
        // :active colour, icon back to scale(1)
        .group_active("bar-btn", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)).size(z(20.)));
    // hoverBarButton_:hover .icon_ scale(1.1), drawn as a 20 -> 22px glyph, eased both ways.
    let sized = match ui::states::state(k) {
        Some((on, at)) if at.elapsed() < std::time::Duration::from_millis(ICON_EASE_MS) => glyph
            .with_animation(
                gpui::ElementId::Name(format!("tb-icon-{id}-{on}-{at:?}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(ICON_EASE_MS)).with_easing(ui::states::cubic_bezier(0.16, 1., 0.3, 1.)),
                move |g, t| g.size(z(if on { 20. + 2. * t } else { 22. - 2. * t })),
            )
            .into_any_element(),
        Some((true, _)) => glyph.size(z(22.)).into_any_element(),
        _ => glyph.size(z(20.)).into_any_element(),
    };
    ui::states::track(bar_button(id), k).child(sized)
}

/// The floating hover toolbar (top-right of the hovered message).
pub fn toolbar(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, shift: bool) -> gpui::AnyElement {
    let (ch, id) = (msg.channel, msg.id);
    let own = msg.author == app.me.id;
    let e = entity.clone();
    let local = super::pending::is_local(msg);
    let mut bar = div()
        .absolute()
        .right(z(14.)) // buttons_ right 0 + padding-inline-end 14px (msg-hover-toolbar #1443)
        .top(z(-16.)) // #1443 top -16px
        .invisible()
        .group_hover("msg", |s| s.visible())
        .when(app.chat.menu.is_some_and(|m| m.msg == id), |d| d.visible())
        // popover_: --background-surface-high, 1px --border-muted, radius 8px, padding 2px,
        // --shadow-low. (Position comes from the client's inline style.)
        .flex()
        .items_center()
        .p(z(2.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .rounded(z(8.))
        .shadow_sm();
    if local {
        // Live failed send (20261124): the bar holds only "Réessayer" then "Supprimer"
        // (28px hoverBarButton_, --interactive-text-default icons). Nothing while sending.
        if msg.send_state != crate::model::SendState::Failed {
            return div().into_any_element();
        }
        let e2 = e.clone();
        return bar
            .child(tool_button("tb-retry", id.0, ChatIcon::Retry.path(), 20.).on_click(move |_, _, cx| e2.update(cx, |a, cx| a.retry_send(ch, id, cx))))
            .child(tool_button("tb-discard", id.0, ChatIcon::Trash.path(), 20.).on_click(move |_, _, cx| e.update(cx, |a, cx| a.discard_local(ch, id, cx))))
            .into_any_element();
    }
    // Quick reactions (Discord shows frequent emoji first).
    for (i, q) in QUICK_REACTIONS.iter().take(3).enumerate() {
        let e = e.clone();
        bar = bar.child(
            bar_button(("quick", i))
                .child(crate::media::emoji::emoji_element(q, 20.))
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.react_with(ch, id, (*q).into(), None, cx))),
        );
    }
    // separator_: 1px x 24px, margin 2px 4px, --border-subtle, radius 100px.
    bar = bar.child(div().w(z(1.)).h(z(24.)).my(z(2.)).mx(z(4.)).rounded_full().bg(tk(idx::BORDER_SUBTLE)));
    bar = bar.child(tool_button("tb-react", id.0, ChatIcon::AddReaction.path(), 20.).on_click({
        let e = e.clone();
        move |_, _, cx| e.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Reaction { channel: ch, msg: id }, cx))
    }));
    // Vanilla dumps (guild-chat-busy, inbox, apps-launcher): others' messages show
    // Add reaction / Reply / Forward / More, own messages Add reaction / Edit / Forward / More.
    if own {
        bar = bar.child(tool_button("tb-edit", id.0, ChatIcon::Edit.path(), 18.).on_click({
            let e = e.clone();
            move |_, w, cx| e.update(cx, |a, cx| a.start_edit(id, w, cx))
        }));
    } else {
        bar = bar.child(tool_button("tb-reply", id.0, Icon::Reply.path(), 20.).on_click({
            let e = e.clone();
            move |_, w, cx| e.update(cx, |a, cx| a.start_reply(id, w, cx))
        }));
    }
    bar = bar.child(tool_button("tb-forward", id.0, crate::icons::path("nav/forward"), 20.).on_click({
        let e = e.clone();
        move |_, _, cx| e.update(cx, |a, cx| a.open_forward(ch, id, cx))
    }));
    if shift && own {
        bar = bar.child(
            // .button_.dangerous_ --control-critical-secondary-text-default
            bar_button("tb-delete")
                .child(icon(ChatIcon::Trash, 20., tk(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
                .on_click({
                    let e = e.clone();
                    move |ev, _, cx| {
                        let skip = ev.modifiers().shift;
                        e.update(cx, |a, cx| a.request_delete(ch, id, skip, cx))
                    }
                }),
        );
    }
    bar.child(tool_button("tb-more", id.0, ChatIcon::More.path(), 20.).on_click(move |ev, _, cx| {
        let pos = ev.position();
        e.update(cx, |a, cx| a.open_message_menu(ch, id, pos, cx))
    }))
    .into_any_element()
}

pub fn on_right_click(entity: &Entity<PhoenixApp>, msg: &Message) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let e = entity.clone();
    let (ch, id) = (msg.channel, msg.id);
    move |ev, _, cx| {
        cx.stop_propagation();
        let pos = ev.position;
        e.update(cx, |a, cx| a.open_message_menu(ch, id, pos, cx));
    }
}

/// Reaction pills; click toggles ours. Ends with the always-visible "add
/// reaction" button (`forceShow_`).
pub fn reactions(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> AnyElement {
    let _ = app;
    let (ch, id) = (msg.channel, msg.id);
    // reactions_: padding-top .125rem; reaction_ margin-inline-end / margin-bottom 4px.
    // reactions_ padding-bottom .375rem + the last row's 4px reaction_ margin-bottom,
    // margin-bottom -.5rem.
    let mut row = div().pt(z(2.)).pb(z(10.)).mb(z(-8.)).flex().flex_wrap().gap(z(4.)).items_center();
    for (ix, r) in msg.reactions.iter().enumerate() {
        let e = entity.clone();
        let rc = r.clone();
        // reaction_ .emoji 20px, margin 2px 0.
        let emoji_el = div().my(z(2.)).child(crate::media::emoji::emoji_element(&r.emoji, 20.));
        let fade_key = crate::ui::states::key(("reaction", msg.id.0, ix));
        row = row.child(
            // reaction_: radius 8px, 1px transparent border, --reaction-background-default;
            // :hover --reaction-background-hover / --reaction-border-hover; .reactionMe_
            // --reaction-background-reacted-default / --reaction-border-reacted-default.
            // reactionInner_ padding 2px 6px. reactionCount_ semibold --text-subtle
            // (reacted: --reaction-text-reacted-default), margin-inline-start 6px.
            crate::ui::states::track(div().id(("reaction", ix)), fade_key)
                .relative()
                .py(z(2.))
                .px(z(6.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(6.))
                .border_1()
                .when(r.me, |d| {
                    // `.theme-light .reaction_.reactionMe_`: --brand-160 on --brand-500.
                    if crate::theme::is_light() {
                        d.bg(tk(idx::BRAND_160)).border_color(tk(idx::BRAND_500))
                    } else {
                        d.bg(tk(idx::REACTION_BACKGROUND_REACTED_DEFAULT))
                            .border_color(tk(idx::REACTION_BORDER_REACTED_DEFAULT))
                    }
                })
                .when(!r.me, |d| {
                    // reaction_MmKct8 transition: background-color .1s ease (layer below).
                    d.child(crate::ui::states::fade_layer_pressed(
                        fade_key,
                        tk(idx::REACTION_BACKGROUND_DEFAULT),
                        tk(idx::REACTION_BACKGROUND_HOVER),
                        Some(("reaction", tk(idx::REACTION_BACKGROUND_ACTIVE))),
                        100,
                        8.,
                    ))
                    .border_color(gpui::transparent_black())
                    .hover(|s| s.border_color(tk(idx::REACTION_BORDER_HOVER)))
                    // `.high-contrast-mode .reaction_MmKct8`: --border-subtle, hover --border-strong.
                    .when(crate::theme::high_contrast(), |d| {
                        d.border_color(tk(idx::BORDER_SUBTLE)).hover(|s| s.border_color(tk(idx::BORDER_STRONG)))
                    })
                })
                // reaction_MmKct8:active background / border
                .active(|s| s.bg(tk(idx::REACTION_BACKGROUND_ACTIVE)).border_color(tk(idx::REACTION_BORDER_ACTIVE)))
                .cursor_pointer()
                .group("reaction")
                .child(emoji_el)
                .child(
                    div()
                        // reactionCount_ has no font-size: inherits 16px / line-height 16px
                        // (vanilla-guild-chat-busy #1160: "156" box 36x16, weight 600).
                        .text_size(z(16.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        // `.theme-light .reactionMe_ .reactionCount_`: --brand-560.
                        .text_color(tk(match (r.me, crate::theme::is_light()) {
                            (true, true) => idx::BRAND_560,
                            (true, false) => idx::REACTION_TEXT_REACTED_DEFAULT,
                            _ => idx::TEXT_SUBTLE,
                        }))
                        // reaction_:hover / :active .reactionCount_ colours
                        .group_hover("reaction", |s| s.text_color(tk(idx::REACTION_TEXT_HOVER)))
                        .id("count")
                        .group_active("reaction", |s| s.text_color(tk(idx::REACTION_TEXT_ACTIVE)))
                        .w(z(12. * r.count.to_string().len() as f32)) // vanilla-guild-chat-busy: reactionCount_ is 12px per digit ("1" 12, "79" 24, "156" 36)
                        .text_center() // reactionCount_ text-align center
                        .child(SharedString::from(r.count.to_string())),
                )
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_reaction(ch, id, &rc, cx))),
        );
    }
    let e = entity.clone();
    row.child(
        div()
            // vanilla-guild-text #1275 `reactionBtn_MmKct8 forceShow_`: always shown,
            // padding .3125rem .4375rem, 1px transparent border, radius .5rem,
            // --reaction-background-default, 18px icon in --reaction-text-default;
            // :hover --reaction-background-hover / --reaction-border-hover.
            .id("reaction-add")
            .py(z(5.))
            .px(z(7.))
            .rounded(z(8.))
            .border_1()
            .border_color(gpui::transparent_black())
            .flex()
            .items_center()
            .bg(tk(idx::REACTION_BACKGROUND_DEFAULT))
            .cursor_pointer()
            .hover(|s| s.bg(tk(idx::REACTION_BACKGROUND_HOVER)).border_color(tk(idx::REACTION_BORDER_HOVER)))
            .active(|s| s.bg(tk(idx::REACTION_BACKGROUND_ACTIVE)).border_color(tk(idx::REACTION_BORDER_ACTIVE))) // reactionBtn_:active
            // `.high-contrast-mode .reactionBtn_MmKct8.forceShow_`: --border-subtle, hover --border-strong.
            .when(crate::theme::high_contrast(), |d| {
                d.border_color(tk(idx::BORDER_SUBTLE)).hover(|s| s.border_color(tk(idx::BORDER_STRONG)))
            })
            .child(icon(ChatIcon::AddReaction, 18., tk(idx::REACTION_TEXT_DEFAULT)))
            .on_click(move |_, _, cx| {
                e.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Reaction { channel: ch, msg: id }, cx))
            }),
    )
    .into_any_element()
}

/// Inline edit box that replaces the message content.
pub fn edit_box(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> Option<Div> {
    let ed = app.chat.edit.as_ref().filter(|e| e.id == msg.id)?;
    let e1 = entity.clone();
    let e2 = entity.clone();
    let link = |id: &'static str, label: gpui::SharedString| {
        div()
            .id(id)
            .text_color(tk(idx::TEXT_LINK)) // operations_ > a --text-link
            .cursor_pointer()
            .hover(|s| s.underline())
            .child(label)
    };
    Some(
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                // Inline edit (live sandbox 2026-11-24): channelTextArea_SY_P2T margin-top 8,
                // radius 8, 1px --border-subtle, 56px tall (textArea 16/22).
                div()
                    .id("edit-box")
                    .mt(z(8.))
                    .min_h(z(56.))
                    .px(z(16.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .bg(tk(idx::CHAT_BACKGROUND_DEFAULT))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .text_color(c(Palette::TEXT_NORMAL))
                    .capture_key_down({
                        let e = entity.clone();
                        move |ev, w, cx| {
                            if ev.keystroke.key == "escape" {
                                cx.stop_propagation();
                                e.update(cx, |a, cx| a.cancel_edit(w, cx));
                            }
                        }
                    })
                    .child(div().flex_1().min_w_0().child(Input::new(&ed.input).appearance(false).bordered(false)))
                    .child(ui::icon(Icon::Emoji, 22., c(Palette::INTERACTIVE))),
            )
            .child(
                // operations_h_zE4S: 12px / 12px, weight 400 (live), --text-default, padding
                // 7px 0; links --text-link.
                div()
                    .py(z(7.))
                    .flex()
                    .gap(z(4.))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(gpui::FontWeight::NORMAL)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(crate::t!("edit.escape_to"))
                    .child(link("edit-cancel", crate::t!("edit.cancel")).on_click(move |_, w, cx| e1.update(cx, |a, cx| a.cancel_edit(w, cx))))
                    .child(crate::t!("edit.enter_to"))
                    .child(link("edit-save", crate::t!("edit.save")).on_click(move |_, w, cx| e2.update(cx, |a, cx| a.save_edit(w, cx)))),
            ),
    )
}

/// Thread card under its starter (or "a commencé un fil" system) message, measured live
/// 2026-11-24 in the sandbox channel (container_g2fiji): margin-top 8 (0 under a system
/// line), padding 8, radius 4, --background-surface-high, 1px --border-subtle, max-width
/// min(100%, 480). topLine_: name body-sm/semibold 14/18 --text-strong (margin-inline-end 8,
/// ellipsis) then cta_ "N messages ›" (rfAXDV) --text-link, underlined on card hover.
/// bottomLine_ (margin-top 2, body-sm/normal 14/18 --text-default): the latest message
/// (ThreadMessageStore) = 16px avatar (margin-inline-end 8), name 500 at opacity .64
/// (margin-inline-end 4), content with ellipsis, then timestamp_ "Il y a 2 min"
/// (--text-muted, margin-inline-start 8, no shrink).
pub fn thread_chip(app: &PhoenixApp, entity: &Entity<PhoenixApp>, t: &ThreadRef, system: bool) -> AnyElement {
    let e = entity.clone();
    let t2 = t.clone();
    let recent = crate::thread_recent::get(t.id);
    let count = recent.as_ref().map_or(t.message_count, |_| t.message_count.max(1));
    let group = SharedString::from(format!("thread-card-{}", t.id.0));
    // rfAXDV, CLDR "one" (French: 0 and 1).
    let one = if crate::i18n::locale() == "fr" { count < 2 } else { count == 1 };
    let cta = if one { crate::t!("thread.one_message_cta", n = count) } else { crate::t!("thread.messages_cta", n = count) };
    let bottom = recent.map(|r| {
        let img = app.users.get(&r.author).map_or_else(|| r.avatar.clone(), |u| u.avatar.clone());
        let now = chrono::Utc::now().timestamp();
        div()
            .mt(z(2.))
            .h(z(18.))
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .flex_initial()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .child(div().mr(z(8.)).child(ui::avatar_ref(img, 16.)))
                    .child(div().flex_none().mr(z(4.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).opacity(0.64).child(r.name.clone()))
                    .child(div().min_w_0().text_ellipsis().whitespace_nowrap().child(SharedString::from(r.content.replace('\n', " ")))),
            )
            .child(div().flex_none().ml(z(8.)).text_color(tk(idx::TEXT_MUTED)).child(crate::thread_recent::ago(now - r.ts)))
    });
    div()
        .id(("thread-chip", t.id.0))
        .group(group.clone())
        .when(!system, |d| d.mt(z(8.)))
        .max_w(z(480.))
        .p(z(8.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .cursor_pointer()
        .child(
            div()
                .flex()
                .h(z(18.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().min_w_0().mr(z(8.)).text_ellipsis().whitespace_nowrap().text_color(tk(idx::TEXT_STRONG)).child(t.name.clone()))
                .child(div().flex_none().text_color(tk(idx::TEXT_LINK)).group_hover(group, |s| s.underline()).child(cta)),
        )
        .children(bottom)
        .on_click(move |_, w, cx| {
            let t = t2.clone();
            e.update(cx, |a, cx| a.open_thread(t, w, cx));
        })
        .into_any_element()
}

/// .cozy_.hasThread_::after: the L from under the avatar to the thread card, 2px
/// --spine-default left and bottom borders, bottom-left radius 8, bottom 29, at
/// left = margin-horizontal + avatar / 2 - 1 and width = content margin minus
/// margin-horizontal, avatar / 2 and 4 (39 and 36 live); top 3rem (4.375rem with a reply),
/// 1.75rem on a system line.
pub fn thread_spine(top: f32) -> Div {
    let mh = crate::theme::space::md();
    let av = super::msg_avatar_size();
    div()
        .absolute()
        .left(z(mh + av / 2. - 1.))
        .top(crate::theme::fz(top))
        .bottom(z(29.))
        .w(z(super::msg_left() - mh - av / 2. - 4.))
        .border_l_2()
        .border_b_2()
        .rounded_bl(z(8.))
        .border_color(tk(idx::SPINE_DEFAULT))
}

/// Red "NEW" line drawn above the first unread message (divider_ .isUnread_, no content).
/// .group-spacing-16: before a group start (`beforeGroup_`) it takes no room (margin-top 0,
/// margin-bottom -1px, `top: .5rem`, so it sits in the group gap); else margin 4px top/bottom.
pub fn new_divider(before_group: bool) -> AnyElement {
    // divider_ margin-inline 1rem .875rem; .isUnread_ border --divider-color
    // (--background-feedback-notification), border-top thin.
    let line = div().ml(z(16.)).mr(z(14.)).h(z(1.)).bg(tk(idx::BACKGROUND_FEEDBACK_NOTIFICATION)).relative().child(unread_end_cap());
    if before_group {
        div().h_0().relative().child(line.absolute().left_0().right_0().top(z(8.))).into_any_element()
    } else {
        line.my(z(4.)).into_any_element()
    }
}

/// endCap_ .unreadPill_: 13px tall, 10px bold uppercase, line-height 9px, padding-inline
/// 1px 4px, radius 0 4px 4px 0, top -7px, inset-inline-end 0, white on --divider-color;
/// unreadPillCap_ 13px tall at inset-inline-start -8px.
pub fn unread_end_cap() -> gpui::Div {
    let red = tk(idx::BACKGROUND_FEEDBACK_NOTIFICATION);
    div()
        .absolute()
        .right_0()
        .top(z(-7.))
        .h(z(13.))
        .pl(z(1.))
        .pr(z(4.))
        .rounded_r(z(4.))
        .bg(red)
        .text_size(z(10.))
        .line_height(z(9.))
        .font_weight(FontWeight::BOLD)
        .text_color(tk(idx::WHITE))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path("chat/unread-cap")).absolute().left(z(-8.)).top_0().w(z(8.)).h(z(13.)).text_color(red))
        .child(crate::t!("chat.new").to_uppercase())
}

/// Click on an author's avatar or name: open the profile popout there.
pub fn open_author(entity: &Entity<PhoenixApp>, msg: &Message) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let e = entity.clone();
    let (user, ch) = (msg.author, msg.channel);
    move |ev, w, cx| {
        let pos = ev.position();
        e.update(cx, |a, cx| {
            let guild = a.channels.get(&ch).and_then(|c| c.guild);
            a.open_profile(user, guild, pos, w, cx)
        });
    }
}

/// `range` of `text` cut after each run of whitespace ("a b  c" -> "a ", "b  ", "c").
pub(crate) fn words(text: &str, range: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = range.start;
    let mut in_space = false;
    for (i, ch) in text[range.clone()].char_indices() {
        let i = range.start + i;
        if ch.is_whitespace() {
            in_space = true;
        } else if in_space {
            out.push(start..i);
            start = i;
            in_space = false;
        }
    }
    if start < range.end {
        out.push(start..range.end);
    }
    out
}

/// Byte ranges of the inline code spans of `t` (the only monospace runs of message text),
/// except inside a hidden spoiler (text drawn in its background colour).
pub(crate) fn code_ranges(t: &crate::richtext::RichText) -> Vec<std::ops::Range<usize>> {
    let mono = gpui::font(crate::theme::FONT_MONO()).family;
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    let mut pos = 0;
    for r in &t.runs {
        let (s, e) = (pos, pos + r.len);
        pos = e;
        if r.font.family != mono || r.background_color == Some(r.color) {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == s => last.end = e,
            _ => out.push(s..e),
        }
    }
    out
}

/// `range` cut at the boundaries of the `code` spans: (piece, is code).
pub(crate) fn split_code(code: &[std::ops::Range<usize>], range: std::ops::Range<usize>) -> Vec<(std::ops::Range<usize>, bool)> {
    let mut out = Vec::new();
    let mut at = range.start;
    for c in code {
        let (a, b) = (c.start.max(range.start), c.end.min(range.end));
        if a >= b {
            continue;
        }
        if at < a {
            out.push((at..a, false));
        }
        out.push((a..b, true));
        at = b;
    }
    if at < range.end {
        out.push((at..range.end, false));
    }
    out
}

/// One word of an inline code span. `.markup_ code.inline` (2026 CSS): font --font-code 85%,
/// padding 0 .2em, margin -.2em 0 (no effect on the 22px line), radius 4px, `.markup_ code`
/// --background-code + 1px solid --border-normal, `box-decoration-break: slice` (computed):
/// a span cut over several flex items keeps its start edge on the first one and its end edge
/// on the last one. Live dump (#phoenix, 24/11): 13.6px text in a 19.2px box.
fn code_box(open: bool, close: bool, k: f32, child: AnyElement) -> Div {
    let fs = 16. * 0.85 * k;
    let pad = 0.2 * fs;
    let b = div()
        .h(z(19.2 * k))
        .flex()
        .items_center()
        .text_size(z(fs))
        .line_height(z(17.6 * k))
        .bg(tk(idx::BACKGROUND_CODE))
        .border_color(tk(idx::BORDER_NORMAL))
        .border_t(z(1.))
        .border_b(z(1.))
        .when(open, |d| d.border_l(z(1.)).rounded_l(z(4.)).pl(z(pad)))
        .when(close, |d| d.border_r(z(1.)).rounded_r(z(4.)).pr(z(pad)))
        .child(child);
    div().flex_none().max_w_full().h(z(22. * k)).flex().items_center().child(b)
}

/// Text runs covering `range` of a run list that covers the whole text.
pub(crate) fn slice_runs(runs: &[gpui::TextRun], range: std::ops::Range<usize>) -> Vec<gpui::TextRun> {
    let mut out = Vec::new();
    let mut pos = 0;
    for r in runs {
        let (s, e) = (pos, pos + r.len);
        pos = e;
        let (a, b) = (s.max(range.start), e.min(range.end));
        if a < b {
            let mut n = r.clone();
            n.len = b - a;
            out.push(n);
        }
    }
    out
}

/// Rich text with inline emoji images (jumbo when the block is emoji-only).
pub fn rich_with_emoji(
    entity: &Entity<PhoenixApp>,
    msg: crate::model::Id,
    bi: usize,
    t: &crate::richtext::RichText,
) -> AnyElement {
    rich_with_emoji_scaled(entity, msg, bi, t, 1.)
}

/// [`rich_with_emoji`] with the text and emoji scaled by `k` (0.875 in a
/// root_ sizeReduced_ container: 14px text, emoji 1.375em = 19.25px).
pub fn rich_with_emoji_scaled(
    entity: &Entity<PhoenixApp>,
    msg: crate::model::Id,
    bi: usize,
    t: &crate::richtext::RichText,
    k: f32,
) -> AnyElement {
    let jumbo = crate::richtext::is_jumbo(&[crate::richtext::RBlock::Text(t.clone())]);
    // Text, line height and emoji are rem / em based: they follow the font size setting too
    // (a 24px client rendered emoji messages at 16px).
    let k = k * crate::theme::font_scale();
    // markup_ .emoji 1.375em, jumbo 3em
    let size = k * if jumbo { 48. } else { 22. };
    let mut row = div().flex().flex_wrap().items_center().text_size(z(16. * k)).line_height(z(22. * k));
    let mut last = 0;
    let mut seg = 0usize;
    let code = code_ranges(t);
    // `code`: this word is (part of) an inline code span; (opens it, closes it).
    let push_text = |row: Div, range: std::ops::Range<usize>, seg: &mut usize, code: Option<(bool, bool)>| -> Div {
        if range.is_empty() {
            return row;
        }
        let text: SharedString = t.text[range.clone()].to_owned().into();
        let mut seg_runs = slice_runs(&t.runs, range.clone());
        if code.is_some() {
            // The box below draws --background-code (a search highlight stays on the text).
            let bg = Some(c(crate::theme::tok(idx::BACKGROUND_CODE)));
            for r in seg_runs.iter_mut().filter(|r| r.background_color == bg) {
                r.background_color = None;
            }
        }
        // Click ranges fully inside this segment, shifted.
        let mut ranges = Vec::new();
        let mut targets = Vec::new();
        for (cr, tg) in t.click_ranges.iter().zip(t.click_targets.iter()) {
            // Overlapping ranges are clamped: a channel mention's "#" is drawn as an icon,
            // so its segment starts after it.
            if cr.start < range.end && cr.end > range.start {
                ranges.push(cr.start.max(range.start) - range.start..cr.end.min(range.end) - range.start);
                targets.push(tg.clone());
            }
        }
        *seg += 1;
        // Each segment is a flex item of the wrapping row. Taffy may shrink a segment that
        // follows an emoji / channel icon to the rest of the line, but the text keeps the
        // wrap of its wider intrinsic measure and spills past the message column (real
        // announcements with a #channel mention). Never shrink: a segment that doesn't fit
        // wraps to the next flex line, at most the full column wide.
        let cell = |child: AnyElement| match code {
            None => div().flex_none().max_w_full().child(child),
            Some((open, close)) => code_box(open, close, k, child),
        };
        if ranges.is_empty() {
            return row.child(cell(gpui::StyledText::new(text).with_runs(seg_runs).into_any_element()));
        }
        let sd = *seg as u64;
        let seg_runs = super::mention::hovered_runs(msg, bi, sd, &ranges, &targets, &seg_runs).unwrap_or(seg_runs);
        let styled = gpui::StyledText::new(text).with_runs(seg_runs);
        let e = entity.clone();
        let (hr, ht) = (ranges.clone(), targets.clone());
        row.child(cell(
            gpui::InteractiveText::new(("rte", msg.0.wrapping_mul(31).wrapping_add(((bi as u64) << 32) | *seg as u64)), styled)
                .on_hover(move |ix, _ev, w, _cx| super::mention::on_hover(msg, bi, sd, &hr, &ht, ix, w))
                .on_click(
                ranges,
                move |ix, w, cx| match &targets[ix] {
                    crate::richtext::ClickTarget::Link(url) => cx.open_url(url),
                    crate::richtext::ClickTarget::Spoiler(sp) => {
                        let sp = *sp;
                        e.update(cx, |a, cx| a.reveal_spoiler(msg, sp, cx));
                    }
                    crate::richtext::ClickTarget::Mention(m) => super::mention::click(&e, m, w, cx),
                },
            )
            .into_any_element(),
        ))
    };
    // A segment holding a newline would be one tall flex item: the next emoji/icon then sits
    // centred beside the whole block and every following line is indented by it (real
    // announcements). Split at newlines and break the flex line instead, with an empty
    // line-height row for blank lines.
    let push_lines = |mut row: Div, range: std::ops::Range<usize>, seg: &mut usize| -> Div {
        let mut start = range.start;
        for (i, piece) in t.text[range.clone()].split('\n').enumerate() {
            let r = start..start + piece.len();
            if i > 0 {
                row = row.child(div().w_full().h_0());
            }
            // An empty line (between two newlines, or a newline opening the block) keeps
            // its line height, as in the single StyledText it replaces.
            if piece.is_empty() && r.end < range.end && (i > 0 || range.start == 0) {
                row = row.child(div().w_full().h(z(22. * k)));
            }
            // One flex item per word, so text after an emoji keeps flowing on the same line
            // like Chromium's inline layout instead of starting a new one. Inline code spans
            // are cut out first: each of their words is drawn in a slice of the code box.
            for (piece, in_code) in split_code(&code, r.clone()) {
                let ws = words(&t.text, piece.clone());
                let n = ws.len();
                for (wi, w) in ws.into_iter().enumerate() {
                    let edge = in_code.then(|| {
                        let span = code.iter().find(|c| c.start <= piece.start && piece.end <= c.end);
                        let open = wi == 0 && span.is_some_and(|c| c.start == piece.start);
                        let close = wi + 1 == n && span.is_some_and(|c| c.end == piece.end);
                        (open, close)
                    });
                    row = push_text(row, w, seg, edge);
                }
            }
            start = r.end + 1;
        }
        row
    };
    for (r, token) in &t.emoji {
        if r.start < last {
            continue;
        }
        row = push_lines(row, last..r.start, &mut seg);
        if token.as_ref() == crate::richtext::CHANNEL_ICON_TOKEN {
            // The icon and the channel name stay one unit: as separate flex items the "#"
            // ended a line and the name started the next one.
            let end = t.click_ranges.iter().find(|c| c.start == r.start).map_or(r.end, |c| c.end);
            // icon__RYHZ5: 1em square, margin-inline-end --space-4, inside the
            // .mention pill (--mention-background, radius 3px, padding-left 2px).
            let pill = div().flex().flex_none().items_center().child(
                div()
                    .h(z(22.))
                    .flex()
                    .items_center()
                    .pl(z(2.))
                    .pr(z(4.))
                    .rounded_l(z(3.))
                    .bg(crate::theme::ca(crate::theme::Palette::MENTION_BG))
                    .child(crate::ui::icon(crate::assets::Icon::Hash, 16., crate::theme::c(crate::theme::Palette::MENTION_FG))),
            );
            row = row.child(push_text(pill, r.end..end, &mut seg, None));
            last = end;
            continue;
        }
        // `.emoji.jumboable` padding 4px 0
        row = row.child(div().mx(z(1.)).when(jumbo, |d| d.py(z(4.))).child(crate::media::emoji::emoji_element(token, size)));
        last = r.end;
    }
    row = push_lines(row, last..t.text.len(), &mut seg);
    row.into_any_element()
}
