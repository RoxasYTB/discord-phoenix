use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::{Channel, Id},
    theme::z,
    ui::{
        controls::{BtnKind, button},
        modals::{Modal, card, close_action},
    },
};

pub fn active_threads(app: &PhoenixApp, guild: Id) -> Vec<Channel> {
    let mut out: Vec<Channel> = app
        .guild_channels
        .get(&guild)
        .into_iter()
        .flatten()
        .filter_map(|ch| app.nav.threads.get(ch))
        .flatten()
        .filter(|t| app.backend.thread_info(t.id).is_none_or(|i| !i.archived))
        .cloned()
        .collect();
    out.sort_by_key(|t| std::cmp::Reverse(crate::thread_recent::get(t.id).map_or(((t.id.0 >> 22) / 1000 + 1_420_070_400) as i64, |r| r.ts)));
    out
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut list = div().id("active-threads").max_h(z(560.)).overflow_y_scroll().flex().flex_col().pb(z(8.));
    for t in active_threads(app, guild) {
        let tid = t.id;
        let parent = t.parent.and_then(|p| app.channels.get(&p)).map(|p| SharedString::from(format!("#{}", p.name)));
        list = list.child(crate::ui::inbox::thread_browser_row(app, &t, parent).on_click(cx.listener(move |a, _, _, cx| {
            crate::actions::close_modal();
            a.open_thread_full(tid, cx);
        })));
    }
    let close = close_action(cx);
    let footer = div()
        .w_full()
        .flex()
        .child(button("active-threads-close", crate::t!("common.close"), BtnKind::Primary).flex_1().on_click(move |_, w, cx| close(w, cx)));
    card(480., crate::t!("threads.all_active"), None, list.into_any_element(), Some(footer.into_any_element()), cx)
}

impl PhoenixApp {
    pub fn open_active_threads(&mut self, guild: Id, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::ActiveThreads(guild), None);
        cx.notify();
    }
}
