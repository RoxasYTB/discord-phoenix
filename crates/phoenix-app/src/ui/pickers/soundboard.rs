use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, FontWeight, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};

use super::{Pickers, empty_state, idx, section_header, tk};
use crate::{
    app::PhoenixApp,
    model::{Id, SoundboardSound},
};

#[derive(Default)]
pub struct SoundboardState {
    pub open: bool,
    pub defaults: Option<Vec<SoundboardSound>>,
    pub loading: bool,
    pub guild_sounds: HashMap<Id, Vec<SoundboardSound>>,
    pub requested_guilds: bool,
}

pub fn toggle(app: &mut PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let now_open = {
        let st = &mut cx.global_mut::<Pickers>().soundboard;
        st.open = !st.open;
        st.open
    };
    if now_open {
        ensure_loaded(app, cx);
    }
    cx.notify();
}

pub fn close(cx: &mut App) {
    cx.global_mut::<Pickers>().soundboard.open = false;
}

pub fn on_sounds(guild: Id, sounds: Vec<SoundboardSound>, cx: &mut Context<PhoenixApp>) {
    cx.global_mut::<Pickers>().soundboard.guild_sounds.insert(guild, sounds);
    cx.notify();
}

fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let (need_defaults, need_guilds) = {
        let st = &mut cx.global_mut::<Pickers>().soundboard;
        let nd = st.defaults.is_none() && !st.loading;
        if nd {
            st.loading = true;
        }
        let ng = !st.requested_guilds;
        st.requested_guilds = true;
        (nd, ng)
    };
    if need_defaults {
        let fut = app.backend.soundboard_default_sounds();
        cx.spawn(async move |this, cx| {
            let res = fut.await.unwrap_or_default();
            let _ = this.update(cx, |_, cx| {
                let st = &mut cx.global_mut::<Pickers>().soundboard;
                st.defaults = Some(res);
                st.loading = false;
                cx.notify();
            });
        })
        .detach();
    }
    if need_guilds {
        let guild_ids: Vec<Id> = app.guilds.iter().map(|g| g.id).collect();
        if !guild_ids.is_empty() {
            app.backend.request_soundboard_sounds(guild_ids);
        }
    }
}

pub fn render(app: &PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let st = &cx.global::<Pickers>().soundboard;
    if !st.open {
        return None;
    }

    let mut sections: Vec<(SharedString, Vec<SoundboardSound>)> = Vec::new();
    if let Some(gid) = app.selected_guild
        && let Some(list) = st.guild_sounds.get(&gid)
        && !list.is_empty()
    {
        let title = app.guilds.iter().find(|g| g.id == gid).map_or_else(|| SharedString::from("Server"), |g| g.name.clone());
        sections.push((title, list.clone()));
    }
    for g in &app.guilds {
        if Some(g.id) == app.selected_guild {
            continue;
        }
        if let Some(list) = st.guild_sounds.get(&g.id)
            && !list.is_empty()
        {
            sections.push((g.name.clone(), list.clone()));
        }
    }
    if let Some(defs) = &st.defaults
        && !defs.is_empty()
    {
        sections.push((SharedString::from("Discord"), defs.clone()));
    }

    let body: AnyElement = if st.loading && sections.is_empty() {
        empty_state(SharedString::from("Loading…"))
    } else if sections.is_empty() {
        empty_state(SharedString::from("No sounds available"))
    } else {
        let mut list = div().id("sb-list").flex_1().min_h_0().overflow_y_scroll().px(px(12.)).pb(px(12.)).flex().flex_col().gap(px(8.));
        for (title, sounds) in sections {
            list = list.child(section_header(None, title));
            let mut grid = div().flex().flex_wrap().gap(px(8.));
            for s in sounds {
                grid = grid.child(sound_button(&s));
            }
            list = list.child(grid);
        }
        list.into_any_element()
    };

    let panel = div()
        .id("soundboard-picker")
        .occlude()
        .w(px(484.))
        .h(px(420.))
        .rounded(px(8.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .shadow_lg()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(44.))
                .px(px(14.))
                .flex_none()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(crate::t!("settings.sub.soundboard")),
                )
                .child(
                    div()
                        .id("sb-close")
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(tk(idx::TEXT_MUTED))
                        .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                        .child("✕")
                        .on_click(cx.listener(|_, _, _, cx| {
                            close(cx);
                            cx.notify();
                        })),
                ),
        )
        .child(body);

    Some(
        div()
            .id("soundboard-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_end()
            .justify_start()
            .pl(px(80.))
            .pb(px(68.))
            .on_click(cx.listener(|_, _, _, cx| {
                close(cx);
                cx.notify();
            }))
            .child(panel.on_click(|_, _, cx| cx.stop_propagation()))
            .into_any_element(),
    )
}

fn sound_button(s: &SoundboardSound) -> gpui::Stateful<gpui::Div> {
    let label = s.name.clone().unwrap_or_else(|| SharedString::from("Sound"));
    let emoji = s.emoji_name.clone();
    div()
        .id(gpui::ElementId::Name(format!("sb-{}", s.id.0).into()))
        .w(px(148.))
        .h(px(40.))
        .rounded(px(8.))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .border_1()
        .border_color(gpui::transparent_black())
        .hover(|st| st.bg(tk(idx::BACKGROUND_MOD_STRONG)).border_color(tk(idx::BORDER_SUBTLE)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .px(px(8.))
        .gap(px(8.))
        .overflow_hidden()
        .when_some(emoji, |d, e| d.child(div().flex_none().text_size(px(16.)).child(e)))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_STRONG))
                .child(label),
        )
}
