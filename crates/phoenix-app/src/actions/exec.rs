use phoenix_api::{
    RestClient,
    models as api,
    rest::guild_admin,
};

use super::{ActionReply, ActionRequest, EventPlace};
use crate::{
    backend::{AppEvent, BackendError, EventSink},
    discord_backend::convert::{id, message, message_users, sf, user},
    model::Reaction,
};

fn e(err: phoenix_api::Error) -> BackendError {
    BackendError(err.to_string())
}

pub fn emoji(r: &Reaction) -> api::ReactionEmoji {
    api::ReactionEmoji { id: r.emoji_id.map(sf), name: Some(crate::ui::chat::state::reaction_name(&r.emoji)), animated: false }
}

pub fn event_place(p: &EventPlace) -> guild_admin::EventPlace {
    match p {
        EventPlace::Voice(c) => guild_admin::EventPlace::Voice(sf(*c)),
        EventPlace::Stage(c) => guild_admin::EventPlace::Stage(sf(*c)),
        EventPlace::External(s) => guild_admin::EventPlace::External(s.clone()),
    }
}

pub async fn request(rest: RestClient, req: ActionRequest, emit: EventSink) -> Result<ActionReply, BackendError> {
    use ActionRequest as R;
    Ok(match req {
        R::Admin(c) => ActionReply::Json(rest.admin(&c).await.map_err(e)?),
        R::Forward { from, msg, guild, to, comment } => {
            for ch in to {
                rest.forward_message(sf(ch), sf(from), sf(msg), guild.map(sf)).await.map_err(e)?;
                if !comment.trim().is_empty() {
                    rest.send_message(sf(ch), phoenix_api::rest::CreateMessage::text(comment.trim())).await.map_err(e)?;
                }
            }
            ActionReply::Done
        }
        R::CreateThread { channel, msg, name } => {
            let c = rest.create_thread_from_message(sf(channel), sf(msg), &name).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::CreateStandaloneThread { channel, name, private, announcement } => {
            let kind = phoenix_api::rest::thread_create::thread_type(private, announcement);
            let c = rest.create_standalone_thread(sf(channel), &name, kind, 4320).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::Remind { channel, msg, due_unix } => {
            rest.set_reminder(sf(channel), sf(msg), due_unix).await.map_err(e)?;
            ActionReply::Done
        }
        R::Reminders => {
            let list = rest.get_reminders().await.map_err(e)?;
            let msgs: Vec<&api::Message> = list.iter().filter_map(|r| r.message.as_ref()).collect();
            let mut users = Vec::new();
            for m in &msgs {
                message_users(m, &mut users);
            }
            if !users.is_empty() {
                emit(AppEvent::Users(users));
            }
            ActionReply::Messages(msgs.into_iter().map(message).collect())
        }
        R::Reactors { channel, msg, emoji: r } => {
            let us = rest.get_reactions(sf(channel), sf(msg), &emoji(&r), 100).await.map_err(e)?;
            ActionReply::Users(us.iter().map(user).collect())
        }
        R::RemoveReactions { channel, msg, emoji: r } => {
            rest.remove_reactions(sf(channel), sf(msg), r.as_ref().map(emoji).as_ref()).await.map_err(e)?;
            ActionReply::Done
        }
        R::ReportMenu(t) => ActionReply::ReportMenu(Box::new(rest.get_report_menu(t).await.map_err(e)?)),
        R::SubmitReport { menu, target, breadcrumbs } => {
            rest.submit_report(&menu, target, &breadcrumbs).await.map_err(e)?;
            ActionReply::Done
        }
        R::SetRole { guild, user: u, role, add } => {
            if add {
                rest.add_member_role(sf(guild), sf(u), sf(role)).await
            } else {
                rest.remove_member_role(sf(guild), sf(u), sf(role)).await
            }
            .map_err(e)?;
            ActionReply::Done
        }
        R::SetNick { guild, user: u, nick } => {
            rest.set_member_nick(sf(guild), u.map(sf), &nick).await.map_err(e)?;
            ActionReply::Done
        }
        R::FriendNick { user: u, nick } => {
            rest.set_friend_nickname(sf(u), &nick).await.map_err(e)?;
            ActionReply::Done
        }
        R::Ignore { user: u, ignored } => {
            rest.set_ignored(sf(u), ignored).await.map_err(e)?;
            ActionReply::Done
        }
        R::VoiceMod { guild, user: u, op } => {
            rest.voice_moderate(sf(guild), sf(u), op).await.map_err(e)?;
            ActionReply::Done
        }
        R::CloneChannel { guild, channel, name } => {
            let src = rest.get_channel(sf(channel)).await.map_err(e)?;
            let c = rest.clone_channel(sf(guild), &src, &name).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::MoveChannels { guild, entries } => {
            let v = entries.into_iter().map(|(c, pos, parent)| guild_admin::move_entry(sf(c), pos, parent.map(|p| p.map(sf)))).collect();
            rest.move_channels(sf(guild), v).await.map_err(e)?;
            ActionReply::Done
        }
        R::CreateEvent { guild, name, description, start_unix, end_unix, place } => {
            let body = guild_admin::event_body(&name, &description, start_unix, end_unix, &event_place(&place));
            rest.create_scheduled_event(sf(guild), body).await.map_err(e)?;
            ActionReply::Done
        }
    })
}
