use crate::theme::z;
use gpui::{
    AnyElement, Context, FontWeight, ImageSource, IntoElement, ParentElement, SharedString, Styled, Window, div,
    img, prelude::*, px,
};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef},
    theme::{DEFAULT_AVATAR, Palette, c, ca},
    ui::home::data::{InviteDialog, InvitePreview, NavReply, NavRequest, Overlay, RelationshipKind, invite_code},
};

impl PhoenixApp {
    pub fn open_join_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_add_server(window, cx);
    }

    pub(crate) fn preview_invite(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.nav.invite_input else { return };
        let code = invite_code(&input.read(cx).value());
        if code == self.nav.invite_code {
            return;
        }
        self.nav.invite_code = code.clone();
        self.nav.invite_preview = None;
        let Some(code) = code else {
            cx.notify();
            return;
        };
        self.nav_request(NavRequest::FetchInvite(code.clone()), cx, move |this, res, _| {
            if this.nav.invite_code.as_deref() != Some(code.as_str()) {
                return;
            }
            this.nav.invite_preview = Some(match res {
                Ok(NavReply::Invite(p)) => Ok(*p),
                Ok(_) => Err(crate::t!("invite.invalid")),
                Err(_) => Err(crate::t!("invite.invalid")),
            });
        });
    }

    pub fn join_invite(&mut self, cx: &mut Context<Self>) {
        let Some(code) = self.nav.invite_code.clone() else {
            self.nav.invite_preview = Some(Err(crate::t!("invite.enter_valid")));
            cx.notify();
            return;
        };
        if let Some(Ok(p)) = &self.nav.invite_preview
            && let Some(g) = p.guild
            && self.guilds.iter().any(|x| x.id == g)
        {
            self.close_overlay(cx);
            self.select_guild(Some(g), cx);
            return;
        }
        self.nav.busy = true;
        self.nav_request(NavRequest::AcceptInvite(code), cx, |this, res, cx| {
            this.nav.busy = false;
            match res {
                Ok(_) => {
                    let g = this.nav.invite_preview.as_ref().and_then(|p| p.as_ref().ok()).and_then(|p| p.guild);
                    this.close_overlay(cx);
                    if let Some(g) = g.filter(|g| this.guilds.iter().any(|x| x.id == *g)) {
                        this.select_guild(Some(g), cx);
                    }
                }
                Err(e) => this.nav.invite_preview = Some(Err(e.0.into())),
            }
        });
        cx.notify();
    }

    pub fn open_invite_people(&mut self, guild: Id, cx: &mut Context<Self>) {
        let channel = self
            .selected_channel
            .filter(|c| self.channels.get(c).is_some_and(|c| c.guild == Some(guild)))
            .or_else(|| {
                self.guild_channels.get(&guild).and_then(|l| {
                    l.iter().copied().find(|id| self.channels[id].kind == crate::model::ChannelKind::Text)
                })
            });
        self.open_invite_to(guild, channel, cx);
    }

    pub fn open_invite_to(&mut self, guild: Id, channel: Option<Id>, cx: &mut Context<Self>) {
        self.nav.invite_request = self.nav.invite_request.wrapping_add(1);
        let request = self.nav.invite_request;
        self.show_overlay(Overlay::InviteLink(InviteDialog {
            guild, channel, request, copied: false,
            link: channel.is_none().then(|| Err(crate::t!("invite.no_target"))),
            sending: Default::default(), sent: Default::default(), send_error: None,
        }), cx);
        let Some(channel) = channel else { return };
        self.generate_invite_link(channel, request, cx);
    }

    fn generate_invite_link(&mut self, channel: Id, request: u64, cx: &mut Context<Self>) {
        self.nav_request(NavRequest::CreateInvite(channel), cx, move |this, res, _| {
            if let Some(Overlay::InviteLink(dialog)) = &mut this.nav.overlay
                && dialog.request == request
            {
                dialog.link = Some(match res {
                    Ok(NavReply::Text(link)) if invite_code(&link).is_some() => Ok(link),
                    Ok(_) => Err(crate::t!("invite.invalid")),
                    Err(e) => Err(crate::t!("invite.create_failed", e = e.0)),
                });
            }
        });
    }

    pub(crate) fn prepare_invite_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.nav.overlay, Some(Overlay::InviteLink(_))) || self.nav.invite_friend_search.is_some() {
            return;
        }
        let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!("invite.find_friend")));
        self.nav.invite_friend_search_sub = Some(cx.observe(&input, |_, _, cx| cx.notify()));
        self.nav.invite_friend_search = Some(input);
    }

    fn send_friend_invite(&mut self, user: Id, cx: &mut Context<Self>) {
        let Some(Overlay::InviteLink(dialog)) = &mut self.nav.overlay else { return };
        let Some(Ok(link)) = &dialog.link else { return };
        if dialog.sent.contains(&user) || !dialog.sending.insert(user) { return; }
        dialog.send_error = None;
        let request = dialog.request;
        let guild = dialog.guild;
        let text = link.to_string();
        let backend = self.backend.clone();
        cx.spawn(async move |entity, cx| {
            let result = async {
                let dm = backend.open_dm(user).await?;
                backend.send_message(dm, text, None).await
            }.await;
            let _ = entity.update(cx, |app, cx| {
                if result.is_ok() {
                    crate::ui::chat::welcome::record_invite(app, guild);
                }
                if let Some(Overlay::InviteLink(dialog)) = &mut app.nav.overlay
                    && dialog.request == request
                {
                    dialog.sending.remove(&user);
                    match result {
                        Ok(_) => { dialog.sent.insert(user); }
                        Err(e) => dialog.send_error = Some(crate::t!("invite.send_failed", error = e.0)),
                    }
                    cx.notify();
                }
            });
        }).detach();
        cx.notify();
    }

    pub fn leave_guild(&mut self, guild: Id, cx: &mut Context<Self>) {
        self.close_overlay(cx);
        if crate::ui::join_flow::is_lurking(guild) {
            crate::ui::join_flow::finish_lurking(self, guild, false, cx);
            return;
        }
        self.nav_request(NavRequest::LeaveGuild { guild, lurking: false }, cx, move |this, res, cx| {
            if res.is_ok() && this.guilds.iter().any(|g| g.id == guild) {
                this.handle_event(crate::backend::AppEvent::GuildDelete(guild), cx);
            }
        });
    }
}

pub(crate) fn modal(id: &'static str, cx: &mut Context<PhoenixApp>, inner: gpui::Div) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .bg(ca(0x000000B3))
        .flex()
        .items_center()
        .justify_center()
        .on_click(cx.listener(|a, _, _, cx| a.close_overlay(cx)))
        .child(
            div()
                .id("modal-card")
                .w(z(440.))
                .rounded(z(12.))
                .bg(c(crate::theme::tok(crate::theme::generated::idx::MODAL_BACKGROUND)))
                .border_1()
                .border_color(c(crate::theme::tok(crate::theme::generated::idx::BORDER_NORMAL)))
                .shadow_lg()
                .overflow_hidden()
                .on_click(|_, _, cx| cx.stop_propagation())
                .relative()
                .child(inner)
                .child(
                    div()
                        .id("modal-close")
                        .absolute()
                        .top(z(12.))
                        .right(z(12.))
                        .size(z(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(c(Palette::INTERACTIVE))
                        .hover(|s| s.text_color(c(Palette::TEXT_NORMAL)))
                        .on_click(cx.listener(|a, _, _, cx| a.close_overlay(cx)))
                        .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(Palette::INTERACTIVE))),
                ),
        )
        .into_any_element()
}

fn title(t: impl Into<SharedString>) -> gpui::Div {
    div()
        .px(z(28.))
        .text_size(z(24.))
        .font_weight(FontWeight::BOLD)
        .text_color(c(Palette::TEXT_HEADER))
        .text_center()
        .child(t.into())
}

fn button(id: &'static str, label: impl Into<SharedString>, bg: u32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(38.))
        .px(z(16.))
        .rounded(z(4.))
        .bg(c(bg))
        .text_color(gpui::white())
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .child(label.into())
}

fn footer() -> gpui::Div {
    div().p(z(16.)).bg(c(Palette::BG_SECONDARY)).flex().justify_end().items_center().gap(z(8.))
}

pub fn guild_icon(icon: Option<&ImageRef>, name: &str, size: f32) -> AnyElement {
    match icon {
        Some(ImageRef::Url(u)) => img(ImageSource::from(u.clone())).size(px(size)).rounded(px(size * 0.3)).into_any_element(),
        _ => div()
            .size(px(size))
            .rounded(px(size * 0.3))
            .bg(c(DEFAULT_AVATAR[0]))
            .flex()
            .items_center()
            .justify_center()
            .text_color(gpui::white())
            .text_size(px(size * 0.35))
            .font_weight(FontWeight::SEMIBOLD)
            .child(crate::model::acronym(name))
            .into_any_element(),
    }
}

pub(crate) fn preview_card(p: &InvitePreview) -> gpui::Div {
    let dot = |col: u32| div().size(z(8.)).rounded_full().bg(c(col));
    div()
        .mt(z(12.))
        .p(z(16.))
        .rounded(z(8.))
        .bg(c(Palette::BG_SECONDARY))
        .flex()
        .items_center()
        .gap(z(16.))
        .child(guild_icon(p.guild_icon.as_ref(), &p.guild_name, 50.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(2.))
                .when_some(p.inviter.clone(), |d, i| {
                    d.child(
                        div()
                            .text_size(z(12.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .child(crate::t!("invite.invited_you", i = i)),
                    )
                })
                .child(
                    div()
                        .text_size(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .text_ellipsis()
                        .child(p.guild_name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(z(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(dot(Palette::GREEN))
                        .child(crate::t!("invite.online", n = p.online))
                        .child(div().w(z(8.)))
                        .child(dot(Palette::GREY))
                        .child(crate::t!("invite.members", n = p.members)),
                ),
        )
}

fn invite_dialog(app: &PhoenixApp, dialog: &InviteDialog, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.guilds.iter().find(|g| g.id == dialog.guild).map(|g| g.name.clone()).unwrap_or_default();
    let target = dialog.channel.and_then(|id| app.channels.get(&id)).map(|ch| ch.name.clone());
    let query = app.nav.invite_friend_search.as_ref().map(|input| input.read(cx).value().trim().to_lowercase()).unwrap_or_default();
    let mut friends: Vec<_> = app.nav.relationships.iter()
        .filter(|rel| rel.kind == RelationshipKind::Friend)
        .filter_map(|rel| app.users.get(&rel.user).map(|user| (rel, user)))
        .filter(|(rel, user)| user.name.to_lowercase().contains(&query) || user.display_name.to_lowercase().contains(&query)
            || rel.nickname.as_ref().is_some_and(|name| name.to_lowercase().contains(&query)))
        .collect();
    friends.sort_by_cached_key(|(rel, user)| rel.nickname.as_ref().unwrap_or(&user.display_name).to_lowercase());
    let can_send = matches!(dialog.link.as_ref(), Some(Ok(_)));
    let friend_list = div().id("invite-friends-list").max_h(z(320.)).overflow_y_scroll().flex().flex_col()
        .when(friends.is_empty(), |d| d.child(div().py(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("invite.no_friends"))))
        .children(friends.into_iter().map(|(rel, user)| {
            let id = user.id;
            let sent = dialog.sent.contains(&id);
            let sending = dialog.sending.contains(&id);
            let active = can_send && !sent && !sending;
            div().id(("invite-friend", id.0)).flex().items_center().gap(z(10.)).py(z(5.))
                .child(crate::ui::avatar(Some(user), 32.))
                .child(div().flex_1().min_w_0().flex().flex_col()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_ellipsis().overflow_hidden()
                        .child(rel.nickname.clone().unwrap_or_else(|| user.display_name.clone())))
                    .child(div().text_xs().text_color(c(Palette::TEXT_MUTED)).text_ellipsis().overflow_hidden().child(user.name.clone())))
                .child(crate::ui::controls::button(("send-friend-invite", id.0),
                    if sent { crate::t!("invite.sent") } else if sending { crate::t!("invite.sending") } else { crate::t!("invite.send") },
                    crate::ui::controls::BtnKind::Secondary)
                    .when(active, |b| b.on_click(cx.listener(move |app, _, _, cx| app.send_friend_invite(id, cx))))
                    .when(!active, |b| b.opacity(0.55)))
        }));
    let shown = match &dialog.link {
        Some(Ok(_)) if crate::streamer::hides_invites(&app.prefs.local) => crate::t!("streamer.invite_hidden"),
        Some(Ok(link)) | Some(Err(link)) => link.clone(),
        None => crate::t!("invite.creating"),
    };
    let inner = div().p(z(18.)).flex().flex_col().gap(z(12.))
        .child(div().pr(z(28.)).text_size(z(19.)).font_weight(FontWeight::BOLD).child(crate::t!("invite.friends_to", name = name)))
        .when_some(target, |d, channel| d.child(div().text_sm().text_color(c(Palette::TEXT_MUTED)).child(crate::t!("invite.target", channel = channel))))
        .when_some(app.nav.invite_friend_search.clone(), |d, input| d.child(crate::ui::modals::input_box(&input)))
        .child(friend_list)
        .when_some(dialog.send_error.clone(), |d, error| d.child(div().text_sm().text_color(c(Palette::NOTIFICATION)).child(error)))
        .child(div().pt(z(10.)).border_t_1().border_color(c(Palette::BORDER_MUTED)).text_sm().font_weight(FontWeight::SEMIBOLD).child(crate::t!("invite.or_send")))
        .child(div().px(z(6.)).py(z(4.)).rounded(z(6.)).bg(c(Palette::BG_TERTIARY)).flex().items_center().gap(z(8.))
            .child(div().flex_1().min_w_0().text_sm().text_ellipsis().overflow_hidden().child(shown))
            .child(button("copy-invite", if dialog.copied { crate::t!("invite.copied") } else { crate::t!("common.copy") }, Palette::BLURPLE)
                .h(z(32.)).when(can_send, |b| b.on_click(cx.listener(|app, _, _, cx| {
                    if let Some(Overlay::InviteLink(dialog)) = &mut app.nav.overlay
                        && let Some(Ok(link)) = &dialog.link
                    {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.to_string()));
                        dialog.copied = true;
                        cx.notify();
                    }
                }))).when(!can_send, |b| b.opacity(0.55))))
        .when(can_send, |d| d.child(div().text_xs().text_color(c(Palette::TEXT_MUTED)).child(crate::t!("invite.expires"))));
    modal("invite-modal", cx, inner)
}

pub fn render(app: &PhoenixApp, ov: &Overlay, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    match ov {
        Overlay::JoinServer => crate::ui::add_server::render_modal(app, window, cx),
        Overlay::InviteLink(dialog) => invite_dialog(app, dialog, cx),
        Overlay::LeaveGuild(g) => {
            let g = *g;
            let name = app.guilds.iter().find(|x| x.id == g).map(|x| x.name.clone()).unwrap_or_default();
            let (e1, e2) = (cx.entity(), cx.entity());
            crate::ui::modal::confirm(crate::ui::modal::Confirm {
                title: crate::t!("invite.leave_title", name = name.clone()),
                body: vec![crate::t!("invite.leave_body", name = name).into_any_element()],
                confirm_label: crate::t!("menu.leave_server"),
                danger: true,
                on_confirm: std::rc::Rc::new(move |_, cx| e1.update(cx, |a, cx| a.leave_guild(g, cx))),
                on_cancel: std::rc::Rc::new(move |_, cx| e2.update(cx, |a, cx| a.close_overlay(cx))),
            })
        }
        Overlay::Stub(what) => {
            let inner = div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .p(z(16.))
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(title(what.clone()))
                        .child(
                            div()
                                .text_size(z(14.))
                                .text_color(c(Palette::TEXT_MUTED))
                                .text_center()
                                .child(crate::t!("invite.stub")),
                        ),
                )
                .child(
                    footer().child(
                        button("stub-ok", crate::t!("common.okay"), Palette::BLURPLE)
                            .on_click(cx.listener(|a, _, _, cx| a.close_overlay(cx))),
                    ),
                );
            modal("stub-modal", cx, inner)
        }
        Overlay::Topic(_) => div().into_any_element(),
    }
}
