use std::{cell::RefCell, collections::HashSet};

use gpui::{App, Context};
use phoenix_api::rest::{members::VoiceMod, reporting::ReportTarget};

use super::{ActionRequest, DEAFEN_MEMBERS, MOVE_MEMBERS, MUTE_MEMBERS, has, open_modal};
use crate::{
    app::PhoenixApp,
    discord_backend::convert::sf,
    model::{ChannelKind, Id},
    ui::{context_menu::MenuItem, modals::Modal},
};

thread_local! {
    static IGNORED: RefCell<HashSet<Id>> = RefCell::new(HashSet::new());
}

pub fn is_ignored(u: Id) -> bool {
    IGNORED.with(|s| s.borrow().contains(&u))
}

pub fn note_label(app: &PhoenixApp, u: Id) -> &'static str {
    if app.people.profiles.get(&u).is_some_and(|p| !p.note.is_empty()) { "menu.edit_note" } else { "menu.add_note" }
}

impl PhoenixApp {
    pub fn open_server_profile(&mut self, guild: Id, cx: &mut Context<Self>) {
        crate::ui::settings::open(self, cx);
        if let Some(s) = self.settings.as_mut() {
            s.page = crate::ui::settings::Page::Profiles;
        }
        crate::ui::settings::profile_editor::open_scoped(self, guild, cx);
        cx.notify();
    }

    pub fn toggle_ignore(&mut self, user: Id, cx: &mut Context<Self>) {
        let ignored = IGNORED.with(|s| {
            let mut s = s.borrow_mut();
            if s.remove(&user) {
                false
            } else {
                s.insert(user);
                true
            }
        });
        let ok = if ignored { crate::t!("ignore.done") } else { crate::t!("ignore.undone") };
        self.action_toast(ActionRequest::Ignore { user, ignored }, Some(ok), cx);
    }

    pub fn open_voice_channel(&mut self, cx: &mut Context<Self>) {
        let Some(ch) = self.voice_channel else { return };
        let guild = self.channels.get(&ch).and_then(|c| c.guild);
        if self.selected_guild != guild || self.favorites_view {
            self.select_guild(guild, cx);
        }
        self.open_channel_chat(ch, cx);
        if guild.is_some() {
            crate::ui::call_view::show(ch);
        }
        cx.notify();
    }

    pub fn open_voice_chat(&mut self, channel: Id, cx: &mut Context<Self>) {
        if self.voice_channel == Some(channel) {
            self.open_voice_channel(cx);
            crate::ui::call_view::set_chat_open(true);
            cx.notify();
        } else {
            self.open_channel_chat(channel, cx);
        }
    }

    pub fn open_call_ui(&mut self, user: Id, cx: &mut Context<Self>) {
        self.open_dm_with(user, cx);
        let ch = self.selected_channel.unwrap_or(Id(0));
        open_modal(Modal::Call { channel: ch, user }, None);
        cx.notify();
    }

    pub fn set_member_role(&mut self, guild: Id, user: Id, role: Id, add: bool, cx: &mut Context<Self>) {
        if let Some(m) = self.members.get_mut(&guild).and_then(|m| m.iter_mut().find(|m| m.user == user)) {
            if add {
                if !m.roles.contains(&role) {
                    m.roles.push(role);
                }
            } else {
                m.roles.retain(|r| *r != role);
            }
        }
        self.action_toast(ActionRequest::SetRole { guild, user, role, add }, None, cx);
        cx.notify();
    }

    pub fn remove_profile_role(&mut self, guild: Id, user: Id, role: &crate::model::Role, cx: &mut Context<Self>) {
        let current = crate::ui::profile::member_role_ids(self, guild, user);
        let remaining: Vec<Id> = current.into_iter().filter(|r| *r != role.id).collect();
        if let Some(m) = self.members.get_mut(&guild).and_then(|m| m.iter_mut().find(|m| m.user == user)) {
            m.roles.retain(|r| *r != role.id);
        }
        if let Some(p) = self.people.profiles.get_mut(&user) {
            p.roles.retain(|r| *r != role.id);
        }
        let req = if role.connection {
            ActionRequest::UnassignRoleConnection { guild, role: role.id }
        } else {
            ActionRequest::SetMemberRoles { guild, user, roles: remaining }
        };
        self.action(req, cx, |a, r, cx| {
            if r.is_err() {
                a.parity_toast(crate::t!("profile.role_update_failed"), cx);
            }
        });
        cx.notify();
    }

    pub fn open_mod_view(&mut self, guild: Id, user: Id, cx: &mut Context<Self>) {
        crate::ui::mod_view::open(self, guild, user, cx);
    }

    pub fn report_user(&mut self, user: Id, guild: Option<Id>, cx: &mut Context<Self>) {
        self.open_report(ReportTarget::User { user: sf(user), guild: guild.map(sf) }, cx);
    }

    pub fn voice_moderate(&mut self, guild: Id, user: Id, op: VoiceMod, cx: &mut Context<Self>) {
        self.action_toast(ActionRequest::VoiceMod { guild, user, op }, None, cx);
    }
}

type Ent = gpui::Entity<PhoenixApp>;

fn item(e: &Ent, label: gpui::SharedString, f: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, _, cx: &mut App| e.update(cx, |a, cx| f(a, cx)))
}

#[derive(Default)]
pub struct VoiceItems {
    pub watch: Option<MenuItem>,
    pub move_to: Option<MenuItem>,
    pub mods: Vec<MenuItem>,
}

pub fn voice_items(app: &PhoenixApp, user: Id, guild: Option<Id>, e: &Ent) -> VoiceItems {
    let mut out = VoiceItems::default();
    let Some(vc) = app.channels.values().find(|c| c.kind != ChannelKind::Category && c.guild.is_some() && c.voice_users.contains(&user)).map(|c| (c.id, c.guild)) else {
        return out;
    };
    let g = guild.or(vc.1);
    let f = crate::ui::voice_panel::flags(user);
    if f.stream && user != app.me.id {
        out.watch = Some(item(e, crate::t!("voice.watch_stream"), move |a, cx| {
            let key = crate::voice::stream::key(vc.1.map(|g| g.0), vc.0.0, user.0);
            crate::ui::voice_row::watch_from_sidebar(a, key, cx);
        }));
    }
    let Some(g) = g else { return out };
    let admin = has(app, Some(g), super::ADMINISTRATOR);
    if admin || has(app, Some(g), MUTE_MEMBERS) {
        let on = f.server_mute;
        let e = e.clone();
        out.mods.push(MenuItem::check(crate::t!("voice.server_mute"), on, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.voice_moderate(g, user, VoiceMod::Mute(!on), cx))
        }));
    }
    if admin || has(app, Some(g), DEAFEN_MEMBERS) {
        let on = f.server_deaf;
        let e = e.clone();
        out.mods.push(MenuItem::check(crate::t!("voice.server_deafen"), on, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.voice_moderate(g, user, VoiceMod::Deafen(!on), cx))
        }));
    }
    if admin || has(app, Some(g), MOVE_MEMBERS) {
        let targets: Vec<MenuItem> = app
            .guild_channels
            .get(&g)
            .into_iter()
            .flatten()
            .filter_map(|id| app.channels.get(id))
            .filter(|c| matches!(c.kind, ChannelKind::Voice | ChannelKind::Stage) && c.id != vc.0)
            .map(|c| {
                let to = crate::discord_backend::convert::sf(c.id);
                item(e, c.name.clone(), move |a, cx| a.voice_moderate(g, user, VoiceMod::Move(Some(to)), cx))
            })
            .collect();
        if !targets.is_empty() {
            out.move_to = Some(MenuItem::submenu(crate::t!("voice.move_to"), targets));
        }
        let e2 = e.clone();
        out.mods.push(MenuItem::danger(crate::t!("menu.disconnect_member"), None, move |_, _, cx: &mut App| {
            e2.update(cx, |a, cx| a.voice_moderate(g, user, VoiceMod::Move(None), cx))
        }));
    }
    out
}
