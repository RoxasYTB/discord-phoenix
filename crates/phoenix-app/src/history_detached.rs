use gpui::{Context, ListOffset, ListScrollEvent, px};

use crate::{
    app::PhoenixApp,
    backend::HistoryAt,
    model::{Id, Message},
    ui::home::data::ScrollPosition,
};

const PAGE: u32 = 50;

pub fn is_detached(app: &PhoenixApp) -> bool {
    app.selected_channel.and_then(|c| app.messages.get(&c)).is_some_and(|e| e.detached)
}

impl PhoenixApp {
    pub fn persist_chat_position(&mut self) {
        if self.selected_channel.is_some() {
            self.remember_chat_position();
            self.save_nav_prefs();
        }
    }

    pub fn remember_chat_position(&mut self) {
        let Some(channel) = self.selected_channel else { return };
        if self.channels.get(&channel).and_then(|ch| ch.guild).is_some_and(crate::ui::join_flow::is_lurking) {
            return;
        }
        let Some(entry) = self.messages.get(&channel).filter(|e| e.loaded_once && !e.msgs.is_empty()) else { return };
        if self.message_list.viewport_bounds().size.height <= px(0.) {
            return;
        }
        let top = self.message_list.logical_scroll_top();
        if !entry.detached && (top.item_ix >= self.message_list.item_count() || self.message_list.is_scrolled_to_end() == Some(true)) {
            self.nav.scroll_positions.remove(&channel);
            return;
        }
        let Some(message) = entry.msgs.get(top.item_ix.saturating_sub(1)) else { return };
        let offset = if top.item_ix == 0 { 0 } else { f32::from(top.offset_in_item).round().max(0.) as u32 };
        self.nav.scroll_positions.insert(channel, ScrollPosition { message: message.id, offset });
    }

    pub fn restore_chat_position(&mut self, channel: Id, cx: &mut Context<Self>) {
        if self.selected_channel != Some(channel) {
            return;
        }
        let Some(position) = self.nav.scroll_positions.get(&channel).copied() else { return };
        let Some(entry) = self.messages.get(&channel) else { return };
        let row = entry.index_of(position.message).map(|i| i + 1);
        if let Some(item_ix) = row {
            self.message_list.scroll_to(ListOffset { item_ix, offset_in_item: px(position.offset as f32) });
            self.chat.at_bottom = false;
        } else if !entry.reached_start {
            self.request_around(channel, position.message, Some(position.offset), cx);
        }
    }

    pub fn jump_around(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.request_around(channel, id, None, cx);
    }

    fn request_around(&mut self, channel: Id, id: Id, offset: Option<u32>, cx: &mut Context<Self>) {
        let Some(e) = self.messages.get_mut(&channel) else { return };
        if e.loading {
            return;
        }
        e.loading = true;
        let fut = self.backend.load_messages_at(channel, HistoryAt::Around(id), PAGE);
        cx.spawn(async move |this, cx| {
            let page = fut.await.unwrap_or_default();
            let _ = this.update(cx, |a, cx| {
                if let Some(e) = a.messages.get_mut(&channel) {
                    e.loading = false;
                }
                if page.is_empty() || a.selected_channel != Some(channel) {
                    return;
                }
                let found = page.iter().position(|m| m.id == id);
                if offset.is_some() && (found.is_none() || !a.chat.at_bottom) {
                    return;
                }
                a.set_detached_page(channel, page);
                if let Some(ix) = found {
                    if let Some(offset) = offset {
                        a.message_list.scroll_to(ListOffset { item_ix: ix + 1, offset_in_item: px(offset as f32) });
                    } else {
                        a.message_list.scroll_to_reveal_item(ix + 1);
                        a.flash_message(id, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_detached_page(&mut self, channel: Id, page: Vec<Message>) {
        let rendered = (0..page.len()).map(|i| self.render_msg(&page[i], i.checked_sub(1).map(|p| &page[p]))).collect();
        let mut e = self.messages.remove(&channel).unwrap_or_default();
        e.loaded_once = true;
        e.reached_start = false;
        e.detached = true;
        e.msgs = page;
        e.rendered = rendered;
        let len = e.msgs.len();
        self.messages.insert(channel, e);
        self.chat.at_bottom = false;
        self.chat.new_messages = None;
        self.message_list.reset(len + 1);
    }

    pub fn leave_detached(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ch) = self.selected_channel else { return false };
        let Some(entry) = self.messages.get_mut(&ch).filter(|entry| entry.detached) else { return false };
        if entry.loading {
            return true;
        }
        entry.loading = true;
        let fetch = self.backend.load_messages(ch, None, PAGE);
        cx.spawn(async move |this, cx| {
            let result = fetch.await;
            let _ = this.update(cx, |app, cx| {
                let Some(entry) = app.messages.get_mut(&ch).filter(|entry| entry.detached) else { return };
                entry.loading = false;
                if app.selected_channel != Some(ch) {
                    return;
                }
                match result {
                    Ok(page) => {
                        app.messages.remove(&ch);
                        app.insert_history(ch, page, false, cx);
                        app.message_list.scroll_to_end();
                        app.chat.at_bottom = true;
                        app.mark_channel_read(cx);
                    }
                    Err(_) => app.parity_toast(crate::t!("chat.history_failed"), cx),
                }
            });
        }).detach();
        cx.notify();
        true
    }

    pub fn detached_on_scroll(&mut self, ev: &ListScrollEvent, cx: &mut Context<Self>) {
        let Some(ch) = self.selected_channel else { return };
        let Some(e) = self.messages.get_mut(&ch) else { return };
        if !e.detached || e.loading || ev.visible_range.end < ev.count {
            return;
        }
        let Some(last) = e.msgs.last().map(|m| m.id) else { return };
        e.loading = true;
        let fut = self.backend.load_messages_at(ch, HistoryAt::After(last), PAGE);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(page) => app.append_newer(ch, page, cx),
                    Err(_) => {
                        if let Some(entry) = app.messages.get_mut(&ch) {
                            entry.loading = false;
                        }
                        if app.selected_channel == Some(ch) {
                            app.parity_toast(crate::t!("chat.history_failed"), cx);
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn append_newer(&mut self, ch: Id, page: Vec<Message>, cx: &mut Context<Self>) {
        let Some(mut e) = self.messages.remove(&ch) else { return };
        e.loading = false;
        let caught_up = (page.len() as u32) < PAGE;
        let start = e.msgs.len();
        for m in page {
            if e.msgs.last().is_some_and(|l| l.id.0 >= m.id.0) {
                continue;
            }
            let r = self.render_msg(&m, e.msgs.last());
            e.msgs.push(m);
            e.rendered.push(r);
        }
        if caught_up {
            e.detached = false;
        }
        let added = e.msgs.len() - start;
        self.messages.insert(ch, e);
        if self.selected_channel == Some(ch) && added > 0 {
            crate::ui::chat::rows::insert(&self.message_list, start + 1, added);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::ChannelMessages;

    #[test]
    fn detached_defaults_off() {
        assert!(!ChannelMessages::default().detached);
    }
}
