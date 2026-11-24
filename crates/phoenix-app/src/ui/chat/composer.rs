//! Message composer (moved out of `chat.rs`): reply bar with the mention
//! toggle, upload tray, upload progress, attach button, paste-image, typing
//! indicator.

use std::time::Duration;

use gpui_component::Sizable as _;

use gpui::{
    Animation, AnimationExt, Context, FontWeight, IntoElement, KeyDownEvent, ParentElement, SharedString,
    Styled, Window, div, img, prelude::*, relative,
};

use super::{
    icons::{ChatIcon, icon},
    state::EmojiTarget,
};
use crate::{
    app::PhoenixApp,
    assets::Icon,
    theme::{Palette, c, generated::idx, z},
    ui::icon_button,
};

fn human_size(n: u64) -> SharedString {
    if n >= 1 << 20 {
        format!("{:.2} MB", n as f64 / (1 << 20) as f64).into()
    } else {
        format!("{:.2} KB", n as f64 / 1024.).into()
    }
}

fn reply_bar(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<impl IntoElement + use<>> {
    let rid = app.replying_to?;
    let e = app.messages.get(&app.selected_channel?)?;
    let m = e.msgs.iter().find(|m| m.id == rid)?;
    let name = app.users.get(&m.author).map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("common.unknown"));
    let own = m.author == app.me.id;
    let mention = app.chat.reply_mention;
    Some(
        // replyBar_: transparent (background-color unset), top radius 8px; text_ margin-inline-start
        // --space-16; closeButton_ / mentionButton_ padding 8px 12px; actions_ margin-end --space-4.
        // vanilla-reply-bar: 34px row (mentionButton_ 18 + 8+8 padding); replyLabel_
        // body-sm/normal --text-default, name_ semibold, white-space pre.
        div()
            .h(z(34.))
            .pl(z(16.))
            .pr(z(4.))
            .rounded_t(z(8.))
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(super::tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .flex()
                    .child(crate::t!("chat.replying_to"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().child(name)),
            )
            .when(!own, |d| {
                d.child(
                    div()
                        .id("reply-mention")
                        .px(z(12.))
                        .h(z(32.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        // mentionButton_ body-sm/semibold; mentionIcon_ 16px, margin-inline-end 4.
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(if mention { Palette::TEXT_LINK } else { Palette::TEXT_MUTED }))
                        .child(gpui::svg().path(crate::icons::path("search/at")).size(z(16.)).mr(z(4.)).flex_none().text_color(c(if mention { Palette::TEXT_LINK } else { Palette::TEXT_MUTED })))
                        .child(if mention { crate::t!("chat.on") } else { crate::t!("chat.off") })
                        .on_click(cx.listener(|a, _, _, cx| {
                            a.chat.reply_mention = !a.chat.reply_mention;
                            cx.notify();
                        })),
                )
                .child(div().w(z(1.)).h(z(20.)).bg(super::tk(idx::BORDER_SUBTLE))) // separator_ 1x20
            })
            .child(
                icon_button("cancel-reply", Icon::Close, super::tk(idx::INTERACTIVE_TEXT_DEFAULT), 16.).px(z(12.)).on_click(cx.listener(
                    |a, _, _, cx| {
                        a.replying_to = None;
                        cx.notify();
                    },
                )),
            ),
    )
}

fn upload_tray(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<impl IntoElement + use<>> {
    if app.chat.uploads.is_empty() {
        return None;
    }
    // channelAttachmentArea_: flex, gap 24, padding 20 10 10, overflow-x auto.
    let mut tray = div()
        .id("upload-tray")
        .pt(z(20.))
        .px(z(10.))
        .pb(z(10.))
        .flex()
        .gap(z(24.))
        .overflow_x_scroll();
    for (ix, f) in app.chat.uploads.iter().enumerate() {
        let shown = if f.spoiler { f.blurred.as_ref().or(f.image.as_ref()) } else { f.image.as_ref() };
        let preview = match shown {
            Some(i) => img(i.clone())
                .size_full()
                .rounded(z(12.)) // media_ border-radius --radius-sm
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            None => icon(ChatIcon::Upload, 64., super::tk(idx::TEXT_BRAND)).into_any_element(),
        };
        // actionBar_ (hover-bar popover): --background-surface-high, 1px --border-muted,
        // radius 8, padding 2; translate(25%, -25%) from the card's top-right corner.
        let button = |id: (&'static str, usize), glyph: ChatIcon, color: gpui::Hsla| {
            div()
                .id(id)
                .size(z(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(z(6.))
                .cursor_pointer()
                .hover(|s| s.bg(super::tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .child(icon(glyph, 20., color)) // actionBarIcon_ 20px
        };
        let bar = div()
            .absolute()
            .top(z(-16.))
            .right(z(-16.))
            .p(z(2.))
            .flex()
            .rounded(z(8.))
            .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
            .border_1()
            .border_color(super::tk(idx::BORDER_MUTED))
            .shadow(crate::ui::inbox::low_shadow())
            .child(
                button(("up-spoiler", ix), ChatIcon::Eye, super::tk(if f.spoiler { idx::TEXT_BRAND } else { idx::INTERACTIVE_TEXT_DEFAULT }))
                    .on_click(cx.listener(move |a, _, _, cx| a.toggle_upload_spoiler(ix, cx))),
            )
            .child(
                // Client order: Spoiler, Edit (file name), Remove.
                button(("up-edit", ix), ChatIcon::Edit, super::tk(idx::INTERACTIVE_TEXT_DEFAULT))
                    .on_click(cx.listener(move |a, _, _, cx| open_upload_edit(a, ix, cx))),
            )
            .child(
                button(("up-remove", ix), ChatIcon::Trash, super::tk(idx::TEXT_FEEDBACK_CRITICAL))
                    .on_click(cx.listener(move |a, _, _, cx| a.remove_upload(ix, cx))),
            );
        tray = tray.child(
            // upload_JyxBSo: 200x200, padding 8, --background-surface-high, 1px --border-normal,
            // radius --radius-sm (12), --shadow-low.
            div()
                .relative()
                .flex_none()
                .w(z(200.))
                .h(z(200.))
                .p(z(8.))
                .rounded(z(12.))
                .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
                .border_1()
                .border_color(super::tk(idx::BORDER_NORMAL))
                .shadow(crate::ui::inbox::low_shadow())
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .rounded(z(12.))
                        // spoiler_: blur(--custom-channel-attachment-upload-spoiler-blur-radius 44px),
                        // rendered as a pre-blurred copy (ui/chat/spoiler.rs).
                        // Until the blurred copy is ready, the raw preview is dimmed.
                        .when(f.spoiler && f.blurred.is_none(), |d| d.opacity(0.15).bg(super::tk(idx::BACKGROUND_MOD_STRONG)))
                        .child(preview),
                )
                .child(
                    // filenameContainer_ margin-top auto; filename_ margin-top 8, ellipsis.
                    div()
                        .mt(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(super::tk(idx::TEXT_DEFAULT))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(f.name.clone()),
                )
                .child(bar),
        );
    }
    Some(tray)
}

/// In-flight uploads: file name, size, animated progress bar.
fn sending(app: &PhoenixApp) -> impl IntoElement {
    let ch = app.selected_channel;
    let mut col = div().flex().flex_col().gap(z(8.));
    for s in app.chat.sending.iter().filter(|s| Some(s.channel) == ch) {
        col = col.child(
            div()
                .mx(z(16.))
                .p(z(12.))
                .rounded(z(8.))
                .bg(c(Palette::BG_SECONDARY))
                .flex()
                .items_center()
                .gap(z(12.))
                .child(icon(ChatIcon::Upload, 32., c(Palette::BLURPLE)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(6.))
                        .child(
                            div()
                                .flex()
                                .gap(z(8.))
                                .text_size(z(14.))
                                .child(div().text_color(c(Palette::TEXT_NORMAL)).text_ellipsis().child(s.label.clone()))
                                .child(div().text_color(c(Palette::TEXT_MUTED)).child(human_size(s.size))),
                        )
                        .child(
                            div().h(z(6.)).rounded(z(3.)).bg(c(Palette::INTERACTIVE_MUTED)).overflow_hidden().child(
                                div().h_full().rounded(z(3.)).bg(c(Palette::BLURPLE)).with_animation(
                                    ("upload-bar", s.key as usize),
                                    Animation::new(Duration::from_millis(1400)).repeat(),
                                    |d, t| d.w(relative(0.15 + 0.8 * t)),
                                ),
                            ),
                        ),
                ),
        );
    }
    col
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> gpui::AnyElement {
    if let Some(notice) = blocked_dm_notice(app, cx) {
        return notice;
    }
    render_text_area(app, window, cx).into_any_element()
}

/// 1:1 DM with a user we blocked: the client swaps the text area for its
/// chat blocker ("Tu ne peux pas envoyer de message à un utilisateur que tu
/// as bloqué." + secondary sm "Débloquer"), CSS wrapper_ST_Y69.
fn blocked_dm_notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    use crate::ui::home::data::{NavRequest, RelAction, RelationshipKind};
    let user = crate::ui::dm_profile_panel::peer(app)?;
    if app.nav.relationship(user)?.kind != RelationshipKind::Blocked {
        return None;
    }
    let unblock = crate::ui::controls::button_sm("dm-unblock", crate::t!("friends.unblock"), crate::ui::controls::BtnKind::Secondary)
        .on_click(cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            a.nav.relationships.retain(|x| x.user != user);
            a.nav_request(NavRequest::Relationship { user, action: RelAction::Remove }, cx, |_, _, _| {});
            cx.notify();
        }));
    Some(
        // form_ padding-inline --space-xs, margin-bottom --custom-chat-input-margin-bottom (as the text area).
        div()
            .flex_none()
            .px(z(crate::theme::space::xs()))
            .mb(z(crate::theme::space::xs()))
            .child(
                // wrapper_ST_Y69: --background-base-low, 1px --border-subtle, --radius-sm,
                // height --space-32 (border-box), padding 12px 0, margin-bottom --space-8,
                // flex space-between, align-items center.
                div()
                    .h(z(32.))
                    .mb(z(8.))
                    .py(z(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(z(8.))
                    .border_1()
                    .border_color(super::tk(idx::BORDER_SUBTLE))
                    .bg(super::tk(idx::BACKGROUND_BASE_LOW))
                    .child(
                        // content_ST_Y69: height 20, margin-inline-start 16; title_ heading-sm/semibold, 2-line clamp.
                        div().h(z(20.)).ml(z(16.)).min_w_0().flex().items_center().child(
                            div()
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(super::tk(idx::TEXT_STRONG))
                                .child(crate::t!("chat.blocked_dm")),
                        ),
                    )
                    // buttonContainer_ margin-inline-end 12; button_ margin-inline-start 12.
                    .child(div().flex().flex_none().mr(z(12.)).child(div().flex().items_center().ml(z(12.)).child(unblock))),
            )
            .into_any_element(),
    )
}

fn render_text_area(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    let placeholder: SharedString = match ch {
        // Group DMs read "Message <name>" (no @), like threads.
        Some(c) if c.guild.is_none() && crate::ui::dm_profile_panel::is_group_dm(app, c.id) => crate::t!("chat.placeholder_thread", name = c.name),
        Some(c) if c.guild.is_none() => crate::t!("chat.placeholder_dm", name = c.name),
        // Voice channel text chat: no "#" prefix (the name follows the voice icon).
        Some(c) if matches!(c.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage) => {
            crate::t!("chat.placeholder_voice", name = c.name)
        }
        // Threads quote their name (vanilla-thread-channel composer).
        Some(c) if app.nav.thread_parent(c.id).is_some() => crate::t!("chat.placeholder_thread_quoted", name = c.name),
        Some(c) => crate::t!("chat.placeholder_channel", name = c.name),
        None => "".into(),
    };
    let ic = c(Palette::INTERACTIVE);
    let placeholder_empty = app.composer.read(cx).value().is_empty();
    // Client CharacterCount: max 4000 with Nitro (canUseIncreasedMessageLength), else 2000.
    let (remaining, show_count) = char_count(app.backend.has_premium(), app.composer.read(cx).value().chars().count() as i64);
    let reply = reply_bar(app, cx);
    let has_banner = reply.is_some();
    let tray = upload_tray(app, cx);
    // No SEND_MESSAGES: disabled text area with the client's notice, no buttons (CSS only).
    let blocked = app.selected_channel.and_then(|c| app.nav.extras.get(&c)).is_some_and(|x| x.send_blocked);
    let show_send_button = should_show_send_button(app.prefs.local.flag("send-btn", false), placeholder_empty, blocked);
    let placeholder = if blocked { crate::t!("chat.no_permission") } else { placeholder };
    div()
        .flex_none()
        .relative()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(sending(app))
        .children(super::slash::render(app, cx))
        .child(
            // form_ padding-inline --space-xs; channelTextArea_ --chat-background-default,
            // 1px --border-muted, radius --radius-sm, margin-bottom --custom-chat-input-margin-bottom.
            // #1432 form padding-inline --space-xs; #1435 margin-bottom --custom-chat-input-margin-bottom (= --space-xs).
            // The typing row (inTextChannel_U9Tvvz) hangs above the form: top -24px, over the message list.
            div().relative().px(z(crate::theme::space::xs())).mb(z(crate::theme::space::xs()))
            .child(div().absolute().top(z(-24.)).left_0().right_0().child(typing(app, window)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(z(8.))
                    .border_1()
                    .border_color(super::tk(idx::BORDER_MUTED))
                    .bg(super::tk(idx::CHAT_BACKGROUND_DEFAULT))
                    .relative() // autocompleteAttached_ is positioned against channelTextArea_
                    .child(super::width_probe(&TEXT_AREA_W))
                    .children(crate::ui::autocomplete::render(app, cx))
                    .children(reply)
                    .children(tray)
                    .child(
                        div()
                            .id("composer")
                            .min_h(z(56.)) // #1437 56px (17px padding + 22px line)
                            .when(has_banner, |d| d.rounded_b(z(8.)))
                            .relative()
                            // Client CharacterCount: see char_count;
                            // characterCount_ absolute bottom 10 / inset-inline-end 14; body-sm/semibold
                            // tabular, --text-default, --text-feedback-critical when over the limit.
                            .when(show_count, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .right(z(14.))
                                        .bottom(z(10.))
                                        .text_size(z(14.))
                                        .line_height(z(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(super::tk(if remaining < 0 { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT }))
                                        .child(SharedString::from(remaining.to_string())),
                                )
                            })
                            .flex()
                            // buttons_/attachWrapper_ are text-area-height rows pinned to the top
                            // (position sticky, top 0) while the text grows below.
                            .items_start()
                            .capture_key_down(cx.listener(|a, ev: &KeyDownEvent, w, cx| {
                                let k = &ev.keystroke;
                                if k.modifiers.number_of_modifiers() == 0 && super::slash::on_key(a, &k.key, w, cx) {
                                    cx.stop_propagation();
                                    return;
                                }
                                let edit = k.key == "up" && k.modifiers.number_of_modifiers() == 0;
                                let paste = k.key == "v" && (k.modifiers.control || k.modifiers.platform);
                                if (edit && a.edit_last(w, cx)) || (paste && a.paste_image(cx)) {
                                    cx.stop_propagation();
                                }
                            }))
                            .child(
                                // inner_ padding-inline-start 15px; attachButton_ margin-inline
                                // -(space-xs + space-6) 10px around a 32px button.
                                // #1438 inner_ padding-inline-start 15px; #1439 attachWrapper_ padding 12px 14px;
                                // #1440 attachButton_ 32px, radius 8px, margin 0 10px 0 -16px, PlusLargeIcon 20px.
                                div().pl(z(15.)).h(z(56.)).flex().items_center().when(blocked, |d| d.invisible()).child(div().px(z(14.)).child(
                                    crate::ui::states::composer_button("attach", Icon::Plus)
                                        .ml(z(-16.))
                                        .mr(z(10.))
                                        .on_click(cx.listener(|a, _, _, cx| a.prompt_attach(cx))),
                                )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .relative()
                                    // textArea_ padding 17px 0, 16/22. gpui-component's Input sets 1.25rem on its
                                    // frame, then applies our refinement: the line height set on the Textarea wins.
                                    .py(z(17.))
                                    .text_size(z(16.))
                                    .line_height(z(22.))
                                    .text_color(c(Palette::TEXT_NORMAL))
                                    .child(
                                        // xsmall: no editor padding (py 0, px 4), the 4px cancelled below.
                                        div().mx(z(-4.)).child(
                                            gpui_component::input::Textarea::new(&app.composer)
                                                .appearance(false)
                                                .bordered(false)
                                                .xsmall()
                                                .text_size(z(16.)) // textArea_ 16px (xsmall would impose its own size)
                                                .line_height(z(22.)) // textArea_ line-height 22px
                                                .disabled(blocked),
                                        ),
                                    )
                                    .when(placeholder_empty, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .top(z(17.))
                                                .h(z(22.))
                                                // #1445 placeholder_ sits at the editor's x (496 in live form, glyph at 497.3).
                                                .left_0()
                                                .right_0()
                                                .flex()
                                                .items_center()
                                                .text_color(super::tk(idx::CHANNEL_TEXT_AREA_PLACEHOLDER)) // #1445 placeholder_
                                                // placeholder_ white-space nowrap, overflow hidden, text-overflow ellipsis.
                                                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(placeholder)),
                                        )
                                    }),
                            )
                            .child(
                                // buttons_ gap --space-8, margin-inline-end 6px; button_ 32px;
                                // icons --chat-input-icon-size (20px).
                                // #1452 buttons_ gap 8px, 12px to the right edge (#1437 ends 12px early);
                                // order: gift, GIF, sticker, emoji, app launcher (#1457–#1480).
                                div()
                                    .when(blocked, |d| d.invisible())
                                    .mr(z(12.))
                                    .h(z(56.)) // buttons_ height --custom-channel-textarea-text-area-height
                                    .flex()
                                    .gap(z(8.))
                                    .items_center()
                                    .child(crate::ui::states::composer_button_ext("gift", Icon::Gift, 20., false))
                                    // GIF and sticker buttons only while channelTextArea_ is at least
                                    // 450px wide (live: 449.8 → gift/emoji/apps, 450.6 → all five).
                                    .when(full_buttons(), |d| {
                                        d.child(crate::ui::states::composer_button("gif", Icon::Gif).on_click(cx.listener(|a, _, w, cx| crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Gif, crate::ui::pickers::EmojiTarget::Composer, w, cx))))
                                            .child(crate::ui::states::composer_button("sticker", Icon::Sticker).on_click(cx.listener(|a, _, w, cx| crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Sticker, crate::ui::pickers::EmojiTarget::Composer, w, cx))))
                                    })
                                    .child(crate::ui::states::composer_button_ext("emoji", Icon::Emoji, 18., true).on_click(cx.listener(
                                        |a, _, _, cx| a.open_emoji_picker(EmojiTarget::Composer, cx),
                                    )))
                                    .child(
                                        // app-launcher-entrypoint: button_ 32px radius 8px, 20px glyph (#1479).
                                        div()
                                            .id("apps")
                                            .size(z(32.))
                                            .rounded(z(8.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            // button_hfM7yP states, as the other composer buttons.
                                            .group("apps")
                                            .hover(|s| s.bg(c(crate::theme::tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                                            .child(
                                                gpui::svg()
                                                    .path(crate::icons::path("apps"))
                                                    .size(z(20.))
                                                    .flex_none()
                                                    .text_color(ic)
                                                    .group_hover("apps", |s| s.text_color(c(crate::theme::tok(idx::INTERACTIVE_TEXT_HOVER))))
                                                    .id("g")
                                                    .group_active("apps", |s| s.text_color(c(crate::theme::tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
                                            ),
                                    )
                                    .when(show_send_button, |d| {
                                        d.child(
                                            crate::ui::states::composer_button("send", Icon::Send)
                                                .tooltip(crate::ui::tooltip::tip(crate::t!("menu.send_message")))
                                                .on_click(cx.listener(|a, _, w, cx| a.submit_composer(w, cx))),
                                        )
                                    }),
                            ),
                    ),
            ),
        )
}

/// Edit button of an upload: the client's "Modify Attachment" modal (JS
/// `onSubmit({name, description, spoiler})`): Filename, Description (Alt
/// Text) with "Add a description", the "Mark as spoiler" checkbox, Save.
fn open_upload_edit(app: &mut PhoenixApp, ix: usize, cx: &mut Context<PhoenixApp>) {
    use crate::ui::modals::{Field, Form, Modal};
    let Some(f) = app.chat.uploads.get(ix) else { return };
    crate::actions::open_modal(
        Modal::Form(Form {
            title: crate::t!("upload.modify"),
            subtitle: None,
            fields: vec![
                Field::new(crate::t!("upload.filename"), "".into(), f.name.to_string()),
                Field::new(crate::t!("upload.description"), crate::t!("upload.description_ph"), f.description.clone()),
            ],
            choices: Vec::new(),
            choice: None,
            check: Some((crate::t!("upload.mark_spoiler"), f.spoiler)),
            submit: crate::t!("upload.save"),
            danger: false,
            on_submit: std::rc::Rc::new(move |a: &mut PhoenixApp, v: Vec<String>, _, cx: &mut Context<PhoenixApp>| {
                let mut v = v.into_iter();
                let (name, description, spoiler) = (v.next().unwrap_or_default(), v.next().unwrap_or_default(), v.next().is_some_and(|s| s == "1"));
                let toggle = if let Some(u) = a.chat.uploads.get_mut(ix) {
                    if !name.trim().is_empty() {
                        u.name = name.trim().to_string().into();
                    }
                    u.description = description.trim().to_string();
                    u.spoiler != spoiler
                } else {
                    false
                };
                if toggle {
                    a.toggle_upload_spoiler(ix, cx);
                }
                cx.notify();
            }),
        }),
        None,
    );
    cx.notify();
}

/// (remaining, visible) like the client's CharacterCount: shown once
/// remaining <= max/10, and for Nitro users as soon as the text passes 2000.
pub fn char_count(premium: bool, len: i64) -> (i64, bool) {
    let max = if premium { 4000 } else { 2000 };
    let remaining = max - len;
    (remaining, remaining <= max / 10 || (premium && len > 2000 && remaining >= 0))
}

fn should_show_send_button(enabled: bool, draft_empty: bool, blocked: bool) -> bool {
    enabled && !draft_empty && !blocked
}

thread_local! {
    /// Last painted width of the composer box (`channelTextArea_`).
    static TEXT_AREA_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

/// Narrow composers drop the GIF and sticker buttons: the live client shows them only
/// while `channelTextArea_` is at least 450 CSS px wide (docs/parity/responsive.md).
fn full_buttons() -> bool {
    let w = TEXT_AREA_W.with(std::cell::Cell::get);
    w <= 0. || w >= 450.
}

thread_local! {
    /// Our last send per channel (slowmode countdown).
    static LAST_SENT: std::cell::RefCell<std::collections::HashMap<crate::model::Id, std::time::Instant>> = Default::default();
}

fn slowmode_of(app: &PhoenixApp, ch: crate::model::Id) -> u32 {
    app.nav.extras.get(&ch).filter(|x| !x.bypass_slowmode).map_or(0, |x| x.slowmode)
}

/// Seconds left before we may send again in `ch` (0 = free).
pub fn cooldown_left(app: &PhoenixApp, ch: crate::model::Id) -> u64 {
    let secs = slowmode_of(app, ch) as u64;
    LAST_SENT.with(|m| m.borrow().get(&ch).map_or(0, |t| secs.saturating_sub(t.elapsed().as_secs())))
}

/// Slowmode still running: the send is held back (the client keeps the text).
pub fn send_held(app: &PhoenixApp, ch: crate::model::Id) -> bool {
    cooldown_left(app, ch) > 0
}

pub fn note_sent(ch: crate::model::Id) {
    LAST_SENT.with(|m| {
        m.borrow_mut().insert(ch, std::time::Instant::now());
    });
}

/// A countdown is visible somewhere (1 s redraw tick).
pub fn cooling() -> bool {
    LAST_SENT.with(|m| m.borrow().values().any(|t| t.elapsed().as_secs() < 6 * 3600))
}

/// "mm:ss" (or "h:mm:ss") like the client's cooldown text.
pub fn format_cooldown(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    // Client: hours "h:mm:ss", else minutes unpadded "m:ss".
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// cooldownWrapper_bbCgC2: right side of the typing row (margin-inline-start auto,
/// padding 3 8, padding-inline-end 16, gap 4); countdown text, then slowModeIcon_
/// (margin-inline-start 4). CSS only: the icon glyph (clock) and text format are assumed.
fn cooldown(app: &PhoenixApp) -> Option<gpui::AnyElement> {
    let ch = app.selected_channel?;
    let x = app.nav.extras.get(&ch).filter(|x| x.slowmode > 0)?;
    let left = cooldown_left(app, ch);
    // Client text: "Immunité au mode lent" when we bypass it, the countdown
    // while it runs, else "Mode lent activé.".
    let text: SharedString = if x.bypass_slowmode {
        crate::t!("chat.slowmode_immune")
    } else if left > 0 {
        format_cooldown(left).into()
    } else {
        crate::t!("chat.slowmode_on")
    };
    Some(
        div()
            .ml_auto()
            .pt(z(3.))
            .pb(z(3.))
            .pl(z(8.))
            .pr(z(16.))
            .flex()
            .items_center()
            .gap(z(4.))
            .whitespace_nowrap()
            .child(text)
            .child(div().ml(z(4.)).child(gpui::svg().path(crate::icons::path("inbox/clock")).size(z(16.)).text_color(super::tk(idx::TEXT_MUTED))))
            .into_any_element(),
    )
}

/// Opacity and scale of typing dot `i` at `t` (0..1 of the 1.2 s loop).
/// dot_U9Tvvz: typing-dot-pulse 1.2s steps(12) infinite, delays -1.2s / -1.05s / -0.9s;
/// keyframes 0%,50%,100% {opacity .32; scale .8}, 25% {opacity 1; scale 1}.
/// The timing function applies per keyframe segment, so each segment jumps in 12 steps.
pub(crate) fn dot_frame(i: usize, t: f32) -> (f32, f32) {
    let p = (t + 1.0 - i as f32 * 0.125).rem_euclid(1.0);
    let step = |x: f32| (x * 12.).floor() / 12.;
    let k = if p < 0.25 {
        step(p / 0.25)
    } else if p < 0.5 {
        1. - step((p - 0.25) / 0.25)
    } else {
        0.
    };
    (0.32 + 0.68 * k, 0.8 + 0.2 * k)
}

thread_local! {
    /// Last laid-out width of the typingDots_ box (the client's `getBoundingClientRect`).
    static DOTS_BOX_W: std::cell::Cell<f32> = const { std::cell::Cell::new(f32::MAX) };
}

fn text_w(window: &Window, text: &str, weight: FontWeight) -> f32 {
    let mut font = gpui::font(crate::theme::FONT_UI());
    font.weight = weight;
    let run = gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().layout_line(text, z(12.), &[run], None).width)
}

/// TypingUsers (inTextChannel): dots, then "**Name** est en train d'écrire..." with the
/// name from `getName(guild, channel, user)` (nick → global name → username); 4+ typers
/// give "**Plusieurs personnes** sont en train d'écrire...". Slowmode countdown on the right.
fn typing(app: &PhoenixApp, window: &Window) -> impl IntoElement {
    let names: Vec<SharedString> = app
        .selected_channel
        .and_then(|c| app.typing.get(&c).map(|v| (c, v)))
        .map(|(c, v)| v.iter().filter_map(|(u, _)| app.typing_name(c, *u)).collect())
        .unwrap_or_default();
    // inTextChannel_ .text_ > strong: --text-default, --font-weight-semibold.
    let bold = |s: SharedString| div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(super::tk(idx::TEXT_DEFAULT)).child(s);
    // text_: margin-inline-start calc(--space-md + --space-8), nowrap, ellipsis.
    let mut line = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap().ml(z(crate::theme::space::md() + 8.));
    // Client: 1–3 typers whose sentence (scrollWidth) + 56px overflows the typingDots_ box
    // switch to qD/0qZ "**Plusieurs personnes** sont en train d'écrire…".
    let full_w = || -> f32 {
        let (strong, rest): (Vec<&str>, String) = match names.as_slice() {
            [a] => (vec![&a[..]], crate::t!("chat.typing_suffix_one").to_string()),
            [a, b] => (vec![&a[..], &b[..]], format!("{}{}", crate::t!("chat.typing_and"), crate::t!("chat.typing_suffix_many"))),
            [a, b, c3] => (vec![&a[..], &b[..], &c3[..]], format!(", {}{}", crate::t!("chat.typing_comma_and"), crate::t!("chat.typing_suffix_many"))),
            _ => (vec![], String::new()),
        };
        strong.iter().map(|t| text_w(window, t, FontWeight::SEMIBOLD)).sum::<f32>() + text_w(window, &rest, FontWeight::MEDIUM)
    };
    // A box narrower than the 56px slack is not laid out yet (first frame / zero-size canvas).
    let box_w = DOTS_BOX_W.with(|w| w.get());
    let overflow = (1..=3).contains(&names.len()) && box_w > f32::from(z(56.)) && full_w() + f32::from(z(56.)) > box_w;
    match names.as_slice() {
        [] => {}
        _ if overflow => line = line.child(bold(crate::t!("chat.typing_multiple"))).child(crate::t!("chat.typing_suffix_multiple")),
        [a] => line = line.child(bold(a.clone())).child(crate::t!("chat.typing_suffix_one")),
        [a, b] => line = line.child(bold(a.clone())).child(crate::t!("chat.typing_and")).child(bold(b.clone())).child(crate::t!("chat.typing_suffix_many")),
        [a, b, c3] => {
            line = line
                .child(bold(a.clone()))
                .child(", ")
                .child(bold(b.clone()))
                .child(crate::t!("chat.typing_comma_and"))
                .child(bold(c3.clone()))
                .child(crate::t!("chat.typing_suffix_many"))
        }
        _ => line = line.child(bold(crate::t!("chat.typing_several"))).child(crate::t!("chat.typing_suffix_many")),
    }
    let active = !names.is_empty();
    // dot_: 7px circle, currentColor = dots_ color --text-default; scale animated around the centre.
    let dot = |i: usize| {
        div().size(z(7.)).flex().items_center().justify_center().child(
            div().rounded_full().bg(super::tk(idx::TEXT_DEFAULT)).with_animation(
                ("typing-dot", i),
                Animation::new(Duration::from_millis(1200)).repeat(),
                move |d, t| {
                    let (o, s) = dot_frame(i, t);
                    d.size(z(7. * s)).opacity(o)
                },
            ),
        )
    };
    // base_U9Tvvz: height 24px, 12px / 16px, weight 500, --text-subtle;
    // inTextChannel_: padding-inline-start --space-md; typingDots_ padding-inline-start 8px;
    // dots_ gap 2px, margin -1px, padding 1px.
    div()
        .h(z(24.))
        .pl(z(crate::theme::space::md()))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(super::tk(idx::TEXT_SUBTLE))
        .child(
            div().relative().flex_1().min_w_0().overflow_hidden().flex().items_center().pl(z(8.))
            .child(gpui::canvas(|b, _, _| DOTS_BOX_W.with(|w| w.set(f32::from(b.size.width))), |_, _, _, _| {}).absolute().top_0().left_0().w_full().h_full())
            .when(active, |d| {
                d.child(div().flex_none().flex().items_center().gap(z(2.)).child(dot(0)).child(dot(1)).child(dot(2))).child(line)
            }),
        )
        .when_some(app.chat.error.clone(), |d, err| {
            d.child(div().text_color(c(Palette::RED)).text_ellipsis().child(err))
        })
        .children(cooldown(app))
}

#[cfg(test)]
mod tests {
    use super::{char_count, dot_frame, format_cooldown, should_show_send_button};

    #[test]
    fn send_button_requires_enabled_nonempty_sendable_draft() {
        assert!(!should_show_send_button(false, false, false));
        assert!(!should_show_send_button(true, true, false));
        assert!(!should_show_send_button(true, false, true));
        assert!(should_show_send_button(true, false, false));
    }

    #[test]
    fn typing_dots_follow_the_keyframes() {
        // Dot 0 starts its loop at 0 (delay -1.2s): rest, peak at 25 %, back to rest at 50 %.
        assert_eq!(dot_frame(0, 0.0), (0.32, 0.8));
        assert_eq!(dot_frame(0, 0.25), (1.0, 1.0));
        assert_eq!(dot_frame(0, 0.6), (0.32, 0.8));
        // Dot 1 lags by 0.15s (1/8 of the loop).
        assert_eq!(dot_frame(1, 0.375), (1.0, 1.0));
    }

    #[test]
    fn counter_follows_the_client() {
        assert_eq!(char_count(false, 1799), (201, false));
        assert_eq!(char_count(false, 1800), (200, true));
        assert_eq!(char_count(false, 2100), (-100, true));
        assert_eq!(char_count(true, 2000), (2000, false));
        assert_eq!(char_count(true, 2001), (1999, true));
        assert_eq!(char_count(true, 3600), (400, true));
    }

    #[test]
    fn cooldown_text() {
        assert_eq!(format_cooldown(5), "0:05");
        assert_eq!(format_cooldown(125), "2:05");
        assert_eq!(format_cooldown(3725), "1:02:05");
    }
}
