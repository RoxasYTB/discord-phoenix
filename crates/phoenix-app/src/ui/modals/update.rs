use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    markdown::{Block, Inline, NoResolve, Style, parse},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
    updater::{self, UpdateStatus},
};

pub(crate) fn render_inline(inline: &Inline) -> AnyElement {
    let mut row = div().flex().flex_wrap().items_baseline();
    if inline.spans.is_empty() {
        return div().child(SharedString::from(inline.text.clone())).into_any_element();
    }
    let mut cursor = 0;
    for span in &inline.spans {
        if span.range.start > cursor && span.range.start <= inline.text.len() {
            row = row.child(SharedString::from(inline.text[cursor..span.range.start].to_owned()));
        }
        if span.range.end <= inline.text.len() && span.range.start <= span.range.end {
            let seg = SharedString::from(inline.text[span.range.start..span.range.end].to_owned());
            let mut el = div().child(seg);
            if span.style.has(Style::BOLD) {
                el = el.font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_STRONG)));
            }
            if span.style.has(Style::ITALIC) {
                el = el.italic();
            }
            if span.style.has(Style::CODE) {
                el = el.px(z(4.)).py(z(1.)).rounded(z(4.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).text_size(z(13.));
            }
            if span.style.has(Style::LINK) {
                el = el.text_color(c(tok(idx::TEXT_LINK)));
            }
            row = row.child(el);
            cursor = span.range.end;
        }
    }
    if cursor < inline.text.len() {
        row = row.child(SharedString::from(inline.text[cursor..].to_owned()));
    }
    row.into_any_element()
}

fn render_markdown(body: &str) -> AnyElement {
    let blocks = parse(body, &NoResolve);
    let mut col = div().flex().flex_col().gap(z(8.)).text_size(z(14.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT)));
    if blocks.is_empty() {
        return col.child(div().text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("update.no_notes"))).into_any_element();
    }
    for block in blocks {
        match block {
            Block::Heading(level, inline) => {
                let sz = match level {
                    1 => 18.,
                    2 => 16.,
                    _ => 15.,
                };
                col = col.child(
                    div()
                        .pt(z(4.))
                        .text_size(z(sz))
                        .line_height(z(sz + 6.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(render_inline(&inline)),
                );
            }
            Block::Paragraph(inline) | Block::Subtext(inline) => {
                col = col.child(render_inline(&inline));
            }
            Block::ListItem { ordered, depth, inline } => {
                let bullet = match ordered {
                    Some(n) => format!("{n}."),
                    None => "•".to_owned(),
                };
                col = col.child(
                    div()
                        .pl(z(8. + f32::from(depth) * 14.))
                        .flex()
                        .items_start()
                        .gap(z(8.))
                        .child(div().flex_none().text_color(c(tok(idx::TEXT_MUTED))).child(bullet))
                        .child(div().flex_1().min_w_0().child(render_inline(&inline))),
                );
            }
            Block::Quote(inline) => {
                col = col.child(
                    div().pl(z(10.)).border_l_4().border_color(c(tok(idx::BORDER_NORMAL))).text_color(c(tok(idx::TEXT_SUBTLE))).child(render_inline(&inline)),
                );
            }
            Block::Code { code, .. } => {
                col = col.child(
                    div()
                        .p(z(10.))
                        .rounded(z(6.))
                        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .text_size(z(13.))
                        .line_height(z(18.))
                        .child(SharedString::from(code)),
                );
            }
        }
    }
    col.into_any_element()
}

pub fn render(_app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let status = updater::status();
    let Some(release) = status.release().cloned() else {
        let body = div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("update.up_to_date"));
        return super::card(460., crate::t!("update.modal_title"), None, body.into_any_element(), None, cx);
    };

    let current = crate::diagnostics::VERSION;
    let size_part = release.asset.as_ref().filter(|a| a.size > 0).map(|a| format!(" • {}", updater::format_bytes(a.size))).unwrap_or_default();
    let subtitle: SharedString = format!("v{current} → {}{size_part}", release.tag).into();

    let mut body = div().flex().flex_col().gap(z(12.));

    if let Some(asset) = &release.asset {
        body = body.child(
            div()
                .px(z(12.))
                .py(z(8.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .items_center()
                .justify_between()
                .gap(z(8.))
                .child(div().text_size(z(13.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(asset.name.clone()))
                .when(asset.size > 0, |d| d.child(div().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child(updater::format_bytes(asset.size)))),
        );
    }

    body = body.child(div().id("update-notes-scroll").max_h(z(320.)).overflow_y_scroll().pr(z(4.)).child(render_markdown(&release.body)));

    match &status {
        UpdateStatus::Downloading { downloaded, total, .. } => {
            let pct = if *total > 0 { ((*downloaded as f64 / *total as f64) * 100.0).min(100.0) } else { 0.0 };
            let label = format!("{} ({} / {})", crate::t!("update.downloading"), updater::format_bytes(*downloaded), updater::format_bytes(*total));
            body = body.child(
                div().flex().flex_col().gap(z(6.)).child(div().text_size(z(13.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(label)).child(
                    div()
                        .w_full()
                        .h(z(6.))
                        .rounded(z(3.))
                        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                        .overflow_hidden()
                        .child(div().h_full().w(gpui::relative((pct / 100.0) as f32)).bg(c(tok(idx::STATUS_POSITIVE)))),
                ),
            );
        }
        UpdateStatus::Verifying(_) => {
            body = body.child(div().text_size(z(13.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("update.verifying")));
        }
        UpdateStatus::ReadyToRestart { .. } => {
            body = body.child(
                div()
                    .text_size(z(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_FEEDBACK_POSITIVE)))
                    .child(crate::t!("update.ready_restart")),
            );
        }
        UpdateStatus::Installed { path, .. } => {
            body = body.child(div().text_size(z(13.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_FEEDBACK_POSITIVE))).child(format!(
                "{} {}",
                crate::t!("update.downloaded_to"),
                path.display()
            )));
        }
        UpdateStatus::Error(err) => {
            body = body.child(div().text_size(z(13.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(err.clone()));
        }
        _ => {}
    }

    let open_url = release.html_url.clone();
    let open_btn = button("update-open-release", crate::t!("update.open_release"), BtnKind::Secondary).on_click(move |_, _, cx| cx.open_url(&open_url));

    let primary_btn = match status {
        UpdateStatus::ReadyToRestart { path, .. } => {
            button("update-restart", crate::t!("update.restart"), BtnKind::Active).on_click(move |_, _, _| updater::restart_app(&path))
        }
        UpdateStatus::Downloading { .. } | UpdateStatus::Verifying(_) => {
            button("update-busy", crate::t!("update.downloading"), BtnKind::Secondary).opacity(0.6)
        }
        _ => {
            let rel_for_dl = release.clone();
            let has_asset = release.asset.is_some();
            let html_fallback = release.html_url.clone();
            button("update-install", crate::t!("update.install_now"), BtnKind::Active).on_click(cx.listener(move |_, _, _, cx| {
                if has_asset {
                    updater::spawn_install(rel_for_dl.clone(), cx);
                } else {
                    cx.open_url(&html_fallback);
                }
            }))
        }
    };

    let footer = div().flex().items_center().justify_end().gap(z(8.)).child(open_btn).child(primary_btn);

    super::card(520., crate::t!("update.modal_title"), Some(subtitle), body.into_any_element(), Some(footer.into_any_element()), cx)
}
