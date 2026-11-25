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
    SettingsMore,
    DmMore,
    Image(SharedString),
    WishlistAdd,
    WishlistMore(Option<String>),
    AnnouncementSend { channel: Id, can_thread: bool },
    ModViewRoles { user: Id, guild: Id },
}

pub const NITRO_SKU: &str = "521847234246082599";
pub const WISHLIST_ARTICLE: &str = "4403147417623#h_01K6DY1CJBBQHBFPETCDTJHE9B";

thread_local! {
    static ANNOUNCE: RefCell<std::collections::HashMap<Id, (bool, bool)>> = RefCell::new(std::collections::HashMap::new());
    static WISHLIST_PUBLIC: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub fn announce_flags(channel: Id) -> (bool, bool) {
    ANNOUNCE.with(|a| a.borrow().get(&channel).copied().unwrap_or((true, true)))
}

fn set_announce(channel: Id, f: impl FnOnce(&mut (bool, bool))) {
    ANNOUNCE.with(|a| f(a.borrow_mut().entry(channel).or_insert((true, true))));
}

pub fn wishlist_public() -> bool {
    WISHLIST_PUBLIC.with(|w| w.get())
}

pub fn wishlist_article_url() -> String {
    format!("https://support.discord.com/hc/{}/articles/{WISHLIST_ARTICLE}", crate::i18n::locale().to_lowercase())
}

pub fn link_keys(url: &str) -> (&'static str, &'static str) {
    let (host, path) = url.split_once("://").map(|(_, r)| r.split_once('/').map_or((r, ""), |(h, p)| (h, p))).unwrap_or(("", url));
    let path = path.split(['?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    let image = ["png", "jpg", "jpeg", "webp", "avif", "bmp", "svg"].contains(&ext);
    let media = image || ["gif", "mp4", "webm", "mov"].contains(&ext);
    if host == "cdn.discordapp.com" && image {
        ("msg.copy_image_link", "msg.open_image_link")
    } else if media {
        ("viewer.copy_link", "msg.open_media_link")
    } else {
        ("menu.copy_link", "msg.open_link")
    }
}

pub fn image_items(e: &Ent, url: SharedString) -> Vec<MenuItem> {
    let (ci, si, cl) = (url.clone(), url.clone(), url.clone());
    let (ec, es) = (e.clone(), e.clone());
    let (copy_key, open_key) = link_keys(&url);
    vec![
        MenuItem::action(crate::t!("msg.copy_image"), Some(crate::icons::path("image")), move |_, _, cx: &mut App| {
            let u = ci.clone();
            ec.update(cx, |a, cx| a.copy_image(u, cx))
        }),
        MenuItem::action(crate::t!("msg.save_image"), Some(crate::icons::path("image")), move |_, _, cx: &mut App| {
            let u = si.clone();
            es.update(cx, |a, cx| a.save_image(u, cx))
        }),
        MenuItem::Separator,
        MenuItem::action(crate::t!(copy_key), Some(crate::icons::path("chat/link")), move |_, _, cx: &mut App| {
            cx.write_to_clipboard(ClipboardItem::new_string(cl.to_string()))
        }),
        MenuItem::action(crate::t!(open_key), Some(crate::icons::path("menu/open-link")), move |_, _, cx: &mut App| cx.open_url(&url)),
    ]
}

pub fn wishlist_add_items(e: &Ent) -> Vec<MenuItem> {
    vec![
        act(e, crate::t!("wishlist.discord_shop"), |a, _, cx| a.open_home_view(crate::ui::home::data::HomeView::Shop, cx)).with_icon("shop"),
        act(e, crate::t!("wishlist.nitro"), |a, _, cx| a.blocked_write(phoenix_api::rest::account::GuardedWrite::wishlist_add_sku(NITRO_SKU), cx)).with_icon("nitro"),
    ]
}

pub fn wishlist_more_items(e: &Ent, wishlist: Option<String>) -> Vec<MenuItem> {
    let ev = e.clone();
    vec![
        MenuItem::check(crate::t!("wishlist.public"), wishlist_public(), move |_, _, cx: &mut App| {
            let w = phoenix_api::rest::account::GuardedWrite::wishlist_visibility(wishlist.as_deref().unwrap_or_default(), !wishlist_public());
            ev.update(cx, |a, cx| a.blocked_write(w, cx))
        })
        .with_subtext(crate::t!("wishlist.public_sub")),
        MenuItem::Separator,
        MenuItem::action(crate::t!("wishlist.learn_more"), Some(crate::icons::path("menu/open-link")), |_, _, cx: &mut App| cx.open_url(&wishlist_article_url())),
    ]
}

pub fn announcement_items(channel: Id, can_thread: bool) -> Vec<MenuItem> {
    let (thread, publish) = announce_flags(channel);
    let thread_item = if can_thread {
        MenuItem::check(crate::t!("announce.create_thread"), thread && can_thread, move |_, w, _| {
            set_announce(channel, |f| f.0 = !f.0);
            w.refresh();
        })
    } else {
        MenuItem::disabled(crate::t!("announce.create_thread"), None)
    };
    vec![
        thread_item,
        MenuItem::check(crate::t!("announce.publish"), publish, move |_, w, _| {
            set_announce(channel, |f| f.1 = !f.1);
            w.refresh();
        }),
    ]
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
pub const SPELLCHECK_ARTICLE: &str = "360000877191";

pub fn spellcheck_article_url() -> String {
    format!("https://support.discord.com/hc/{}/articles/{SPELLCHECK_ARTICLE}", crate::i18n::locale().to_lowercase())
}

fn synced_check(app: &PhoenixApp, e: &Ent, label: SharedString, key: &'static str) -> MenuItem {
    let on = app.prefs.synced.flag(key);
    let e = e.clone();
    MenuItem::check(label, on, move |_, _, cx: &mut App| {
        e.update(cx, |a, cx| {
            let w = crate::ui::settings::synced_map::bool_wire(&a.prefs.synced.proto, key, !on);
            crate::ui::settings::save(a, crate::ui::settings::SettingsPatch::Proto(key, w), cx);
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextareaEntry {
    Suggestions,
    SendButton,
    Spellcheck,
    Languages,
    Cut,
    Copy,
    Paste,
    Separator,
}

pub fn textarea_layout(commands: bool, send_button: bool, spellcheck_on: bool, selection: bool) -> Vec<TextareaEntry> {
    use TextareaEntry as T;
    let mut out = Vec::new();
    if commands {
        out.push(T::Suggestions);
    }
    if send_button {
        out.push(T::SendButton);
    }
    out.push(T::Separator);
    out.push(T::Spellcheck);
    if spellcheck_on {
        out.push(T::Languages);
    }
    out.push(T::Separator);
    if selection {
        out.push(T::Cut);
        out.push(T::Copy);
    }
    out.push(T::Paste);
    out
}

pub fn textarea_items(app: &PhoenixApp, e: &Ent, selection: bool) -> Vec<MenuItem> {
    use TextareaEntry as T;
    let spell_on = app.prefs.local.flag(SPELLCHECK, true);
    textarea_layout(true, true, spell_on, selection)
        .into_iter()
        .map(|entry| match entry {
            T::Suggestions => MenuItem::submenu(crate::t!("ta.suggestions"), vec![synced_check(app, e, crate::t!("ta.slash_commands"), SLASH_SUGGEST)]),
            T::SendButton => pref_check(app, e, crate::t!("ta.send_button"), "send-btn", false),
            T::Spellcheck => pref_check(app, e, crate::t!("ta.spellcheck"), SPELLCHECK, true),
            T::Languages => MenuItem::action(crate::t!("ta.languages"), None, |_, _, cx: &mut App| cx.open_url(&spellcheck_article_url())),
            T::Cut => edit(e, crate::t!("ta.cut"), "Ctrl+X", || Box::new(gpui_component::input::Cut)),
            T::Copy => edit(e, crate::t!("ta.copy"), "Ctrl+C", || Box::new(gpui_component::input::Copy)),
            T::Paste => edit(e, crate::t!("ta.paste"), "Ctrl+V", || Box::new(gpui_component::input::Paste)),
            T::Separator => MenuItem::Separator,
        })
        .collect()
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

pub fn mod_view_role_items(app: &PhoenixApp, e: &Ent, user: Id, guild: Id) -> Vec<MenuItem> {
    let has: Vec<Id> = app.members.get(&guild).and_then(|m| m.iter().find(|m| m.user == user)).map(|m| m.roles.clone()).unwrap_or_default();
    let mut roles: Vec<&crate::model::Role> = app.roles.get(&guild).into_iter().flatten().filter(|r| r.id != guild && !r.managed).collect();
    roles.sort_by_key(|r| -r.position);
    roles
        .into_iter()
        .map(|r| {
            let e = e.clone();
            let (role, on) = (r.id, has.contains(&r.id));
            MenuItem::check(r.name.clone(), on, move |_, _, cx: &mut gpui::App| e.update(cx, |a, cx| a.set_member_role(guild, user, role, !on, cx)))
        })
        .collect()
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
        items.push(act(e, crate::t!(ign), move |a, _, cx| a.request_ignore(user, cx)));
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
        Target::SettingsMore => settings_more_items(&e),
        Target::DmMore => vec![act(&e, crate::t!("gdm.cleanup.menu"), |a, _, cx| a.open_gdm_cleanup(cx))],
        Target::Image(url) => image_items(&e, url.clone()),
        Target::WishlistAdd => wishlist_add_items(&e),
        Target::WishlistMore(w) => wishlist_more_items(&e, w.clone()),
        Target::AnnouncementSend { channel, can_thread } => announcement_items(*channel, *can_thread),
        Target::ModViewRoles { user, guild } => mod_view_role_items(app, &e, *user, *guild),
    };
    let dismiss: context_menu::Dismiss = Rc::new(|w, _| {
        close();
        w.refresh();
    });
    if target == Target::DmMore {
        return Some(context_menu::render_anchored(pos, gpui::Anchor::TopLeft, items, dismiss));
    }
    if matches!(target, Target::WishlistAdd | Target::WishlistMore(_)) {
        return Some(context_menu::render_anchored(pos, gpui::Anchor::TopRight, items, dismiss));
    }
    if matches!(target, Target::AnnouncementSend { .. }) {
        return Some(context_menu::render_anchored(pos, gpui::Anchor::BottomRight, items, dismiss));
    }
    if target == Target::SettingsMore {
        return Some(context_menu::render_anchored(pos, gpui::Anchor::BottomCenter, items, dismiss));
    }
    Some(context_menu::render(pos, items, dismiss))
}

pub const ACKNOWLEDGEMENTS_URL: &str = "https://discord.com/acknowledgements";
pub const SUPPORT_URL: &str = "https://support.discord.com";
pub const PRIVACY_URL: &str = "https://discord.com/privacy";
pub const TERMS_URL: &str = "https://discord.com/terms";

pub fn settings_more_items(e: &Ent) -> Vec<MenuItem> {
    let url = |label: SharedString, url: &'static str| MenuItem::action(label, None, move |_, _, cx: &mut App| cx.open_url(url));
    vec![
        act(e, crate::t!("settings.more.changelog"), |a, _, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.show_changelog = true;
            }
            crate::updater::spawn_changelog(cx);
            cx.notify();
        }),
        url(crate::t!("settings.more.acknowledgements"), ACKNOWLEDGEMENTS_URL),
        url(crate::t!("settings.more.support"), SUPPORT_URL),
    ]
}

impl PhoenixApp {
    pub fn blocked_write(&mut self, w: phoenix_api::rest::account::GuardedWrite, cx: &mut Context<Self>) {
        let fut = self.backend.guarded_write(w);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |a, cx| {
                if res.is_err() {
                    a.parity_toast(crate::t!("acct.blocked"), cx);
                }
            });
        })
        .detach();
    }

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
    fn textarea_menu_follows_the_client_order() {
        use TextareaEntry as T;
        assert_eq!(
            textarea_layout(true, true, true, true),
            vec![T::Suggestions, T::SendButton, T::Separator, T::Spellcheck, T::Languages, T::Separator, T::Cut, T::Copy, T::Paste]
        );
        assert_eq!(textarea_layout(false, false, false, false), vec![T::Separator, T::Spellcheck, T::Separator, T::Paste]);
        assert!(spellcheck_article_url().ends_with("/articles/360000877191"));
    }

    #[test]
    fn slash_suggestions_read_text_and_images_field_8() {
        let e = crate::ui::settings::synced_map::ENTRIES.iter().find(|e| e.id == SLASH_SUGGEST).expect("entry");
        assert_eq!(e.path, &[6, 8]);
        assert_eq!(e.default, 1);
    }

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
    fn image_link_labels_follow_the_client() {
        assert_eq!(link_keys("https://cdn.discordapp.com/avatars/1/a.webp?size=128"), ("msg.copy_image_link", "msg.open_image_link"));
        assert_eq!(link_keys("https://cdn.discordapp.com/avatars/1/a_b.gif?size=128"), ("viewer.copy_link", "msg.open_media_link"));
        assert_eq!(link_keys("https://media.tenor.com/x.png"), ("viewer.copy_link", "msg.open_media_link"));
        assert_eq!(link_keys("https://discord.com/x"), ("menu.copy_link", "msg.open_link"));
    }

    #[test]
    fn announcement_options_default_on_and_toggle_locally() {
        let ch = Id(42);
        assert_eq!(announce_flags(ch), (true, true));
        set_announce(ch, |f| f.1 = false);
        assert_eq!(announce_flags(ch), (true, false));
        assert_eq!(announce_flags(Id(43)), (true, true));
    }

    #[test]
    fn wishlist_writes_are_guarded() {
        let w = phoenix_api::rest::account::GuardedWrite::wishlist_add_sku(NITRO_SKU);
        assert!(w.refusal().is_write_blocked());
        assert_eq!(w.body, Some(serde_json::json!({ "sku_id": NITRO_SKU })));
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
