//! Offscreen MOCK render of components that the plain mock screenshot does not show:
//! member list Activity cards, a nameplated member row, and the profile popout opened
//! from a voice channel member (with its "In a voice channel" card).
//! `cargo run -p phoenix-app --example component_shots -- out_dir`

use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, commands::InventoryEntry, mock::MockBackend, model::Id};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| ".".into());

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), || {
        gpui_platform::current_headless_renderer()
    });
    // The headless text system doesn't scan system fonts: load them explicitly.
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Italic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]
    .iter()
    .filter_map(|p| std::fs::read(p).ok().map(Into::into))
    .collect();
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    // PHX_SCALE: device scale (default: the platform's). 1.25 matches the user's client.
    if let Some(scale) = std::env::var("PHX_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).filter(|s| *s > 0.) {
        cx.update_window(win.into(), |_, window, _| window.set_scale_factor(scale))?;
    }
    let pump = |cx: &mut HeadlessAppContext, n: usize| {
        for _ in 0..n {
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    pump(&mut cx, 2);
    let v = view.borrow().clone().expect("view");
    cx.update_window(win.into(), |_, window, cx| {
        v.update(cx, |app, cx| {
            app.login_with_token = true;
            app.login_token.update(cx, |s, cx| s.set_value("mock-token", window, cx));
            app.submit_login(window, cx);
        });
    })?;
    pump(&mut cx, 30);
    // Mock feed: three PLAYED_GAME cards by mock members; one long name.
    let entry = |id: u64, author: u64, game: &str, resurrected: bool, trending: bool| InventoryEntry {
        id: Id((1_700_000_000_000u64 - 1_420_070_400_000) << 22 | id),
        author: Id(author),
        app: None,
        game: Some(game.into()),
        content_type: 1,
        first_time: false,
        resurrected,
        trending,
        streak_days: None,
        icon: None,
    };
    phoenix_app::ui::activity_traits::seed(vec![
        entry(1, 5, "Fortnite", false, false),
        entry(2, 13, "VALORANT", true, false),
        InventoryEntry { content_type: 8, ..entry(6, 9, "Word Bomb", false, false) },
        entry(3, 17, "Roblox", false, true),
        entry(4, 19, "Minecraft", false, false),
    ]);
    // Mock nameplate art generated here (a diagonal gradient), never a Discord asset.
    let plate = format!("{}/phoenix-mock-nameplate.png", std::env::temp_dir().display());
    image::RgbaImage::from_fn(426, 80, |x, y| {
        let t = (x as f32 / 426. + y as f32 / 160.).min(1.);
        image::Rgba([(40. + 160. * t) as u8, (60. + 40. * t) as u8, (180. - 80. * t) as u8, 255])
    })
    .save(&plate)?;
    cx.update_window(win.into(), |_, _, cx| {
        v.update(cx, |app, _| {
            // A nameplate (palette only: no network in offscreen renders) and a long name.
            // Nelly still plays Fortnite: live green timer on her card.
            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
            app.people.activities.insert(
                Id(5),
                vec![phoenix_app::presence::Activity {
                    kind: phoenix_app::presence::ActivityKind::Playing,
                    name: "Fortnite".into(),
                    details: None,
                    state: None,
                    emoji: None,
                    url: None,
                    display_type: None,
                    image: None,
                    start: Some(now_ms - 463_000),
                }],
            );
            if let Some(u) = app.users.get_mut(&Id(5)) {
                u.nameplate = Some((plate.clone().into(), Some(0x5865f2)));
            }
            // Emoji in channel names, like "〢❓・aide-membres" on real servers.
            for ch in app.channels.values_mut() {
                match ch.name.as_ref() {
                    "help" => ch.name = "〢❓・help".into(),
                    "off-topic" => ch.name = "〢📞・off-topic".into(),
                    "memes" => ch.name = "〢🎟️・memes".into(),
                    _ => {}
                }
            }
            // Emoji in names (member list, cards, messages): color emoji font.
            if let Some(u) = app.users.get_mut(&Id(9)) {
                u.display_name = "Mira 🌸❓".into();
            }
            if let Some(u) = app.users.get_mut(&Id(13)) {
                u.display_name = "Quartz (a very long display name)".into();
            }
            app.member_list.remeasure();
        });
    })?;
    pump(&mut cx, 60); // let Twemoji images load
    let shot = |cx: &mut HeadlessAppContext| -> anyhow::Result<image::RgbaImage> {
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(cx, 2);
        let _ = cx.capture_screenshot(win.into())?;
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(cx, 2);
        cx.capture_screenshot(win.into())
    };
    let img = shot(&mut cx)?;
    // At PHX_SCALE the content fills only the top-left 1280*scale px of the capture.
    let s = std::env::var("PHX_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(img.width() as f32 / 1280.);
    let crop = |img: &image::RgbaImage, x: f32, y: f32, w: f32, h: f32| {
        image::imageops::crop_imm(img, (x * s) as u32, (y * s) as u32, (w * s) as u32, (h * s) as u32).to_image()
    };
    crop(&img, 1280. - 268., 40., 268., 760.).save(format!("{out}/members.png"))?;
    crop(&img, 0., 40., 72., 500.).save(format!("{out}/rail.png"))?;
    crop(&img, 72., 40., 280., 700.).save(format!("{out}/channels.png"))?;
    // Profile popout from the voice row of Aiko (mock user 7, in "Lounge").
    cx.update_window(win.into(), |_, window, cx| {
        v.update(cx, |app, cx| {
            let at = phoenix_app::ui::voice_row::popout_anchor(Id(7), gpui::point(px(300.), px(400.)));
            let g = app.selected_guild;
            app.open_profile(Id(7), g, at, window, cx);
        });
    })?;
    pump(&mut cx, 6);
    let img = shot(&mut cx)?;
    crop(&img, 72., 40., 640., 760.).save(format!("{out}/voice_popout.png"))?;
    println!("wrote {out}/members.png rail.png channels.png voice_popout.png");
    Ok(())
}
