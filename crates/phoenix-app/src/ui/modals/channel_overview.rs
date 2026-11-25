use std::rc::Rc;

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::Textarea;
use serde_json::Value;

use super::{
    channel_settings::{AUTO_ARCHIVE, REGIONS, REQUIRE_TAG, SLOWMODE, State, TAG_MAX, TagDraft, slowmode_label, tag_json, with_state},
    input_box,
    settings_form::{OnPick, description, dropdown, field, format_toolbar, helper, info_notice, radio_group, switch_field, text, value_slider, wrap_selection},
};
use crate::{
    app::PhoenixApp,
    model::ChannelKind,
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button, checkbox_option},
        pickers::Field,
    },
};

fn refresh(e: &gpui::Entity<PhoenixApp>, cx: &mut App) {
    e.update(cx, |_, cx| cx.notify());
}

fn toggle(which: u8) -> Rc<dyn Fn(&mut App)> {
    Rc::new(move |_| with_state(|s| s.open_select = if s.open_select == Some(which) { None } else { Some(which) }))
}

fn picker(e: &gpui::Entity<PhoenixApp>, f: impl Fn(&mut State, usize) + 'static) -> OnPick {
    let e = e.clone();
    Rc::new(move |k, _, cx| {
        with_state(|s| {
            f(s, k);
            s.open_select = None;
        });
        refresh(&e, cx);
    })
}

pub fn max_bitrate(app: &PhoenixApp, s: &State) -> u32 {
    if s.kind == ChannelKind::Stage {
        return 64_000;
    }
    let g = s.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let tier = match g.map_or(0, |g| g.boost_tier) {
        1 => 128_000,
        2 => 256_000,
        3 => 384_000,
        _ => 96_000,
    };
    let feats = s.guild.map(|g| app.backend.guild_features(g)).unwrap_or_default();
    let feat = feats
        .iter()
        .filter_map(|f| match f.as_str() {
            "AUDIO_BITRATE_128_KBPS" => Some(128_000),
            "AUDIO_BITRATE_256_KBPS" => Some(256_000),
            "AUDIO_BITRATE_384_KBPS" | "VIP_REGIONS" => Some(384_000),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    tier.max(feat)
}

fn slow_select(
    id: &'static str,
    which: u8,
    value: u32,
    open: bool,
    e: &gpui::Entity<PhoenixApp>,
    set: fn(&mut State, u32),
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let sel = SLOWMODE.iter().position(|x| *x == value).unwrap_or(0);
    dropdown(
        id,
        SLOWMODE.iter().map(|x| slowmode_label(*x)).collect(),
        sel,
        open,
        toggle(which),
        picker(e, move |s, k| set(s, SLOWMODE[k.min(SLOWMODE.len() - 1)])),
        cx,
    )
}

fn topic_block(s: &State, forum: bool, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let area = s.topic.clone()?;
    let max: i64 = if forum { 4096 } else { 1024 };
    let left = max - area.read(cx).value().encode_utf16().count() as i64;
    let a2 = area.clone();
    let wrap: super::settings_form::OnWrap = Rc::new(move |mark, w, cx| {
        a2.update(cx, |st, cx| {
            let (t, r) = wrap_selection(&st.value(), st.selected_range(), mark);
            st.set_value(t, w, cx);
            st.set_selected_range(r, cx);
            st.focus(w, cx);
        });
    });
    Some(
        div()
            .flex()
            .flex_col()
            .child(format_toolbar(
                "cs-topic-tools",
                forum,
                wrap,
                Some(emoji_button("cs-topic-emoji", Field::ChannelTopic, 26., 32., 8., cx).into_any_element()),
            ))
            .child(
                div()
                    .relative()
                    .h(z(202.))
                    .rounded_b(z(8.))
                    .border_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                    .pl(z(6.))
                    .pr(z(12.))
                    .py(z(4.))
                    .text_size(z(16.))
                    .line_height(z(22.))
                    .child(Textarea::new(&area).appearance(false).bordered(false))
                    .child(
                        div().absolute().right(z(13.)).bottom(z(10.)).child(
                            text(left.to_string(), 14., 18., if left < 0 { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT })
                                .font_weight(FontWeight::SEMIBOLD),
                        ),
                    ),
            )
            .into_any_element(),
    )
}

pub const ILLUSTRATION: &str = "https://discord.com/assets/b00bbf36ff21d401.svg";

fn illustration() -> gpui::Div {
    div().mt(z(60.)).flex().justify_center().child(gpui::img(SharedString::from(ILLUSTRATION)).w(z(280.)).h(z(165.)))
}

fn divider() -> gpui::Div {
    div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let v = &s.v;
    let kind = s.kind;
    let mut d = div().flex().flex_col().gap(z(24.));
    if let Some(i) = &s.name {
        let key = if kind == ChannelKind::Category { "admin.category_name" } else { "admin.channel_name" };
        d = d.child(field(
            crate::t!(key),
            None,
            input_box(i).pl(z(0.)).pr(z(4.)).gap(z(4.)).child(div().flex_none().w(z(32.)).h(z(30.)).flex().items_center().child(emoji_button(
                "cs-name-emoji",
                Field::ChannelName,
                30.,
                18.,
                12.,
                cx,
            ))),
            None,
        ));
    }
    if kind == ChannelKind::Category {
        return d.child(illustration()).into_any_element();
    }
    let forum = kind == ChannelKind::Forum;
    if matches!(kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum)
        && let Some(t) = topic_block(s, forum, cx)
    {
        d = d.child(field(crate::t!(if forum { "cs.post_guidelines" } else { "admin.channel_topic" }), None, t, None));
    }
    if forum {
        let tags = {
            let mut row = div().flex().flex_wrap().items_center().gap(z(8.)).mb(z(16.));
            for (n, t) in s.tags.iter().enumerate() {
                let tag = t.clone();
                row = row.child(tag_pill(n, t).on_click(cx.listener(move |_, _, _, cx| {
                    let d = TagDraft::edit(n, &tag);
                    with_state(|s| s.tag_edit = Some(d));
                    cx.notify();
                })));
            }
            row = row.child(
                div()
                    .id("cs-new-tag")
                    .size(z(30.))
                    .rounded_full()
                    .bg(c(tok(idx::BRAND_500)))
                    .hover(|d| d.bg(c(tok(idx::BRAND_560))))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|_, _, _, cx| {
                        with_state(|s| s.tag_edit = Some(TagDraft::new()));
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("plus")).size(z(20.)).text_color(c(tok(idx::WHITE)))),
            );
            let req = v.flags & REQUIRE_TAG != 0;
            div().flex().flex_col().child(row).child(checkbox_option("cs-req-tags", crate::t!("cs.require_tags"), req).on_click(cx.listener(|_, _, _, cx| {
                with_state(|s| s.v.flags ^= REQUIRE_TAG);
                cx.notify();
            })))
        };
        d = d.child(field(crate::t!("cs.tags"), Some(crate::t!("cs.tags_desc")), tags, None));
        let reaction = div()
            .flex()
            .justify_between()
            .child(
                div()
                    .w(z(284.))
                    .mr(z(16.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(button("cs-react", crate::t!("cs.select_emoji"), BtnKind::Primary).on_click(cx.listener(|a, _, w, cx| {
                        crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Field(Field::ForumReaction), w, cx)
                    })))
                    .when(s.default_reaction.is_some(), |d| {
                        d.child(
                            div()
                                .id("cs-react-remove")
                                .cursor_pointer()
                                .hover(|d| d.underline())
                                .on_click(cx.listener(|_, _, _, cx| {
                                    with_state(|s| s.default_reaction = None);
                                    cx.notify();
                                }))
                                .child(text(crate::t!("cs.remove"), 16., 20., idx::TEXT_FEEDBACK_CRITICAL).font_weight(FontWeight::MEDIUM)),
                        )
                    }),
            )
            .child(reaction_preview(s.default_reaction.as_ref()));
        d = d.child(field(crate::t!("cs.default_reaction"), Some(crate::t!("cs.default_reaction_desc")), reaction, None));
        d = d.child(
            div()
                .flex()
                .flex_col()
                .gap(z(16.))
                .child(text(crate::t!("admin.slowmode"), 20., 24., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM))
                .child(field(
                    crate::t!("cs.posts"),
                    None,
                    slow_select("cs-slow", 1, v.slow, s.open_select == Some(1), &e, |s, x| s.v.slow = x, cx),
                    Some(crate::t!("cs.slow_posts_help")),
                ))
                .child(field(
                    crate::t!("cs.messages"),
                    None,
                    slow_select("cs-tslow", 2, v.thread_slow, s.open_select == Some(2), &e, |s, x| s.v.thread_slow = x, cx),
                    Some(crate::t!("cs.slow_messages_help")),
                )),
        );
        let layouts = vec![crate::t!("cs.layout_list"), crate::t!("cs.layout_gallery")];
        let lsel = if v.layout == 2 { 1 } else { 0 };
        d = d.child(
            div()
                .flex()
                .justify_between()
                .child(div().w(z(312.)).mr(z(16.)).child(field(
                    crate::t!("cs.default_layout"),
                    Some(crate::t!("cs.default_layout_desc")),
                    dropdown("cs-layout", layouts, lsel, s.open_select == Some(3), toggle(3), picker(&e, |s, k| s.v.layout = if k == 1 { 2 } else { 1 }), cx),
                    None,
                )))
                .child(layout_preview(lsel == 1)),
        );
        let sorts = vec![crate::t!("cs.sort_activity"), crate::t!("cs.sort_creation")];
        d = d.child(field(
            crate::t!("cs.sort_order"),
            Some(crate::t!("cs.sort_order_desc")),
            dropdown("cs-sort", sorts, v.sort.unwrap_or(0) as usize, s.open_select == Some(4), toggle(4), picker(&e, |s, k| s.v.sort = Some(k as u8)), cx),
            None,
        ));
        let matching = vec![crate::t!("cs.match_some"), crate::t!("cs.match_all")];
        let msel = usize::from(v.tag_setting.as_deref() == Some("match_all"));
        d = d.child(field(
            crate::t!("cs.tag_matching"),
            Some(crate::t!("cs.tag_matching_desc")),
            dropdown(
                "cs-match",
                matching,
                msel,
                s.open_select == Some(5),
                toggle(5),
                picker(&e, |s, k| s.v.tag_setting = Some(if k == 1 { "match_all" } else { "match_some" }.into())),
                cx,
            ),
            None,
        ));
    } else if kind != ChannelKind::Announcement {
        let help = if matches!(kind, ChannelKind::Text | ChannelKind::Voice | ChannelKind::Stage) { "cs.slow_text_help" } else { "cs.slow_plain_help" };
        d = d.child(field(
            crate::t!("admin.slowmode"),
            None,
            slow_select("cs-slow", 1, v.slow, s.open_select == Some(1), &e, |s, x| s.v.slow = x, cx),
            Some(crate::t!(help)),
        ));
    }
    let vis = vec![
        (crate::t!("cs.vis_default"), Some(crate::t!("cs.vis_default_desc"))),
        (crate::t!("cs.vis_spoiler"), Some(crate::t!("cs.vis_spoiler_desc"))),
        (crate::t!("admin.nsfw"), Some(crate::t!("cs.vis_age_desc"))),
    ];
    d = d.child(div().py(z(8.)).child(field(
        crate::t!("cs.visibility"),
        None,
        radio_group("cs-vis", vis, v.visibility(), picker(&e, |s, k| s.v.set_visibility(k))),
        None,
    )));
    let community = s.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).is_some_and(|g| g.community);
    if community && matches!(kind, ChannelKind::Text | ChannelKind::Announcement) {
        let on = v.kind == 5;
        let desc = description(crate::t!("cs.announcement_desc")).into_any_element();
        let mut block = div().flex().flex_col().gap(z(4.)).child(switch_field(
            "cs-news",
            crate::t!("cs.announcement"),
            Some(desc),
            on,
            Rc::new(|_| with_state(|s| s.v.kind = if s.v.kind == 5 { 0 } else { 5 })),
            cx,
        ));
        if !on {
            block = block.child(info_notice(crate::t!("cs.announcement_notice")));
        }
        d = d.child(block);
    }
    if matches!(kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum) {
        let sel = AUTO_ARCHIVE.iter().position(|x| x.0 == v.archive).unwrap_or(2);
        let help = if forum { "cs.hide_posts_help" } else { "cs.hide_threads_help" };
        d = d.child(div().mt(z(16.)).child(field(
            crate::t!("thread.hide_after"),
            None,
            dropdown(
                "cs-archive",
                AUTO_ARCHIVE.iter().map(|(_, k)| crate::t!(k)).collect(),
                sel,
                s.open_select == Some(6),
                toggle(6),
                picker(&e, |s, k| s.v.archive = AUTO_ARCHIVE[k.min(3)].0),
                cx,
            ),
            Some(crate::t!(help)),
        )));
    }
    if matches!(kind, ChannelKind::Voice | ChannelKind::Stage) {
        let max = max_bitrate(app, s) / 1000;
        let mut marks = vec![(8, SharedString::from("8kbps")), (64, SharedString::from("64kbps"))];
        if max > 64 {
            marks.push((max, SharedString::from(format!("{max}kbps"))));
        }
        let e1 = e.clone();
        d = d.child(divider().my(z(16.))).child(field(
            crate::t!("cs.bitrate"),
            None,
            value_slider(
                "cs-bitrate",
                8,
                max,
                v.bitrate / 1000,
                marks,
                Rc::new(move |k, _, cx| {
                    with_state(|s| s.v.bitrate = k * 1000);
                    refresh(&e1, cx);
                }),
            ),
            Some(crate::t!("cs.bitrate_help", bitrate = 64)),
        ));
        let quality = vec![(crate::t!("cs.auto"), None), (SharedString::from("720p"), None)];
        d = d.child(field(
            crate::t!("cs.video_quality"),
            None,
            div()
                .flex()
                .flex_col()
                .child(radio_group("cs-video", quality, if v.video == 2 { 1 } else { 0 }, picker(&e, |s, k| s.v.video = if k == 1 { 2 } else { 1 })))
                .child(helper(crate::t!("cs.video_quality_help")).mt(z(4.))),
            None,
        ));
        let lim_max = if kind == ChannelKind::Stage { 10_000 } else { 99 };
        let e2 = e.clone();
        d = d.child(field(
            crate::t!("cs.user_limit"),
            None,
            value_slider(
                "cs-limit",
                0,
                lim_max,
                v.user_limit,
                vec![(0, "∞".into()), (lim_max, lim_max.to_string().into())],
                Rc::new(move |k, _, cx| {
                    with_state(|s| s.v.user_limit = k);
                    refresh(&e2, cx);
                }),
            ),
            Some(crate::t!(if kind == ChannelKind::Stage { "cs.user_limit_stage_help" } else { "cs.user_limit_help" })),
        ));
        if kind == ChannelKind::Voice {
            let mut opts = vec![crate::t!("cs.auto")];
            opts.extend(REGIONS.iter().map(|(_, n)| SharedString::from(*n)));
            let sel = v.region.as_deref().and_then(|r| REGIONS.iter().position(|x| x.0 == r)).map_or(0, |i| i + 1);
            d = d.child(field(
                crate::t!("cs.region"),
                Some(crate::t!("cs.region_desc")),
                dropdown(
                    "cs-region",
                    opts,
                    sel,
                    s.open_select == Some(7),
                    toggle(7),
                    picker(&e, |s, k| s.v.region = k.checked_sub(1).map(|i| REGIONS[i].0.to_owned())),
                    cx,
                ),
                None,
            ));
        }
    }
    if kind != ChannelKind::Voice {
        d = d.child(illustration());
    }
    d.into_any_element()
}

pub fn emoji_button(id: &'static str, f: Field, w: f32, h: f32, pr: f32, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .w(z(w))
        .h(z(h))
        .pr(z(pr))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group(id)
        .on_click(cx.listener(move |a, _, w, cx| crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Field(f), w, cx)))
        .child(
            gpui::svg()
                .path(crate::ui::Icon::Emoji.path())
                .size(z(18.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
        )
}

fn emoji_glyph(e: &Value, size: f32) -> Option<AnyElement> {
    if let Some(id) = e.get("emoji_id").and_then(Value::as_str) {
        return Some(
            gpui::img(SharedString::from(format!("https://cdn.discordapp.com/emojis/{id}.webp?size=48"))).size(z(size)).flex_none().into_any_element(),
        );
    }
    let n = e.get("emoji_name").and_then(Value::as_str).filter(|n| !n.is_empty())?;
    let url = crate::media::emoji::twemoji_url(n)?;
    Some(gpui::img(url).size(z(size)).flex_none().into_any_element())
}

fn tag_pill(n: usize, t: &Value) -> gpui::Stateful<gpui::Div> {
    let name = t.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
    div()
        .id(("cs-tag", n))
        .h(z(32.))
        .px(z(12.))
        .rounded(z(20.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|d| d.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .active(|d| d.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(z(4.))
        .overflow_hidden()
        .children(emoji_glyph(t, 16.))
        .child(text(name, 14., 18., idx::CONTROL_SECONDARY_TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD).truncate())
}

fn placeholder_line(w: f32) -> gpui::Div {
    div().w(z(w)).h(z(8.)).rounded(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
}

fn reaction_preview(r: Option<&Value>) -> gpui::Div {
    div()
        .w(z(360.))
        .flex_none()
        .p(z(12.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .justify_between()
                .child(
                    div()
                        .w(z(254.))
                        .flex()
                        .flex_col()
                        .child(placeholder_line(254.).mb(z(8.)))
                        .child(placeholder_line(204.).mb(z(16.)))
                        .child(placeholder_line(127.).mb(z(8.)))
                        .child(placeholder_line(153.).mb(z(16.))),
                )
                .child(div().ml(z(16.)).size(z(64.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))),
        )
        .child(
            div()
                .h(z(26.))
                .flex()
                .items_center()
                .child(
                    div()
                        .px(z(6.))
                        .py(z(4.))
                        .mr(z(8.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .when_some(r.and_then(|e| emoji_glyph(e, 16.)), |d, g| d.child(div().mr(z(4.)).child(g)))
                        .child(text("17", 14., 18., idx::TEXT_SUBTLE).font_weight(FontWeight::SEMIBOLD)),
                )
                .child(placeholder_line(24.))
                .child(div().mx(z(8.)).size(z(4.)).rounded_full().bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .child(placeholder_line(24.)),
        )
}

const LAYOUT_LIST: &str = r##"<svg width="272" height="143" viewBox="0 0 272 143" fill="none" xmlns="http://www.w3.org/2000/svg"><rect opacity="0.5" x="12" y="27.5" width="168" height="8" rx="4" fill="C"/><rect opacity="0.5" x="12" y="43.5" width="96" height="8" rx="4" fill="C"/><rect opacity="0.5" x="12" y="67.5" width="168" height="8" rx="4" fill="C"/><rect opacity="0.5" x="12" y="83.5" width="168" height="8" rx="4" fill="C"/><rect opacity="0.5" x="196" y="27.5" width="64" height="64" rx="8" fill="C"/><path fill-rule="evenodd" clip-rule="evenodd" d="M222 49.5C219.791 49.5 218 51.2909 218 53.5V65.5C218 67.7091 219.791 69.5 222 69.5H234C236.209 69.5 238 67.7091 238 65.5V53.5C238 51.2909 236.209 49.5 234 49.5H222ZM226 55.5C226 54.3952 225.103 53.5 224 53.5C222.894 53.5 222 54.3952 222 55.5C222 56.6056 222.894 57.5 224 57.5C225.103 57.5 226 56.6056 226 55.5ZM225 61.5L222 65.5H234L231 58.5L227 63.5L225 61.5Z" fill="#C4C4C4"/><rect opacity="0.5" x="12" y="107.5" width="32" height="8" rx="4" fill="C"/><circle opacity="0.5" cx="54" cy="111.5" r="2" fill="C" fill-opacity="0.48"/><rect opacity="0.5" x="64" y="107.5" width="32" height="8" rx="4" fill="C"/><rect x="0.5" y="16" width="271" height="111" rx="11.5" stroke="C" stroke-opacity="0.3"/></svg>"##;

const LAYOUT_GALLERY_CARD: &str = r##"<path opacity="0.5" d="M0 12C0 5.37259 5.37258 0 12 0H118C124.627 0 130 5.37258 130 12V83H0V12Z" fill="C"/><path fill-rule="evenodd" clip-rule="evenodd" d="M57.641 30.3944C54.9317 30.3944 52.7354 32.488 52.7354 35.0705V49.0987C52.7354 51.6812 54.9317 53.7747 57.641 53.7747H72.358C75.0673 53.7747 77.2637 51.6812 77.2637 49.0987V35.0705C77.2637 32.488 75.0673 30.3944 72.358 30.3944H57.641ZM62.5467 37.4085C62.5467 36.117 61.4468 35.0705 60.0938 35.0705C58.7379 35.0705 57.641 36.117 57.641 37.4085C57.641 38.701 58.7379 39.7466 60.0938 39.7466C61.4468 39.7466 62.5467 38.701 62.5467 37.4085ZM61.3203 44.4226L57.641 49.0987H72.358L68.6787 40.9156L63.7731 46.7606L61.3203 44.4226Z" fill="#C4C4C4"/><rect opacity="0.5" x="12" y="99" width="106" height="8" rx="4" fill="C"/><rect opacity="0.5" x="12" y="123" width="32" height="8" rx="4" fill="C"/><circle opacity="0.5" cx="54" cy="127" r="2" fill="C" fill-opacity="0.48"/><rect opacity="0.5" x="64" y="123" width="54" height="8" rx="4" fill="C"/><rect x="0.5" y="0.5" width="129" height="142" rx="11.5" stroke="C" stroke-opacity="0.3"/>"##;

pub fn layout_svg(gallery: bool, color: &str) -> String {
    let body = if gallery {
        format!(
            r#"<svg width="272" height="143" viewBox="0 0 272 143" fill="none" xmlns="http://www.w3.org/2000/svg">{LAYOUT_GALLERY_CARD}<g transform="translate(142 0)">{LAYOUT_GALLERY_CARD}</g></svg>"#
        )
    } else {
        LAYOUT_LIST.to_owned()
    };
    body.replace("\"C\"", &format!("\"{color}\""))
}

thread_local! {
    static LAYOUT_IMG: std::cell::RefCell<std::collections::HashMap<(bool, u32), std::sync::Arc<gpui::Image>>> = Default::default();
}

fn layout_preview(gallery: bool) -> gpui::Div {
    let rgba = gpui::Rgba::from(c(tok(idx::TEXT_SUBTLE)));
    let hex = (((rgba.r * 255.).round() as u32) << 16) | (((rgba.g * 255.).round() as u32) << 8) | ((rgba.b * 255.).round() as u32);
    let image = LAYOUT_IMG.with(|m| {
        m.borrow_mut()
            .entry((gallery, hex))
            .or_insert_with(|| std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, layout_svg(gallery, &format!("#{hex:06X}")).into_bytes())))
            .clone()
    });
    div().flex_none().w(z(272.)).h(z(143.)).child(gpui::img(image).w(z(272.)).h(z(143.)))
}

pub fn tag_modal(s: &State, t: &TagDraft, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        with_state(|s| s.tag_edit = None);
        e.update(cx, |_, cx| cx.notify());
    });
    let editing = t.index.is_some();
    let name = t.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| t.name0.clone());
    let valid = !name.trim().is_empty();
    let field_box = div()
        .h(z(40.))
        .pl(z(4.))
        .when(editing, |d| d.pr(z(4.)))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(16.))
        .child(
            div().flex_none().size(z(30.)).flex().items_center().justify_center().child(match t.emoji.as_ref().and_then(|e| emoji_glyph(e, 18.)) {
                Some(g) => div()
                    .id("cs-tag-emoji")
                    .cursor_pointer()
                    .on_click(cx.listener(|a, _, w, cx| {
                        crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Field(Field::TagEmoji), w, cx)
                    }))
                    .child(g)
                    .into_any_element(),
                None => emoji_button("cs-tag-emoji", Field::TagEmoji, 18., 18., 0., cx).into_any_element(),
            }),
        )
        .children(t.name.as_ref().map(|i| div().flex_1().min_w_0().child(gpui_component::input::Input::new(i).appearance(false).bordered(false))))
        .when(editing, |d| {
            d.child(
                div()
                    .id("cs-tag-clear")
                    .flex_none()
                    .size(z(32.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .on_click(cx.listener(|_, _, w, cx| {
                        let mut input = None;
                        with_state(|s| {
                            if let Some(t) = s.tag_edit.as_mut() {
                                t.emoji = None;
                                input = t.name.clone();
                            }
                        });
                        if let Some(i) = input {
                            i.update(cx, |st, cx| st.set_value("", w, cx));
                        }
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("close-small")).size(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))),
            )
        });
    let body = div()
        .px(z(24.))
        .pt(z(8.))
        .flex()
        .flex_col()
        .child(text(crate::t!("cs.tag_modal_desc", max = TAG_MAX), 16., 20., idx::TEXT_SUBTLE).mb(z(16.)))
        .child(field_box)
        .child(div().h(z(16.)))
        .child(checkbox_option("cs-tag-mod", crate::t!("cs.tag_moderated"), t.moderated).on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| {
                if let Some(t) = s.tag_edit.as_mut() {
                    t.moderated = !t.moderated;
                }
            });
            cx.notify();
        })))
        .into_any_element();
    let c2 = close.clone();
    let c3 = close.clone();
    let save = button("cs-tag-save", crate::t!("cs.tag_save"), BtnKind::Primary).when(!valid, |b| b.opacity(0.5)).on_click(cx.listener(move |_, _, w, cx| {
        let mut done = false;
        with_state(|s| {
            let Some(t) = s.tag_edit.as_ref() else { return };
            let name = t.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| t.name0.clone());
            if name.trim().is_empty() {
                return;
            }
            let prev = t.index.and_then(|i| s.tags.get(i)).cloned();
            let tag = tag_json(prev.as_ref(), &name, t.emoji.as_ref(), t.moderated);
            match t.index {
                Some(i) if i < s.tags.len() => s.tags[i] = tag,
                _ => s.tags.push(tag),
            }
            done = true;
        });
        if done {
            c3(w, cx);
        }
    }));
    let cancel = button("cs-tag-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| c2(w, cx));
    let footer = if editing {
        let index = t.index;
        let c4 = close.clone();
        div()
            .w_full()
            .flex()
            .justify_between()
            .child(
                super::server_settings::kit::critical_secondary("cs-tag-delete", crate::t!("cs.tag_delete"))
                    .h(z(40.))
                    .px(z(16.))
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .on_click(move |_, w, cx| {
                        with_state(|s| {
                            if let Some(i) = index.filter(|i| *i < s.tags.len()) {
                                s.tags.remove(i);
                            }
                        });
                        c4(w, cx);
                    }),
            )
            .child(div().flex().gap(z(8.)).child(cancel).child(save))
    } else {
        div().w_full().flex().gap(z(8.)).child(cancel.flex_1()).child(save.flex_1())
    };
    let title = crate::t!(if editing { "cs.tag_edit_title" } else { "cs.tag_create_title" });
    let _ = s;
    super::channel_perms::sub_frame("cs-tag", 480., title, body, footer.into_any_element(), close)
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_previews_use_the_client_svgs() {
        let list = super::layout_svg(false, "#ABACB2");
        assert!(list.contains("fill=\"#ABACB2\"") && !list.contains("\"C\""));
        let gallery = super::layout_svg(true, "#ABACB2");
        assert_eq!(gallery.matches("M0 12C0").count(), 2);
        assert!(gallery.contains("translate(142 0)"));
    }

    #[test]
    fn regions_and_slowmode() {
        assert_eq!(super::REGIONS.len(), 12);
        assert_eq!(super::SLOWMODE.last(), Some(&21_600));
    }
}
