//! Chat overlays and side panels: message context menu, delete confirm,
//! pinned-messages popout, thread panel, "new messages" / "jump to present"
//! bars, drag & drop target.

use crate::ui::tooltip::Tipped as _;
use std::rc::Rc;

use gpui::{
    AnyElement, Context, Div, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*,
};
use gpui_component::input::Input;

use super::{
    icons::{ChatIcon, icon},
    state::{EmojiTarget, QUICK_REACTIONS},
};
use crate::{
    app::{PhoenixApp, format_time_full, format_time_short},
    assets::Icon,
    model::Message,
    theme::{Palette, c, z},
    ui::{avatar, context_menu, modal},
};

/// Everything drawn above the whole window (root `children` hook).
pub fn overlays(app: &PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if let Some(m) = app.chat.menu {
        out.push(message_menu(app, m, cx));
    }
    if let Some((ch, id)) = app.chat.confirm_delete {
        out.push(delete_modal(app, ch, id, cx));
    }
    out
}

thread_local! {
    /// Image URL under the cursor at the last right-click, then the one the
    /// open message menu was opened on.
    static MENU_MEDIA: std::cell::RefCell<(Option<SharedString>, Option<SharedString>)> = const { std::cell::RefCell::new((None, None)) };
}

pub fn set_menu_media(url: SharedString) {
    MENU_MEDIA.with(|m| m.borrow_mut().0 = Some(url));
}

thread_local! {
    /// Same pair for a link of the message text under the cursor.
    static MENU_LINK: std::cell::RefCell<(Option<SharedString>, Option<SharedString>)> = const { std::cell::RefCell::new((None, None)) };
}

pub fn set_menu_link(url: SharedString) {
    MENU_LINK.with(|m| m.borrow_mut().0 = Some(url));
}

/// Called when a message menu opens: the pending image and link (if any) become its targets.
pub fn arm_menu_media() {
    for slot in [&MENU_MEDIA, &MENU_LINK] {
        slot.with(|m| {
            let mut m = m.borrow_mut();
            m.1 = m.0.take();
        });
    }
}

fn message_menu(app: &PhoenixApp, m: super::state::MenuState, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use context_menu::MenuItem;
    let e = cx.entity();
    let (ch, id) = (m.channel, m.msg);
    let msg = app.messages.get(&ch).and_then(|x| x.msgs.iter().find(|x| x.id == id));
    let own = msg.is_some_and(|x| x.author == app.me.id);
    let pinned = msg.is_some_and(|x| x.pinned);
    let has_text = msg.is_some_and(|x| !x.content.is_empty());
    let ic = |i: ChatIcon| Some(i.path());
    // A message that only exists locally (pending/failed send, local ephemeral) has no
    // server id: every API action would target a fake id, so only local ones are offered.
    // Live failed-send menu (20261124): ... Copy Text, ..., Resend Message, ..., Delete Message.
    if let Some(local) = msg.filter(|x| super::pending::is_local(x)) {
        let mut items = Vec::new();
        if has_text {
            let ee = e.clone();
            items.push(MenuItem::action(crate::t!("msg.copy_text"), ic(ChatIcon::Copy), move |_, _, cx| {
                ee.update(cx, |a, cx| a.copy_message_text(ch, id, cx))
            }));
        }
        if local.send_state == crate::model::SendState::Failed {
            let ee = e.clone();
            items.push(MenuItem::action(crate::t!("msg.resend"), ic(ChatIcon::Retry), move |_, _, cx| {
                ee.update(cx, |a, cx| {
                    a.chat.menu = None;
                    a.retry_send(ch, id, cx)
                })
            }));
        }
        if local.send_state != crate::model::SendState::Sending {
            items.push(MenuItem::Separator);
            let ee = e.clone();
            items.push(MenuItem::danger(crate::t!("msg.delete"), ic(ChatIcon::Trash), move |_, _, cx| {
                ee.update(cx, |a, cx| {
                    a.chat.menu = None;
                    a.discard_local(ch, id, cx)
                })
            }));
        }
        let ed = e.clone();
        let dismiss: context_menu::Dismiss = Rc::new(move |_, cx| {
            ed.update(cx, |a, cx| {
                a.chat.menu = None;
                cx.notify();
            })
        });
        return context_menu::render(m.pos, items, dismiss);
    }

    // Vanilla order (vanilla-menu-message dump, 2026-11-23): 4 quick reactions
    // as 44px tiles, Add Reaction, View Reactions | Edit (own), Reply, Forward,
    // Create Thread | Copy Text, Pin, Bookmark, Remind Me >, Apps >, Mark
    // Unread, Copy Message Link, Speak Message | Delete, Report (others) |
    // Copy Message ID. Every row has a left icon.
    let has_reactions = msg.is_some_and(|x| !x.reactions.is_empty());
    let in_guild = app.channels.get(&ch).is_some_and(|c| c.guild.is_some());
    let stub = |key: &'static str, icon: &str| {
        let ee = e.clone();
        MenuItem::action(crate::t!(key), Some(crate::icons::path(icon)), move |_, _, cx| {
            ee.update(cx, |a, cx| a.stub(&crate::t!(key), cx))
        })
    };
    let mut strip = div().flex().gap(z(8.)).px(z(2.)).py(z(2.));
    for (i, q) in QUICK_REACTIONS.iter().take(4).enumerate() {
        let e = e.clone();
        strip = strip.child(
            div()
                .id(("menu-quick", i))
                .size(z(44.))
                .rounded(z(8.))
                .bg(crate::ui::chat::tk(crate::theme::generated::idx::BACKGROUND_MOD_SUBTLE))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(crate::ui::chat::tk(crate::theme::generated::idx::BACKGROUND_MOD_NORMAL)))
                .child(crate::media::emoji::emoji_element(q, 24.))
                .on_click(move |_, _, cx| {
                    e.update(cx, |a, cx| {
                        a.chat.menu = None;
                        a.react_with(ch, id, (*q).into(), None, cx);
                    })
                }),
        );
    }
    let mut items = vec![MenuItem::Custom(strip.into_any_element())];
    // vanilla-menu-message-addreaction: "Add Reaction >" lists the 12 most used
    // emoji (20px emojiIcon_ + ":name:"), a separator, then "View More" (picker).
    let mut frequent: Vec<MenuItem> = crate::ui::pickers::grid::build(app, "", cx)
        .items
        .into_iter()
        .take(12)
        .map(|it| {
            use crate::ui::pickers::grid::PickItem;
            let (token, eid): (gpui::SharedString, _) = match &it {
                PickItem::Unicode(ix) => (crate::ui::pickers::emoji::glyph(*ix).into(), None),
                PickItem::Custom(em) => (em.token().into(), Some(em.id)),
            };
            let label: gpui::SharedString = format!(":{}:", it.name()).into();
            let icon: gpui::SharedString = format!("{}{token}", context_menu::EMOJI_ICON).into();
            let ee = e.clone();
            MenuItem::action(label, Some(icon), move |_, _, cx| {
                let token = token.clone();
                ee.update(cx, |a, cx| {
                    a.chat.menu = None;
                    a.react_with(ch, id, token, eid, cx)
                })
            })
        })
        .collect();
    if !frequent.is_empty() {
        frequent.push(MenuItem::Separator);
    }
    let ee = e.clone();
    frequent.push(MenuItem::action(crate::t!("msg.view_more"), ic(ChatIcon::AddReaction), move |_, _, cx| {
        ee.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Reaction { channel: ch, msg: id }, cx))
    }));
    items.push(MenuItem::Submenu { label: crate::t!("msg.add_reaction"), items: frequent, icon: ic(ChatIcon::AddReaction), subtext: None });
    if has_reactions {
        let ee = e.clone();
        items.push(MenuItem::action(crate::t!("msg.view_reactions"), Some(crate::icons::path("menu/view-reactions")), move |_, _, cx| {
            ee.update(cx, |a, cx| {
                a.chat.menu = None;
                a.open_reactions(ch, id, 0, cx)
            })
        }));
    }
    items.push(MenuItem::Separator);
    if own {
        let ee = e.clone();
        items.push(MenuItem::action(crate::t!("msg.edit"), ic(ChatIcon::Edit), move |_, w, cx| {
            ee.update(cx, |a, cx| a.start_edit(id, w, cx))
        }));
    }
    let ee = e.clone();
    items.push(MenuItem::action(crate::t!("msg.reply"), Some(Icon::Reply.path()), move |_, w, cx| {
        ee.update(cx, |a, cx| a.start_reply(id, w, cx))
    }));
    let ee = e.clone();
    items.push(MenuItem::action(crate::t!("msg.forward"), Some(crate::icons::path("nav/forward")), move |_, _, cx| {
        ee.update(cx, |a, cx| a.open_forward(ch, id, cx))
    }));
    if in_guild && msg.is_some_and(|m| m.thread.is_none()) {
        let ee = e.clone();
        items.push(MenuItem::action(crate::t!("msg.create_thread"), Some(crate::icons::path("chat/thread")), move |_, _, cx| {
            ee.update(cx, |a, cx| a.open_thread_form(ch, id, cx))
        }));
    }
    items.push(MenuItem::Separator);
    if has_text {
        let ee = e.clone();
        items.push(MenuItem::action(crate::t!("msg.copy_text"), ic(ChatIcon::Copy), move |_, _, cx| {
            ee.update(cx, |a, cx| a.copy_message_text(ch, id, cx))
        }));
    }
    // Pin needs Manage Messages or Pin Messages (1 << 51) in a server; always in DMs.
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let manage = crate::actions::has(app, guild, crate::actions::MANAGE_MESSAGES);
    if guild.is_none() || crate::actions::has(app, guild, crate::actions::MANAGE_MESSAGES | (1 << 51)) {
        let ee = e.clone();
        items.push(MenuItem::action(
            if pinned { crate::t!("msg.unpin") } else { crate::t!("msg.pin") },
            Some(Icon::Pin.path()),
            move |_, _, cx| ee.update(cx, |a, cx| a.toggle_pin(ch, id, cx)),
        ));
    }
    let ee = e.clone();
    items.push(MenuItem::action(crate::t!("msg.bookmark"), Some(crate::icons::path("menu/bookmark")), move |_, _, cx| {
        ee.update(cx, |a, cx| {
            a.chat.menu = None;
            a.save_message(ch, id, cx)
        })
    }));
    let mut reminders: Vec<MenuItem> = crate::actions::message::REMINDERS
        .iter()
        .map(|&(k, secs)| {
            let ee = e.clone();
            MenuItem::action(crate::t!(k), None, move |_, _, cx| {
                ee.update(cx, |a, cx| a.remind_message(ch, id, crate::actions::message::due_for(secs), cx))
            })
        })
        .collect();
    if let Some(due) = crate::actions::message::next_monday_9am(chrono::Local::now()) {
        let ee = e.clone();
        reminders.push(MenuItem::action(crate::t!("msg.remind.monday"), None, move |_, _, cx| {
            ee.update(cx, |a, cx| a.remind_message(ch, id, due, cx))
        }));
    }
    let ee = e.clone();
    reminders.push(MenuItem::action(crate::t!("msg.remind.custom"), None, move |_, _, cx| {
        ee.update(cx, |a, cx| a.open_custom_reminder(ch, id, cx))
    }));
    items.push(MenuItem::submenu_with_icon(crate::t!("msg.remind"), "inbox/clock", reminders));
    items.push(MenuItem::submenu_with_icon(crate::t!("menu.apps"), "apps", vec![stub("menu.apps_none", "apps")]));
    let ee = e.clone();
    items.push(MenuItem::action(crate::t!("msg.mark_unread"), ic(ChatIcon::Unread), move |_, _, cx| {
        ee.update(cx, |a, cx| a.mark_unread_from(ch, id, cx))
    }));
    let ee = e.clone();
    items.push(MenuItem::action(crate::t!("msg.copy_link"), ic(ChatIcon::Link), move |_, _, cx| {
        ee.update(cx, |a, cx| a.copy_message_link(ch, id, cx))
    }));
    items.push(stub("msg.speak", "menu/speak"));
    items.push(MenuItem::Separator);
    if has_reactions && manage {
        let emojis: Vec<MenuItem> = msg
            .map(|m| m.reactions.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let ee = e.clone();
                let label: gpui::SharedString = format!("{} {}", r.emoji, r.count).into();
                MenuItem::action(label, None, move |_, _, cx| ee.update(cx, |a, cx| a.remove_reactions(ch, id, Some(r.clone()), cx)))
            })
            .collect();
        items.push(MenuItem::submenu_with_icon(crate::t!("msg.remove_emoji_reactions"), "menu/remove-reactions", emojis));
        let ee = e.clone();
        items.push(MenuItem::danger(crate::t!("msg.remove_all_reactions"), Some(crate::icons::path("menu/remove-all-reactions")), move |_, _, cx| {
            ee.update(cx, |a, cx| a.remove_reactions(ch, id, None, cx))
        }));
    }
    // vanilla-menu-message (someone else's message, Manage Messages): Delete
    // Message then Report Message.
    if own || manage {
        let ee = e.clone();
        items.push(MenuItem::danger(crate::t!("msg.delete"), ic(ChatIcon::Trash), move |ev, _, cx| {
            let skip = ev.modifiers().shift;
            ee.update(cx, |a, cx| a.request_delete(ch, id, skip, cx))
        }));
    }
    if !own {
        let ee = e.clone();
        items.push(MenuItem::danger(crate::t!("msg.report"), Some(crate::icons::path("flag")), move |_, _, cx| {
            ee.update(cx, |a, cx| a.report_message(ch, id, cx))
        }));
    }
    if app.prefs.synced.developer_mode {
    items.push(MenuItem::Separator);
    items.push(MenuItem::action(crate::t!("msg.copy_id"), ic(ChatIcon::IdBadge), move |_, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(id.0.to_string()))
    }));
    }
    // vanilla-menu-image: | Copy Image, Save Image | Copy Image Link, Open Image Link.
    if let Some(url) = MENU_MEDIA.with(|m| m.borrow().1.clone()) {
        items.push(MenuItem::Separator);
        items.push(stub("msg.copy_image", "image"));
        items.push(stub("msg.save_image", "image"));
        items.push(MenuItem::Separator);
        let u = url.clone();
        items.push(MenuItem::action(crate::t!("msg.copy_image_link"), Some(crate::icons::path("chat/link")), move |_, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(u.to_string()))
        }));
        items.push(MenuItem::action(crate::t!("msg.open_image_link"), Some(crate::icons::path("menu/open-link")), move |_, _, cx| cx.open_url(&url)));
    }
    // vanilla-menu-link (2026-11-24): a link of the message text adds | Copy Link,
    // Open Link (`message-copy-native-link`, `message-open-native-link`, 36px icon rows).
    if let Some(url) = MENU_LINK.with(|m| m.borrow().1.clone()) {
        items.push(MenuItem::Separator);
        let u = url.clone();
        items.push(MenuItem::action(crate::t!("menu.copy_link"), Some(crate::icons::path("chat/link")), move |_, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(u.to_string()))
        }));
        items.push(MenuItem::action(crate::t!("msg.open_link"), Some(crate::icons::path("menu/open-link")), move |_, _, cx| cx.open_url(&url)));
    }
    let ed = e.clone();
    let dismiss: context_menu::Dismiss = Rc::new(move |_, cx| {
        ed.update(cx, |a, cx| {
            a.chat.menu = None;
            cx.notify();
        })
    });
    context_menu::render(m.pos, items, dismiss)
}

/// Small message card used in the delete confirm and the pins popout.
fn message_card(app: &PhoenixApp, m: &Message) -> gpui::Div {
    let u = app.users.get(&m.author);
    div()
        .p(z(12.))
        .rounded(z(4.))
        .bg(c(Palette::BG_SECONDARY))
        .flex()
        .gap(z(12.))
        .child(avatar(u, 40.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(z(8.))
                        .child(
                            div()
                                .text_size(z(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(crate::theme::role_color(app.author_color(m.author)))
                                .child(u.map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("common.unknown"))),
                        )
                        .child(
                            div()
                                .text_size(z(12.))
                                .text_color(c(Palette::TEXT_MUTED))
                                .child(format_time_full(m.timestamp)),
                        ),
                )
                .child(
                    div()
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(if m.content.is_empty() && !m.attachments.is_empty() {
                            crate::t!("msg.attachments", n = m.attachments.len())
                        } else {
                            m.content.clone()
                        }),
                ),
        )
}

fn delete_modal(app: &PhoenixApp, ch: crate::model::Id, id: crate::model::Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let e2 = e.clone();
    let msg = app.messages.get(&ch).and_then(|x| x.msgs.iter().find(|x| x.id == id));
    let mut body: Vec<AnyElement> = vec![
        div()
            .text_size(z(16.))
            .child(crate::t!("msg.delete_confirm"))
            .into_any_element(),
    ];
    if let Some(m) = msg {
        body.push(message_card(app, m).shadow_md().into_any_element());
    }
    body.push(
        div()
            .flex()
            .flex_col()
            .gap(z(4.))
            .child(div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::GREEN)).child(crate::t!("msg.protip")))
            .child(
                div()
                    .text_size(z(14.))
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(crate::t!("msg.protip_delete")),
            )
            .into_any_element(),
    );
    modal::confirm(modal::Confirm {
        title: crate::t!("msg.delete"),
        body,
        confirm_label: crate::t!("common.delete"),
        danger: true,
        on_confirm: Rc::new(move |_, cx| e.update(cx, |a, cx| a.confirm_delete(cx))),
        on_cancel: Rc::new(move |_, cx| {
            e2.update(cx, |a, cx| {
                a.chat.confirm_delete = None;
                cx.notify();
            })
        }),
    })
}

/// Pinned messages popout under the header pin icon (lives in `ui::inbox`).
pub fn pins_popout(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    crate::ui::inbox::pins_popout(app, cx)
}

thread_local! {
    static THREAD_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

/// container_HeWz1C header_HeWz1C (vanilla-thread-channel #1336–#1371):
/// forum post action bar with left `reactButtons_HeWz1C` (starter message
/// reactions + add-reaction button) and right `buttons_HeWz1C` (Follow/Following
/// + Copy Link + Jump to Top, gap 4; sm secondary h32 radius 8).
fn forum_post_bar(app: &PhoenixApp, thread: crate::model::Id, scroll: &gpui::ScrollHandle, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    use crate::theme::{generated::idx, tok};
    // Posts opened from the forum list are not in the channel cache: the open forum.
    let ch = app.channels.get(&thread).or_else(|| app.nav.find_thread(thread));
    let forum = ch.and_then(|c| c.parent).or(app.selected_channel)?;
    let f = app.channels.get(&forum)?;
    if f.kind != crate::model::ChannelKind::Forum {
        return None;
    }
    let link = format!("https://discord.com/channels/{}/{}", f.guild.map_or(0, |g| g.0), thread.0);
    let followed = app.backend.is_thread_joined(thread);
    let starter = app.messages.get(&thread).and_then(|e| e.msgs.first());

    let mut reacts = div().flex().items_center().flex_wrap().gap(z(4.)).mr(z(20.));
    if let Some(m) = starter {
        for (i, r) in m.reactions.iter().enumerate() {
            let token = match r.emoji_id {
                Some(id) => format!("<:{}:{}>", r.emoji, id.0),
                None => r.emoji.to_string(),
            };
            let bg = if r.me { c(tok(idx::BACKGROUND_BRAND)).opacity(0.15) } else { c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)) };
            let border = if r.me { c(tok(idx::BORDER_FOCUS)) } else { c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)) };
            let count_color = if r.me { c(tok(idx::TEXT_BRAND)) } else { c(tok(idx::TEXT_DEFAULT)) };
            reacts = reacts.child(
                div()
                    .id(("forum-post-react", i))
                    .h(z(28.))
                    .px(z(6.))
                    .rounded(z(8.))
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(crate::media::emoji::emoji_element(&token, 18.))
                    .child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(count_color)
                            .child(r.count.to_string()),
                    ),
            );
        }
    }
    // addReactButton_HeWz1C (vanilla-thread-channel #1352): 32x28, radius 8, secondary border, 18px icon.
    reacts = reacts.child(
        div()
            .id("forum-post-add-react")
            .w(z(32.))
            .h(z(28.))
            .flex_none()
            .rounded(z(8.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .tip(crate::t!("forum.add_reaction"))
            .child(gpui::svg().path(crate::icons::path("emoji")).size(z(18.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))),
    );

    let btn = |id: &'static str, path: &'static str, tip: SharedString| {
        div()
            .id(id)
            .size(z(32.))
            .flex_none()
            .rounded(z(8.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .tip(tip)
            .child(gpui::svg().path(crate::icons::path(path)).size(z(16.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
    };
    // button_F7qvhY sm_F7qvhY secondary_F7qvhY hasText_F7qvhY (#1356): Follow / Following.
    let follow_btn = div()
        .id("forum-post-follow")
        .h(z(32.))
        .px(z(12.))
        .flex_none()
        .rounded(z(8.))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(z(4.))
        .tip(crate::t!("forum.follow_tip"))
        .child(
            gpui::svg()
                .path(crate::icons::path(if followed { "check" } else { "bell" }))
                .size(z(16.))
                .flex_none()
                .text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))),
        )
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
                .child(if followed { crate::t!("forum.following") } else { crate::t!("forum.follow") }),
        );
    let top = scroll.clone();
    Some(
        div()
            .flex_none()
            .px(z(16.))
            .py(z(6.))
            .mt(z(8.))
            .flex()
            .items_center()
            .justify_between()
            .child(reacts)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(follow_btn)
                    .child(btn("forum-post-link", "chat/link", crate::t!("menu.copy_link")).on_click(move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.clone()));
                    }))
                    .child(btn("forum-post-top", "nav/up", crate::t!("forum.go_top")).on_click(cx.listener(move |_, _, _, cx| {
                        top.scroll_to_item(0);
                        cx.notify();
                    }))),
            )
            .into_any_element(),
    )
}

/// Thread side panel (replaces the member list while open).
pub fn thread_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let t = app.chat.thread.as_ref()?;
    let entity: Entity<PhoenixApp> = cx.entity();
    let id = t.thread.id;
    let scroll = THREAD_SCROLL.with(|h| h.clone());
    let mut body = div().id("thread-msgs").flex_1().min_h_0().overflow_y_scroll().track_scroll(&scroll).flex().flex_col().pb(z(8.));
    if let Some(bar) = forum_post_bar(app, id, &scroll, cx) {
        body = body.child(bar);
    }
    match app.messages.get(&id) {
        Some(entry) if entry.loaded_once => {
            for (m, r) in entry.msgs.iter().zip(entry.rendered.iter()) {
                body = body.child(super::message(app, &entity, m, r, entry, cx));
            }
        }
        _ => {
            body = body.child(div().p(z(16.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("common.loading")));
        }
    }
    let empty = t.input.read(cx).value().is_empty();
    let is_post = app
        .channels
        .get(&id)
        .or_else(|| app.nav.find_thread(id))
        .and_then(|ch| ch.parent)
        .or(app.selected_channel)
        .and_then(|f| app.channels.get(&f))
        .is_some_and(|f| f.kind == crate::model::ChannelKind::Forum);
    // Same chat sidebar frame as the "New Thread" sidebar (ref-cache
    // create-thread-sidebar-header / -frame; see ui/thread_create.rs).
    let tk = |i: usize| c(crate::theme::tok(i));
    use crate::theme::generated::idx;
    Some(
        div()
            .w(z(480.)) // chatLayerWrapper_ 480
            .flex_none()
            .h_full()
            .border_l_1()
            .border_t_1()
            .rounded_tl(z(12.)) // container_FrbQJp border-radius 12px 0 0
            .border_color(tk(idx::BORDER_SUBTLE))
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .flex_col()
            .child(
                // container_ybUK7Z: height 49, padding 8 8 8 20, 1px --border-subtle bottom
                div()
                    .h(z(49.))
                    .flex_none()
                    .pt(z(8.))
                    .pr(z(8.))
                    .pb(z(8.))
                    .pl(z(20.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    // iconWrapper_ 32, svg 20 --icon-muted. A forum post has no icon: its
                    // title starts at the 20px padding (dumps/vanilla-forum-post-sidebar).
                    .when(!is_post, |d| {
                        d.child(div().size(z(32.)).flex_none().flex().items_center().justify_center().child(
                            gpui::svg().path(crate::icons::path("chat/thread")).size(z(20.)).flex_none().text_color(tk(idx::ICON_MUTED)),
                        ))
                    })
                    .child(
                        // titleWrapper_ > h1 body-md/medium 16/20 --text-strong
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(z(16.))
                            .line_height(z(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::TEXT_STRONG))
                            .child(t.thread.name.clone()),
                    )
                    .child({
                        // toolbar_ (vanilla-forum-post-sidebar #1634): padding-left 10, gap 10;
                        // More then Close, iconWrapper_ clickable 32 with a 20px glyph,
                        // --icon-muted, :hover --icon-subtle.
                        let btn = |id: &'static str, path: &'static str| {
                            div()
                                .id(id)
                                .group(id)
                                .size(z(32.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(
                                    gpui::svg()
                                        .path(crate::icons::path(path))
                                        .size(z(20.))
                                        .flex_none()
                                        .text_color(tk(idx::ICON_MUTED))
                                        .group_hover(id, |s| s.text_color(tk(idx::ICON_SUBTLE))),
                                )
                        };
                        div()
                            .flex_none()
                            .pl(z(10.))
                            .flex()
                            .gap(z(10.))
                            .child(btn("thread-more", "more-horizontal").tip(crate::t!("header.more")).on_click(move |ev, w, _| {
                                crate::ui::thread_menu::open(id, ev.position(), w)
                            }))
                            .child(btn("close-thread", "close").on_click(cx.listener(|a, _, _, cx| a.close_thread(cx))))
                    }),
            )
            .child(body)
            .child(
                // channelTextArea_: margin 0 10 10, radius 8, 1px --border-subtle;
                // inner --chat-background-default, textArea 16/22
                div()
                    .flex_none()
                    .mx(z(10.))
                    .mb(z(10.))
                    .min_h(z(58.))
                    .px(z(16.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .bg(tk(idx::CHAT_BACKGROUND_DEFAULT))
                    .relative()
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .line_height(z(22.))
                    .child(div().flex_1().min_w_0().child(Input::new(&t.input).appearance(false).bordered(false)))
                    .when(empty, |d| {
                        d.child(
                            // Long post titles end with an ellipsis inside the field instead of
                            // running past its border.
                            div()
                                .absolute()
                                .left(z(18.))
                                .right(z(16.))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .text_color(tk(idx::TEXT_MUTED))
                                .child(div().flex_1().min_w_0().text_ellipsis().child(crate::t!("chat.placeholder_thread", name = t.thread.name))),
                        )
                    }),
            )
            .into_any_element(),
    )
}

/// Blurple "N new messages since ..." bar at the top of the list.
/// newMessagesBar_ barBase_: absolute, top 0, inset-inline --space-8 --space-16, height 32px,
/// padding-inline --space-16, --brand-500, radius 0 0 8px 8px, --elevation-low, white,
/// 1px --border-subtle border. barButtonMain_ (body-sm/medium 14px, ellipsis, flex 1) jumps
/// to the first unread; barButtonAlt_ (semibold, padding --space-4 --space-8, --radius-sm,
/// hover --control-secondary-background-hover) "Mark As Read" + ChatCheckIcon (barButtonIcon_
/// 16px, margin-inline-start 8px, top -1px).
pub fn new_messages_bar(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    use crate::{theme::generated::idx, ui::chat::tk};
    use gpui::px;
    let (n, ts) = app.chat.new_messages?;
    if app.chat.at_bottom {
        return None;
    }
    // Client unread bar: same day "N nouveaux messages depuis 10:42" (BctFH/), older
    // "… depuis le 23 novembre 2026 à 10:42" (3wXb9P, its \n rendered as a space).
    let today = chrono::TimeZone::timestamp_opt(&chrono::Local, ts, 0)
        .single()
        .is_some_and(|d| d.date_naive() == chrono::Local::now().date_naive());
    let since: SharedString = if today {
        format_time_short(ts)
    } else {
        crate::t!("chat.new_since_date", date = crate::app::format_day(ts), time = format_time_short(ts))
    };
    let text = if n == 1 { crate::t!("chat.new_since_one", since = since) } else { crate::t!("chat.new_since_many", n = n, since = since) };
    Some(
        div()
            .id("new-messages-bar")
            .absolute()
            .top_0()
            .left(z(8.))
            .right(z(16.))
            .h(z(32.))
            .px(z(16.))
            .rounded_b(z(8.))
            .border_1()
            .border_color(tk(idx::BORDER_SUBTLE))
            .bg(tk(idx::BRAND_500))
            .shadow(vec![gpui::BoxShadow {
                // --elevation-low: 0 1px 0 --opacity-black-20 (+ two 4% layers, too faint to draw).
                color: gpui::hsla(0., 0., 0., 0.2), // --opacity-black-20 (dark)
                offset: gpui::point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
                inset: false,
            }])
            .flex()
            .items_center()
            .text_size(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::WHITE))
            .child(
                div()
                    .id("new-messages-jump")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .pl(z(4.))
                    .pr(z(12.))
                    .cursor_pointer()
                    .on_click(cx.listener(|a, _, _, cx| a.jump_to_present(cx)))
                    .child(div().truncate().child(text)),
            )
            .child(
                div()
                    .id("mark-read")
                    .flex_none()
                    .flex()
                    .items_center()
                    .py(z(4.))
                    .pl(z(4.))
                    .pr(z(8.))
                    .rounded(z(8.)) // --radius-sm (:root 8px)
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
                    .on_click(cx.listener(|a, _, _, cx| {
                        cx.stop_propagation();
                        a.mark_channel_read(cx)
                    }))
                    .child(crate::t!("menu.mark_read"))
                    .child(gpui::svg().path(crate::icons::path("chat-check")).flex_none().size(z(16.)).ml(z(8.)).mt(z(-1.)).text_color(tk(idx::WHITE))),
            )
            .into_any_element(),
    )
}

/// "You're viewing older messages — Jump To Present" above the composer.
pub fn jump_bar(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let scrolled_up = (!app.chat.at_bottom && app.message_list.is_scrolled_to_end() == Some(false))
        || crate::history_detached::is_detached(app);
    if !scrolled_up {
        return None;
    }
    use crate::theme::{generated::idx, tok};
    Some(
        // jumpToPresentBar_ bottomBar_ barBase_: centred max-content pill (min 284x44),
        // --background-surface-highest, 1px --border-normal, --radius-lg, --shadow-medium,
        // padding 8px 12px (jumpToPresentBar_ overrides bottomBar_),
        // color --text-default.
        div()
            .absolute()
            // Live (ref-cache jump-present-bar-hover): pill bottom 20px above the form.
            .bottom(z(20.))
            .left_0()
            .right(z(16.))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("jump-present")
                    .min_w(z(284.))
                    .min_h(z(44.))
                    .py(z(8.))
                    .px(z(12.)) // live computed padding 8px 12px (jumpToPresentBar_ padding-inline --space-12)
                    .rounded(z(16.))
                    .border_1()
                    .border_color(c(tok(idx::BORDER_NORMAL)))
                    .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
                    .shadow(crate::ui::inbox::shadow_medium())
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .on_click(cx.listener(|a, _, _, cx| a.jump_to_present(cx)))
                    // barButtonMain_ barButtonBase_ body-sm/medium, padding-inline-start --space-4, end 12.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pl(z(4.))
                            .pr(z(12.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(crate::t!("chat.older")),
                    )
                    // Button variant primary, size sm: "Revenir aux messages les plus récents".
                    .child(
                        crate::ui::controls::button_sm("jump-present-btn", crate::t!("chat.jump_present"), crate::ui::controls::BtnKind::Primary)
                            .on_click(cx.listener(|a, _, _, cx| a.jump_to_present(cx))),
                    ),
            )
            .into_any_element(),
    )
}

thread_local! {
    /// External drag over the chat: (entered, last move), to replay the fly-in.
    static DROP_DRAG: std::cell::Cell<Option<(std::time::Instant, std::time::Instant)>> = const { std::cell::Cell::new(None) };
}

/// Whole fly-in: icons .8s, title 1s after .175s, instructions 1.25s after .185s.
const DROP_ANIM_MS: f32 = 1435.;

/// Track file drags over `el` (the `chat-drop` column) so the overlay can
/// replay the client's enter keyframes each time a drag comes in.
pub fn track_drop(el: gpui::Stateful<Div>) -> gpui::Stateful<Div> {
    el.on_drag_move::<gpui::ExternalPaths>(|ev, window, _| {
        let now = std::time::Instant::now();
        let inside = ev.bounds.contains(&ev.event.position);
        let cur = DROP_DRAG.get();
        match (inside, cur) {
            // A pause of over a second means the previous drag left the window unseen.
            (true, Some((start, last))) if now.duration_since(last).as_secs_f32() < 1. => DROP_DRAG.set(Some((start, now))),
            (true, _) => {
                DROP_DRAG.set(Some((now, now)));
                window.refresh();
            }
            (false, _) => DROP_DRAG.set(None),
        }
    })
}

/// Files dropped: the next drag starts a fresh fly-in.
pub fn drop_ended() {
    DROP_DRAG.set(None);
}

/// Piecewise-linear keyframes with CSS `ease` inside each segment.
fn keyframes(stops: &[(f32, f32)], p: f32) -> f32 {
    let ease = crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.);
    let p = p.clamp(0., 1.);
    for w in stops.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if p <= t1 {
            let k = if t1 > t0 { ease((p - t0) / (t1 - t0)) } else { 1. };
            return v0 + (v1 - v0) * k;
        }
    }
    stops.last().map_or(0., |s| s.1)
}

/// uploadIconAnimateInLeft/Middle/Right_TLgD1o (.8s ease forwards):
/// (x, y, opacity, tilt in degrees) of the left icon at progress `p`; the
/// right one mirrors x and the tilt. The tilt was sampled every 50ms on the
/// live client (overshoot to -35.9° at 250ms, -28° at 450ms, -32° at 600ms,
/// settling at -30°) and fits these stops within 0.2°.
fn icon_pose(p: f32) -> (f32, f32, f32, f32) {
    const T: [f32; 7] = [0., 0.041667, 0.308334, 0.583334, 0.75, 0.833334, 1.];
    const X: [f32; 7] = [0., 0., -48., -41., -45., -44., -44.];
    const Y: [f32; 7] = [0., -1., -10., -3., -8., -7., -6.];
    const O: [f32; 7] = [0.6, 0.68, 1., 1., 1., 1., 1.];
    const R: [f32; 7] = [0., 0., -36., -28., -32., -30.5, -30.];
    let at = |v: &[f32; 7]| keyframes(&T.iter().copied().zip(v.iter().copied()).collect::<Vec<_>>(), p);
    (at(&X), at(&Y), at(&O), at(&R))
}

/// uploadTextFadeIn_TLgD1o: (opacity, y) at `ms` after the overlay appeared.
fn text_pose(ms: f32, delay: f32, dur: f32) -> (f32, f32) {
    let p = ((ms - delay) / dur).clamp(0., 1.);
    let k = keyframes(&[(0., 0.), (0.08334, 0.), (0.458, 1.), (1., 1.)], p);
    (k, -4. * (1. - k))
}

/// 865429 `icons_` (100x60, relative): `wrapOne/Three_` translateY -70, `wrapTwo_` -80,
/// each holding an absolute 100x130 `icon_` that flies in to its final keyframe
/// (= the reduce-motion pose): one translate(-44,-6) rotate(-30deg), two (0,-6)
/// with its blurple glow, three (44,-6) rotate(30deg). Painted one, three, two
/// like the client's DOM order.
/// The art comes from the local Discord cache ([`crate::icons::art`]); without it, one glyph.
///
/// The three icons at `ms` into the fly-in (`None`: final pose).
fn upload_icons_at(ms: Option<f32>) -> AnyElement {
    use crate::icons::art;
    let Some(arts) = art::upload_icons() else {
        return icon(ChatIcon::Upload, 48., gpui::white()).into_any_element();
    };
    // The side canvases are 10px wider than icon_ on each side, the middle one
    // UPLOAD_GLOW_PAD on every side for its glow (see art::UPLOAD_CANVAS_W).
    let pad = (art::UPLOAD_CANVAS_W - 100.) / 2.;
    let place = |p: &std::path::PathBuf, x: f32, y: f32| {
        gpui::img(p.clone()).absolute().left(z(x - pad)).top(z(y)).w(z(art::UPLOAD_CANVAS_W)).h(z(art::UPLOAD_CANVAS_H))
    };
    let (x, y, o, r) = ms.map_or((-44., -6., 1., -30.), |ms| icon_pose(ms / 800.));
    // One pre-rotated file per whole degree of tilt.
    let tilt = ((-r).round().max(0.) as usize).min(art::UPLOAD_MAX_TILT);
    let g = art::UPLOAD_GLOW_PAD;
    div()
        .relative()
        .flex_none()
        .w(z(100.))
        .h(z(60.))
        .child(place(&arts.one[tilt], x, -70. + y).opacity(o))
        .child(place(&arts.three[tilt], -x, -70. + y).opacity(o))
        .child(gpui::img(arts.two.clone()).absolute().left(z(-g)).top(z(-80. + y - g)).w(z(100. + 2. * g)).h(z(130. + 2. * g)))
        .into_any_element()
}

/// uploadDropModal_ inner_: 2px dashed --opacity-white-40, radius 6, centred
/// column; title_ 22px bold white; instructions_ 12/16 white, margin-top 4.
/// `ms` into the fly-in (`None`: settled).
fn drop_inner(name: SharedString, ms: Option<f32>) -> Div {
    let (to, ty) = ms.map_or((1., 0.), |ms| text_pose(ms, 175., 1000.));
    let (io, iy) = ms.map_or((1., 0.), |ms| text_pose(ms, 185., 1250.));
    div()
        .flex_1()
        .rounded(z(6.))
        .border_2()
        .border_dashed()
        .border_color(super::tk(crate::theme::generated::idx::OPACITY_WHITE_40))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(upload_icons_at(ms))
        .child(
            div()
                .relative()
                .top(z(ty))
                .opacity(to)
                .px(z(8.))
                .text_size(z(22.))
                .font_weight(FontWeight::BOLD)
                .text_color(gpui::white())
                .text_center()
                .child(crate::t!("chat.upload_to", name = name)),
        )
        .child(
            div()
                .relative()
                .top(z(iy))
                .opacity(io)
                .mt(z(4.))
                .px(z(8.))
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(gpui::white())
                .text_center()
                .child(crate::t!("chat.upload_hint")),
        )
}

/// Full-area overlay shown while files are dragged over the chat. Each new
/// drag (see [`track_drop`]) replays the client's enter keyframes: icons fly
/// out (.8s), bgScale_ bounces (uploadModalBounce_ .99 → 1.005 → .995 → 1),
/// title and instructions fade in from -4px.
pub fn drop_overlay(app: &PhoenixApp) -> impl IntoElement {
    use crate::theme::generated::idx;
    use gpui::AnimationExt;
    let name = app.selected_channel.and_then(|c| app.channels.get(&c)).map(|c| c.name.clone()).unwrap_or_default();
    // uploadDropModal_ 310x170; bgScale_ --brand-500 radius 10.
    let modal = div().w(z(310.)).h(z(170.)).rounded(z(10.)).bg(super::tk(idx::BRAND_500)).flex();
    let modal = match DROP_DRAG.get().map(|(start, _)| start) {
        Some(start) if start.elapsed().as_secs_f32() * 1000. < DROP_ANIM_MS => modal
            .with_animation(
                gpui::ElementId::Name(format!("drop-in-{start:?}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(DROP_ANIM_MS as u64)),
                move |d, t| {
                    let ms = t * DROP_ANIM_MS;
                    let k = keyframes(&[(0., 0.99), (0.333334, 1.005), (0.541667, 0.995), (0.708334, 1.), (1., 1.)], ms / 800.);
                    d.w(z(310. * k)).h(z(170. * k)).child(drop_inner(name.clone(), Some(ms)))
                },
            )
            .into_any_element(),
        _ => modal.child(drop_inner(name, None)).into_any_element(),
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .invisible()
        .group_drag_over::<gpui::ExternalPaths>("chat-drop", |s| s.visible())
        .bg(super::tk(idx::OPACITY_BLACK_80)) // uploadArea_ --opacity-black-80
        .flex()
        .items_center()
        .justify_center()
        .child(modal)
}

#[cfg(test)]
mod tests {
    /// Tilt and position of the left upload icon, sampled on the live client
    /// with the animation paused at these times (ms, x, y, degrees).
    #[test]
    fn upload_icon_pose_matches_live_samples() {
        for (ms, x, y, deg) in [
            (100., -25.80, -5.84, -19.35),
            (250., -47.95, -9.95, -35.94),
            (450., -41.02, -3.02, -28.03),
            (600., -45., -8., -32.),
            (800., -44., -6., -30.),
        ] {
            let (px, py, _, pr) = super::icon_pose(ms / 800.);
            assert!((px - x).abs() < 0.6 && (py - y).abs() < 0.6, "{ms}ms: ({px}, {py}) vs ({x}, {y})");
            assert!((pr - deg).abs() < 0.3, "{ms}ms: {pr}° vs {deg}°");
        }
    }
}
