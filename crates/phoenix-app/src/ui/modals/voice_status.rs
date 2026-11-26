use std::sync::Arc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    actions::{close_modal, open_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button},
        modals::{Modal, close_action},
    },
};

pub const MAX_LEN: usize = 500;
pub const SHOW_REMAINING: usize = 250;
pub const WIDTH: f32 = 480.;
pub const ART: &str = "https://discord.com/assets/4ed48b8404cc8dd4.svg";

pub struct State {
    pub channel: Id,
    pub channel_name: SharedString,
    pub initial: String,
    pub input: Option<Entity<InputState>>,
    pub saving: bool,
    pub error: Option<SharedString>,
    _enter: Option<gpui::Subscription>,
}

pub fn open(channel: Id, channel_name: SharedString, initial: String) {
    open_modal(Modal::VoiceStatus(State { channel, channel_name, initial, input: None, saving: false, error: None, _enter: None }), None);
}

pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn remaining(len: usize) -> Option<i64> {
    let left = MAX_LEN as i64 - len as i64;
    (left <= (MAX_LEN - SHOW_REMAINING) as i64).then_some(left)
}

pub fn too_long(len: usize) -> bool {
    len > MAX_LEN
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.input.is_some() {
        return;
    }
    let ph = crate::t!("vstatus.placeholder", channel = s.channel_name);
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
    let initial = s.initial.clone();
    input.update(cx, |st, cx| {
        if !initial.is_empty() {
            st.set_value(initial, window, cx);
        }
        st.focus(window, cx);
    });
    s._enter = Some(cx.subscribe_in(&input, window, |a, _, ev: &InputEvent, _, cx| {
        if matches!(ev, InputEvent::PressEnter { .. }) {
            a.submit_voice_status(cx);
        }
        cx.notify();
    }));
    s.input = Some(input);
}

pub fn insert(text: &str, window: &mut Window, cx: &mut gpui::App) {
    let input = with_modal(|m| match m {
        Some(Modal::VoiceStatus(s)) => s.input.clone(),
        _ => None,
    });
    if let Some(i) = input {
        let text = text.to_owned();
        i.update(cx, |st, cx| {
            let (t, r) = crate::ui::modals::channel_settings::insert_at(&st.value(), st.selected_range(), &text);
            st.set_value(t, window, cx);
            st.set_selected_range(r, cx);
            st.focus(window, cx);
        });
    }
}

fn set(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::VoiceStatus(s)) = m {
            f(s);
        }
    });
}

impl PhoenixApp {
    pub fn open_voice_status(&mut self, channel: Id, cx: &mut Context<Self>) {
        let name = self.channels.get(&channel).map(|c| c.name.clone()).unwrap_or_default();
        let current = crate::voice::start_time::status(channel.0).unwrap_or_default();
        open(channel, name, current);
        cx.notify();
    }

    pub fn submit_voice_status(&mut self, cx: &mut Context<Self>) {
        let got = with_modal(|m| match m {
            Some(Modal::VoiceStatus(s)) if !s.saving => s.input.as_ref().map(|i| (s.channel, s.initial.clone(), i.read(cx).value().to_string())),
            _ => None,
        });
        let Some((channel, initial, text)) = got else { return };
        if too_long(utf16_len(&text)) {
            return;
        }
        if text == initial {
            close_modal();
            cx.notify();
            return;
        }
        set(|s| {
            s.saving = true;
            s.error = None;
        });
        let fut = self.backend.guarded_write(crate::actions::guard::GuardedWrite::voice_channel_status(channel.0, &text));
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                match res {
                    Ok(_) => close_modal(),
                    Err(_) => set(|s| {
                        s.saving = false;
                        s.error = Some(crate::t!("acct.blocked"));
                    }),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn gradient_pixel(fx: f32, fy: f32, start: [f32; 3], end: [f32; 3]) -> [u8; 4] {
    let col = [0, 1, 2].map(|i| start[i] + (end[i] - start[i]) * fx);
    let d = ((fx - 0.5).powi(2) + (fy - 1.).powi(2)).sqrt();
    let a = ((d - 0.6) / 0.4).clamp(0., 1.);
    [col[2] as u8, col[1] as u8, col[0] as u8, (a * 255.) as u8]
}

fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 24) & 0xff) as f32, ((hex >> 16) & 0xff) as f32, ((hex >> 8) & 0xff) as f32]
}

fn gradient() -> Arc<gpui::RenderImage> {
    thread_local! {
        static IMG: std::cell::RefCell<Option<(u64, Arc<gpui::RenderImage>)>> = const { std::cell::RefCell::new(None) };
    }
    let (s, e) = (crate::theme::resolve(tok(idx::EXPRESSIVE_GRADIENT_PURPLE_START)), crate::theme::resolve(tok(idx::EXPRESSIVE_GRADIENT_PURPLE_END)));
    let key = ((s as u64) << 32) | e as u64;
    if let Some(i) = IMG.with(|h| h.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, i)| i.clone())) {
        return i;
    }
    let (w, h) = (192u32, 96u32);
    let (sa, ea) = (rgb(s), rgb(e));
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let fx = (x as f32 + 0.5) / w as f32;
        let fy = (y as f32 + 0.5) / h as f32;
        *p = image::Rgba(gradient_pixel(fx, fy, sa, ea));
    }
    let arc = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(out)]));
    IMG.with(|h| *h.borrow_mut() = Some((key, arc.clone())));
    arc
}

fn emoji_button(cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .id("vstatus-emoji")
        .flex_none()
        .size(z(32.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("vstatus-emoji")
        .on_click(cx.listener(|a, _, w, cx| {
            crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Field(crate::ui::pickers::Field::VoiceStatus), w, cx)
        }))
        .child(
            gpui::svg()
                .path(crate::ui::Icon::Emoji.path())
                .size(z(24.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover("vstatus-emoji", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
        )
        .into_any_element()
}

pub fn render(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let text = s.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let len = utf16_len(&text);
    let long = too_long(len);
    let left = remaining(len);
    let close = close_action(cx);
    let close2 = close.clone();
    let field = div()
        .relative()
        .min_h(z(44.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if s.error.is_some() { idx::INPUT_BORDER_ERROR_DEFAULT } else { idx::INPUT_BORDER_DEFAULT })))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .pl(z(12.))
        .pr(z(6.))
        .gap(z(4.))
        .text_size(z(16.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .children(s.input.as_ref().map(|i| div().flex_1().min_w_0().child(Input::new(i).appearance(false).bordered(false))))
        .child(emoji_button(cx))
        .children(left.map(|n| {
            div()
                .absolute()
                .right(z(8.))
                .bottom(z(-20.))
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(c(tok(if n < 0 { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_MUTED })))
                .child(SharedString::from(n.to_string()))
        }));
    let body = div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("vstatus.label")))
        .child(field)
        .children(s.error.clone().map(|e| div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(e)));
    let cancel = button("vstatus-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| close(w, cx));
    let set_btn = button("vstatus-set", crate::t!("vstatus.set"), BtnKind::Primary).flex_1();
    let set_btn = if long || s.saving { set_btn.opacity(0.5).cursor_default() } else { set_btn.on_click(cx.listener(|a, _, _, cx| a.submit_voice_status(cx))) };
    let header = div()
        .relative()
        .pt(z(16.))
        .px(z(32.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(img(gradient()).absolute().top(z(-9.5)).left(z(-1.5)).right(z(-1.5)).bottom_0().rounded_t(z(8.)))
        .child(div().w_full().pt(z(20.)).pb(z(16.)).flex().justify_center().child(img(SharedString::from(ART)).max_w(z(288.)).max_h(z(120.)).object_fit(gpui::ObjectFit::Contain)))
        .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("vstatus.title")))
        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("vstatus.subtitle")));
    let el = div()
        .relative()
        .w(z(WIDTH))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(crate::ui::inbox::shadow_high())
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(header)
        .child(div().px(z(32.)).pt(z(16.)).pb(z(if left.is_some() { 28. } else { 16. })).child(body))
        .child(div().px(z(32.)).pt(z(16.)).flex().gap(z(8.)).child(cancel).child(set_btn))
        .child(
            div()
                .id("vstatus-close")
                .absolute()
                .top(z(12.))
                .right(z(12.))
                .size(z(32.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|st| st.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click({
                    let close = close2.clone();
                    move |_, w, cx| close(w, cx)
                })
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    crate::ui::modal::frame("vstatus-backdrop", close2, el)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_matches_the_client() {
        assert_eq!(remaining(249), None);
        assert_eq!(remaining(250), Some(250));
        assert_eq!(remaining(501), Some(-1));
        assert!(!too_long(500) && too_long(501));
        assert_eq!(utf16_len("😀a"), 3);
    }

    #[test]
    fn gradient_is_clear_at_the_bottom_centre() {
        assert_eq!(gradient_pixel(0.5, 1., [0., 0., 0.], [255., 255., 255.])[3], 0);
        assert_eq!(gradient_pixel(0., 0., [10., 20., 30.], [0., 0., 0.]), [30, 20, 10, 255]);
    }
}
