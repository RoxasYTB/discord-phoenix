use gpui::{
    AnyElement, Context, FontWeight, ObjectFit, ParentElement, SharedString, Styled, StyledImage,
    div, img, prelude::*, px,
};

use super::{Pickers, PICKER_W, close, debounce, empty_state, types::GifItem, with_app};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok},
};

const GAP: f32 = 12.;
const COL_GAP: f32 = 11.;
const PAD: f32 = 12.;
const TILE_H: f32 = 110.;

pub fn load_home(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let g = &mut cx.global_mut::<Pickers>().gifs;
    if g.home.is_some() || g.home_loading {
        return;
    }
    g.home_loading = true;
    let fut = app.backend.gif_trending();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            let g = &mut cx.global_mut::<Pickers>().gifs;
            g.home_loading = false;
            match res {
                Ok(h) => g.home = Some(h),
                Err(e) => g.error = Some(e.0.into()),
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn search_debounced(_app: &mut PhoenixApp, q: String, cx: &mut Context<PhoenixApp>) {
    let g = &mut cx.global_mut::<Pickers>().gifs;
    g.seq += 1;
    let seq = g.seq;
    if q.trim().is_empty() {
        g.shown = None;
        g.results.clear();
        g.loading = false;
        return;
    }
    debounce(cx, 350, move |app, cx| {
        if cx.global::<Pickers>().gifs.seq == seq {
            run(app, q, cx);
        }
    });
}

pub fn run(app: &mut PhoenixApp, q: String, cx: &mut Context<PhoenixApp>) {
    let g = &mut cx.global_mut::<Pickers>().gifs;
    g.seq += 1;
    let seq = g.seq;
    g.loading = true;
    g.error = None;
    let fut = if q.is_empty() { app.backend.gif_trending_gifs() } else { app.backend.gif_search(q.clone()) };
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            let g = &mut cx.global_mut::<Pickers>().gifs;
            if g.seq != seq {
                return;
            }
            g.loading = false;
            g.shown = Some(q);
            match res {
                Ok(v) => g.results = v,
                Err(e) => {
                    g.results.clear();
                    g.error = Some(e.0.into());
                }
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

fn show_favorites(cx: &mut Context<PhoenixApp>) {
    let p = cx.global_mut::<Pickers>();
    let favs = p.frecency.as_ref().map(|f| f.favorite_gifs.clone()).unwrap_or_default();
    p.gifs.seq += 1;
    p.gifs.loading = false;
    p.gifs.error = None;
    p.gifs.shown = Some(crate::t!("picker.favorites").to_string());
    p.gifs.results = favs;
    cx.notify();
}

fn send(app: &mut PhoenixApp, gif: &GifItem, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = app.selected_channel else { return };
    let gif = gif.clone();
    let http = cx.http_client();
    let backend = app.backend.clone();
    let account = app.me.id;
    let attach = backend.channel_permissions(ch).is_none_or(|permissions| {
        let attach = phoenix_api::models::Permissions::ATTACH_FILES.bits();
        permissions & attach != 0
    });
    let limit = backend.max_upload_size(ch);
    let nonce = crate::ui::chat::pending::make_nonce();
    cx.spawn(async move |this, cx| {
        let media = crate::media::gif_file::download(http, gif, limit, cx.background_executor()).await;
        let result = match media {
            Ok((bytes, source)) => {
                let upload = crate::backend::Upload {
                    filename: "animation.gif".into(), bytes, spoiler: false, description: None, pre: None, hero: false,
                };
                let (content, files) = if attach { (String::new(), vec![upload]) } else { (source.to_string(), Vec::new()) };
                let Ok(Some(send)) = this.update(cx, |app, _| {
                    (app.me.id == account && app.screen == crate::app::Screen::Main)
                        .then(|| app.backend.send_message_ex(ch, content, None, files, nonce, None))
                }) else { return };
                send.await
            }
            Err(error) => Err(error),
        };
        let _ = this.update(cx, |app, cx| {
            if app.me.id == account && app.screen == crate::app::Screen::Main {
                match result {
                    Ok(message) => app.append_message(message, cx),
                    Err(error) => app.parity_toast(crate::t!("picker.gif_send_failed", error = error.0), cx),
                }
            }
        });
    }).detach();
    close(app, window, cx);
}

fn hc_outline<E: Styled>(d: E) -> E {
    d.border_1().border_color(c(tok(idx::BORDER_STRONG)))
}

fn tile(id: (&'static str, usize), name: SharedString, preview: Option<SharedString>, w: f32, blurple: bool, icon: Option<&'static str>) -> gpui::Stateful<gpui::Div> {
    let fade = if blurple { c(tok(idx::BACKGROUND_BRAND)).opacity(0.8) } else { c(tok(idx::BACKGROUND_SCRIM)) };
    div()
        .id(id)
        .relative()
        .w(px(w))
        .h(px(TILE_H))
        .flex_none()
        .rounded(px(5.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .when(crate::theme::high_contrast(), hc_outline)
        .cursor_pointer()
        .when_some(preview, |d, u| {
            let mut im = img(u.clone());
            if let Some(cache) = crate::media::lru::chat() {
                im = im.image_cache(&cache);
            }
            d.child(im.id(crate::media::anim_id(&u)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover))
        })
        .child(div().absolute().inset_0().rounded(px(4.)).bg(fade).hover(|s| s.bg(fade.opacity(0.5))))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.))
                .line_height(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::white())
                .when_some(icon, |d, i| d.child(gpui::svg().path(crate::icons::path(i)).size(px(20.)).mr(px(4.)).text_color(gpui::white())))
                .child(name),
        )
}

thread_local! {
    static FAVORITE_TILE: std::cell::RefCell<(Option<gpui::EntityId>, usize)> = const { std::cell::RefCell::new((None, 0)) };
    static PINNED_FAVORITE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub fn pin_favorite_tile(url_part: Option<String>) {
    PINNED_FAVORITE.with(|p| *p.borrow_mut() = url_part);
}

fn favorite_tile_preview(favs: &[GifItem], picker: Option<gpui::EntityId>) -> Option<SharedString> {
    if favs.is_empty() {
        return None;
    }
    if let Some(key) = PINNED_FAVORITE.with(|p| p.borrow().clone()) {
        let stem = key.rsplit('/').next().unwrap_or(key.as_str());
        let hit = |part: &str| favs.iter().find(|g| g.url.contains(part) || g.preview.contains(part));
        if let Some(g) = hit(key.as_str()).or_else(|| hit(stem)) {
            return Some(g.preview.clone());
        }
    }
    let ix = FAVORITE_TILE.with(|t| {
        let mut t = t.borrow_mut();
        if t.0 != picker {
            use std::hash::BuildHasher;
            *t = (picker, std::collections::hash_map::RandomState::new().hash_one(picker) as usize);
        }
        t.1
    });
    favs.get(ix % favs.len()).map(|g| g.preview.clone())
}

pub fn render_body(_app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let p = cx.global::<Pickers>();
    let g = &p.gifs;
    let typed = p.open.as_ref().map(|o| !o.input.read(cx).value().trim().is_empty()).unwrap_or(false);
    let col_w = (PICKER_W - 2. - 2. * PAD - COL_GAP) / 2.;
    let content: AnyElement = if g.shown.is_none() && !typed {
        let favs = p.frecency.as_ref().map(|f| f.favorite_gifs.clone()).unwrap_or_default();
        let Some(home) = g.home.clone() else {
            return empty_state(if g.home_loading { crate::t!("picker.loading_gifs") } else { crate::t!("picker.gifs_unavailable") });
        };
        let mut tiles: Vec<AnyElement> = vec![
            tile(("gif-favorites", 0), crate::t!("picker.favorites"), favorite_tile_preview(&favs, p.open.as_ref().map(|o| o.input.entity_id())), col_w, true, None)
                .on_click(|_, w, cx| with_app(cx, w, |_, _, cx| show_favorites(cx)))
                .into_any_element(),
            tile(("gif-trending", 0), crate::t!("picker.trending"), home.trending_preview.clone(), col_w, false, Some("picker/trending"))
                .on_click(|_, w, cx| with_app(cx, w, |a, _, cx| run(a, String::new(), cx)))
                .into_any_element(),
        ];
        for (i, cat) in home.categories.iter().enumerate() {
            let name = cat.name.clone();
            tiles.push(
                tile(("gif-cat", i), cat.name.clone(), Some(cat.preview.clone()), col_w, false, None)
                    .on_click(move |_, w, cx| {
                        let name = name.clone();
                        with_app(cx, w, move |a, w, cx| {
                            if let Some(op) = cx.global::<Pickers>().open.as_ref() {
                                let input = op.input.clone();
                                input.update(cx, |s, cx| s.set_value(name.to_string(), w, cx));
                            }
                            run(a, name.to_string(), cx);
                        })
                    })
                    .into_any_element(),
            );
        }
        div().flex().flex_wrap().gap_x(px(COL_GAP)).gap_y(px(GAP)).children(tiles).into_any_element()
    } else if g.loading && g.results.is_empty() {
        return empty_state(crate::t!("picker.loading_gifs"));
    } else if g.results.is_empty() {
        return empty_state(if g.error.is_some() { crate::t!("picker.gifs_unavailable") } else { crate::t!("picker.no_gifs") });
    } else {
        let mut cols: [(f32, Vec<AnyElement>); 2] = [(0., Vec::new()), (0., Vec::new())];
        for (i, gif) in g.results.iter().enumerate() {
            let h = if gif.width > 0 { col_w * gif.height as f32 / gif.width as f32 } else { col_w * 0.75 };
            let h = h.clamp(60., 400.);
            let k = if cols[0].0 <= cols[1].0 { 0 } else { 1 };
            cols[k].0 += h + GAP;
            let item = gif.clone();
            cols[k].1.push(
                div()
                    .id(("gif", i))
                    .w(px(col_w))
                    .h(px(h))
                    .rounded(px(5.))
                    .overflow_hidden()
                    .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                    .when(crate::theme::high_contrast(), hc_outline)
                    .cursor_pointer()
                    .hover(|s| s.border_2().border_color(c(tok(idx::BORDER_FOCUS))))
                    .on_click(move |_, w, cx| {
                        let item = item.clone();
                        with_app(cx, w, move |a, w, cx| send(a, &item, w, cx));
                    })
                    .child({
                        let mut im = img(gif.preview.clone());
                        if let Some(cache) = crate::media::lru::chat() {
                            im = im.image_cache(&cache);
                        }
                        im.id(crate::media::anim_id(&gif.preview)).size_full().object_fit(ObjectFit::Cover)
                    })
                    .into_any_element(),
            );
        }
        let [(_, a), (_, b)] = cols;
        div()
            .flex()
            .gap(px(COL_GAP))
            .child(div().flex().flex_col().gap(px(GAP)).children(a))
            .child(div().flex().flex_col().gap(px(GAP)).children(b))
            .into_any_element()
    };
    crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::ThinFade)
        .size_full()
        .child(div().id("gif-scroll").size_full().overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("gif-scroll")).p(px(PAD)).child(content))
        .children(crate::ui::chat::scrollbar::bar_for("gif-scroll", crate::ui::chat::scrollbar::Bar::ThinFade))
        .into_any_element()
}
