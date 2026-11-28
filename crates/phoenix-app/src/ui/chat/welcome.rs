use std::{path::PathBuf, sync::LazyLock};
use std::time::{Duration, SystemTime};

use gpui::{
    Div, Entity, FontWeight, InteractiveText, IntoElement, ParentElement, SharedString, Styled, StyledText, div,
    prelude::*,
};

use crate::{
    app::PhoenixApp,
    assets::shortcut_icon,
    model::{ChannelKind, Guild, Id, ImageRef, Message, MessageKind, SendState},
    theme::{generated::idx, z},
    ui::tooltip::Tipped as _,
};

use super::tk;

fn step_icon(name: &'static str) -> Div {
    const ART: [(&str, &str); 5] = [
        ("welcome-invite", "https://discord.com/assets/ea08bfae3e0ab96d.svg"),
        ("welcome-icon", "https://discord.com/assets/428a003b3c729aa6.svg"),
        ("welcome-message", "https://discord.com/assets/2ed198e767bd5423.svg"),
        ("welcome-app", "https://discord.com/assets/261f952bf028fa34.svg"),
        ("welcome-boost", "https://cdn.discordapp.com/assets/content/bc3217e772906510d881b75ebefea754b9c3ba903ddf6f994e46e5c5a85770a3.svg"),
    ];
    static LOCAL_ART: LazyLock<[Option<PathBuf>; ART.len()]> = LazyLock::new(|| {
        let directory = crate::icons::out_dir().map(|dir| dir.join("welcome"));
        ART.map(|(name, _)| {
            let path = directory.as_ref()?.join(format!("{name}.svg"));
            path.is_file().then_some(path)
        })
    });

    let Some(index) = ART.iter().position(|(entry, _)| *entry == name) else {
        return shortcut_icon(name, 24.);
    };
    let source = LOCAL_ART[index].clone().map_or_else(|| ART[index].1.into(), gpui::ImageSource::from);
    div().size(z(24.)).flex_none().child(
        gpui::img(source)
            .size_full()
            .object_fit(gpui::ObjectFit::Contain)
            .with_loading(move || shortcut_icon(name, 24.).into_any_element())
            .with_fallback(move || shortcut_icon(name, 24.).into_any_element()),
    )
}

fn channel_key(guild: Id) -> String {
    format!("server-welcome-channel-{}", guild.0)
}

fn message_key(guild: Id) -> String {
    format!("server-welcome-message-{}", guild.0)
}

fn invite_key(guild: Id) -> String {
    format!("server-welcome-invite-sent-{}", guild.0)
}

fn primary_channel(app: &PhoenixApp, guild: &Guild) -> Option<Id> {
    let candidates = app
        .guild_channels
        .get(&guild.id)?
        .iter()
        .copied()
        .filter_map(|id| {
            let channel = app.channels.get(&id)?;
            (channel.kind == ChannelKind::Text
                && app.nav.thread_parent(id).is_none()
                && app.backend.thread_info(id).is_none())
            .then_some(channel)
        });
    let saved = app
        .prefs
        .local
        .ui
        .get(&channel_key(guild.id))
        .copied()
        .map(Id);
    candidates
        .clone()
        .find(|ch| Some(ch.id) == saved)
        .or_else(|| {
            candidates
                .clone()
                .find(|ch| matches!(ch.name.to_lowercase().as_str(), "general" | "général"))
        })
        .or_else(|| {
            candidates
                .clone()
                .find(|ch| Some(ch.id) == guild.system_channel)
        })
        .or_else(|| candidates.clone().next())
        .map(|ch| ch.id)
}

fn owner_guild(app: &PhoenixApp, channel: Id) -> Option<&Guild> {
    let guild = app.channels.get(&channel)?.guild?;
    let guild = app
        .guilds
        .iter()
        .find(|g| g.id == guild && g.owner == Some(app.me.id))?;
    (primary_channel(app, guild) == Some(channel)).then_some(guild)
}

fn save_progress(app: &mut PhoenixApp, entries: impl IntoIterator<Item = (String, u64)>) {
    let mut changed = false;
    for (key, value) in entries {
        changed |= app.prefs.local.ui.insert(key, value) != Some(value);
    }
    if changed && (crate::paths::is_sandboxed() || !crate::actions::guard::agent_session()) {
        app.prefs.local.save_blocking();
    }
}

fn recently_created(guild: Id, now: SystemTime) -> bool {
    const RECOVERY_WINDOW: Duration = Duration::from_secs(30 * 24 * 60 * 60);
    let created = phoenix_api::models::Snowflake(guild.0).created_at();
    now.duration_since(created)
        .is_ok_and(|age| age <= RECOVERY_WINDOW)
}

pub fn record_creation(app: &mut PhoenixApp, guild: Id) {
    let key = channel_key(guild);
    if !app.prefs.local.ui.contains_key(&key) {
        save_progress(app, [(key, 0)]);
    }
}

fn own_message(app: &PhoenixApp, message: &Message) -> bool {
    message.author == app.me.id
        && message.send_state == SendState::Sent
        && matches!(message.kind, MessageKind::Default | MessageKind::Reply)
}

pub fn observe_history(app: &mut PhoenixApp, channel: Id, messages: &[Message]) {
    let Some(guild) = owner_guild(app, channel).map(|g| g.id) else {
        return;
    };
    let key = channel_key(guild);
    if !app.prefs.local.ui.contains_key(&key)
        && !messages.is_empty()
        && !recently_created(guild, SystemTime::now())
    {
        return;
    }
    let sent = messages.iter().any(|message| own_message(app, message));
    save_progress(
        app,
        std::iter::once((key, channel.0)).chain(sent.then(|| (message_key(guild), 1))),
    );
}

pub fn record_message(app: &mut PhoenixApp, message: &Message) {
    if !own_message(app, message) {
        return;
    }
    if let Some(guild) = owner_guild(app, message.channel).map(|g| g.id)
        && app.prefs.local.ui.contains_key(&channel_key(guild))
    {
        save_progress(app, [(message_key(guild), 1)]);
    }
}

pub fn record_invite(app: &mut PhoenixApp, guild: Id) {
    if app.prefs.local.ui.contains_key(&channel_key(guild)) {
        save_progress(app, [(invite_key(guild), 1)]);
    }
}

pub fn render(app: &PhoenixApp, entity: &Entity<PhoenixApp>, ch: Id) -> Option<Div> {
    let guild = owner_guild(app, ch)?;
    let g = guild.id;
    if !app.prefs.local.ui.contains_key(&channel_key(g)) {
        return None;
    }
    let people = app
        .members
        .get(&g)
        .into_iter()
        .flatten()
        .filter_map(|member| {
            (member.user != app.me.id)
                .then(|| app.users.get(&member.user))
                .flatten()
        });
    let invited =
        app.prefs.local.flag(&invite_key(g), false) || people.clone().any(|user| !user.bot);
    let icon_set = matches!(guild.icon, ImageRef::Url(_));
    let sent = app.prefs.local.flag(&message_key(g), false)
        || app
            .messages
            .get(&ch)
            .is_some_and(|entry| entry.msgs.iter().any(|message| own_message(app, message)));
    let app_added = app.backend.guild_has_application(g) || people.clone().any(|user| user.bot);
    let boosted = guild.boosts > 0
        || app
            .backend
            .guild_boosts(g)
            .is_some_and(|(count, _)| count > 0);
    if [invited, icon_set, sent, app_added, boosted].into_iter().all(|done| done) {
        return None;
    }
    let step = |id: &'static str, asset: &'static str, label: SharedString, done: bool| {
        div()
            .id(id)
            .w_full()
            .min_h(z(56.))
            .px(z(16.))
            .py(z(12.))
            .flex()
            .items_center()
            .gap(z(12.))
            .rounded(z(8.))
            .border_1()
            .border_color(tk(idx::BORDER_SUBTLE))
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .when(!done, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
            })
            .child(step_icon(asset).when(done, |d| d.opacity(0.65)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tk(if done {
                        idx::TEXT_MUTED
                    } else {
                        idx::TEXT_STRONG
                    }))
                    .child(label),
            )
            .when(done, |d| {
                d.tip(crate::t!("chat.new_server_complete")).child(
                    div()
                        .size(z(24.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(tk(idx::BACKGROUND_FEEDBACK_MOD_POSITIVE))
                        .child(
                            gpui::svg()
                                .path(crate::icons::path("check"))
                                .size(z(16.))
                                .text_color(tk(idx::ICON_FEEDBACK_POSITIVE)),
                        ),
                )
            })
            .when(!done, |d| {
                d.child(
                    gpui::svg()
                        .path(crate::icons::path("chevron-right"))
                        .size(z(24.))
                        .flex_none()
                        .text_color(tk(idx::ICON_MUTED)),
                )
            })
    };
    let (e1, e2, e3, e4) = (
        entity.clone(),
        entity.clone(),
        entity.clone(),
        entity.clone(),
    );
    let desc = crate::t!("chat.new_server_desc");
    let guide = crate::t!("chat.new_server_guide");
    let full: SharedString = format!(
        "{desc} {}",
        crate::t!("chat.new_server_guide_sentence", guide = guide.clone())
    )
    .into();
    let link = full.find(&*guide).map(|i| i..i + guide.len());
    let styled = StyledText::new(full.clone()).with_highlights(link.clone().map(|r| {
        (
            r,
            gpui::HighlightStyle {
                color: Some(tk(idx::TEXT_LINK)),
                ..Default::default()
            },
        )
    }));
    let desc_el = InteractiveText::new("nsw-guide", styled)
        .on_click(link.into_iter().collect(), |_, _, cx| {
            cx.open_url("https://support.discord.com/hc/articles/360045138571")
        });
    Some(
        div()
            .w_full()
            .pt(z(32.))
            .pb(z(16.))
            .flex()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .max_w(z(440.))
                    .px(z(16.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(z(28.))
                            .line_height(z(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tk(idx::TEXT_STRONG))
                            .text_center()
                            .child(crate::t!("chat.new_server_title")),
                    )
                    .child(
                        div()
                            .text_size(z(28.))
                            .line_height(z(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tk(idx::TEXT_STRONG))
                            .text_center()
                            .child(guild.name.clone()),
                    )
                    .child(
                        div()
                            .mt(z(8.))
                            .mb(z(24.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(tk(idx::TEXT_DEFAULT))
                            .text_center()
                            .child(desc_el),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(z(8.))
                            .child(
                                step(
                                    "nsw-invite",
                                    "welcome-invite",
                                    crate::t!("chat.new_server_invite"),
                                    invited,
                                )
                                .when(!invited, |d| {
                                    d.on_click(move |_, _, cx| {
                                        e1.update(cx, |a, cx| a.open_invite_people(g, cx))
                                    })
                                }),
                            )
                            .child(
                                step(
                                    "nsw-icon",
                                    "welcome-icon",
                                    crate::t!("chat.new_server_icon"),
                                    icon_set,
                                )
                                .when(!icon_set, |d| {
                                    d.on_click(move |_, w, cx| {
                                        e2.update(cx, |a, cx| {
                                            a.open_server_settings(g, 0, cx);
                                            w.refresh();
                                        })
                                    })
                                }),
                            )
                            .child(
                                step(
                                    "nsw-message",
                                    "welcome-message",
                                    crate::t!("chat.new_server_message"),
                                    sent,
                                )
                                .when(!sent, |d| {
                                    d.on_click(move |_, w, cx| {
                                        e3.update(cx, |a, cx| {
                                            a.composer.update(cx, |s, cx| s.focus(w, cx));
                                        })
                                    })
                                }),
                            )
                            .child(
                                step(
                                    "nsw-app",
                                    "welcome-app",
                                    crate::t!("chat.new_server_app"),
                                    app_added,
                                )
                                .when(!app_added, |d| {
                                    d.on_click(move |_, _, cx| {
                                        e4.update(cx, |_, cx| {
                                            crate::ui::modals::server_settings::open_app_directory(
                                                cx,
                                            );
                                        })
                                    })
                                }),
                            )
                            .child(
                                step(
                                    "nsw-boost",
                                    "welcome-boost",
                                    crate::t!("chat.new_server_boost"),
                                    boosted,
                                )
                                .when(!boosted, |d| {
                                    d.on_click(move |_, w, _| {
                                        crate::ui::boost_layer::open(g);
                                        w.refresh();
                                    })
                                }),
                            ),
                    ),
            ),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui::TestAppContext;

    use super::*;
    use crate::{
        backend::{Backend, Credentials},
        mock::MockBackend,
    };

    #[gpui::test]
    fn welcome_recovers_recent_server_with_a_message(cx: &mut TestAppContext) {
        let mock = MockBackend::synchronous();
        let mut ready =
            futures::executor::block_on(mock.login(Credentials::Token("test".into()))).unwrap();
        let old_guild = ready.guilds[0].id;
        let guild = Id(phoenix_api::models::Snowflake::now().0);
        ready.guilds[0].id = guild;
        ready.guilds[0].owner = Some(ready.me.id);
        for channel in &mut ready.channels {
            if channel.guild == Some(old_guild) {
                channel.guild = Some(guild);
            }
        }
        let general = ready
            .channels
            .iter()
            .find(|ch| ch.guild == Some(guild) && ch.name == "general")
            .unwrap()
            .id;
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(Arc::new(mock), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let message = Message {
                    channel: general,
                    author: app.me.id,
                    ..Default::default()
                };
                observe_history(app, general, &[message]);
                assert!(app.prefs.local.flag(&message_key(guild), false));
                assert!(render(app, &cx.entity(), general).is_some());

                let saved = crate::ui::settings::LocalPrefs::load();
                assert_eq!(saved.ui.get(&channel_key(guild)), Some(&general.0));
                assert!(saved.flag(&message_key(guild), false));
            })
            .unwrap();
    }

    #[gpui::test]
    fn welcome_stays_in_general_after_sending(cx: &mut TestAppContext) {
        let mock = MockBackend::synchronous();
        let mut ready =
            futures::executor::block_on(mock.login(Credentials::Token("test".into()))).unwrap();
        let guild = ready.guilds[0].id;
        ready.guilds[0].owner = Some(ready.me.id);
        let general = ready
            .channels
            .iter()
            .find(|ch| ch.guild == Some(guild) && ch.name == "general")
            .unwrap()
            .id;
        let other = ready
            .channels
            .iter()
            .find(|ch| ch.guild == Some(guild) && ch.name == "welcome")
            .unwrap()
            .id;
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(Arc::new(mock), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let entity = cx.entity();

                observe_history(app, other, &[]);
                assert!(render(app, &entity, other).is_none());
                assert!(render(app, &entity, general).is_none());

                observe_history(app, general, &[]);
                assert!(render(app, &entity, general).is_some());
                assert!(!app.prefs.local.flag(&message_key(guild), false));

                let mut message = Message {
                    channel: general,
                    author: app.me.id,
                    send_state: SendState::Sending,
                    ..Default::default()
                };
                record_message(app, &message);
                assert!(!app.prefs.local.flag(&message_key(guild), false));

                message.send_state = SendState::Sent;
                record_message(app, &message);
                assert!(app.prefs.local.flag(&message_key(guild), false));
                assert!(render(app, &entity, general).is_some());
                assert!(render(app, &entity, other).is_none());
            })
            .unwrap();
    }
}
