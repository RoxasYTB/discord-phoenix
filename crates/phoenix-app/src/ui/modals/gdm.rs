use std::sync::Arc;

use gpui::{AnyElement, Context, Entity, FontWeight, Image, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*};
use gpui_component::input::InputState;

use super::{Modal, card, close_action, field_label, input_box};
use crate::{
    actions::{ActionReply, ActionRequest, open_modal, with_modal},
    app::PhoenixApp,
    model::{Id, ImageRef},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};
use crate::ui::home::data::GuildFolder;
use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};

pub struct EditGroup {
    pub channel: Id,
    pub initial: String,
    pub placeholder: SharedString,
    pub current_icon: Option<SharedString>,
    pub icon: Option<Option<String>>,
    pub preview: Option<Arc<Image>>,
    pub name: Option<Entity<InputState>>,
}

pub struct Invite {
    pub code: SharedString,
    pub inviter: SharedString,
    pub avatar: Option<crate::model::User>,
    pub expires: Option<i64>,
}

pub struct Invites {
    pub channel: Id,
    pub list: Option<Vec<Invite>>,
}

pub struct Folder {
    pub id: u64,
    pub initial: String,
    pub color: Option<u32>,
    pub name: Option<Entity<InputState>>,
}

pub const FOLDER_DEFAULT: u32 = 0x5865F2;
pub const FOLDER_COLORS: [u32; 20] = [
    0x1ABC9C, 0x2ECC71, 0x3498DB, 0x9B59B6, 0xE91E63, 0xF1C40F, 0xE67E22, 0xE74C3C, 0x95A5A6, 0x607D8B, 0x11806A, 0x1F8B4C, 0x206694, 0x71368A,
    0xAD1457, 0xC27C0E, 0xA84300, 0x992D22, 0x979C9F, 0x546E7A,
];
pub const FOLDER_NAME_MAX: usize = 32;
pub const GROUP_NAME_MAX: usize = 100;

pub fn stored_color(c: u32) -> Option<u32> {
    (c != FOLDER_DEFAULT).then_some(c)
}

pub fn group_patch(initial: &str, name: &str, icon: &Option<Option<String>>) -> Option<AdminCall> {
    let name = (name != initial).then(|| name.to_owned());
    if name.is_none() && icon.is_none() {
        return None;
    }
    Some(AdminCall::EditGroupDm { channel: Snowflake(0), name, icon: icon.clone() })
}

pub fn parse_invites(v: &serde_json::Value) -> Vec<Invite> {
    let mut out: Vec<Invite> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let code = i.get("code")?.as_str()?.to_owned();
            let inv = i.get("inviter");
            let name = inv
                .and_then(|u| u.get("global_name").and_then(|n| n.as_str()).or_else(|| u.get("username").and_then(|n| n.as_str())))
                .unwrap_or_default()
                .to_owned();
            let avatar = inv.and_then(|u| serde_json::from_value::<phoenix_api::models::User>(u.clone()).ok()).map(|u| crate::discord_backend::convert::user(&u));
            let expires = i
                .get("expires_at")
                .and_then(|e| e.as_str())
                .and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())
                .map(|d| d.timestamp());
            Some(Invite { code: code.into(), inviter: name.into(), avatar, expires })
        })
        .collect();
    out.sort_by_key(|i| i.inviter.to_lowercase());
    out
}

pub fn countdown(expires: Option<i64>, now: i64) -> String {
    let Some(e) = expires else { return "∞".into() };
    let s = (e - now).max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

pub fn prepare(m: &mut Modal, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    match m {
        Modal::EditGroup(g) if g.name.is_none() => g.name = Some(mk(g.placeholder.clone(), &g.initial, true)),
        Modal::FolderSettings(f) if f.name.is_none() => f.name = Some(mk(crate::t!("folder.name_placeholder"), &f.initial, true)),
        _ => {}
    }
}

fn clamp(input: &Entity<InputState>, max: usize, cx: &gpui::App) -> String {
    input.read(cx).value().chars().take(max).collect()
}

impl PhoenixApp {
    pub fn open_edit_group(&mut self, channel: Id, cx: &mut Context<Self>) {
        let ex = self.nav.extras.get(&channel);
        let placeholder = self.channels.get(&channel).map(|c| c.name.clone()).unwrap_or_default();
        let initial = if ex.is_some_and(|e| e.group_named) {
            ex.and_then(|e| e.group_name.clone()).unwrap_or_else(|| placeholder.clone()).to_string()
        } else {
            String::new()
        };
        let current_icon = match ex.and_then(|e| e.group_icon.clone()) {
            Some(ImageRef::Url(u)) => Some(u),
            _ => None,
        };
        open_modal(Modal::EditGroup(EditGroup { channel, initial, placeholder, current_icon, icon: None, preview: None, name: None }), None);
        cx.notify();
    }

    pub fn pick_group_icon(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let Some((uri, png)) = cx.background_executor().spawn(async move { icon_data_uri(&std::fs::read(path).ok()?) }).await else { return };
            let _ = this.update(cx, |_, cx| {
                with_modal(|m| {
                    if let Some(Modal::EditGroup(g)) = m {
                        g.icon = Some(Some(uri));
                        g.preview = Some(Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png)));
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub fn save_edit_group(&mut self, cx: &mut Context<Self>) {
        let taken = with_modal(|m| match m.take() {
            Some(Modal::EditGroup(g)) => Some(g),
            other => {
                *m = other;
                None
            }
        });
        let Some(g) = taken else { return };
        let name = g.name.as_ref().map(|i| clamp(i, GROUP_NAME_MAX, cx)).unwrap_or_default();
        if let Some(AdminCall::EditGroupDm { name, icon, .. }) = group_patch(&g.initial, &name, &g.icon) {
            if let (Some(n), Some(ex)) = (&name, self.nav.extras.get_mut(&g.channel)) {
                ex.group_name = (!n.is_empty()).then(|| n.clone().into());
                ex.group_named = !n.is_empty();
            }
            let call = AdminCall::EditGroupDm { channel: Snowflake(g.channel.0), name, icon };
            self.action(ActionRequest::Admin(call), cx, |a, r, cx| {
                if let Err(e) = r {
                    a.parity_toast(e.0, cx);
                }
            });
        }
        cx.notify();
    }

    pub fn open_group_invites(&mut self, channel: Id, cx: &mut Context<Self>) {
        open_modal(Modal::GroupInvites(Invites { channel, list: None }), None);
        self.action(ActionRequest::Admin(AdminCall::ChannelInvites { channel: Snowflake(channel.0) }), cx, move |_, r, cx| {
            let list = match r {
                Ok(ActionReply::Json(v)) => parse_invites(&v),
                _ => Vec::new(),
            };
            with_modal(|m| {
                if let Some(Modal::GroupInvites(s)) = m
                    && s.channel == channel
                {
                    s.list = Some(list);
                }
            });
            cx.notify();
        });
        cx.notify();
    }

    pub fn revoke_group_invite(&mut self, code: SharedString, cx: &mut Context<Self>) {
        with_modal(|m| {
            if let Some(Modal::GroupInvites(s)) = m
                && let Some(l) = &mut s.list
            {
                l.retain(|i| i.code != code);
            }
        });
        self.action(ActionRequest::Admin(AdminCall::DeleteInvite { code: code.to_string() }), cx, |_, _, _| {});
        cx.notify();
    }

    pub fn open_folder_settings(&mut self, id: u64, cx: &mut Context<Self>) {
        let f = self.nav.folders.iter().find(|f| f.id == Some(id));
        let initial = f.and_then(|f| f.name.clone()).map(|n| n.to_string()).unwrap_or_default();
        let color = f.and_then(|f| f.color);
        open_modal(Modal::FolderSettings(Folder { id, initial, color, name: None }), None);
        cx.notify();
    }

    pub fn save_folder_settings(&mut self, cx: &mut Context<Self>) {
        let taken = with_modal(|m| match m.take() {
            Some(Modal::FolderSettings(f)) => Some(f),
            other => {
                *m = other;
                None
            }
        });
        let Some(f) = taken else { return };
        let name = f.name.as_ref().map(|i| clamp(i, FOLDER_NAME_MAX, cx)).unwrap_or_default();
        self.nav.folders = apply_folder(&self.nav.folders, f.id, &name, f.color);
        self.sync_guild_folders(cx);
        cx.notify();
    }

    pub fn move_in_rail(&mut self, item: RailRef, dest: Dest, cx: &mut Context<Self>) {
        let tree = crate::ui::inbox::folder_tree(self);
        if let Some(next) = move_tree(&tree, item, dest) {
            self.nav.folders = next;
            self.sync_guild_folders(cx);
        }
        cx.notify();
    }
}

pub fn apply_folder(folders: &[crate::ui::home::data::GuildFolder], id: u64, name: &str, color: Option<u32>) -> Vec<crate::ui::home::data::GuildFolder> {
    folders
        .iter()
        .cloned()
        .map(|mut f| {
            if f.id == Some(id) {
                f.name = (!name.is_empty()).then(|| name.to_owned().into());
                f.color = color.and_then(stored_color);
            }
            f
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailRef {
    Guild(Id),
    Folder(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    Folder(u64),
    NoFolder,
    First,
    Last,
}

pub fn locate(tree: &[GuildFolder], g: Id) -> Option<(usize, Option<usize>)> {
    tree.iter().enumerate().find_map(|(i, f)| {
        let at = f.guilds.iter().position(|x| *x == g)?;
        Some((i, f.id.map(|_| at)))
    })
}

pub fn placement(tree: &[GuildFolder], item: RailRef) -> Option<(bool, bool, bool)> {
    match item {
        RailRef::Folder(id) => {
            let i = tree.iter().position(|f| f.id == Some(id))?;
            Some((false, i == 0, i + 1 == tree.len()))
        }
        RailRef::Guild(g) => match locate(tree, g)? {
            (i, Some(at)) => Some((true, at == 0, at + 1 == tree[i].guilds.len())),
            (i, None) => Some((false, i == 0, i + 1 == tree.len())),
        },
    }
}

pub fn move_tree(tree: &[GuildFolder], item: RailRef, dest: Dest) -> Option<Vec<GuildFolder>> {
    let mut t = tree.to_vec();
    let lone = |g: Id| GuildFolder { id: None, name: None, color: None, guilds: vec![g] };
    match item {
        RailRef::Folder(id) => {
            let i = t.iter().position(|f| f.id == Some(id))?;
            let f = t.remove(i);
            match dest {
                Dest::First => t.insert(0, f),
                Dest::Last => t.push(f),
                _ => return None,
            }
        }
        RailRef::Guild(g) => {
            let (i, inside) = locate(&t, g)?;
            match (dest, inside) {
                (Dest::First | Dest::Last, Some(at)) => {
                    let gs = &mut t[i].guilds;
                    gs.remove(at);
                    if dest == Dest::First { gs.insert(0, g) } else { gs.push(g) }
                }
                (Dest::First | Dest::Last, None) => {
                    let f = t.remove(i);
                    if dest == Dest::First { t.insert(0, f) } else { t.push(f) }
                }
                (Dest::NoFolder, Some(at)) => {
                    t[i].guilds.remove(at);
                    t.insert(i + 1, lone(g));
                }
                (Dest::NoFolder, None) => return None,
                (Dest::Folder(fid), _) => {
                    if t[i].id == Some(fid) {
                        return None;
                    }
                    match inside {
                        Some(at) => {
                            t[i].guilds.remove(at);
                        }
                        None => t[i].guilds.clear(),
                    }
                    t.iter_mut().find(|f| f.id == Some(fid))?.guilds.push(g);
                }
            }
        }
    }
    t.retain(|f| !f.guilds.is_empty());
    Some(t)
}

fn icon_data_uri(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    use base64::Engine;
    let img = image::load_from_memory(bytes).ok()?;
    let side = img.width().min(img.height());
    let sq = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let sq = if side > 1024 { sq.resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3) } else { sq };
    let mut out = std::io::Cursor::new(Vec::new());
    sq.write_to(&mut out, image::ImageFormat::Png).ok()?;
    let png = out.into_inner();
    Some((format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png)), png))
}

fn footer_two(cancel: SharedString, ok: SharedString, enabled: bool, on_ok: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let close = close_action(cx);
    div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("gdm-cancel", cancel, BtnKind::Secondary).flex_1().on_click(move |_, w, cx| close(w, cx)))
        .child(
            button("gdm-ok", ok, BtnKind::Primary)
                .flex_1()
                .when(!enabled, |d| d.opacity(0.5).cursor_default())
                .on_click(cx.listener(move |a, _, _, cx| {
                    if enabled {
                        on_ok(a, cx)
                    }
                })),
        )
        .into_any_element()
}

pub fn render_edit_group(g: &EditGroup, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ring = c(tok(idx::BACKGROUND_SURFACE_HIGH));
    let picture: AnyElement = match (&g.preview, &g.current_icon, &g.icon) {
        (Some(p), _, Some(Some(_))) => img(p.clone()).size_full().rounded_full().into_any_element(),
        (_, Some(u), None) => img(u.clone()).size_full().rounded_full().into_any_element(),
        _ => div()
            .size_full()
            .rounded_full()
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path("members")).size(z(64.)).text_color(c(tok(idx::BACKGROUND_MOD_NORMAL))))
            .into_any_element(),
    };
    let has_icon = matches!(g.icon, Some(Some(_))) || (g.icon.is_none() && g.current_icon.is_some());
    let icon = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(12.))
        .child(
            div()
                .id("gdm-icon")
                .relative()
                .size(z(120.))
                .cursor_pointer()
                .group("gdm-icon")
                .on_click(cx.listener(|a, _, _, cx| a.pick_group_icon(cx)))
                .child(picture)
                .child(
                    div()
                        .absolute()
                        .top(z(-4.))
                        .right(z(-4.))
                        .size(z(32.))
                        .rounded_full()
                        .border(z(4.))
                        .border_color(ring)
                        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            gpui::svg()
                                .path(crate::icons::path("pen"))
                                .size(z(16.))
                                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                                .group_hover("gdm-icon", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
                        ),
                ),
        )
        .when(has_icon, |d| {
            d.child(
                div()
                    .id("gdm-remove-icon")
                    .cursor_pointer()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))
                    .hover(|s| s.underline())
                    .on_click(cx.listener(|_, _, _, cx| {
                        with_modal(|m| {
                            if let Some(Modal::EditGroup(g)) = m {
                                g.icon = Some(None);
                                g.preview = None;
                            }
                        });
                        cx.notify();
                    }))
                    .child(crate::t!("gdm.remove_icon")),
            )
        });
    let body = div().flex().flex_col().gap(z(24.)).child(icon).children(g.name.as_ref().map(input_box));
    let name = g.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let changed = group_patch(&g.initial, &name, &g.icon).is_some();
    let footer = footer_two(crate::t!("common.cancel"), crate::t!("common.save"), changed, |a, cx| a.save_edit_group(cx), cx);
    card(480., crate::t!("menu.edit_group"), None, body.into_any_element(), Some(footer), cx)
}

pub fn render_invites(s: &Invites, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let muted = c(tok(idx::TEXT_MUTED));
    let body: AnyElement = match &s.list {
        None => div().h(z(120.)).flex().items_center().justify_center().text_color(muted).child(crate::t!("common.loading")).into_any_element(),
        Some(l) if l.is_empty() => div()
            .py(z(32.))
            .flex()
            .justify_center()
            .text_size(z(16.))
            .line_height(z(20.))
            .text_color(muted)
            .child(crate::t!("gdm.no_invites"))
            .into_any_element(),
        Some(l) => {
            let now = chrono::Utc::now().timestamp();
            let head = |t: SharedString| div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(muted).child(t);
            let mut col = div().flex().flex_col().child(
                div()
                    .flex()
                    .pb(z(8.))
                    .border_b_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .child(div().flex_1().child(head(crate::t!("gdm.host"))))
                    .child(div().w(z(100.)).child(head(crate::t!("gdm.code"))))
                    .child(div().w(z(140.)).child(head(crate::t!("gdm.expires")))),
            );
            for (n, i) in l.iter().enumerate() {
                let code = i.code.clone();
                col = col.child(
                    div()
                        .id(("gdm-inv", n))
                        .flex()
                        .items_center()
                        .py(z(8.))
                        .border_b_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .child(crate::ui::avatar(i.avatar.as_ref(), 24.))
                                .child(div().min_w_0().text_ellipsis().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(i.inviter.clone())),
                        )
                        .child(div().w(z(100.)).min_w_0().text_ellipsis().font_family(crate::fonts::mono_family()).opacity(0.6).text_color(c(tok(idx::TEXT_STRONG))).child(i.code.clone()))
                        .child(
                            div()
                                .w(z(140.))
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .child(div().flex_1().flex().justify_end().font_family(crate::fonts::mono_family()).text_color(c(tok(idx::TEXT_STRONG))).child(countdown(i.expires, now)))
                                .child(
                                    div()
                                        .id(("gdm-inv-del", n))
                                        .size(z(32.))
                                        .rounded(z(8.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT)))
                                        .hover(|s| s.bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_HOVER))))
                                        .on_click(cx.listener(move |a, _, _, cx| a.revoke_group_invite(code.clone(), cx)))
                                        .child(
                                            gpui::svg()
                                                .path(crate::icons::path("chat/trash"))
                                                .size(z(16.))
                                                .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT))),
                                        ),
                                ),
                        ),
                );
            }
            col.into_any_element()
        }
    };
    let close = close_action(cx);
    let footer = button("gdm-done", crate::t!("common.done"), BtnKind::Primary).flex_1().on_click(move |_, w, cx| close(w, cx));
    card(480., crate::t!("menu.invites"), Some(crate::t!("gdm.invites_sub")), body, Some(footer.into_any_element()), cx)
}

pub fn luminance(rgb: u32) -> f32 {
    let ch = |s: u32| {
        let v = ((rgb >> s) & 0xFF) as f32 / 255.;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
}

fn swatch(id: (&'static str, usize), color: u32, size: (f32, f32), selected: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let dark = luminance(color) > 0.5;
    div()
        .id(id)
        .w(z(size.0))
        .h(z(size.1))
        .rounded(z(8.))
        .bg(c(color))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| {
            d.child(
                gpui::svg()
                    .path(crate::icons::path("check"))
                    .size(z(if size.1 > 30. { 32. } else { 16. }))
                    .text_color(if dark { gpui::black() } else { gpui::white() }),
            )
        })
        .on_click(cx.listener(move |_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::FolderSettings(f)) = m {
                    f.color = Some(color);
                }
            });
            cx.notify();
        }))
}

pub fn render_folder(f: &Folder, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let current = f.color.unwrap_or(FOLDER_DEFAULT);
    let custom = current != FOLDER_DEFAULT;
    let off_palette = custom && !FOLDER_COLORS.contains(&current);
    let big = |n: usize, color: u32, sel: bool, cx: &mut Context<PhoenixApp>| {
        div().relative().flex_1().mt(z(10.)).mr(z(10.)).min_w(z(60.)).max_w(z(70.)).child(swatch(("folder-big", n), color, (70., 50.), sel, cx).w_full())
    };
    let mut rows = div().w_full().flex_none().flex().flex_col();
    for (r, chunk) in FOLDER_COLORS.chunks(10).enumerate() {
        let mut row = div().flex().mt(z(10.)).h(z(20.));
        for (i, col) in chunk.iter().enumerate() {
            row = row.child(swatch(("folder-sw", r * 10 + i), *col, (20., 20.), current == *col, cx).mr(z(10.)));
        }
        rows = rows.child(row);
    }
    let picker = div()
        .flex()
        .flex_wrap()
        .mt(z(-10.))
        .mr(z(-10.))
        .child(big(0, FOLDER_DEFAULT, current == FOLDER_DEFAULT, cx))
        .child(if custom {
            big(1, current, off_palette, cx).child(
                div().absolute().top(z(4.)).right(z(4.)).child(
                    gpui::svg()
                        .path(crate::icons::path("pen"))
                        .size(z(14.))
                        .text_color(if luminance(current) > 0.5 { gpui::black() } else { gpui::white() }),
                ),
            )
        } else {
            div().flex_1().mt(z(10.)).mr(z(10.)).min_w(z(60.)).max_w(z(70.)).child(
                div().w_full().h(z(50.)).rounded(z(8.)).border_1().border_color(c(tok(idx::OPACITY_WHITE_8))),
            )
        })
        .child(rows);
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(div().flex().flex_col().child(field_label(crate::t!("folder.name"), false)).children(f.name.as_ref().map(input_box)))
        .child(div().flex().flex_col().pb(z(2.)).child(field_label(crate::t!("folder.color"), false)).child(picker));
    let footer = button("folder-done", crate::t!("common.done"), BtnKind::Primary).flex_1().on_click(cx.listener(|a, _, _, cx| a.save_folder_settings(cx)));
    card(480., crate::t!("menu.folder_settings"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_edit_only_sends_changes() {
        assert!(group_patch("Squad", "Squad", &None).is_none());
        match group_patch("Squad", "Crew", &None) {
            Some(AdminCall::EditGroupDm { name: Some(n), icon: None, .. }) => assert_eq!(n, "Crew"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(group_patch("", "", &Some(None)), Some(AdminCall::EditGroupDm { name: None, icon: Some(None), .. })));
    }

    #[test]
    fn folder_settings_rewrite_one_folder() {
        let folders = vec![
            GuildFolder { id: Some(5), name: Some("Old".into()), color: Some(0xE91E63), guilds: vec![Id(1), Id(2)] },
            GuildFolder { id: None, name: None, color: None, guilds: vec![Id(3)] },
        ];
        let out = apply_folder(&folders, 5, "Jeux", Some(FOLDER_DEFAULT));
        assert_eq!(out[0].name.as_deref(), Some("Jeux"));
        assert_eq!(out[0].color, None);
        assert_eq!(out[1].guilds, vec![Id(3)]);
        let out = apply_folder(&folders, 5, "", Some(0x3498DB));
        assert_eq!((out[0].name.clone(), out[0].color), (None, Some(0x3498DB)));
        let protos = crate::actions::exec::folder_protos(&out);
        assert_eq!(protos[0].guilds, vec![1, 2]);
        assert_eq!((protos[0].id, protos[0].color), (Some(5), Some(0x3498DB)));
        assert_eq!(protos[1].id, None);
    }

    #[test]
    fn invites_are_sorted_by_host_and_counted_down() {
        let v = serde_json::json!([
            { "code": "zz", "inviter": { "id": "2", "username": "zed", "global_name": null, "avatar": null, "discriminator": "0" }, "expires_at": "2026-11-26T00:00:00+00:00" },
            { "code": "aa", "inviter": { "id": "3", "username": "amy", "global_name": "Amy", "avatar": null, "discriminator": "0" }, "expires_at": null }
        ]);
        let l = parse_invites(&v);
        assert_eq!(l.iter().map(|i| i.code.to_string()).collect::<Vec<_>>(), ["aa", "zz"]);
        assert_eq!(l[0].inviter.as_ref(), "Amy");
        let exp = l[1].expires.unwrap();
        assert_eq!(countdown(Some(exp), exp - 3661), "01:01:01");
        assert_eq!(countdown(None, 0), "∞");
    }

    #[test]
    fn rail_moves_rewrite_the_folder_tree() {
        let tree = vec![
            GuildFolder { id: None, name: None, color: None, guilds: vec![Id(1)] },
            GuildFolder { id: Some(7), name: Some("F".into()), color: None, guilds: vec![Id(2), Id(3)] },
            GuildFolder { id: None, name: None, color: None, guilds: vec![Id(4)] },
        ];
        let order = |t: &[GuildFolder]| t.iter().map(|f| (f.id, f.guilds.iter().map(|g| g.0).collect::<Vec<_>>())).collect::<Vec<_>>();
        assert_eq!(placement(&tree, RailRef::Guild(Id(1))), Some((false, true, false)));
        assert_eq!(placement(&tree, RailRef::Guild(Id(3))), Some((true, false, true)));
        assert_eq!(placement(&tree, RailRef::Folder(7)), Some((false, false, false)));
        let t = move_tree(&tree, RailRef::Guild(Id(4)), Dest::Folder(7)).unwrap();
        assert_eq!(order(&t), vec![(None, vec![1]), (Some(7), vec![2, 3, 4])]);
        let t = move_tree(&tree, RailRef::Guild(Id(2)), Dest::NoFolder).unwrap();
        assert_eq!(order(&t), vec![(None, vec![1]), (Some(7), vec![3]), (None, vec![2]), (None, vec![4])]);
        let t = move_tree(&tree, RailRef::Guild(Id(3)), Dest::First).unwrap();
        assert_eq!(order(&t)[1], (Some(7), vec![3, 2]));
        let t = move_tree(&tree, RailRef::Guild(Id(1)), Dest::Last).unwrap();
        assert_eq!(order(&t).last().unwrap(), &(None, vec![1]));
        let t = move_tree(&tree, RailRef::Folder(7), Dest::First).unwrap();
        assert_eq!(order(&t)[0].0, Some(7));
        assert!(move_tree(&tree, RailRef::Guild(Id(1)), Dest::NoFolder).is_none());
        let t = move_tree(&tree, RailRef::Guild(Id(2)), Dest::Folder(7));
        assert!(t.is_none());
        let single = vec![GuildFolder { id: Some(9), name: None, color: None, guilds: vec![Id(5)] }, tree[0].clone()];
        let t = move_tree(&single, RailRef::Guild(Id(5)), Dest::NoFolder).unwrap();
        assert_eq!(order(&t), vec![(None, vec![5]), (None, vec![1])]);
    }

    #[test]
    fn check_mark_contrast_follows_the_swatch() {
        assert!(luminance(0xF1C40F) > 0.5);
        assert!(luminance(0x206694) < 0.5);
        assert!(luminance(FOLDER_DEFAULT) < 0.5);
    }
}
