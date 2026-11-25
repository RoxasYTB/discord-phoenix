use crate::theme::z;
use gpui::{
    AnyElement, Context, FontWeight, ImageSource, IntoElement, ParentElement, SharedString, Styled, Window, div,
    img, prelude::*, px,
};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef},
    theme::{DEFAULT_AVATAR, Palette, c, ca},
    ui::home::data::{InvitePreview, NavReply, NavRequest, Overlay, invite_code},
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
        self.show_overlay(Overlay::InviteLink { guild, link: None }, cx);
        let channel = self
            .selected_channel
            .filter(|c| self.channels.get(c).is_some_and(|c| c.guild == Some(guild)))
            .or_else(|| {
                self.guild_channels.get(&guild).and_then(|l| {
                    l.iter().copied().find(|id| self.channels[id].kind == crate::model::ChannelKind::Text)
                })
            });
        let Some(channel) = channel else { return };
        self.nav_request(NavRequest::CreateInvite(channel), cx, move |this, res, _| {
            let link: SharedString = match res {
                Ok(NavReply::Text(t)) => t,
                Ok(_) => "".into(),
                Err(e) => crate::t!("invite.create_failed", e = e.0),
            };
            if let Some(Overlay::InviteLink { guild: g, .. }) = &this.nav.overlay
                && *g == guild
            {
                this.nav.overlay = Some(Overlay::InviteLink { guild, link: Some(link) });
            }
        });
    }

    pub fn leave_guild(&mut self, guild: Id, cx: &mut Context<Self>) {
        self.close_overlay(cx);
        self.nav_request(NavRequest::LeaveGuild(guild), cx, move |this, res, cx| {
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

pub fn render(app: &PhoenixApp, ov: &Overlay, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    match ov {
        Overlay::JoinServer => crate::ui::add_server::render_modal(app, window, cx),
        Overlay::InviteLink { guild, link } => {
            let name = app.guilds.iter().find(|g| g.id == *guild).map(|g| g.name.clone()).unwrap_or_default();
            let link = link.clone();
            let shown: SharedString = match &link {
                Some(_) if crate::streamer::hides_invites(&app.prefs.local) => crate::t!("streamer.invite_hidden"),
                Some(l) => l.clone(),
                None => crate::t!("invite.creating"),
            };
            let inner = div()
                .p(z(16.))
                .flex()
                .flex_col()
                .gap(z(12.))
                .child(title(crate::t!("invite.friends_to", name = name)))
                .child(
                    div()
                        .text_size(z(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("invite.or_send")),
                )
                .child(
                    div()
                        .h(z(44.))
                        .pl(z(10.))
                        .pr(z(4.))
                        .rounded(z(4.))
                        .bg(c(Palette::BG_TERTIARY))
                        .flex()
                        .items_center()
                        .child(div().flex_1().text_size(z(16.)).text_color(c(Palette::TEXT_NORMAL)).child(shown))
                        .child(button("copy-invite", crate::t!("common.copy"), Palette::BLURPLE).h(z(32.)).on_click(cx.listener(
                            move |_, _, _, cx| {
                                if let Some(l) = &link {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(l.to_string()));
                                }
                            },
                        ))),
                )
                .child(
                    div()
                        .text_size(z(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("invite.expires")),
                );
            modal("invite-modal", cx, inner)
        }
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
