use gpui::{
    AnyElement, Context, Div, ElementId, Entity, FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Point, SharedString, Stateful, Styled,
    Window, div, img, prelude::*, px,
};

use crate::{
    app::PhoenixApp,
    commands::guide::GuildProfile,
    model::Id,
    presence::TagPopout,
    theme::{c, generated::idx, tok, z},
};

const POPOUT_W: f32 = 300.;
const POPOUT_H: f32 = 360.;

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn clickable(
    chip: Div,
    id: impl Into<ElementId>,
    guild_id: Option<Id>,
    tag: &(SharedString, Option<SharedString>),
    entity: Entity<PhoenixApp>,
) -> Stateful<Div> {
    let tip = crate::t!("tag_popout.tooltip", tag = tag.0.as_ref());
    let t = tag.0.clone();
    let b = tag.1.clone();
    chip.id(id)
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_MUTED)))
        .tooltip(crate::ui::tooltip::tip(tip))
        .when_some(guild_id, |d, gid| {
            d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(move |ev, w, cx| {
                cx.stop_propagation();
                entity.update(cx, |a, cx| a.open_tag_popout(gid, t.clone(), b.clone(), ev.position(), w, cx));
            })
        })
}

fn cdn(path: String) -> SharedString {
    format!("https://cdn.discordapp.com/{path}").into()
}

fn parse_hex_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn acronym(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace() {
        if let Some(ch) = word.chars().next() {
            out.push(ch);
            if out.chars().count() >= 3 {
                break;
            }
        }
    }
    if out.is_empty() { "?".into() } else { out }
}

impl PhoenixApp {
    pub fn open_tag_popout(
        &mut self,
        guild_id: Id,
        tag: SharedString,
        badge: Option<SharedString>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.people.tag_popout.as_ref().is_some_and(|p| p.guild_id == guild_id) {
            self.people.tag_popout = None;
            cx.notify();
            return;
        }
        self.people.tag_popout = Some(TagPopout {
            guild_id,
            tag,
            badge,
            at,
            profile: None,
            loading: true,
            private: false,
            error: None,
        });
        let fut = self.backend.guild_profile(guild_id);
        cx.spawn_in(window, async move |this, cx| {
            let res = fut.await;
            let _ = this.update_in(cx, |a, _, cx| {
                let Some(pop) = a.people.tag_popout.as_mut().filter(|p| p.guild_id == guild_id) else {
                    return;
                };
                pop.loading = false;
                match res {
                    Ok(profile) => pop.profile = Some(profile),
                    Err(e) => {
                        if e.0.contains("50001") || e.0.contains("Missing Access") {
                            pop.private = true;
                        } else {
                            pop.error = Some(e.0.into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

pub fn render(app: &PhoenixApp, window: &Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let pop = app.people.tag_popout.as_ref()?;
    let vp = window.viewport_size();
    let w = z(POPOUT_W);
    let h = z(POPOUT_H);
    let left = (pop.at.x + px(8.)).min(vp.width - w - px(12.)).max(px(12.));
    let top = (pop.at.y - px(24.)).min(vp.height - h - px(12.)).max(px(12.));

    let card = if pop.loading {
        div()
            .w(w)
            .rounded(z(16.))
            .border_1()
            .border_color(tk(idx::BORDER_SUBTLE))
            .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
            .p(z(16.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(14.))
            .text_color(tk(idx::TEXT_MUTED))
            .child(crate::t!("tag_popout.loading"))
    } else if pop.private {
        render_fallback(pop, crate::t!("tag_popout.private"), false, cx)
    } else if let Some(p) = &pop.profile {
        render_profile(app, pop, p, cx)
    } else {
        render_fallback(pop, crate::t!("tag_popout.error"), true, cx)
    };

    Some(
        div()
            .id("tag-popout-backdrop")
            .absolute()
            .inset_0()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|a, _, _, cx| {
                    a.people.tag_popout = None;
                    cx.notify();
                }),
            )
            .child(crate::ui::popout_motion::popout(
                "tag-popout-pt",
                crate::ui::popout_motion::Kind::Translate,
                crate::ui::popout_motion::side_of(f32::from(pop.at.x), f32::from(left)),
                div()
                    .id("tag-popout-card")
                    .absolute()
                    .left(left)
                    .top(top)
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(card),
            ))
            .into_any_element(),
    )
}

fn render_fallback(pop: &TagPopout, msg: SharedString, is_error: bool, cx: &mut Context<PhoenixApp>) -> Div {
    let title: SharedString = if pop.tag.is_empty() { crate::t!("tag_popout.unknown_server") } else { pop.tag.clone() };
    let gid = pop.guild_id;
    let tag = pop.tag.clone();
    let badge = pop.badge.clone();
    let at = pop.at;
    div()
        .w(z(POPOUT_W))
        .rounded(z(16.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().h(z(120.)).w_full().bg(tk(idx::BACKGROUND_MOD_STRONG)))
        .child(
            div().mt(z(-34.)).px(z(16.)).child(
                div()
                    .size(z(70.))
                    .rounded(z(21.))
                    .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(z(64.))
                            .rounded(z(19.))
                            .bg(tk(idx::BACKGROUND_MOD_STRONG))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(z(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tk(idx::TEXT_STRONG))
                            .child(acronym(&title)),
                    ),
            ),
        )
        .child(
            div()
                .mt(z(8.))
                .mb(z(8.))
                .px(z(16.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_STRONG))
                .child(title),
        )
        .child(
            div()
                .id("tag-popout-retry")
                .px(z(16.))
                .pb(z(16.))
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(tk(if is_error { idx::TEXT_FEEDBACK_WARNING } else { idx::TEXT_SUBTLE }))
                .when(is_error, |d| {
                    d.cursor_pointer().on_click(cx.listener(move |a, _, w, cx| {
                        a.people.tag_popout = None;
                        a.open_tag_popout(gid, tag.clone(), badge.clone(), at, w, cx);
                    }))
                })
                .child(msg),
        )
}

fn render_profile(app: &PhoenixApp, pop: &TagPopout, p: &GuildProfile, cx: &mut Context<PhoenixApp>) -> Div {
    let gid = pop.guild_id;
    let is_member = app.guilds.iter().any(|g| g.id == gid);
    let banner_bg = p.brand_color_primary.as_deref().and_then(parse_hex_color).map(c).unwrap_or_else(|| tk(idx::BACKGROUND_MOD_STRONG));
    let banner = div()
        .relative()
        .h(z(120.))
        .w_full()
        .flex_none()
        .overflow_hidden()
        .bg(banner_bg)
        .when_some(p.custom_banner_hash.clone(), |d, h| {
            d.child(
                img(cdn(format!("discovery-splashes/{}/{h}.jpg?size=640", gid.0)))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            )
        });

    let icon_inner = if let Some(h) = &p.icon_hash {
        img(cdn(format!("icons/{}/{h}.png?size=128", gid.0)))
            .size(z(64.))
            .rounded(z(19.))
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element()
    } else {
        div()
            .size(z(64.))
            .rounded(z(19.))
            .bg(tk(idx::BACKGROUND_MOD_STRONG))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .child(acronym(&p.name))
            .into_any_element()
    };

    let avatar = div().mt(z(-34.)).px(z(16.)).child(
        div()
            .size(z(70.))
            .rounded(z(21.))
            .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
            .flex()
            .items_center()
            .justify_center()
            .child(icon_inner),
    );

    let tag_chip = p.tag.clone().or_else(|| (!pop.tag.is_empty()).then(|| pop.tag.to_string())).map(|tag| {
        let badge_url = p
            .badge_hash
            .as_ref()
            .map(|h| cdn(format!("guild-tag-badges/{}/{h}.png?size=16", gid.0)))
            .or_else(|| pop.badge.clone());
        div()
            .flex_none()
            .h(z(16.))
            .px(z(4.))
            .rounded(z(4.))
            .bg(tk(idx::BACKGROUND_MOD_STRONG))
            .flex()
            .items_center()
            .gap(z(2.))
            .when_some(badge_url, |d, u| d.child(img(u).size(z(14.)).flex_none()))
            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).child(tag))
    });

    let name_el = div()
        .id("tag-popout-guild-name")
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_STRONG))
        .when(is_member, |d| {
            d.cursor_pointer().hover(|s| s.underline()).on_click(cx.listener(move |a, _, _, cx| {
                a.people.tag_popout = None;
                a.select_guild(Some(gid), cx);
            }))
        })
        .child(p.name.clone());

    let count_item = |online: bool, label: SharedString| {
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .child(div().size(z(8.)).rounded_full().bg(tk(if online { idx::TEXT_STATUS_ONLINE } else { idx::TEXT_STATUS_OFFLINE })))
            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_SUBTLE)).child(label))
    };

    let header = div()
        .mt(z(8.))
        .mb(z(8.))
        .px(z(16.))
        .w_full()
        .flex()
        .flex_col()
        .gap(z(2.))
        .child(div().flex().items_center().gap(z(4.)).w_full().child(name_el).children(tag_chip))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(z(8.))
                .child(count_item(true, crate::t!("guide.online", n = p.online_count.to_string())))
                .child(count_item(false, crate::t!("guide.members", n = p.member_count.to_string()))),
        )
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(tk(idx::TEXT_SUBTLE))
                .child(crate::ui::guide::established(gid)),
        );

    let mut body = div().px(z(16.)).mb(z(16.)).w_full().flex().flex_col().gap(z(12.));
    let mut has_body = false;
    if !p.description.is_empty() {
        has_body = true;
        body = body.child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(p.description.clone()));
    }
    if !p.traits.is_empty() {
        has_body = true;
        let mut traits_row = div().flex().flex_wrap().gap(z(4.));
        for tr in p.traits.iter().take(5) {
            if tr.label.is_empty() {
                continue;
            }
            traits_row = traits_row.child(
                div()
                    .py(z(4.))
                    .px(z(8.))
                    .rounded(z(16.))
                    .border_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .when_some(tr.emoji.token(), |d, tk_e| d.child(crate::media::emoji::emoji_element(&tk_e, 16.)))
                    .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(tr.label.clone())),
            );
        }
        body = body.child(traits_row);
    }

    let cta_label = if is_member {
        crate::t!("tag_popout.go_to_server")
    } else if p.visibility == 3 {
        crate::t!("tag_popout.apply_to_join")
    } else {
        crate::t!("tag_popout.join_server")
    };

    let cta = div().px(z(16.)).pb(z(16.)).w_full().child(
        div()
            .id("tag-popout-cta")
            .w_full()
            .h(z(32.))
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_BRAND))
            .hover(|s| s.opacity(0.9))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::WHITE))
            .on_click(cx.listener(move |a, _, window, cx| {
                if is_member {
                    a.people.tag_popout = None;
                    a.select_guild(Some(gid), cx);
                } else {
                    let fut = a.backend.join_guild(gid);
                    cx.spawn_in(window, async move |this, cx| {
                        if let Err(e) = fut.await {
                            let _ = this.update_in(cx, |a, _, cx| {
                                if let Some(pop) = a.people.tag_popout.as_mut() {
                                    pop.error = Some(e.0.into());
                                }
                                cx.notify();
                            });
                        }
                    })
                    .detach();
                }
            }))
            .child(cta_label),
    );

    div()
        .w(z(POPOUT_W))
        .rounded(z(16.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(banner)
        .child(avatar)
        .child(header)
        .when(has_body, |d| d.child(body))
        .when_some(pop.error.clone(), |d, err| {
            d.child(div().px(z(16.)).pb(z(8.)).text_size(z(12.)).text_color(tk(idx::TEXT_FEEDBACK_WARNING)).child(err))
        })
        .child(cta)
}
