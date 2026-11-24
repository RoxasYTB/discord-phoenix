//! Direct Messages home (Friends page, Message Requests) and the glue that
//! connects navigation state to the app: nav events, persisted prefs, thread
//! selection, backend requests.

pub mod data;
pub mod friends;
pub mod topic;

use gpui::{AnyElement, Context, IntoElement, SharedString, Window};

use crate::{
    app::PhoenixApp,
    backend::BackendError,
    model::Id,
    ui::home::data::{HomeView, NavEvent, NavPrefs, NavReply, NavRequest, Overlay},
};

/// The pane right of the sidebar: chat, or the Friends / Message Requests
/// page when Home is open without a DM.
pub fn main_pane(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let page = page_content(app, window, cx);
    // `.theme-midnight .page_pog62O` (and `.high-contrast-mode`): border-inline-start
    // 1px `--border-subtle` between the channel list and the page (box-sizing border-box).
    if crate::theme::frame_borders() {
        use gpui::{ParentElement, Styled};
        return gpui::div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .border_l_1()
            .border_color(crate::theme::c(crate::theme::tok(crate::theme::generated::idx::BORDER_SUBTLE)))
            .child(page)
            .into_any_element();
    }
    page
}

fn page_content(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if let Some(call) = crate::ui::call_view::render(app, window, cx) {
        return call;
    }
    if let Some(page) = crate::ui::community::render(app, cx) {
        return page;
    }
    if app.selected_guild.is_none() && app.nav.home == HomeView::Shop {
        return crate::ui::shop_page::render(app, window, cx);
    }
    if app.selected_guild.is_none() && app.nav.home != HomeView::Dm {
        friends::render(app, window, cx).into_any_element()
    } else {
        crate::ui::chat::render(app, window, cx).into_any_element()
    }
}

/// Modals and popouts above everything else.
pub fn overlay(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let ov = app.nav.overlay.clone()?;
    Some(match ov {
        Overlay::Topic(ch) => topic::render(app, ch, cx),
        other => crate::ui::invite::render(app, &other, window, cx),
    })
}

impl PhoenixApp {
    pub fn on_nav_event(&mut self, ev: NavEvent, _cx: &mut Context<Self>) {
        let threads_changed = matches!(ev, NavEvent::Snapshot { .. } | NavEvent::ThreadUpsert(_) | NavEvent::ThreadDelete(_));
        let extras = matches!(ev, NavEvent::Extras(_) | NavEvent::Snapshot { .. });
        self.nav.apply(ev);
        if threads_changed || extras {
            self.rebuild_sidebar();
        }
    }

    /// Restore persisted prefs once per process (last channels, collapsed
    /// categories, open folders).
    pub fn nav_after_ready(&mut self) {
        if self.nav.prefs_loaded {
            return;
        }
        self.nav.prefs_loaded = true;
        let p = NavPrefs::load();
        for (g, c) in p.last_channel {
            self.last_channel.insert(g.map(Id), Id(c));
        }
        self.collapsed.extend(p.collapsed.into_iter().map(Id));
        self.nav.expanded_folders.extend(p.folders);
        self.nav.hide_muted.extend(p.hide_muted.into_iter().map(Id));
    }

    pub fn nav_home_target(&mut self, guild: Option<Id>, has_target: bool) {
        self.nav.guild_menu = false;
        if guild.is_none() {
            self.nav.home = if has_target { HomeView::Dm } else { HomeView::Friends };
        }
    }

    pub fn nav_channel_selected(&mut self, guild: Option<Id>) {
        if guild.is_none() {
            self.nav.home = HomeView::Dm;
        }
        self.save_nav_prefs();
    }

    pub fn save_nav_prefs(&self) {
        let mut last: Vec<(Option<u64>, u64)> = self
            .last_channel
            .iter()
            .filter(|(g, _)| g.is_none_or(|g| self.guilds.iter().any(|x| x.id == g)))
            .map(|(g, c)| (g.map(|g| g.0), c.0))
            .collect();
        last.sort();
        let mut collapsed: Vec<u64> = self.collapsed.iter().map(|i| i.0).collect();
        collapsed.sort();
        let mut folders: Vec<u64> = self.nav.expanded_folders.iter().copied().collect();
        folders.sort();
        let mut hide: Vec<u64> = self.nav.hide_muted.iter().map(|i| i.0).collect();
        hide.sort();
        NavPrefs { last_channel: last, collapsed, folders, hide_muted: hide }.save();
    }

    pub fn toggle_category_saved(&mut self, id: Id, cx: &mut Context<Self>) {
        self.toggle_category(id, cx);
        self.save_nav_prefs();
    }

    /// Home -> Friends (or Message Requests).
    pub fn open_home_view(&mut self, view: HomeView, cx: &mut Context<Self>) {
        if self.selected_guild.is_some() {
            self.select_guild(None, cx);
        }
        self.nav.home = view;
        self.selected_channel = None;
        self.last_channel.remove(&None);
        self.save_nav_prefs();
        cx.notify();
    }

    /// Threads aren't part of the guild channel list: register on demand.
    pub fn select_thread(&mut self, id: Id, cx: &mut Context<Self>) {
        if let Some(t) = self.nav.find_thread(id).cloned() {
            self.channels.entry(id).or_insert(t);
            self.select_channel(id, cx);
        }
    }

    pub fn toggle_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        if !self.nav.expanded_folders.remove(&folder) {
            self.nav.expanded_folders.insert(folder);
        }
        self.save_nav_prefs();
        cx.notify();
    }

    /// Fire a navigation request; `done` runs on the UI thread.
    pub fn nav_request(
        &mut self,
        req: NavRequest,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<NavReply, BackendError>, &mut Context<Self>) + 'static,
    ) {
        let fut = self.backend.nav(req);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                done(this, res, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_dm_with(&mut self, user: Id, cx: &mut Context<Self>) {
        if let Some(existing) = self
            .dms
            .iter()
            .copied()
            .find(|d| self.channels.get(d).is_some_and(|c| c.voice_users.first() == Some(&user) && self.nav.extras.get(d).is_none_or(|e| e.recipients <= 1)))
        {
            self.select_channel(existing, cx);
            return;
        }
        self.nav_request(NavRequest::OpenDm(user), cx, |this, res, cx| {
            if let Ok(NavReply::Channel(ch)) = res {
                let id = ch.id;
                this.channels.entry(id).or_insert(ch);
                if !this.dms.contains(&id) {
                    this.dms.insert(0, id);
                }
                this.rebuild_sidebar();
                this.select_channel(id, cx);
            }
        });
    }

    pub fn close_dm(&mut self, id: Id, cx: &mut Context<Self>) {
        self.dms.retain(|d| *d != id);
        if self.selected_channel == Some(id) {
            self.open_home_view(HomeView::Friends, cx);
        }
        self.rebuild_sidebar();
        self.nav_request(NavRequest::CloseDm(id), cx, |_, _, _| {});
    }

    pub fn show_overlay(&mut self, ov: Overlay, cx: &mut Context<Self>) {
        self.nav.guild_menu = false;
        self.nav.overlay = Some(ov);
        cx.notify();
    }

    pub fn close_overlay(&mut self, cx: &mut Context<Self>) {
        self.nav.overlay = None;
        self.nav.invite_input = None;
        self.nav.invite_preview = None;
        self.nav.invite_code = None;
        self.nav.subs.clear();
        self.nav.busy = false;
        cx.notify();
    }

    pub fn stub(&mut self, what: &str, cx: &mut Context<Self>) {
        self.show_overlay(Overlay::Stub(SharedString::from(what.to_owned())), cx);
    }
}
