//! Plain data model used by the UI. Deliberately independent from
//! `phoenix-api` types: the adapter layer converts API objects into these.

use gpui::SharedString;

/// Discord snowflake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Id(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Presence {
    Online,
    Idle,
    Dnd,
    #[default]
    Offline,
}

/// Where an avatar/icon comes from. `Url` is resolved lazily by the image
/// element only when the row is actually visible (virtualized lists never
/// build off-screen rows), which is our lazy-loading hook.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageRef {
    /// Default avatar: one of Discord's 5 palette colours (index) + initials.
    Default(u8),
    Url(SharedString),
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: Id,
    pub name: SharedString,
    pub display_name: SharedString,
    pub avatar: ImageRef,
    pub bot: bool,
    /// `public_flags` VERIFIED_BOT (1 << 16) or a system user: the APP tag gets its check.
    pub verified_bot: bool,
    /// Server tag shown next to the name (`primary_guild.tag`) and its badge.
    pub tag: Option<(SharedString, Option<SharedString>)>,
    /// Nameplate behind the name in the DM / member list: static image URL and
    /// the palette's `darkBackground` (0xRRGGBB) for its gradient.
    pub nameplate: Option<(SharedString, Option<u32>)>,
    /// Display name style: (`font_id`, `effect_id`, colors 0xRRGGBB).
    pub name_style: Option<(u32, u32, Vec<u32>)>,
    /// Static avatar decoration image URL (`avatar_decoration_data.asset`).
    pub decoration: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub struct Guild {
    pub id: Id,
    pub name: SharedString,
    /// Initials shown when the guild has no icon ("Rust Programming" -> "RP").
    pub acronym: SharedString,
    pub icon: ImageRef,
    pub unread: bool,
    pub mentions: u32,
    pub banner_color: Option<u32>,
    pub owner: Option<Id>,
    /// Guild feature `DISCOVERABLE` ("Mode Découverte" header label + channel notice).
    pub discoverable: bool,
    /// Guild feature `COMMUNITY` (channel list unread / mention bars).
    pub community: bool,
    /// `premium_subscription_count` (boost settings stats row).
    pub boosts: u32,
    /// `premium_tier` 0..=3.
    pub boost_tier: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    Text,
    Voice,
    Announcement,
    Forum,
    Stage,
    Category,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: Id,
    pub guild: Option<Id>,
    pub parent: Option<Id>,
    pub name: SharedString,
    pub topic: Option<SharedString>,
    pub kind: ChannelKind,
    pub position: i32,
    pub unread: bool,
    pub muted: bool,
    pub mentions: u32,
    /// Users currently connected (voice channels only).
    pub voice_users: Vec<Id>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EmbedMedia {
    /// Proxy URL when available (resizable), else the original.
    pub url: SharedString,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Embed {
    pub color: Option<u32>,
    pub provider: Option<SharedString>,
    pub title: Option<SharedString>,
    pub description: Option<SharedString>,
    pub url: Option<SharedString>,
    /// "rich", "image", "gifv", "video", "article", ...
    pub kind: Option<SharedString>,
    pub thumbnail: Option<EmbedMedia>,
    pub image: Option<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub author: Option<EmbedAuthor>,
    pub fields: Vec<EmbedField>,
    pub footer: Option<EmbedFooter>,
    /// Unix seconds, shown next to the footer.
    pub timestamp: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct EmbedAuthor {
    pub name: SharedString,
    pub url: Option<SharedString>,
    pub icon: Option<SharedString>,
}

#[derive(Clone, Debug, Default)]
pub struct EmbedField {
    pub name: SharedString,
    pub value: SharedString,
    pub inline: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EmbedFooter {
    pub text: SharedString,
    pub icon: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttachmentKind {
    #[default]
    File,
    Image,
    Video,
    /// Message sticker rendered as an attachment-like image.
    Sticker(crate::media::StickerFormat),
    /// `audio/*` attachment: inline audio player card (`wrapperAudio_`).
    Audio,
}

#[derive(Clone, Debug, Default)]
pub struct Attachment {
    pub filename: SharedString,
    pub url: SharedString,
    pub width: u32,
    pub height: u32,
    pub is_image: bool,
    pub size: u64,
    pub kind: AttachmentKind,
    /// Full-resolution URL (viewer / open in browser). Empty = same as `url`.
    pub original: SharedString,
}

#[derive(Clone, Debug, Default)]
pub struct Reaction {
    pub emoji: SharedString,
    pub count: u32,
    pub me: bool,
    /// Custom (guild) emoji id; `None` for unicode emoji.
    pub emoji_id: Option<Id>,
}

/// Non-default message types rendered as system lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
    #[default]
    Default,
    Reply,
    /// Someone joined the guild.
    Join,
    /// "X pinned a message to this channel."
    Pin,
    /// Server boost, with the tier reached (0 = plain boost).
    Boost(u8),
    ThreadCreated,
    /// Added / removed group DM member (`message.mentions[0]`); `None` = unknown.
    RecipientAdd(Option<Id>),
    RecipientRemove(Option<Id>),
    Call,
    ChannelNameChange,
    /// Stage instance started / ended (content = topic), new speaker, topic change.
    StageStart,
    StageEnd,
    StageSpeaker,
    StageTopic,
    /// Any other non-user type: shown as a generic system line.
    OtherSystem,
}

/// Thread started from a message.
#[derive(Clone, Debug, Default)]
pub struct ThreadRef {
    pub id: Id,
    pub name: SharedString,
    pub message_count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Message {
    pub id: Id,
    pub channel: Id,
    pub author: Id,
    /// Unix seconds.
    pub timestamp: i64,
    pub edited: bool,
    pub content: SharedString,
    pub reply_to: Option<Id>,
    /// The reply pinged the replied-to author (`message.mentions` holds the referenced
    /// author): the client then prefixes the replied username with "@".
    pub reply_mentions_author: bool,
    /// The replied-to message as sent with the reply (`referenced_message`), used when
    /// it is not in the loaded history: (author, author nick, content).
    pub reply_preview: Option<(Id, Option<SharedString>, SharedString)>,
    /// Forwarded message (`message_snapshots`): content/attachments/embeds above are the
    /// snapshot's; this keeps where it came from.
    pub forward: Option<Forward>,
    /// Guild nickname of the author at send time (`message.member.nick`); the client shows it
    /// instead of the global display name.
    pub author_nick: Option<SharedString>,
    pub embeds: Vec<Embed>,
    pub attachments: Vec<Attachment>,
    pub soundboard_sounds: Vec<SoundboardSound>,
    pub reactions: Vec<Reaction>,
    pub kind: MessageKind,
    pub pinned: bool,
    pub mention_everyone: bool,
    pub thread: Option<ThreadRef>,
}

#[derive(Clone, Debug, Default)]
pub struct SoundboardSound {
    pub id: Id,
    pub guild: Option<Id>,
    pub name: Option<SharedString>,
    pub emoji_id: Option<Id>,
    pub emoji_name: Option<SharedString>,
}

/// Origin of a forwarded message.
#[derive(Clone, Debug, Default)]
pub struct Forward {
    pub guild: Option<Id>,
    pub channel: Id,
    /// Unix seconds of the original message.
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
pub struct Role {
    pub id: Id,
    pub name: SharedString,
    pub color: Option<u32>,
    pub position: i32,
    /// Hoisted roles get their own section in the member list.
    pub hoist: bool,
    /// Role icon image URL (`role.icon`, else the `unicode_emoji` twemoji), shown after the author name in messages.
    pub icon: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub user: Id,
    pub roles: Vec<Id>,
    pub presence: Presence,
    pub activity: Option<SharedString>,
    /// Guild nickname (`member.nick`), shown instead of the display name.
    pub nick: Option<SharedString>,
    /// Per-guild avatar URL (`member.avatar`), shown instead of the user's.
    pub avatar: Option<SharedString>,
    /// Per-guild avatar decoration URL (`member.avatar_decoration_data`).
    pub decoration: Option<SharedString>,
    /// `member.joined_at` as unix seconds (drives the "New member" badge).
    pub joined_at: Option<i64>,
    /// `member.flags` DID_REJOIN (bit 0): rejoining members never get the badge.
    pub rejoined: bool,
    /// `member.premium_since` as unix seconds: boosting the guild (booster icon).
    pub premium_since: Option<i64>,
}

pub fn acronym(name: &str) -> SharedString {
    // Client `oN`: `'s ` -> space, each `\w+` run -> its first char, drop whitespace
    // (non-ASCII letters aren't `\w`, so "La Retaudière" -> "LRèr").
    let name = name.replace("'s ", " ");
    let mut s = String::new();
    let mut in_word = false;
    for ch in name.chars() {
        let w = ch.is_ascii_alphanumeric() || ch == '_';
        if (w && !in_word) || (!w && !ch.is_whitespace()) {
            s.push(ch);
        }
        in_word = w;
    }
    s.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acronyms() {
        assert_eq!(acronym("Rust Programming Language").as_ref(), "RPL");
        assert_eq!(acronym("gpui").as_ref(), "g");
        assert_eq!(acronym("La Retaudière").as_ref(), "LRèr");
        assert_eq!(acronym("Roxas's World").as_ref(), "RW");
    }
}

/// Flags of a user's voice state, shown as icons on voice channel member rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoiceFlags {
    pub self_mute: bool,
    pub self_deaf: bool,
    pub server_mute: bool,
    pub server_deaf: bool,
    pub stream: bool,
    pub video: bool,
    /// Stage audience (VOICE_STATE `suppress`): listed in the audience row, not as a speaker.
    pub suppress: bool,
}
