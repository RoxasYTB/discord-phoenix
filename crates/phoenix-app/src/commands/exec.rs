use phoenix_api::{
    RestClient,
    rest::{
        commands::{self as cmd, ApplicationCommand, CommandIndex, CommandScope, InteractionTarget},
        forum::ThreadSort,
        inbox::MentionsQuery,
    },
};
use serde_json::{Value, json};

use super::{FolderSync, ForumPost, ModAction, ParityReply, ParityRequest, SlashCommand, session_id, slash};
use crate::{
    backend::{AppEvent, BackendError, EventSink},
    discord_backend::convert::{id, message, message_users, sf},
    model::Id,
};

fn e(err: phoenix_api::Error) -> BackendError {
    BackendError(err.to_string())
}

fn target(guild: Option<Id>, channel: Id) -> InteractionTarget {
    InteractionTarget { guild: guild.map(sf), channel: sf(channel) }
}

fn ids(v: &[Id]) -> Vec<phoenix_api::models::Snowflake> {
    v.iter().copied().map(sf).collect()
}

pub fn merge_commands(indexes: &[CommandIndex]) -> Vec<SlashCommand> {
    let mut out: Vec<SlashCommand> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for idx in indexes {
        for c in idx.application_commands.iter().filter(|c| matches!(c.kind, 1..=3)) {
            if !seen.insert(c.id) {
                continue;
            }
            let app = idx.applications.iter().find(|a| a.id == c.application_id);
            let app_name = app.map(|a| a.name.clone()).unwrap_or_default();
            let app_icon = app.and_then(|a| {
                a.icon.as_ref().map(|i| format!("https://cdn.discordapp.com/app-icons/{}/{i}.png?size=64&keep_aspect_ratio=false", a.id))
            });
            let raw = serde_json::to_value(c).unwrap_or(Value::Null);
            let leaves = if c.kind == 1 { slash::leaves(&raw) } else { vec![(Vec::new(), json!({}))] };
            for (path, leaf) in leaves {
                out.push(SlashCommand {
                    name: slash::display_name(&raw, &path),
                    description: if path.is_empty() { c.description.clone() } else { slash::description(&leaf).to_owned() },
                    app_name: app_name.clone(),
                    app_icon: app_icon.clone(),
                    raw: raw.clone(),
                    id: c.id.to_string(),
                    app_id: Some(id(c.application_id)),
                    bot_id: app.and_then(|a| a.bot_user()).map(id),
                    kind: c.kind,
                    path,
                    leaf,
                    guild_scoped: c.guild_id.is_some(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_command(command: &Value) -> Result<ApplicationCommand, BackendError> {
    serde_json::from_value(command.clone()).map_err(|x| BackendError(x.to_string()))
}

fn emit_users(emit: &EventSink, msgs: &[&phoenix_api::models::Message]) {
    let mut users = Vec::new();
    for m in msgs {
        message_users(m, &mut users);
    }
    if !users.is_empty() {
        emit(AppEvent::Users(users));
    }
}

pub fn folders_json(folders: &[FolderSync]) -> Value {
    let list: Vec<Value> = folders
        .iter()
        .map(|f| {
            json!({
                "guild_ids": f.guilds.iter().map(|g| g.0.to_string()).collect::<Vec<_>>(),
                "id": f.id,
                "name": f.name,
                "color": f.color,
            })
        })
        .collect();
    json!({ "guild_folders": list })
}

pub async fn request(rest: RestClient, req: ParityRequest, emit: EventSink) -> Result<ParityReply, BackendError> {
    request_with(crate::actions::guard::writes_enabled(), rest, req, emit).await
}

pub(crate) async fn request_with(allowed: bool, rest: RestClient, req: ParityRequest, emit: EventSink) -> Result<ParityReply, BackendError> {
    if let Some(err) = crate::actions::guard::refuse_parity(allowed, &req) {
        return Err(e(err));
    }
    let session = session_id();
    Ok(match req {
        ParityRequest::Commands { guild, channel } => {
            let scope = guild.map(|g| CommandScope::Guild(sf(g))).unwrap_or(CommandScope::Channel(sf(channel)));
            let (a, b) = tokio::join!(rest.get_command_index(scope), rest.get_command_index(CommandScope::Me));
            let list: Vec<_> = [a, b].into_iter().filter_map(Result::ok).collect();
            ParityReply::Commands(merge_commands(&list))
        }
        ParityRequest::RunCommand { guild, channel, command, options, nonce } => {
            let c = parse_command(&command)?;
            rest.create_interaction(cmd::slash_body(&c, target(guild, channel), &session, options, nonce)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::Autocomplete { guild, channel, command, options, nonce } => {
            let c = parse_command(&command)?;
            rest.create_interaction(cmd::autocomplete_body(&c, target(guild, channel), &session, options, nonce)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::ContextCommand { guild, channel, command, target: t, nonce } => {
            let c = parse_command(&command)?;
            rest.create_interaction(cmd::context_body(&c, target(guild, channel), &session, sf(t), nonce)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::Component { guild, channel, message, app, flags, custom_id, kind, values } => {
            let body = cmd::component_body(
                sf(app),
                target(guild, channel),
                &session,
                sf(message),
                flags,
                &custom_id,
                kind,
                values.as_deref(),
            );
            rest.create_interaction(body).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::ModalSubmit { guild, channel, app, modal, custom_id, fields } => {
            let rows: Vec<Value> = fields
                .into_iter()
                .map(|(id, v)| json!({ "type": 1, "components": [{ "type": 4, "custom_id": id, "value": v }] }))
                .collect();
            let body = cmd::modal_body(sf(app), target(guild, channel), &session, sf(modal), &custom_id, json!(rows));
            rest.create_interaction(body).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::Mentions { before, guild, everyone, roles } => {
            let q = MentionsQuery { before: before.map(sf), guild: guild.map(sf), everyone, roles, ..Default::default() };
            let msgs = rest.get_mentions(q).await.map_err(e)?;
            emit_users(&emit, &msgs.iter().collect::<Vec<_>>());
            ParityReply::Messages(msgs.iter().map(message).collect())
        }
        ParityRequest::DismissMention(m) => {
            rest.dismiss_mention(sf(m)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::SavedMessages => {
            let s = rest.get_saved_messages().await.map_err(e)?;
            let msgs: Vec<_> = s.results.iter().filter_map(|r| r.message.as_ref()).collect();
            emit_users(&emit, &msgs);
            ParityReply::Messages(msgs.into_iter().map(message).collect())
        }
        ParityRequest::ScheduledMessages => {
            let list = rest.get_scheduled_messages().await.map_err(e)?;
            ParityReply::Messages(
                list.iter()
                    .map(|s| crate::model::Message {
                        id: id(s.id),
                        channel: id(s.channel_id()),
                        timestamp: chrono::DateTime::parse_from_rfc3339(&s.scheduled_timestamp).map_or(0, |t| t.timestamp()),
                        content: s.content().to_owned().into(),
                        flags: s.create_args.flags,
                        ..Default::default()
                    })
                    .collect(),
            )
        }
        ParityRequest::ContentInventory => {
            let list = rest.get_content_inventory().await.map_err(e)?;
            let mut out: Vec<super::InventoryEntry> = list
                .into_iter()
                .map(|x| super::InventoryEntry {
                    id: crate::model::Id(x.id),
                    author: crate::model::Id(x.author_id),
                    app: x.application_id.map(crate::model::Id),
                    game: x.game_name,
                    content_type: x.content_type,
                    first_time: x.first_time,
                    resurrected: x.resurrected,
                    trending: x.trending,
                    streak_days: x.streak_days,
                    icon: None,
                    live: x.live,
                    started_ms: x.started_at.as_deref().and_then(iso_ms),
                    expires_ms: x.expires_at.as_deref().and_then(iso_ms),
                })
                .collect();
            let mut ids: Vec<u64> = out.iter().filter_map(|x| x.app.map(|a| a.0)).collect();
            ids.sort_unstable();
            ids.dedup();
            for chunk in ids.chunks(50) {
                let q = chunk.iter().map(|i| format!("application_ids={i}")).collect::<Vec<_>>().join("&");
                let Ok(apps) = rest.request_json(reqwest::Method::GET, &format!("/applications/public?{q}"), None).await else { continue };
                for a in apps.as_array().into_iter().flatten() {
                    let id = a.get("id").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok());
                    let (Some(id), Some(icon)) = (id, a.get("icon").and_then(|v| v.as_str())) else { continue };
                    let url: gpui::SharedString = format!("https://cdn.discordapp.com/app-icons/{id}/{icon}.png?size=240&keep_aspect_ratio=false").into();
                    for x in out.iter_mut().filter(|x| x.app.map(|a| a.0) == Some(id)) {
                        x.icon = Some(url.clone());
                    }
                }
            }
            ParityReply::Inventory(out)
        }
        ParityRequest::SetSaved { channel, message: m, saved } => {
            if saved {
                rest.save_message(sf(channel), sf(m)).await
            } else {
                rest.unsave_message(sf(channel), sf(m)).await
            }
            .map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::VotePoll { channel, message: m, answers } => {
            rest.vote_poll(sf(channel), sf(m), &answers).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::CreatePoll { channel, question, answers, hours, multi } => {
            let answers: Vec<_> = answers.into_iter().map(|(t, em)| (t, em.map(|(i, n)| (i.map(sf), n)))).collect();
            let m = rest.create_poll(sf(channel), &question, &answers, hours, multi).await.map_err(e)?;
            ParityReply::Messages(vec![message(&m)])
        }
        ParityRequest::EndPoll { channel, message: m } => {
            rest.end_poll(sf(channel), sf(m)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::ForumPosts { forum, tags, offset, by_creation, archived, match_all } => {
            let sort = if by_creation { ThreadSort::Created } else { ThreadSort::LastMessage };
            let r = rest.search_threads_matching(sf(forum), sort, &ids(&tags), offset, archived, match_all).await.map_err(e)?;
            emit_users(&emit, &r.first_messages.iter().collect::<Vec<_>>());
            let posts = r
                .threads
                .iter()
                .map(|t| ForumPost {
                    thread: id(t.id),
                    title: t.name.clone().unwrap_or_default(),
                    tags: t.applied_tags.iter().copied().map(id).collect(),
                    message_count: t.message_count.unwrap_or(0),
                    first_message: r.first_messages.iter().find(|m| m.channel_id == t.id).map(message),
                    owner: t.owner_id.map(id),
                    last_message: t.last_message_id.map(id),
                    member_ids_preview: t.member_ids_preview.iter().copied().map(id).collect(),
                    member_count: t.member_count.unwrap_or(0),
                    pinned: t.flags & 2 != 0,
                    archived,
                    locked: t.thread_metadata.as_ref().is_some_and(|m| m.locked),
                })
                .collect();
            ParityReply::ForumPosts { posts, has_more: r.has_more }
        }
        ParityRequest::CreatePost { forum, title, content, tags, files } => {
            let mut uploads = Vec::with_capacity(files.len());
            for p in &files {
                let bytes = std::fs::read(p).map_err(|x| BackendError(x.to_string()))?;
                let filename = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
                uploads.push(phoenix_api::rest::FileUpload { filename, bytes, content_type: crate::ui::file_pick::accepts(p, &crate::ui::file_pick::IMAGES).then(|| crate::ui::file_pick::mime_of(p).into()), ..Default::default() });
            }
            let ch = rest.create_forum_post_with_files(sf(forum), &title, &content, &ids(&tags), uploads).await.map_err(e)?;
            ParityReply::Channel(id(ch.id))
        }
        ParityRequest::Moderate { guild, user, action } => {
            let (g, u) = (sf(guild), sf(user));
            match action {
                ModAction::Timeout(s) => rest.timeout_member(g, u, s).await,
                ModAction::Kick => rest.kick_member(g, u).await,
                ModAction::Ban(s) => rest.ban_member(g, u, s).await,
                ModAction::Unban => rest.unban_member(g, u).await,
            }
            .map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::BulkDelete { channel, messages } => {
            rest.bulk_delete_messages(sf(channel), &ids(&messages)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::CreateGroupDm(users) => {
            let ch = rest.create_group_dm(&ids(&users)).await.map_err(e)?;
            ParityReply::Channel(id(ch.id))
        }
        ParityRequest::GuildFolders(folders) => {
            rest.patch_settings_json(folders_json(&folders)).await.map_err(e)?;
            ParityReply::Done
        }
        ParityRequest::ScheduledEvents { guild } => {
            let raw = rest.scheduled_events(sf(guild)).await.map_err(e)?;
            ParityReply::Events(raw.iter().filter_map(super::parse_event).collect())
        }
        ParityRequest::FetchThread { id: t } => {
            let ch = rest.get_channel(sf(t)).await.map_err(e)?;
            if !ch.kind.is_thread() {
                return Err(BackendError("not a thread".into()));
            }
            ParityReply::Thread(crate::model::Channel {
                id: id(ch.id),
                guild: ch.guild_id.map(id),
                parent: ch.parent_id.map(id),
                name: ch.display_name().into(),
                topic: None,
                kind: crate::model::ChannelKind::Text,
                position: 0,
                unread: false,
                muted: false,
                mentions: 0,
                voice_users: Vec::new(),
            })
        }
        ParityRequest::ServerGuide { guild } => {
            let raw = rest.guild_home_settings(sf(guild)).await.map_err(e)?;
            let mut g = super::guide::parse_home_settings(&raw);
            g.profile = rest.guild_profile(sf(guild)).await.ok().map(|p| super::guide::parse_profile(&p));
            ParityReply::Guide(Box::new(g))
        }
        ParityRequest::Onboarding { guild } => {
            let raw = rest.guild_onboarding(sf(guild)).await.map_err(e)?;
            ParityReply::Onboarding(super::parse_onboarding(&raw))
        }
        ParityRequest::ArchivedThreads { channel } => {
            use phoenix_api::rest::threads::{ArchiveKind, merge_archived};
            let public = rest.archived_threads(sf(channel), ArchiveKind::Public, None).await.map_err(e)?;
            let private = rest.archived_threads(sf(channel), ArchiveKind::JoinedPrivate, None).await.unwrap_or_default();
            let has_more = public.has_more || private.has_more;
            let posts = merge_archived(public.threads, private.threads)
                .iter()
                .map(|t| ForumPost {
                    thread: id(t.id),
                    title: t.name.clone().unwrap_or_default(),
                    tags: vec![],
                    message_count: t.message_count.unwrap_or(0),
                    first_message: None,
                    owner: t.owner_id.map(id),
                    last_message: t.last_message_id.map(id),
                    member_ids_preview: t.member_ids_preview.iter().copied().map(id).collect(),
                    member_count: t.member_count.unwrap_or(0),
                    pinned: false,
                    archived: true,
                    locked: t.thread_metadata.as_ref().is_some_and(|m| m.locked),
                })
                .collect();
            ParityReply::ForumPosts { posts, has_more }
        }
    })
}

fn iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_dedups() {
        let a: CommandIndex = serde_json::from_str(
            r#"{"applications":[{"id":"9","name":"Bot"}],"application_commands":[
              {"id":"2","application_id":"9","type":1,"name":"zeta"},
              {"id":"1","application_id":"9","type":1,"name":"alpha"},
              {"id":"3","application_id":"9","type":3,"name":"Quote"}]}"#,
        )
        .unwrap();
        let m = merge_commands(&[a.clone(), a]);
        assert_eq!(m.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Quote", "alpha", "zeta"]);
        assert_eq!(m[0].app_name, "Bot");
    }

    #[test]
    fn folders_body() {
        let f = [
            FolderSync { id: None, name: None, color: None, guilds: vec![Id(1)] },
            FolderSync { id: Some(9), name: Some("Games".into()), color: Some(255), guilds: vec![Id(2), Id(3)] },
        ];
        let v = folders_json(&f);
        assert_eq!(v["guild_folders"][0]["guild_ids"], json!(["1"]));
        assert_eq!(v["guild_folders"][0]["id"], Value::Null);
        assert_eq!(v["guild_folders"][1]["name"], "Games");
        assert_eq!(v["guild_folders"][1]["guild_ids"], json!(["2", "3"]));
    }
}
