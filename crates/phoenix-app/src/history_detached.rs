use gpui::{Context, ListScrollEvent};

use crate::{
    app::PhoenixApp,
    backend::HistoryAt,
    model::{Id, Message},
};

const PAGE: u32 = 50;

pub fn is_detached(app: &PhoenixApp) -> bool {
    app.selected_channel.and_then(|c| app.messages.get(&c)).is_some_and(|e| e.detached)
}

impl PhoenixApp {
    pub fn jump_around(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
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
                a.set_detached_page(channel, page);
                if let Some(ix) = found {
                    a.message_list.scroll_to_reveal_item(ix + 1);
                    a.flash_message(id, cx);
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
        if !self.messages.get(&ch).is_some_and(|e| e.detached) {
            return false;
        }
        self.messages.remove(&ch);
        let e = self.messages.entry(ch).or_default();
        e.loading = true;
        self.message_list.reset(1);
        crate::history_ui::load_first_page(self, ch, cx);
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
            let page = fut.await.unwrap_or_default();
            let _ = this.update(cx, |a, cx| a.append_newer(ch, page, cx));
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
            self.message_list.splice(start + 1..start + 1, added);
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
