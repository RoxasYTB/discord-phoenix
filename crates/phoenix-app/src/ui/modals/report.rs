use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::reporting::{ReportMenu, ReportNode, ReportTarget};

use super::{Modal, card};
use crate::{
    actions::{ActionReply, ActionRequest, close_modal, with_modal},
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub struct Report {
    pub target: ReportTarget,
    pub menu: Option<Box<ReportMenu>>,
    pub path: Vec<u64>,
    pub error: Option<SharedString>,
    pub sending: bool,
}

impl Report {
    pub fn current(&self) -> Option<&ReportNode> {
        let m = self.menu.as_ref()?;
        m.node(*self.path.last().unwrap_or(&m.root_node_id))
    }
}

impl PhoenixApp {
    pub fn open_report(&mut self, target: ReportTarget, cx: &mut gpui::Context<Self>) {
        crate::actions::open_modal(Modal::Report(Report { target, menu: None, path: Vec::new(), error: None, sending: false }), None);
        self.action(ActionRequest::ReportMenu(target), cx, |_, r, _| {
            with_modal(|m| {
                if let Some(Modal::Report(rep)) = m {
                    match r {
                        Ok(ActionReply::ReportMenu(menu)) => {
                            rep.path = vec![menu.root_node_id];
                            rep.menu = Some(menu);
                        }
                        Err(e) => rep.error = Some(e.0.into()),
                        _ => {}
                    }
                }
            });
        });
        cx.notify();
    }

    fn report_go(&mut self, node: u64, cx: &mut gpui::Context<Self>) {
        with_modal(|m| {
            if let Some(Modal::Report(r)) = m {
                r.path.push(node);
            }
        });
        cx.notify();
    }

    fn report_submit(&mut self, cx: &mut gpui::Context<Self>) {
        let req = with_modal(|m| match m {
            Some(Modal::Report(r)) if !r.sending => {
                r.sending = true;
                let menu = r.menu.clone()?;
                Some(ActionRequest::SubmitReport { menu, target: r.target, breadcrumbs: r.path.clone() })
            }
            _ => None,
        });
        let Some(req) = req else { return };
        self.action(req, cx, |_, res, _| {
            with_modal(|m| {
                if let Some(Modal::Report(r)) = m {
                    r.sending = false;
                    match res {
                        Ok(_) => {
                            if let Some(ok) = r.menu.as_ref().map(|m| m.success_node_id) {
                                r.path.push(ok);
                            }
                        }
                        Err(e) => r.error = Some(e.0.into()),
                    }
                }
            });
        });
    }
}

pub fn render(r: &Report, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let title = match r.target {
        ReportTarget::Message { .. } => crate::t!("report.message_title"),
        ReportTarget::User { .. } => crate::t!("report.user_title"),
    };
    let mut body = div().flex().flex_col().gap(z(8.));
    let mut footer = div().flex().gap(z(8.));
    if r.path.len() > 1 {
        footer = footer.child(button("report-back", crate::t!("common.back"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Report(r)) = m {
                    r.path.pop();
                }
            });
            cx.notify();
        })));
    }
    let subtitle;
    match (r.current(), &r.error) {
        (_, Some(e)) => {
            subtitle = None;
            body = body.child(div().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(e.clone()));
        }
        (None, None) => {
            subtitle = None;
            body = body.child(div().text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("common.loading")));
        }
        (Some(node), None) => {
            body = body.child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(node.header.clone()));
            subtitle = node.subheader.clone().map(SharedString::from);
            if let Some(info) = &node.info {
                body = body.child(div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(info.clone()));
            }
            for (i, (label, target)) in node.children.iter().enumerate() {
                let target = *target;
                body = body.child(
                    div()
                        .id(("report-opt", i))
                        .min_h(z(44.))
                        .px(z(12.))
                        .rounded(z(8.))
                        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(z(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .on_click(cx.listener(move |a, _, _, cx| a.report_go(target, cx)))
                        .child(label.clone())
                        .child("›"),
                );
            }
            if let Some(b) = &node.button {
                let target = b.target;
                footer = match b.kind.as_str() {
                    "submit" => footer.child(
                        button("report-submit", crate::t!("report.submit"), BtnKind::Critical)
                            .when(r.sending, |d| d.opacity(0.5))
                            .on_click(cx.listener(|a, _, _, cx| a.report_submit(cx))),
                    ),
                    "next" => footer.child(button("report-next", crate::t!("common.next"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
                        if let Some(t) = target {
                            a.report_go(t, cx)
                        }
                    }))),
                    _ => footer.child(button("report-done", crate::t!("common.done"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| {
                        close_modal();
                        cx.notify();
                    }))),
                };
            }
        }
    }
    card(440., title, subtitle, body.into_any_element(), Some(footer.into_any_element()), cx)
}
