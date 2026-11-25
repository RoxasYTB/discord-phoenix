use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, ClipboardEntry, ClipboardItem, Context, Image, Pixels, Point, SharedString, Window};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef},
    ui::{
        context_menu::{self, MenuItem},
        home::data::{RelAction, RelationshipKind},
    },
};

type Ent = gpui::Entity<PhoenixApp>;

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Textarea,
    FriendRow(Id),
    ProfileOverflow { user: Id, guild: Option<Id> },
    Viewer,
}

thread_local! {
    static OPEN: RefCell<Option<(Target, Point<Pixels>)>> = const { RefCell::new(None) };
}

pub fn open(target: Target, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some((target, pos)));
    window.refresh();
}

pub fn opened() -> Option<Target> {
    OPEN.with(|o| o.borrow().as_ref().map(|(t, _)| t.clone()))
}

pub fn close() {
    OPEN.with(|o| *o.borrow_mut() = None);
}

fn act(e: &Ent, label: SharedString, f: impl Fn(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>) + 'static) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, w, cx: &mut App| e.update(cx, |a, cx| f(a, w, cx)))
}

fn pref_check(app: &PhoenixApp, e: &Ent, label: SharedString, key: &'static str, default: bool) -> MenuItem {
    let on = app.prefs.local.flag(key, default);
    let e = e.clone();
    MenuItem::check(label, on, move |_, _, cx: &mut App| {
        e.update(cx, |a, cx| {
            let v = !a.prefs.local.flag(key, default);
            a.prefs.local.ui.insert(key.into(), v as u64);
            a.prefs.local.save();
            cx.notify();
        })
    })
}

fn edit(e: &Ent, label: SharedString, keys: &str, action: fn() -> Box<dyn gpui::Action>) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, w, cx: &mut App| {
        let composer = e.read(cx).composer.clone();
        composer.update(cx, |s, cx| s.focus(w, cx));
        w.dispatch_action(action(), cx);
    })
    .with_shortcut(keys)
}

pub const SLASH_SUGGEST: &str = "slash-suggest";
pub const SPELLCHECK: &str = "spellcheck";

pub fn textarea_items(app: &PhoenixApp, e: &Ent, selection: bool) -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem::submenu(crate::t!("ta.suggestions"), vec![pref_check(app, e, crate::t!("ta.slash_commands"), SLASH_SUGGEST, true)]),
        pref_check(app, e, crate::t!("ta.send_button"), "send-btn", false),
        MenuItem::Separator,
        pref_check(app, e, crate::t!("ta.spellcheck"), SPELLCHECK, true),
        MenuItem::Separator,
    ];
    if selection {
        items.push(edit(e, crate::t!("ta.cut"), "Ctrl+X", || Box::new(gpui_component::input::Cut)));
        items.push(edit(e, crate::t!("ta.copy"), "Ctrl+C", || Box::new(gpui_component::input::Copy)));
    }
    items.push(edit(e, crate::t!("ta.paste"), "Ctrl+V", || Box::new(gpui_component::input::Paste)));
    items
}

pub fn friend_row_items(e: &Ent, user: Id) -> Vec<MenuItem> {
    let rm = e.clone();
    vec![
        act(e, crate::t!("friends.start_video_call"), move |a, _, cx| a.open_call_ui(user, cx)),
        act(e, crate::t!("friends.start_voice_call"), move |a, _, cx| a.open_call_ui(user, cx)),
        MenuItem::danger(crate::t!("menu.remove_friend"), None, move |_, _, cx: &mut App| {
            rm.update(cx, |a, cx| a.confirm_remove_friend(user, cx))
        }),
    ]
}

pub fn sized(url: &str, size: u32) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let mut parts: Vec<String> = query.split('&').filter(|p| !p.is_empty() && !p.starts_with("size=")).map(str::to_owned).collect();
    parts.push(format!("size={size}"));
    format!("{base}?{}", parts.join("&"))
}

pub fn profile_items(app: &PhoenixApp, e: &Ent, user: Id, guild: Option<Id>) -> Vec<MenuItem> {
    let me = user == app.me.id;
    let mut items = Vec::new();
    items.push(act(e, crate::t!("profile.view_full"), move |a, _, cx| a.open_profile_modal(user, cx)));
    let bot = app.users.get(&user).is_some_and(|u| u.bot);
    if !me && !bot {
        let invites: Vec<MenuItem> = app
            .guilds
            .iter()
            .filter(|x| Some(x.id) != guild)
            .map(|x| {
                let gid = x.id;
                act(e, x.name.clone(), move |a, _, cx| a.open_invite_people(gid, cx))
            })
            .collect();
        if !invites.is_empty() {
            items.push(MenuItem::submenu(crate::t!("menu.invite_to_server"), invites));
        }
    }
    if !me {
        items.push(MenuItem::Separator);
        let ign = if crate::actions::user::is_ignored(user) { "menu.unignore" } else { "menu.ignore" };
        items.push(act(e, crate::t!(ign), move |a, _, cx| a.toggle_ignore(user, cx)));
        let blocked = app.nav.relationship(user).is_some_and(|r| r.kind == RelationshipKind::Blocked);
        if blocked {
            items.push(act(e, crate::t!("friends.unblock"), move |a, _, cx| a.relationship_action(user, RelAction::Remove, cx)));
        } else {
            let eb = e.clone();
            items.push(MenuItem::danger(crate::t!("menu.block"), None, move |_, _, cx: &mut App| {
                eb.update(cx, |a, cx| a.confirm_block(user, cx))
            }));
        }
        let er = e.clone();
        items.push(MenuItem::danger(crate::t!("profile.report_user"), None, move |_, _, cx: &mut App| {
            er.update(cx, |a, cx| a.report_user(user, guild, cx))
        }));
    }
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        let mut copies = vec![copy(crate::t!("common.copy_user_id"), user.0.to_string())];
        copies.push(copy(crate::t!("profile.copy_profile_link"), format!("https://discord.com/users/{}", user.0)));
        if let Some(ImageRef::Url(u)) = app.users.get(&user).map(|u| &u.avatar) {
            copies.push(copy(crate::t!("profile.copy_avatar_link"), sized(u, 1024)));
        }
        if let Some(b) = app.people.profiles.get(&user).and_then(|p| p.banner.clone()) {
            copies.push(copy(crate::t!("profile.copy_banner_link"), sized(&b, 1024)));
        }
        items.push(MenuItem::submenu(crate::t!("profile.copy_user_info"), copies));
    }
    items
}

fn copy(label: SharedString, text: String) -> MenuItem {
    MenuItem::action(label, None, move |_, _, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

pub fn viewer_items(e: &Ent, v: &crate::media::ViewerImage, dev: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    let (img, link) = (v.original.clone(), v.original.clone());
    let ei = e.clone();
    items.push(MenuItem::action(crate::t!("msg.copy_image"), Some(crate::icons::path("chat/copy")), move |_, _, cx: &mut App| {
        let url = img.clone();
        ei.update(cx, |a, cx| a.copy_image(url, cx))
    }));
    let el = e.clone();
    items.push(MenuItem::action(crate::t!("viewer.copy_link"), Some(crate::icons::path("chat/link")), move |_, _, cx: &mut App| {
        cx.write_to_clipboard(ClipboardItem::new_string(link.to_string()));
        el.update(cx, |a, cx| a.parity_toast(crate::t!("viewer.link_copied"), cx))
    }));
    if let Some(att) = attachment_id(&v.original) {
        items.push(MenuItem::Separator);
        let mut details = vec![copy(crate::t!("viewer.file_name"), v.filename.to_string()).with_subtext(v.filename.clone())];
        if v.width > 0 && v.height > 0 {
            let dims = format!("{} x {}", v.width, v.height);
            details.push(copy(crate::t!("viewer.size"), dims.clone()).with_subtext(dims));
        }
        items.push(MenuItem::submenu(crate::t!("viewer.details"), details));
        if dev {
            items.push(MenuItem::Separator);
            items.push(copy(crate::t!("viewer.copy_attachment_id"), att.to_string()).with_icon("id"));
        }
    }
    items
}

pub fn attachment_id(url: &str) -> Option<u64> {
    let path = url.split('?').next()?;
    let mut it = path.split('/').skip_while(|s| *s != "attachments");
    it.next()?;
    it.next()?;
    it.next()?.parse().ok()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (target, pos) = OPEN.with(|o| o.borrow().clone())?;
    let e = cx.entity();
    let items = match &target {
        Target::Textarea => {
            let sel = !app.composer.read(cx).selected_range().is_empty();
            textarea_items(app, &e, sel)
        }
        Target::FriendRow(u) => friend_row_items(&e, *u),
        Target::ProfileOverflow { user, guild } => profile_items(app, &e, *user, *guild),
        Target::Viewer => {
            let v = cx.try_global::<crate::media::MediaState>().and_then(|m| m.viewer.clone())?;
            viewer_items(&e, &v, app.prefs.synced.developer_mode)
        }
    };
    let dismiss: context_menu::Dismiss = Rc::new(|w, _| {
        close();
        w.refresh();
    });
    Some(context_menu::render(pos, items, dismiss))
}

impl PhoenixApp {
    pub fn confirm_remove_friend(&mut self, user: Id, cx: &mut Context<Self>) {
        let name = self.users.get(&user).map(|u| u.display_name.to_string()).unwrap_or_default();
        crate::actions::open_modal(
            crate::ui::modals::Modal::Form(crate::ui::modals::Form {
                title: crate::t!("friends.remove_title", name = name.clone()),
                subtitle: Some(crate::t!("friends.remove_body", name = name)),
                fields: Vec::new(),
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!("menu.remove_friend"),
                danger: true,
                on_submit: Rc::new(move |a, _, _, cx| a.relationship_action(user, RelAction::Remove, cx)),
                preview: None,
                width: 400.,
            }),
            None,
        );
        cx.notify();
    }

    pub fn confirm_block(&mut self, user: Id, cx: &mut Context<Self>) {
        let name = self.users.get(&user).map(|u| u.display_name.to_string()).unwrap_or_default();
        crate::actions::open_modal(
            crate::ui::modals::Modal::Form(crate::ui::modals::Form {
                title: crate::t!("block.title", name = name.clone()),
                subtitle: Some(crate::t!("block.body", name = name)),
                fields: Vec::new(),
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!("menu.block"),
                danger: true,
                on_submit: Rc::new(move |a, _, _, cx| a.relationship_action(user, RelAction::Block, cx)),
                preview: None,
                width: 400.,
            }),
            None,
        );
        cx.notify();
    }

    pub fn copy_image(&mut self, url: SharedString, cx: &mut Context<Self>) {
        let http = cx.http_client();
        cx.spawn(async move |this, cx| {
            use futures::AsyncReadExt;
            let mut bytes = Vec::new();
            let ok = match http.get(&url, Default::default(), true).await {
                Ok(mut r) => r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok(),
                Err(_) => false,
            };
            let png = if ok { cx.background_executor().spawn(async move { to_png(&bytes) }).await } else { None };
            let _ = this.update(cx, |a, cx| match png {
                Some(png) => {
                    cx.write_to_clipboard(ClipboardItem { entries: vec![ClipboardEntry::Image(Image::from_bytes(gpui::ImageFormat::Png, png))] });
                    a.parity_toast(crate::t!("viewer.image_copied"), cx);
                }
                None => a.parity_toast(crate::t!("viewer.image_copy_failed"), cx),
            });
        })
        .detach();
    }

    pub fn save_image(&mut self, url: SharedString, cx: &mut Context<Self>) {
        let name = file_name(&url);
        let dir = dirs_download();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let http = cx.http_client();
        cx.spawn(async move |this, cx| {
            use futures::AsyncReadExt;
            let Ok(Ok(Some(path))) = rx.await else { return };
            let mut bytes = Vec::new();
            let ok = match http.get(&url, Default::default(), true).await {
                Ok(mut r) => r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok(),
                Err(_) => false,
            };
            let written = ok && cx.background_executor().spawn(async move { std::fs::write(path, bytes).is_ok() }).await;
            if !written {
                let _ = this.update(cx, |a, cx| a.parity_toast(crate::t!("viewer.save_failed"), cx));
            }
        })
        .detach();
    }
}

fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or("");
    if last.is_empty() { "image.png".into() } else { last.to_owned() }
}

fn dirs_download() -> std::path::PathBuf {
    crate::paths::downloads_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_links_are_copied_at_1024() {
        assert_eq!(sized("https://cdn.discordapp.com/avatars/1/a.webp?size=80", 1024), "https://cdn.discordapp.com/avatars/1/a.webp?size=1024");
        assert_eq!(sized("https://cdn.discordapp.com/avatars/1/a.png", 1024), "https://cdn.discordapp.com/avatars/1/a.png?size=1024");
        assert_eq!(
            sized("https://cdn.discordapp.com/banners/1/b.png?animated=true&size=600", 1024),
            "https://cdn.discordapp.com/banners/1/b.png?animated=true&size=1024"
        );
    }

    #[test]
    fn attachment_ids_and_names_come_from_the_cdn_path() {
        let u = "https://cdn.discordapp.com/attachments/1403447573389381652/1442000000000000000/shot.png?ex=1&is=2";
        assert_eq!(attachment_id(u), Some(1442000000000000000));
        assert_eq!(file_name(u), "shot.png");
        assert_eq!(attachment_id("https://media.tenor.com/x.gif"), None);
        assert_eq!(file_name("https://example.com/"), "image.png");
    }
}
