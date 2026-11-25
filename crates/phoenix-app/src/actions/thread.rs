use std::rc::Rc;

use gpui::Context;
use phoenix_api::rest::guild_manage::{AdminCall, ThreadEdit, ThreadMemberPatch};

use super::{
    admin::sf,
    open_modal,
};
use crate::{
    app::PhoenixApp,
    model::{Id, ThreadInfo, ThreadRef},
    ui::modals::{Form, Modal},
};

pub const PINNED: u64 = 1 << 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    ContextMenu,
    Toolbar,
    Sidebar,
    ForumToolbar,
}

impl Place {
    pub fn location(self) -> &'static str {
        match self {
            Place::ContextMenu => "Context Menu",
            Place::Toolbar => "Toolbar Overflow",
            Place::Sidebar => "Sidebar Overflow",
            Place::ForumToolbar => "Forum Toolbar",
        }
    }
}

pub fn archive_edit(lock: bool) -> ThreadEdit {
    ThreadEdit { archived: Some(true), locked: lock.then_some(true), ..Default::default() }
}

pub fn unarchive_edit(unlock: bool) -> ThreadEdit {
    ThreadEdit { archived: Some(false), locked: unlock.then_some(false), ..Default::default() }
}

pub fn lock_steps(archived: bool, lock: bool) -> Vec<ThreadEdit> {
    let mut v = Vec::new();
    if archived {
        v.push(unarchive_edit(!lock));
    }
    v.push(ThreadEdit { locked: Some(lock), archived: Some(archived), ..Default::default() });
    v
}

pub fn pin_edit(flags: u64, archived: bool, pin: bool) -> ThreadEdit {
    let flags = if pin { flags | PINNED } else { flags & !PINNED };
    ThreadEdit { flags: Some(flags), archived: (pin && archived).then_some(false), ..Default::default() }
}

pub fn mute_patch(window: i64, now: chrono::DateTime<chrono::Utc>) -> ThreadMemberPatch {
    let end_time = (window > 0).then(|| (now + chrono::Duration::seconds(window)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    ThreadMemberPatch::MuteFor { window, end_time }
}

pub fn can_manage(app: &PhoenixApp, thread: Id) -> bool {
    let guild = app.channels.get(&thread).and_then(|c| c.guild).or_else(|| app.nav.find_thread(thread).and_then(|c| c.guild)).or(app.selected_guild);
    guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 34))
}

impl PhoenixApp {
    pub fn thread_state(&self, thread: Id) -> ThreadInfo {
        self.backend.thread_info(thread).unwrap_or_default()
    }

    fn thread_call(&mut self, call: AdminCall, cx: &mut Context<Self>) {
        self.admin(call, cx, |a, r, cx| {
            if let Err(e) = r {
                a.parity_toast(e, cx);
            }
            a.rebuild_sidebar();
            cx.notify();
        });
    }

    fn thread_edits(&mut self, thread: Id, mut steps: Vec<ThreadEdit>, cx: &mut Context<Self>) {
        if steps.is_empty() {
            return;
        }
        let first = steps.remove(0);
        self.admin(AdminCall::EditThread { thread: sf(thread), edit: first }, cx, move |a, r, cx| match r {
            Ok(_) => a.thread_edits(thread, steps, cx),
            Err(e) => a.parity_toast(e, cx),
        });
    }

    pub fn set_thread_archived(&mut self, thread: Id, archived: bool, cx: &mut Context<Self>) {
        let edit = if archived { archive_edit(false) } else { unarchive_edit(false) };
        self.thread_edits(thread, vec![edit], cx);
    }

    pub fn set_thread_locked(&mut self, thread: Id, lock: bool, cx: &mut Context<Self>) {
        let archived = self.thread_state(thread).archived;
        self.thread_edits(thread, lock_steps(archived, lock), cx);
    }

    pub fn set_thread_pinned(&mut self, thread: Id, pin: bool, cx: &mut Context<Self>) {
        let st = self.thread_state(thread);
        self.thread_edits(thread, vec![pin_edit(st.flags, st.archived, pin)], cx);
    }

    pub fn set_thread_tags(&mut self, thread: Id, tags: Vec<Id>, cx: &mut Context<Self>) {
        let edit = ThreadEdit { applied_tags: Some(tags.into_iter().map(sf).collect()), ..Default::default() };
        self.thread_edits(thread, vec![edit], cx);
    }

    pub fn join_thread(&mut self, thread: Id, place: Place, cx: &mut Context<Self>) {
        self.thread_call(AdminCall::JoinThread { thread: sf(thread), location: place.location().into() }, cx);
    }

    pub fn leave_thread(&mut self, thread: Id, place: Place, cx: &mut Context<Self>) {
        self.thread_call(AdminCall::LeaveThread { thread: sf(thread), location: place.location().into() }, cx);
    }

    pub fn add_me_to_thread(&mut self, thread: Id, cx: &mut Context<Self>) {
        let me = self.me.id;
        self.thread_call(AdminCall::AddThreadMember { thread: sf(thread), user: sf(me), location: "Members".into() }, cx);
    }

    pub fn set_thread_notify(&mut self, thread: Id, level: u8, cx: &mut Context<Self>) {
        self.thread_member_settings(thread, ThreadMemberPatch::Flags(level), cx);
    }

    pub fn mute_thread_for(&mut self, thread: Id, window: i64, cx: &mut Context<Self>) {
        self.thread_member_settings(thread, mute_patch(window, chrono::Utc::now()), cx);
    }

    fn thread_member_settings(&mut self, thread: Id, patch: ThreadMemberPatch, cx: &mut Context<Self>) {
        let patch = AdminCall::ThreadMemberSettings { thread: sf(thread), patch };
        if self.thread_state(thread).joined {
            self.thread_call(patch, cx);
            return;
        }
        let join = AdminCall::JoinThread { thread: sf(thread), location: "Change Notification Settings".into() };
        self.admin(join, cx, move |a, r, cx| match r {
            Ok(_) => a.thread_call(patch, cx),
            Err(e) => a.parity_toast(e, cx),
        });
    }

    pub fn unmute_thread(&mut self, thread: Id, cx: &mut Context<Self>) {
        self.thread_call(AdminCall::ThreadMemberSettings { thread: sf(thread), patch: ThreadMemberPatch::Muted(false) }, cx);
    }

    pub fn confirm_delete_thread(&mut self, thread: Id, post: bool, cx: &mut Context<Self>) {
        let name = self.channels.get(&thread).or_else(|| self.nav.find_thread(thread)).map(|c| c.name.to_string()).unwrap_or_default();
        let key = if post { "menu.post.delete" } else { "thread.delete" };
        open_modal(
            Modal::Form(Form {
                title: crate::t!(key),
                subtitle: Some(crate::t!("thread.delete_body", name = name)),
                fields: Vec::new(),
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!(key),
                danger: true,
                on_submit: Rc::new(move |a, _, _, cx| a.delete_thread(thread, cx)),
                preview: None,
                width: 440.,
            }),
            None,
        );
        cx.notify();
    }

    pub fn delete_thread(&mut self, thread: Id, cx: &mut Context<Self>) {
        let parent = self.thread_state(thread).parent.or_else(|| self.nav.thread_parent(thread));
        self.admin(AdminCall::DeleteChannel { channel: sf(thread) }, cx, move |a, r, cx| match r {
            Ok(_) => {
                if crate::actions::with_modal(|m| matches!(m, Some(Modal::ChannelSettings(s)) if s.channel == thread)) {
                    crate::actions::close_modal();
                }
                a.nav.apply(crate::ui::home::data::NavEvent::ThreadDelete(thread));
                a.channels.remove(&thread);
                if a.chat.thread.as_ref().is_some_and(|p| p.thread.id == thread) {
                    a.close_thread(cx);
                }
                if a.selected_channel == Some(thread)
                    && let Some(p) = parent
                {
                    a.select_channel(p, cx);
                }
                a.rebuild_sidebar();
                cx.notify();
            }
            Err(e) => a.parity_toast(e, cx),
        });
    }

    pub fn open_thread_settings(&mut self, thread: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.channels.get(&thread).or_else(|| self.nav.find_thread(thread)).cloned() else { return };
        let info = self.thread_state(thread);
        open_modal(Modal::ChannelSettings(crate::ui::modals::channel_settings::State::for_thread(&ch, &info)), None);
        cx.notify();
    }

    pub fn jump_thread_top(&mut self, thread: Id, cx: &mut Context<Self>) {
        let start = self.messages.get(&thread).is_some_and(|e| e.reached_start);
        if start && self.selected_channel == Some(thread) {
            self.message_list.scroll_to_reveal_item(0);
            cx.notify();
        } else {
            self.jump_to_message(thread, thread, cx);
        }
    }

    pub fn open_thread_split(&mut self, thread: Id, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let parent = self.thread_state(thread).parent.or_else(|| self.nav.thread_parent(thread)).or_else(|| self.channels.get(&thread).and_then(|c| c.parent));
        let Some(parent) = parent else { return };
        let name = self.channels.get(&thread).or_else(|| self.nav.find_thread(thread)).map(|c| c.name.clone()).unwrap_or_default();
        self.select_channel(parent, cx);
        self.open_thread(ThreadRef { id: thread, name, message_count: 0 }, window, cx);
    }

    pub fn open_thread_link(&mut self, thread: ThreadRef, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.prefs.synced.flag("split-threads") {
            self.open_thread(thread, window, cx);
        } else {
            self.open_thread_full(thread.id, cx);
        }
    }

    pub fn open_thread_full(&mut self, thread: Id, cx: &mut Context<Self>) {
        self.close_thread(cx);
        if self.channels.contains_key(&thread) {
            self.select_channel(thread, cx);
        } else if self.nav.find_thread(thread).is_some() {
            self.select_thread(thread, cx);
        } else {
            crate::ui::forum::open_by_id(self, thread, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::rest::guild_manage::{build, thread_body};
    use serde_json::json;

    #[test]
    fn archive_bodies_follow_the_client() {
        assert_eq!(thread_body(&archive_edit(false)), json!({"archived": true}));
        assert_eq!(thread_body(&archive_edit(true)), json!({"archived": true, "locked": true}));
        assert_eq!(thread_body(&unarchive_edit(false)), json!({"archived": false}));
        assert_eq!(thread_body(&unarchive_edit(true)), json!({"archived": false, "locked": false}));
    }

    #[test]
    fn lock_unarchives_first() {
        let open: Vec<_> = lock_steps(false, false).iter().map(thread_body).collect();
        assert_eq!(open, vec![json!({"locked": false, "archived": false})]);
        let locked: Vec<_> = lock_steps(false, true).iter().map(thread_body).collect();
        assert_eq!(locked, vec![json!({"locked": true, "archived": false})]);
        let closed: Vec<_> = lock_steps(true, true).iter().map(thread_body).collect();
        assert_eq!(closed, vec![json!({"archived": false}), json!({"locked": true, "archived": true})]);
        let closed: Vec<_> = lock_steps(true, false).iter().map(thread_body).collect();
        assert_eq!(closed, vec![json!({"archived": false, "locked": false}), json!({"locked": false, "archived": true})]);
    }

    #[test]
    fn forum_post_writes() {
        assert_eq!(thread_body(&pin_edit(0, false, true)), json!({"flags": 2}));
        assert_eq!(thread_body(&pin_edit(2 | 16, true, true)), json!({"flags": 18, "archived": false}));
        assert_eq!(thread_body(&pin_edit(18, true, false)), json!({"flags": 16}));
        let r = build(&AdminCall::JoinThread { thread: sf(Id(5)), location: Place::ForumToolbar.location().into() });
        assert_eq!(r.path, "/channels/5/thread-members/@me?location=Forum%20Toolbar");
        let r = build(&AdminCall::EditThread { thread: sf(Id(5)), edit: ThreadEdit { applied_tags: Some(vec![sf(Id(8)), sf(Id(9))]), ..Default::default() } });
        assert_eq!(r.body.unwrap(), json!({"applied_tags": ["8", "9"]}));
    }

    #[test]
    fn timed_mute_matches_the_client_body() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-25T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let r = build(&AdminCall::ThreadMemberSettings { thread: sf(Id(5)), patch: mute_patch(900, now) });
        assert_eq!(r.body.unwrap(), json!({"muted": true, "mute_config": {"selected_time_window": 900, "end_time": "2026-11-25T12:15:00.000Z"}}));
        let r = build(&AdminCall::ThreadMemberSettings { thread: sf(Id(5)), patch: mute_patch(-1, now) });
        assert_eq!(r.body.unwrap(), json!({"muted": true, "mute_config": {"selected_time_window": -1, "end_time": null}}));
    }

    #[test]
    fn locations() {
        assert_eq!(Place::ContextMenu.location(), "Context Menu");
        assert_eq!(Place::Toolbar.location(), "Toolbar Overflow");
        assert_eq!(Place::Sidebar.location(), "Sidebar Overflow");
    }

    #[test]
    fn notify_level_reads_the_member_flags() {
        let mut i = ThreadInfo { member_flags: 1 | 2, ..Default::default() };
        assert_eq!(i.notify_level(), 2);
        i.member_flags = 1 | 4;
        assert_eq!(i.notify_level(), 4);
        i.member_flags = 8;
        assert_eq!(i.notify_level(), 8);
        i.member_flags = 0;
        assert_eq!(i.notify_level(), 2);
    }
}
