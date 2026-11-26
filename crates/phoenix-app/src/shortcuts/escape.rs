use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{App, Context, Window};

use crate::{
    app::PhoenixApp,
    model::Id,
    ui::{self, nav_menus::Target},
};

static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

pub fn quit_app(cx: &mut App) {
    match super::main_app() {
        Some(app) => app.update(cx, |a, cx| a.quit(cx)),
        None => cx.quit(),
    }
}

fn close_modals(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    if ui::chat::voice_message::close_modal(cx) {
        return true;
    }
    if app.chat.confirm_delete.take().is_some() {
        return true;
    }
    if crate::actions::with_modal(|m| m.take().is_some()) {
        return true;
    }
    if ui::server_pages::escape() {
        return true;
    }
    if app.parity.modal.take().is_some() || app.parity.moderation.take().is_some() || app.parity.group_dm.take().is_some() {
        return true;
    }
    if ui::events::is_open(cx) {
        ui::events::close(cx);
        return true;
    }
    if let Some(f) = app.selected_channel.and_then(|c| app.parity.forums.get_mut(&c))
        && std::mem::take(&mut f.guidelines_modal)
    {
        return true;
    }
    if app.nav.overlay.take().is_some() {
        return true;
    }
    if crate::shortcuts::on_escape() {
        return true;
    }
    if ui::boost_layer::is_open() {
        ui::boost_layer::close();
        return true;
    }
    false
}

fn close_popouts(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    if app.people.any_overlay() || app.people.tag_popout.is_some() {
        app.people.close_overlays();
        return true;
    }
    if app.chat.menu.take().is_some() || app.chat.emoji_target.take().is_some() || std::mem::take(&mut app.chat.attach_menu) {
        return true;
    }
    if ui::voice_panel::close_popout() {
        return true;
    }
    app.escape_header_popouts(cx)
}

impl PhoenixApp {
    pub fn escape_stack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::media::close_viewer(cx) {
            return;
        }
        if self.settings.is_some() {
            if !close_modals(self, cx) {
                ui::settings::on_escape(self, cx);
            }
            return;
        }
        if close_popouts(self, cx) || ui::chat::app_launcher::close(self, window, cx) || close_modals(self, cx) {
            return;
        }
        if self.switcher.is_some() {
            self.close_switcher(window, cx);
            return;
        }
        if ui::mod_view::escape() {
            return;
        }
        if ui::search::is_open(cx) {
            ui::search::close(self, window, cx);
            return;
        }
        if self.chat.edit.is_some() {
            self.cancel_edit(window, cx);
            return;
        }
        if self.editing.take().is_some() {
            self.composer.update(cx, |s, cx| s.set_value("", window, cx));
            return;
        }
        if self.replying_to.take().is_some() {
            return;
        }
        if ui::thread_create::is_open(cx) {
            self.close_create_thread(cx);
            return;
        }
        let side = self.chat.thread.as_ref().map(|t| t.thread.id);
        let main = self.escape_ack(cx);
        let thread = side.is_some_and(|t| self.ack_side(t, cx));
        if !main && !thread && side.is_some() {
            self.close_thread(cx);
            return;
        }
        if !self.leave_detached(cx) {
            self.message_list.scroll_to_end();
        }
    }

    fn channel_unread(&self, id: Id) -> bool {
        self.channels.get(&id).is_some_and(|c| c.unread || c.mentions > 0)
    }

    fn escape_ack(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ch) = self.selected_channel else { return false };
        let detached = crate::history_detached::is_detached(self);
        self.chat.new_messages = None;
        self.chat.unread_marker = None;
        if !self.channel_unread(ch) {
            return detached;
        }
        if let Some(c) = self.channels.get_mut(&ch) {
            c.unread = false;
            c.mentions = 0;
        }
        self.mark_channel_read(cx);
        true
    }

    fn ack_side(&mut self, thread: Id, cx: &mut Context<Self>) -> bool {
        if !self.channel_unread(thread) {
            return false;
        }
        self.mark_nav_read(Target::Thread(thread), cx);
        true
    }

    pub fn guild_has_unread(&self, g: Id) -> bool {
        self.guild_channels
            .get(&g)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
            .any(|c| (c.unread && !c.muted) || c.mentions > 0)
    }

    pub fn mark_server_read_shortcut(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.selected_guild else { return };
        if !self.guild_has_unread(g) {
            return;
        }
        self.mark_nav_read(Target::Guild(g), cx);
        if !self.leave_detached(cx) {
            self.message_list.scroll_to_end();
        }
        cx.notify();
    }

    pub fn quit(&mut self, cx: &mut Context<Self>) {
        if QUITTING.swap(true, Ordering::SeqCst) {
            return;
        }
        let leave = self.voice_channel.take().map(|_| {
            crate::voice::soundboard::stop_all();
            self.backend.join_voice(None)
        });
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        match leave {
            Some(fut) => {
                std::thread::spawn(move || {
                    let _ = futures::executor::block_on(fut);
                    let _ = tx.send(());
                });
            }
            None => {
                let _ = tx.send(());
            }
        }
        cx.spawn(async move |_, cx| {
            let timer = cx.background_executor().timer(std::time::Duration::from_millis(1500));
            futures::future::select(rx, Box::pin(timer)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
pub fn reset_quitting() {
    QUITTING.store(false, Ordering::SeqCst);
}
