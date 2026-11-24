use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use gpui::{Bounds, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, WindowHandle, point, px};

use crate::{
    app::PhoenixApp,
    backend::*,
    mock::MockBackend,
    model::{ChannelKind, Id, Message},
    ui::{
        home::data::{NavReply, NavRequest},
        inbox_unreads::{Entry, Kind},
    },
};

struct Probe {
    mock: MockBackend,
    acks: AtomicUsize,
    unreads: Mutex<Vec<Entry>>,
}

impl Probe {
    fn ack(&self) {
        self.acks.fetch_add(1, Ordering::SeqCst);
    }
}

impl Backend for Probe {
    fn set_event_sink(&self, sink: EventSink) {
        self.mock.set_event_sink(sink)
    }
    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData> {
        self.mock.login(creds)
    }
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.mock.select_guild(guild)
    }
    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.ack();
        self.mock.select_channel(channel)
    }
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.mock.load_messages(channel, before, limit)
    }
    fn load_messages_at(&self, channel: Id, _at: HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.mock.load_messages(channel, None, limit)
    }
    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message> {
        self.mock.send_message(channel, content, reply_to)
    }
    fn edit_message(&self, channel: Id, id: Id, content: String) -> BoxFuture<Message> {
        self.mock.edit_message(channel, id, content)
    }
    fn trigger_typing(&self, channel: Id) -> BoxFuture<()> {
        self.mock.trigger_typing(channel)
    }
    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        self.mock.join_voice(channel)
    }
    fn delete_message(&self, channel: Id, id: Id) -> BoxFuture<()> {
        self.mock.delete_message(channel, id)
    }
    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        self.ack();
        self.mock.mark_unread(channel, last_read)
    }
    fn nav(&self, req: NavRequest) -> BoxFuture<NavReply> {
        if matches!(req, NavRequest::AckChannel(_) | NavRequest::AckGuild(_)) {
            self.ack();
        }
        self.mock.nav(req)
    }
    fn parity(&self, req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        self.mock.parity(req)
    }
    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        self.mock.load_pins(channel)
    }
    fn inbox_unreads(&self, _guild_order: &[Id]) -> Vec<Entry> {
        self.unreads.lock().unwrap().clone()
    }
}

struct Harness {
    window: WindowHandle<PhoenixApp>,
    vcx: VisualTestContext,
    probe: Arc<Probe>,
    channels: Vec<Id>,
}

fn setup(cx: &mut TestAppContext) -> Harness {
    let probe = Arc::new(Probe { mock: MockBackend::synchronous(), acks: AtomicUsize::new(0), unreads: Mutex::new(Vec::new()) });
    let ready = futures::executor::block_on(probe.mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(gpui_component::init);
    let backend: Arc<dyn Backend> = probe.clone();
    let window = cx.add_window(|w, cx| PhoenixApp::new(backend, w, cx));
    let mut channels = Vec::new();
    window
        .update(cx, |app, _, cx| {
            app.apply_ready(ready, cx);
            let g = app.guilds[0].id;
            app.select_guild(Some(g), cx);
            channels = app.guild_channels[&g].iter().copied().filter(|c| app.channels[c].kind == ChannelKind::Text).collect();
            app.select_channel(channels[0], cx);
        })
        .unwrap();
    let vcx = VisualTestContext::from_window(window.into(), cx);
    vcx.run_until_parked();
    probe.acks.store(0, Ordering::SeqCst);
    Harness { window, vcx, probe, channels }
}

impl Harness {
    fn act(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Window, &mut gpui::Context<PhoenixApp>)) {
        self.window.update(&mut self.vcx, f).unwrap();
        self.vcx.run_until_parked();
    }

    fn read<T>(&mut self, f: impl FnOnce(&PhoenixApp) -> T) -> T {
        self.window.update(&mut self.vcx, |a, _, _| f(a)).unwrap()
    }

    fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.vcx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not drawn"))
    }

    fn device(&mut self, at: Point<Pixels>) -> Point<Pixels> {
        let k = self.vcx.update(|w, _| w.page_zoom());
        point(at.x * k, at.y * k)
    }

    fn click(&mut self, at: Point<Pixels>) {
        let at = self.device(at);
        self.vcx.simulate_mouse_move(at, None::<gpui::MouseButton>, Modifiers::none());
        self.vcx.run_until_parked();
        self.vcx.simulate_click(at, Modifiers::none());
        self.vcx.run_until_parked();
    }

    fn click_outside(&mut self) {
        let vp = self.vcx.update(|w, _| w.viewport_size());
        self.click(point(px(30.), vp.height - px(30.)));
    }

    fn press_inside(&mut self, selector: &'static str, is_open: fn(&PhoenixApp) -> bool) {
        let pop = self.bounds(selector);
        let at = self.device(header_point(pop));
        let vp = self.vcx.update(|w, _| w.viewport_size());
        self.vcx.simulate_mouse_move(at, None::<gpui::MouseButton>, Modifiers::none());
        self.vcx.run_until_parked();
        self.vcx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::none());
        self.vcx.run_until_parked();
        let after_down = self.read(is_open);
        self.vcx.simulate_mouse_up(at, gpui::MouseButton::Left, Modifiers::none());
        self.vcx.run_until_parked();
        let after_up = self.read(is_open);
        assert!(
            after_down && after_up,
            "press inside {selector} closed it: popout {pop:?}, press {at:?}, viewport {vp:?}, open after down {after_down}, after up {after_up}"
        );
    }

    fn escape(&mut self) {
        self.act(|a, w, cx| a.on_escape(&crate::app::Escape, w, cx));
    }

    fn acks(&self) -> usize {
        self.probe.acks.load(Ordering::SeqCst)
    }
}

fn header_point(b: Bounds<Pixels>) -> Point<Pixels> {
    b.origin + point(px(30.), px(24.))
}

#[gpui::test]
fn inbox_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
            a.parity.inbox.open = false;
            a.toggle_inbox(cx);
        });
        assert!(h.read(|a| a.parity.inbox.open));
    };

    open(&mut h);
    let saved = h.bounds("inbox-tab-saved");
    h.click(saved.center());
    let (open_after_tab, tab) = h.read(|a| (a.parity.inbox.open, a.parity.inbox.tab));
    assert!(open_after_tab && tab == crate::commands::state::InboxTab::Saved, "the Saved tab did not get its press (open {open_after_tab}, tab {tab:?})");
    h.press_inside("inbox-popout", |a| a.parity.inbox.open);
    h.click_outside();
    assert!(!h.read(|a| a.parity.inbox.open), "press outside did not close the inbox");
    assert!(!h.read(|a| a.people.status_picker), "the closing press went through");

    open(&mut h);
    h.escape();
    assert!(!h.read(|a| a.parity.inbox.open), "Esc did not close the inbox");

    open(&mut h);
    let btn = h.bounds("title-inbox");
    h.click(btn.center());
    assert!(!h.read(|a| a.parity.inbox.open), "the inbox button did not close the inbox");
    h.click(btn.center());
    assert!(h.read(|a| a.parity.inbox.open), "the inbox button did not reopen the inbox");

    h.vcx.deactivate_window();
    h.vcx.run_until_parked();
    assert!(h.read(|a| a.parity.inbox.open), "window blur closed the inbox");

    assert_eq!(h.acks(), 0, "closing the inbox sent a read marker");
}

#[gpui::test]
fn inbox_filter_menu_closes_first_on_escape(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
        a.toggle_inbox(cx);
        a.parity.inbox.filter_menu = true;
    });
    h.escape();
    assert!(h.read(|a| a.parity.inbox.open && !a.parity.inbox.filter_menu));
    h.escape();
    assert!(!h.read(|a| a.parity.inbox.open));
    assert_eq!(h.acks(), 0);
}

#[gpui::test]
fn threads_popout_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.parity.threads_open = false;
            a.toggle_threads_popout(cx);
        });
        assert!(h.read(|a| a.parity.threads_open));
    };
    open(&mut h);
    h.press_inside("threads-popout", |a| a.parity.threads_open);
    h.click_outside();
    assert!(!h.read(|a| a.parity.threads_open), "press outside did not close the threads browser");

    open(&mut h);
    h.escape();
    assert!(!h.read(|a| a.parity.threads_open), "Esc did not close the threads browser");

    open(&mut h);
    let btn = h.bounds("hdr-threads");
    h.click(btn.center());
    assert!(!h.read(|a| a.parity.threads_open), "the threads button did not close it");

    open(&mut h);
    let other = h.channels.get(1).copied().unwrap_or(h.channels[0]);
    h.act(|a, _, cx| a.select_channel(other, cx));
    assert!(!h.read(|a| a.parity.threads_open), "switching channel kept the threads browser");
}

#[gpui::test]
fn pins_popout_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.chat.pins = None;
            a.toggle_pins(cx);
        });
        assert!(h.read(|a| a.chat.pins.is_some()));
    };
    open(&mut h);
    h.press_inside("pins-popout", |a| a.chat.pins.is_some());
    h.click_outside();
    assert!(h.read(|a| a.chat.pins.is_none()), "press outside did not close the pins");

    open(&mut h);
    h.escape();
    assert!(h.read(|a| a.chat.pins.is_none()), "Esc did not close the pins");

    open(&mut h);
    let btn = h.bounds("hdr-pin");
    h.click(btn.center());
    assert!(h.read(|a| a.chat.pins.is_none()), "the pin button did not close the pins");

    if let Some(other) = h.channels.get(1).copied() {
        open(&mut h);
        h.act(|a, _, cx| a.select_channel(other, cx));
        assert!(h.read(|a| a.chat.pins.is_none()), "switching channel kept the pins");
    }
}

#[gpui::test]
fn one_header_popout_at_a_time(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.act(|a, _, cx| a.toggle_inbox(cx));
    h.act(|a, _, cx| a.toggle_threads_popout(cx));
    assert!(h.read(|a| a.parity.threads_open && !a.parity.inbox.open));
    h.click_outside();
    assert!(h.read(|a| !a.header_popout_open()));
}

#[gpui::test]
fn unreads_tab_shows_messages_without_acking(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let entries: Vec<Entry> = h
        .channels
        .iter()
        .take(2)
        .enumerate()
        .map(|(n, c)| Entry { channel: *c, guild: None, kind: Kind::Messages, oldest_read: Id(0), newest: Id(u64::MAX), mentions: 0, sort: 5 + n as u8 })
        .collect();
    *h.probe.unreads.lock().unwrap() = entries.clone();
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Unreads;
        a.parity.inbox.open = false;
        a.toggle_inbox(cx);
    });
    for _ in 0..10 {
        h.act(|_, _, cx| cx.notify());
    }
    let (order, shown): (Vec<Id>, Vec<usize>) =
        h.read(|a| a.parity.inbox.unreads.iter().filter(|c| c.loaded).map(|c| (c.entry.channel, c.msgs.len())).unzip());
    assert_eq!(order, entries.iter().map(|e| e.channel).collect::<Vec<_>>(), "cards out of the backend order");
    assert!(shown.iter().all(|n| (1..=crate::ui::inbox_unreads::SHOWN).contains(n)), "unexpected message counts {shown:?}");
    assert_eq!(h.acks(), 0, "showing Inbox > Unreads sent a read marker");

    h.act(|a, _, cx| a.mark_inbox_read(cx));
    assert_eq!(h.acks(), 1);
    assert_eq!(h.read(|a| a.parity.inbox.unreads.first().map(|c| c.entry.channel)), Some(entries[1].channel));
}
