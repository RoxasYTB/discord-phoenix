use std::{collections::HashMap, sync::Arc};

use gpui::{TestAppContext, VisualTestContext, WindowHandle};

use super::Section;
use crate::{
    app::PhoenixApp,
    backend::*,
    mock::MockBackend,
    model::{ChannelKind, Id},
    ui::switcher::{self, Row, Target},
};

struct H {
    window: WindowHandle<PhoenixApp>,
    vcx: VisualTestContext,
}

fn setup(cx: &mut TestAppContext) -> H {
    let mock = MockBackend::synchronous();
    let ready = futures::executor::block_on(mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(|cx| {
        gpui_component::init(cx);
        super::bind_keys(cx);
        crate::app::bind_keys(cx);
    });
    let backend: Arc<dyn Backend> = Arc::new(mock);
    let window = cx.add_window(|w, cx| PhoenixApp::new(backend, w, cx));
    window
        .update(cx, |app, _, cx| {
            app.apply_ready(ready, cx);
            let g = app.guilds[0].id;
            app.select_guild(Some(g), cx);
            let first = app.guild_channels[&g].iter().copied().find(|c| app.channels[c].kind == ChannelKind::Text).unwrap();
            app.select_channel(first, cx);
        })
        .unwrap();
    let vcx = VisualTestContext::from_window(window.into(), cx);
    vcx.run_until_parked();
    let mut h = H { window, vcx };
    h.root();
    h.act(|_, _, cx| cx.notify());
    h.act(|_, _, cx| cx.notify());
    h
}

impl H {
    fn act(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Window, &mut gpui::Context<PhoenixApp>)) {
        self.window.update(&mut self.vcx, f).unwrap();
        self.vcx.run_until_parked();
    }

    fn read<T>(&mut self, f: impl FnOnce(&PhoenixApp) -> T) -> T {
        self.window.update(&mut self.vcx, |a, _, _| f(a)).unwrap()
    }

    fn keys(&mut self, k: &str) {
        self.vcx.simulate_keystrokes(k);
        self.vcx.run_until_parked();
    }

    fn root(&mut self) {
        self.act(|a, w, cx| {
            let f = a.focus.clone();
            w.focus(&f, cx);
        });
    }

    fn channel(&mut self) -> Option<Id> {
        self.read(|a| a.selected_channel)
    }

    fn section(&mut self) -> Option<Section> {
        self.window.update(&mut self.vcx, |a, w, _| a.current_section(w)).unwrap()
    }

    fn clear_unreads(&mut self) {
        self.act(|a, _, _| {
            for c in a.channels.values_mut() {
                c.unread = false;
                c.mentions = 0;
                c.muted = false;
            }
        });
    }
}

#[gpui::test]
fn alt_arrows_step_through_the_channel_list(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let list = h.read(|a| a.shortcut_channels());
    let n = list.len();
    assert!(n >= 3, "{list:?}");
    let start = h.channel().unwrap();
    let i = list.iter().position(|c| *c == start).unwrap();
    h.keys("alt-down");
    assert_eq!(h.channel(), Some(list[(i + 1) % n]));
    h.keys("alt-up");
    assert_eq!(h.channel(), Some(start));
    h.keys("alt-up");
    assert_eq!(h.channel(), Some(list[(i + n - 1) % n]), "Alt+Up wraps to the last channel");
}

#[gpui::test]
fn ctrl_alt_arrows_step_through_the_rail(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let rail = h.read(|a| a.shortcut_rail());
    let n = rail.len();
    let cur = h.read(|a| a.selected_guild);
    let i = rail.iter().position(|g| *g == cur).unwrap();
    h.keys("ctrl-alt-down");
    assert_eq!(h.read(|a| a.selected_guild), rail[(i + 1) % n]);
    h.keys("ctrl-alt-up");
    assert_eq!(h.read(|a| a.selected_guild), cur);
    h.keys("ctrl-alt-up");
    assert_eq!(h.read(|a| a.selected_guild), rail[(i + n - 1) % n]);
}

#[gpui::test]
fn unread_and_mention_jumps_skip_the_rest(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.clear_unreads();
    let text_in = |a: &PhoenixApp, gi: usize, nth: usize| {
        let g = a.guilds[gi].id;
        a.guild_channels[&g].iter().copied().filter(|c| a.channels[c].kind == ChannelKind::Text).nth(nth).unwrap()
    };
    let (unread, mention) = h.read(|a| (text_in(a, 1, 0), text_in(a, 2, 1)));
    h.act(|a, _, _| {
        a.channels.get_mut(&unread).unwrap().unread = true;
        a.channels.get_mut(&mention).unwrap().mentions = 2;
    });
    let start = h.channel().unwrap();
    h.keys("ctrl-shift-alt-down");
    assert_eq!(h.channel(), Some(mention), "Ctrl+Shift+Alt+Down skips channels without mentions");
    h.act(|a, _, cx| a.select_channel(start, cx));
    h.root();
    h.act(|a, _, _| a.channels.get_mut(&unread).unwrap().unread = true);
    h.keys("alt-shift-down");
    assert_eq!(h.channel(), Some(unread), "Alt+Shift+Down lands on the next unread channel");
    h.keys("alt-shift-up");
    assert_eq!(h.channel(), Some(unread), "nothing else is unread, so Alt+Shift+Up stays put");
}

#[gpui::test]
fn escape_marks_the_channel_read_and_shift_escape_the_server(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let ch = h.channel().unwrap();
    h.act(|a, _, _| a.chat.unread_marker = Some((ch, Id(1))));
    h.keys("escape");
    assert_eq!(h.read(|a| a.chat.unread_marker), None);
    let g = h.read(|a| a.selected_guild).unwrap();
    h.act(|a, _, _| {
        for id in a.guild_channels[&g].clone() {
            a.channels.get_mut(&id).unwrap().unread = true;
        }
        a.guilds.iter_mut().find(|x| x.id == g).unwrap().unread = true;
    });
    h.keys("shift-escape");
    assert!(h.read(|a| a.guild_channels[&g].iter().all(|c| !a.channels[c].unread)));
    assert!(h.read(|a| !a.guilds.iter().find(|x| x.id == g).unwrap().unread));
}

#[gpui::test]
fn ctrl_slash_toggles_the_shortcuts_modal(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    assert!(!super::help_open());
    h.keys("ctrl-/");
    assert!(super::help_open());
    h.keys("escape");
    assert!(!super::help_open());
}

#[gpui::test]
fn page_keys_scroll_the_chat(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let before = h.read(|a| a.message_list.logical_scroll_top());
    let height = h.read(|a| a.message_list.viewport_bounds().size.height);
    h.keys("pageup");
    let up = h.read(|a| a.message_list.logical_scroll_top());
    assert!(height == gpui::px(0.) || up != before, "Page Up did not scroll");
    h.keys("pagedown");
    let down = h.read(|a| a.message_list.logical_scroll_top());
    assert!(height == gpui::px(0.) || down != up, "Page Down did not scroll");
}

#[gpui::test]
fn f6_cycles_the_sections_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let secs = h.read(|a| a.sections());
    assert_eq!(&secs[..5], &[Section::Back, Section::Rail, Section::Channels, Section::Account, Section::Messages]);
    let mut seen = Vec::new();
    for _ in 0..secs.len() {
        h.keys("f6");
        seen.push(h.section());
    }
    assert_eq!(seen, secs.iter().copied().map(Some).collect::<Vec<_>>());
    h.keys("f6");
    assert_eq!(h.section(), Some(Section::Back), "F6 wraps to the first section");
    h.keys("shift-f6");
    assert_eq!(h.section(), secs.last().copied(), "Shift+F6 wraps backwards");
    h.keys("shift-f6");
    assert_eq!(h.section(), Some(secs[secs.len() - 2]));
}

#[gpui::test]
fn ctrl_k_opens_the_switcher_and_arrows_wrap(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.keys("ctrl-k");
    assert!(h.read(|a| a.switcher.is_some()), "Ctrl+K did not open the Quick Switcher");
    h.act(|a, w, cx| {
        let input = a.switcher.as_ref().unwrap().input.clone();
        input.update(cx, |s, cx| s.set_value("#", w, cx));
        switcher::update(a, cx);
    });
    let (rows, sel) = h.read(|a| {
        let sw = a.switcher.as_ref().unwrap();
        (sw.results.clone(), sw.selected)
    });
    assert!(matches!(rows[0], Row::Header(_)), "the # mode starts with its header");
    assert!(rows.len() > 3);
    assert_eq!(sel, 1);
    h.keys("down");
    assert_eq!(h.read(|a| a.switcher.as_ref().unwrap().selected), 2);
    h.keys("tab");
    assert_eq!(h.read(|a| a.switcher.as_ref().unwrap().selected), 2, "Tab does not move the selection");
    h.keys("up up");
    assert_eq!(h.read(|a| a.switcher.as_ref().unwrap().selected), rows.len() - 1, "Up wraps past the header");
    h.keys("down down");
    let want = rows[2].target();
    h.keys("enter");
    assert!(h.read(|a| a.switcher.is_none()));
    if let Some(Target::Channel(id)) = want {
        assert_eq!(h.channel(), Some(id));
    }
    h.keys("ctrl-k");
    h.keys("escape");
    assert!(h.read(|a| a.switcher.is_none()), "Esc closes the Quick Switcher");
}

#[gpui::test]
fn frecency_lifts_a_channel_in_the_switcher(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let (plain, boosted) = h.read(|a| {
        let plain = switcher::compute(a, HashMap::new(), "#");
        let last = plain.iter().rev().find_map(Row::target).unwrap();
        let Target::Channel(id) = last else { panic!("{last:?}") };
        let boosted = switcher::compute(a, HashMap::from([(id.0, 500)]), "#");
        (plain, (boosted, id))
    });
    assert!(plain.len() > 2);
    assert_eq!(boosted.0[1], Row::Hit(Target::Channel(boosted.1)));
}

#[gpui::test]
fn empty_query_lists_previous_channels_first(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let g = h.read(|a| a.selected_guild).unwrap();
    let texts: Vec<Id> = h.read(|a| a.guild_channels[&g].iter().copied().filter(|c| a.channels[c].kind == ChannelKind::Text).take(3).collect());
    for id in &texts {
        h.act(|a, _, cx| a.select_channel(*id, cx));
    }
    h.clear_unreads();
    let rows = h.read(|a| switcher::compute(a, HashMap::new(), ""));
    assert!(matches!(rows[0], Row::Header(_)));
    assert_eq!(rows[1], Row::Hit(Target::Channel(texts[1])), "the current channel is left out, the newest visit comes first");
    assert_eq!(rows[2], Row::Hit(Target::Channel(texts[0])));
}
