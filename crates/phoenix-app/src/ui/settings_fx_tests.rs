use std::sync::Arc;

use gpui::{TestAppContext, VisualTestContext, WindowHandle};

use crate::{
    app::PhoenixApp,
    backend::{Backend, Credentials},
    mock::MockBackend,
    model::{ChannelKind, Id, Message, ThreadRef},
};

struct Harness {
    window: WindowHandle<PhoenixApp>,
    vcx: VisualTestContext,
    channels: Vec<Id>,
}

fn setup(cx: &mut TestAppContext) -> Harness {
    let mock = MockBackend::synchronous();
    let ready = futures::executor::block_on(mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(gpui_component::init);
    let backend: Arc<dyn Backend> = Arc::new(mock);
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
    Harness { window, vcx, channels }
}

impl Harness {
    fn act<T>(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Window, &mut gpui::Context<PhoenixApp>) -> T) -> T {
        let out = self.window.update(&mut self.vcx, f).unwrap();
        self.vcx.run_until_parked();
        out
    }
}

#[gpui::test]
fn thread_links_follow_split_view(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let t = h.channels[1];
    let link = move || ThreadRef { id: t, name: "t".into(), message_count: 0 };
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("split-threads", 1);
        a.open_thread_link(link(), w, cx);
    });
    assert!(h.act(|a, _, _| a.chat.thread.as_ref().is_some_and(|p| p.thread.id == t)));
    h.act(|a, _, cx| a.close_thread(cx));
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("split-threads", 0);
        a.open_thread_link(link(), w, cx);
    });
    assert!(h.act(|a, _, _| a.chat.thread.is_none() && a.selected_channel == Some(t)));
}

#[gpui::test]
fn reads_messages_in_the_viewed_channel_only(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    crate::tts::record();
    let (here, there) = (h.channels[0], h.channels[1]);
    h.act(|a, _, cx| {
        a.deafened = false;
        a.prefs.local.ui.insert("read-all".into(), 1);
        a.selected_channel = Some(here);
        a.deafened = false;
        let mut users: Vec<_> = a.users.keys().copied().filter(|u| *u != a.me.id && !a.nav.is_blocked(*u) && !a.nav.is_ignored(*u)).collect();
        users.sort_by_key(|u| u.0);
        let author = users[0];
        a.append_message(Message { id: Id(9_100_001), channel: here, author, content: "bonjour".into(), ..Default::default() }, cx);
        a.append_message(Message { id: Id(9_100_002), channel: there, author, content: "ailleurs".into(), ..Default::default() }, cx);
        a.prefs.local.ui.insert("read-all".into(), 2);
        a.append_message(Message { id: Id(9_100_003), channel: here, author, content: "jamais".into(), ..Default::default() }, cx);
    });
    let said = crate::tts::recorded();
    assert!(said.iter().any(|u| u.text.ends_with("bonjour") && !u.interrupt), "{said:?}");
    assert!(!said.iter().any(|u| u.text.contains("ailleurs") || u.text.contains("jamais")), "{said:?}");
}

#[gpui::test]
fn composer_markdown_is_styled_while_typing(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let ranges = |h: &mut Harness| h.act(|a, _, cx| a.composer.read(cx).decorations().iter().map(|d| (d.range.clone(), d.style.font_weight)).collect::<Vec<_>>());
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("md-preview", 1);
        a.prefs.synced.proto.insert("legacy-input", 0);
        a.composer.update(cx, |s, cx| s.set_value("a **b**", w, cx));
        crate::ui::chat::rich_input::sync(a, cx);
    });
    assert_eq!(
        ranges(&mut h),
        vec![(2..4, Some(gpui::FontWeight::SEMIBOLD)), (4..5, Some(gpui::FontWeight::BOLD)), (5..7, Some(gpui::FontWeight::SEMIBOLD))]
    );
    h.act(|a, _, cx| assert!(!crate::ui::chat::rich_input::needs_sync(a, cx)));
    h.act(|a, _, cx| {
        a.prefs.synced.proto.insert("md-preview", 0);
        crate::ui::chat::rich_input::sync(a, cx);
    });
    assert!(ranges(&mut h).is_empty());
}

fn caret_after(h: &mut Harness, text: &str, preview: bool) -> f32 {
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("md-preview", preview as u64);
        a.prefs.synced.proto.insert("legacy-input", 0);
        a.composer.update(cx, |s, cx| {
            s.set_value(text.to_string(), w, cx);
            s.focus(w, cx);
        });
        crate::ui::chat::rich_input::sync(a, cx);
        w.refresh();
    });
    h.act(|_, w, _| w.refresh());
    h.act(|a, _, cx| f32::from(a.composer.read(cx).cursor_layout().expect("caret laid out").0.left()))
}

#[gpui::test]
fn composer_quote_hides_its_marker_and_indents_the_caret(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let plain = caret_after(&mut h, "ab", true);
    let quoted = caret_after(&mut h, "> ab", true);
    assert!((quoted - plain - 16.).abs() < 0.5, "plain {plain} quoted {quoted}");
    let lines = h.act(|a, _, cx| a.composer.read(cx).rich_decorations().lines().len());
    assert_eq!(lines, 1);
}

#[gpui::test]
fn composer_inline_code_gets_the_code_font_and_a_bordered_box(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let start = caret_after(&mut h, "", true);
    let off = caret_after(&mut h, "`abcdefgh`", false) - start;
    let on = caret_after(&mut h, "`abcdefgh`", true) - start;
    assert!(on > off, "the code box border moves the caret: off {off} on {on}");
    let code = h.act(|a, _, cx| {
        let rich = a.composer.read(cx).rich_decorations().spans().to_vec();
        rich.iter().any(|s| s.font_size == Some(gpui::px(13.6)) && s.font_family.is_some())
            && rich.iter().any(|s| s.text_box.as_ref().is_some_and(|b| b.border_width == gpui::px(1.)))
    });
    assert!(code);
    let state = h.act(|a, _, cx| crate::ui::chat::rich_input::needs_sync(a, cx));
    assert!(!state);
}

#[gpui::test]
fn composer_tokens_follow_the_preview_setting(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let ch = h.channels[0];
    let tokens = |h: &mut Harness| h.act(|a, _, cx| a.composer.read(cx).tokens().iter().map(|t| t.token().label().to_string()).collect::<Vec<_>>());
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("md-preview", 1);
        a.prefs.synced.proto.insert("legacy-input", 0);
        a.composer.update(cx, |s, cx| s.set_value(format!("go <#{}> now", ch.0), w, cx));
        crate::ui::chat::rich_input::sync(a, cx);
    });
    let name = h.act(|a, _, _| a.channels[&ch].name.to_string());
    assert_eq!(tokens(&mut h), vec![format!("#{name}")]);
    h.act(|a, _, cx| {
        a.prefs.synced.proto.insert("legacy-input", 1);
        crate::ui::chat::rich_input::sync(a, cx);
    });
    assert!(tokens(&mut h).is_empty());
}

#[gpui::test]
fn message_edit_uses_the_rich_composer(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let here = h.channels[0];
    let id = Id(9_200_001);
    h.act(|a, w, cx| {
        a.prefs.synced.proto.insert("md-preview", 1);
        a.prefs.synced.proto.insert("legacy-input", 0);
        let me = a.me.id;
        a.append_message(Message { id, channel: here, author: me, content: "a **b**\nsecond".into(), ..Default::default() }, cx);
        a.start_edit(id, w, cx);
    });
    h.act(|a, _, cx| crate::ui::chat::rich_input::sync(a, cx));
    let (value, bold) = h.act(|a, _, cx| {
        let s = a.chat.edit.as_ref().expect("editing").input.read(cx);
        (s.value().to_string(), s.decorations().iter().any(|d| d.style.font_weight == Some(gpui::FontWeight::BOLD)))
    });
    assert_eq!(value, "a **b**\nsecond");
    assert!(bold);
    h.act(|a, w, cx| {
        crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Edit, w, cx);
        crate::ui::pickers::pick(a, crate::ui::pickers::PickItem::Unicode(0), false, w, cx);
    });
    let (edit_len, composer) = h.act(|a, _, cx| (a.chat.edit.as_ref().unwrap().input.read(cx).value().len(), a.composer.read(cx).value().to_string()));
    assert!(edit_len > "a **b**\nsecond".len());
    assert!(composer.is_empty());
    h.act(|a, w, cx| a.cancel_edit(w, cx));
    assert!(h.act(|a, _, cx| a.chat.edit.is_none() && !crate::ui::pickers::is_open(cx)));
}

#[gpui::test]
fn voice_switch_warning_follows_the_setting(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let (g, voice): (Id, Vec<Id>) = h.act(|a, _, _| {
        a.guilds
            .iter()
            .map(|g| (g.id, a.guild_channels.get(&g.id).into_iter().flatten().copied().filter(|c| a.channels[c].kind == ChannelKind::Voice).collect::<Vec<_>>()))
            .find(|(_, v)| v.len() >= 2)
            .unwrap()
    });
    let (here, there) = (voice[0], voice[1]);
    h.act(|a, _, _| a.voice_channel = Some(here));
    assert!(h.act(|a, _, _| a.voice_switch_needs_confirm(there)));
    assert!(!h.act(|a, _, _| a.voice_switch_needs_confirm(here)));
    h.act(|a, _, cx| a.select_channel(there, cx));
    assert!(crate::actions::with_modal(|m| matches!(m, Some(crate::ui::modals::Modal::VoiceSwitch(_)))));
    assert_eq!(h.act(|a, _, _| a.voice_channel), Some(here));
    crate::actions::close_modal();
    h.act(|a, _, _| {
        a.prefs.local.ui.insert("vo-switch-warn".into(), 0);
    });
    assert!(!h.act(|a, _, _| a.voice_switch_needs_confirm(there)));
    h.act(|a, _, _| {
        a.prefs.local.ui.remove("vo-switch-warn");
        if let Some(gg) = a.guilds.iter_mut().find(|x| x.id == g) {
            gg.afk_channel = Some(here);
        }
    });
    assert!(!h.act(|a, _, _| a.voice_switch_needs_confirm(there)));
}
