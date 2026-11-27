use std::sync::Arc;

use gpui::{ListOffset, TestAppContext, VisualTestContext, WindowHandle, px};

use crate::{
    app::PhoenixApp,
    backend::*,
    mock::MockBackend,
    model::{ChannelKind, Id},
    ui::chat::{rows, scrollbar},
};

struct Chat {
    window: WindowHandle<PhoenixApp>,
    vcx: VisualTestContext,
    channel: Id,
}

fn setup(cx: &mut TestAppContext) -> Chat {
    let mock = MockBackend::synchronous();
    let ready = futures::executor::block_on(mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(gpui_component::init);
    let backend: Arc<dyn Backend> = Arc::new(mock);
    let window = cx.add_window(|w, cx| PhoenixApp::new(backend, w, cx));
    let channel = window
        .update(cx, |app, _, cx| {
            app.apply_ready(ready, cx);
            let g = app.guilds[0].id;
            app.select_guild(Some(g), cx);
            let first = app.guild_channels[&g].iter().copied().find(|c| app.channels[c].kind == ChannelKind::Text).unwrap();
            app.select_channel(first, cx);
            first
        })
        .unwrap();
    let vcx = VisualTestContext::from_window(window.into(), cx);
    let mut chat = Chat { window, vcx, channel };
    chat.draw();
    chat
}

impl Chat {
    fn draw(&mut self) {
        for _ in 0..3 {
            self.vcx.run_until_parked();
            self.vcx.update(|_, cx| cx.refresh_windows());
        }
        self.vcx.run_until_parked();
    }

    fn act(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Context<PhoenixApp>)) {
        self.window.update(&mut self.vcx, |a, _, cx| f(a, cx)).unwrap();
    }

    fn read<T>(&mut self, f: impl FnOnce(&PhoenixApp) -> T) -> T {
        self.window.update(&mut self.vcx, |a, _, _| f(a)).unwrap()
    }

    fn y_of(&mut self, id: Id) -> Option<f32> {
        let ch = self.channel;
        self.read(|a| {
            let ix = a.messages[&ch].index_of(id)? + 1;
            let view = a.message_list.viewport_bounds();
            a.message_list.bounds_for_item(ix).map(|b| f32::from(b.top() - view.top()))
        })
    }
}

#[gpui::test]
fn the_thumb_keeps_its_size_while_scrolling_through_loaded_messages(cx: &mut TestAppContext) {
    let mut chat = setup(cx);
    let (n0, max0, thumb0) = chat.read(|a| {
        (a.message_list.item_count(), f32::from(a.message_list.max_offset_for_scrollbar().y), scrollbar::auto_thumb(&a.message_list))
    });
    assert!(n0 > 1 && max0 > 0., "the first page fills more than the viewport");
    for step in 0..8 {
        chat.act(|a, _| a.message_list.scroll_by(px(-200.)));
        chat.draw();
        let (n, max, thumb) = chat.read(|a| {
            (a.message_list.item_count(), f32::from(a.message_list.max_offset_for_scrollbar().y), scrollbar::auto_thumb(&a.message_list))
        });
        if n != n0 {
            break;
        }
        assert!((max - max0).abs() < 0.5, "step {step}: the scroll range went from {max0} to {max} with no new message");
        assert!((thumb - thumb0).abs() < 0.5, "step {step}: the thumb went from {thumb0} to {thumb} with no new message");
    }
}

#[gpui::test]
fn an_older_page_replaces_the_placeholders_and_keeps_the_view(cx: &mut TestAppContext) {
    let mut chat = setup(cx);
    let ch = chat.channel;
    chat.act(|a, _| {
        a.messages.get_mut(&ch).unwrap().loading = true;
        a.message_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
    });
    chat.draw();
    assert!(chat.read(rows::skeleton_shown), "the placeholders are on screen at the top");
    let (first, n0) = chat.read(|a| (a.messages[&ch].msgs[0].id, a.message_list.item_count()));
    let y0 = chat.y_of(first).expect("the first loaded message is laid out under the placeholders");
    chat.act(|a, cx| {
        a.messages.get_mut(&ch).unwrap().loading = false;
        a.load_older(cx);
    });
    chat.draw();
    let n = chat.read(|a| a.message_list.item_count());
    assert!(n > n0, "the older page was not inserted");
    assert!(!chat.read(rows::skeleton_shown), "the placeholders stayed on screen after the older page arrived");
    let y = chat.y_of(first).expect("the old first message is still laid out");
    assert!((y - y0).abs() < 1., "the old first message moved from {y0} to {y}");
}

#[gpui::test]
fn placeholders_on_screen_load_the_older_page_without_a_scroll(cx: &mut TestAppContext) {
    let mut chat = setup(cx);
    let n0 = chat.read(|a| a.message_list.item_count());
    chat.act(|a, _| a.message_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) }));
    chat.draw();
    chat.draw();
    let n = chat.read(|a| a.message_list.item_count());
    assert!(n > n0, "the older page was never requested while its placeholders were on screen");
    assert!(!chat.read(rows::skeleton_shown), "the placeholders are still on screen");
}

#[gpui::test]
fn a_page_that_arrives_after_the_click_replaces_the_placeholders(cx: &mut TestAppContext) {
    let mut chat = setup(cx);
    let next = chat.read(|a| {
        let g = a.selected_guild.unwrap();
        a.guild_channels[&g].iter().copied().filter(|c| a.channels[c].kind == ChannelKind::Text).nth(1).unwrap()
    });
    chat.act(|a, cx| {
        crate::history_ui::preload(a, next, cx);
        a.select_channel(next, cx);
    });
    chat.channel = next;
    chat.draw();
    let (count, msgs) = chat.read(|a| (a.message_list.item_count(), a.messages[&next].msgs.len()));
    assert!(msgs > 0 && count == msgs + 1, "the list has {count} rows for {msgs} messages");
    assert!(!chat.read(rows::skeleton_shown), "the placeholders are still on screen once the page is in");
    assert_ne!(chat.read(|a| a.message_list.is_scrolled_to_end()), Some(false), "the channel did not open on its newest message");
}
