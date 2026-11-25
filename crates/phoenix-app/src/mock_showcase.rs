use crate::model::*;

pub fn messages(channel: Id, guild: Id, other_channel: Id, next_id: &mut u64, ts: i64) -> Vec<Message> {
    let mut out: Vec<(u64, Message)> = Vec::new();
    let push = |author: u64, m: Message, out: &mut Vec<(u64, Message)>| out.push((author, m));
    let text = |s: &str| Message { content: s.to_owned().into(), ..Default::default() };
    let at = |kind: AttachmentKind, name: &str, w: u32, h: u32, size: u64| Attachment {
        filename: name.to_owned().into(),
        url: media(name),
        width: w,
        height: h,
        is_image: kind == AttachmentKind::Image,
        size,
        kind,
        original: SharedString::default(),
        spoiler: false,
        description: None,
        voice: None,
    };
    let now = ts;

    push(3, text("**bold** *italic* _italic_ __underline__ ~~strike~~ ***bold italic*** __*underlined italic*__"), &mut out);
    push(3, text(&format!("||a hidden spoiler|| · [a masked link](https://example.com) · `inline code` · <t:{}:R> · <t:{}:f>", now - 3600, now)), &mut out);
    push(4, text("# Heading 1\n## Heading 2\n### Heading 3\n-# subtext under the headings"), &mut out);
    push(4, text("- first item\n- second item\n  - nested item\n1. ordered one\n2. ordered two"), &mut out);
    push(5, text("> a block quote\n> spanning two lines\nand a line after it"), &mut out);
    push(
        5,
        text("```rust\nfn main() {\n    let answer = 42; // the answer\n    println!(\"{answer}\");\n}\n```"),
        &mut out,
    );
    push(
        6,
        Message {
            content: "https://example.com/article".into(),
            embeds: vec![Embed {
                color: None,
                provider: Some("Example News".into()),
                title: Some("A link preview with a thumbnail".into()),
                description: Some("Link previews show the provider, a linked title, a short description and a small thumbnail on the right.".into()),
                url: Some("https://example.com/article".into()),
                kind: Some("article".into()),
                thumbnail: Some(EmbedMedia { url: media("thumb.png"), width: 400, height: 400 }),
                ..Default::default()
            }],
            ..Default::default()
        },
        &mut out,
    );
    push(
        7,
        Message {
            embeds: vec![Embed {
                color: Some(0x5865F2),
                title: Some("Rich embed".into()),
                description: Some("Embeds support **markdown**, [links](https://example.com) and fields.".into()),
                url: Some("https://example.com".into()),
                kind: Some("rich".into()),
                author: Some(EmbedAuthor { name: "Embed author".into(), url: Some("https://example.com".into()), icon: None }),
                fields: vec![
                    EmbedField { name: "Inline field".into(), value: "one".into(), inline: true },
                    EmbedField { name: "Inline field".into(), value: "two".into(), inline: true },
                    EmbedField { name: "Inline field".into(), value: "three".into(), inline: true },
                    EmbedField { name: "Full-width field".into(), value: "Takes a whole row of the grid.".into(), inline: false },
                ],
                footer: Some(EmbedFooter { text: "Footer text".into(), icon: None }),
                timestamp: Some(now),
                ..Default::default()
            }],
            ..Default::default()
        },
        &mut out,
    );
    push(
        6,
        Message {
            content: "https://example.com/watch?v=phoenix".into(),
            embeds: vec![Embed {
                color: Some(0xFF0000),
                provider: Some("VideoSite".into()),
                title: Some("A video embed".into()),
                url: Some("https://example.com/watch?v=phoenix".into()),
                kind: Some("video".into()),
                thumbnail: Some(EmbedMedia { url: media("video.png"), width: 1280, height: 720 }),
                video: Some(EmbedMedia { url: "https://example.invalid/embed".into(), width: 1280, height: 720 }),
                ..Default::default()
            }],
            ..Default::default()
        },
        &mut out,
    );
    let post = |img: &str, text: bool| Embed {
        color: Some(0x1D9BF0),
        provider: text.then(|| "SocialSite".into()),
        description: text.then(|| "A post with three pictures: the client merges the embeds that share a url into one.".into()),
        url: Some("https://example.com/post/1".into()),
        kind: Some("rich".into()),
        image: Some(EmbedMedia { url: media(img), width: 1280, height: 720 }),
        ..Default::default()
    };
    push(
        6,
        Message {
            content: "https://example.com/post/1".into(),
            embeds: crate::model::merge_embeds_on_url(vec![post("photo.png", true), post("video.png", false), post("thumb.png", false)]),
            ..Default::default()
        },
        &mut out,
    );
    push(3, Message { attachments: vec![at(AttachmentKind::Image, "photo.png", 1280, 720, 345_678)], ..Default::default() }, &mut out);
    push(3, Message { attachments: vec![at(AttachmentKind::Image, "SPOILER_photo.png", 1280, 720, 345_678)], ..Default::default() }, &mut out);
    push(
        3,
        Message {
            attachments: vec![
                at(AttachmentKind::Image, "one.png", 800, 600, 1),
                at(AttachmentKind::Image, "two.png", 800, 600, 1),
                at(AttachmentKind::Image, "three.png", 800, 600, 1),
            ],
            ..Default::default()
        },
        &mut out,
    );
    push(
        3,
        Message {
            attachments: ["photo.png", "one.png", "two.png", "four.png", "five.png"].iter().map(|n| at(AttachmentKind::Image, n, if *n == "five.png" { 1080 } else { 1920 }, if *n == "five.png" { 1350 } else { 1080 }, 1)).collect(),
            ..Default::default()
        },
        &mut out,
    );
    push(4, Message { attachments: vec![at(AttachmentKind::Video, "clip.mp4", 1280, 720, 9_800_000)], ..Default::default() }, &mut out);
    push(4, Message { attachments: vec![at(AttachmentKind::Video, "SPOILER_clip.mp4", 1280, 720, 9_800_000)], ..Default::default() }, &mut out);
    push(4, Message { attachments: vec![at(AttachmentKind::Audio, "voice-note.ogg", 0, 0, 482_000)], ..Default::default() }, &mut out);
    let wave: Vec<u8> = (0..64u32).map(|i| (40 + (i * 37 % 160) + if i % 7 < 3 { 50 } else { 0 }) as u8).collect();
    push(
        3,
        Message {
            attachments: vec![Attachment {
                voice: Some(crate::model::VoiceMeta { duration_secs: 7.4, waveform: crate::ui::chat::voice_message::waveform_b64(&wave).into() }),
                ..at(AttachmentKind::Audio, "voice-message.ogg", 0, 0, 61_440)
            }],
            flags: phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE,
            ..Default::default()
        },
        &mut out,
    );
    push(5, Message { attachments: vec![at(AttachmentKind::File, "phoenix-0.1.0.tar.gz", 0, 0, 4_213_337)], ..Default::default() }, &mut out);
    push(
        5,
        Message { attachments: vec![Attachment { filename: "Wumpus wave".into(), kind: AttachmentKind::Sticker(crate::media::StickerFormat::Png), ..Default::default() }], ..Default::default() },
        &mut out,
    );
    push(
        6,
        Message {
            content: "Soundboard sample: <sound:900:901>".into(),
            soundboard_sounds: vec![crate::model::SoundboardSound {
                id: Id(901),
                guild: Some(guild),
                name: Some("Synth chime".into()),
                emoji_name: Some("🎵".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
        &mut out,
    );
    push(6, Message { edited: true, content: "this message was edited".into(), ..Default::default() }, &mut out);
    push(
        6,
        Message {
            content: "a forwarded message keeps its origin".into(),
            forward: Some(Forward { guild: Some(guild), channel: other_channel, timestamp: now - 7200 }),
            ..Default::default()
        },
        &mut out,
    );
    push(7, Message { content: "".into(), ..Default::default() }, &mut out);
    let poll_id = Id(*next_id + out.len() as u64);
    crate::commands::store::insert(
        poll_id,
        crate::commands::store::Parts {
            app: Id(0),
            flags: 0,
            components: vec![],
            poll: Some(crate::commands::store::PollView {
                question: "Which theme do you use?".into(),
                answers: ["Dark", "Light", "Ash", "Onyx"]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| crate::commands::store::PollAnswer { id: i as u32 + 1, text: (*t).into(), emoji: None, count: [7, 2, 3, 1][i], me: i == 0 })
                    .collect(),
                multi: false,
                expiry: Some(chrono::Utc::now().timestamp() + 86_400),
                finalized: false,
            }),
        },
    );
    for (author, kind) in [(8, MessageKind::Join), (3, MessageKind::Boost(1)), (4, MessageKind::Pin), (5, MessageKind::ThreadCreated), (6, MessageKind::Call)] {
        let content = if kind == MessageKind::ThreadCreated { "Release planning" } else { "" };
        push(author, Message { kind, content: content.into(), ..Default::default() }, &mut out);
    }
    push(6, Message { kind: MessageKind::Call, call: Some(CallInfo { participants: vec![Id(6), Id(1)], duration_ms: Some(23 * 60_000) }), ..Default::default() }, &mut out);
    push(7, Message { kind: MessageKind::Call, call: Some(CallInfo { participants: vec![Id(7)], duration_ms: Some(12_000) }), ..Default::default() }, &mut out);
    push(5, Message { kind: MessageKind::RecipientRemove(None), ..Default::default() }, &mut out);
    push(4, Message { kind: MessageKind::ChannelNameChange, content: "release-notes".into(), ..Default::default() }, &mut out);
    push(3, Message { kind: MessageKind::Boost(3), content: "2".into(), ..Default::default() }, &mut out);
    push(2, Message { kind: MessageKind::OtherSystem, type_id: 12, content: "Phoenix #annonces".into(), ..Default::default() }, &mut out);
    push(2, Message { kind: MessageKind::OtherSystem, type_id: 16, ..Default::default() }, &mut out);
    push(6, Message { kind: MessageKind::StageStart, content: "Office hours".into(), ..Default::default() }, &mut out);
    push(7, Message { kind: MessageKind::OtherSystem, type_id: 30, ..Default::default() }, &mut out);
    let sys = |type_id: u8, data: SysData| Message { kind: MessageKind::OtherSystem, type_id, sys: Some(Box::new(data)), ..Default::default() };
    push(5, sys(25, SysData { tier_name: Some("Gold".into()), months: 3, ..Default::default() }), &mut out);
    push(6, sys(32, SysData { app_name: Some("Phoenix Bot".into()), ..Default::default() }), &mut out);
    push(6, sys(33, SysData { app_name: Some("Phoenix Bot".into()), ..Default::default() }), &mut out);
    push(6, sys(34, SysData::default()), &mut out);
    push(4, Message { content: (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339().into(), ..sys(36, SysData::default()) }, &mut out);
    push(4, sys(38, SysData::default()), &mut out);
    push(4, sys(39, SysData::default()), &mut out);
    push(5, sys(44, SysData { product: Some("Wallpaper pack".into()), ..Default::default() }), &mut out);
    push(2, sys(52, SysData::default()), &mut out);
    push(2, sys(53, SysData::default()), &mut out);
    push(3, sys(61, SysData { mention: Some(Id(7)), ..Default::default() }), &mut out);
    push(3, sys(58, SysData::default()), &mut out);
    push(3, text("<https://example.com/no-embed> · [docs **Phoenix**](https://example.com/docs) · https://en.wikipedia.org/wiki/Rust_(programming_language)"), &mut out);
    out.into_iter()
        .enumerate()
        .map(|(i, (author, m))| Message { id: Id(*next_id + 1 + i as u64), channel, author: Id(author), timestamp: ts + 60 * (i as i64 + 1), ..m })
        .collect::<Vec<_>>()
        .tap(|v| *next_id += v.len() as u64)
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }
}
impl<T> Tap for T {}

use gpui::SharedString;

fn media(name: &str) -> SharedString {
    let png = name.rsplit_once('.').map_or(name.to_owned(), |(b, _)| format!("{b}.png"));
    match std::env::var("PHX_MOCK_MEDIA_DIR").map(|d| format!("{d}/{png}")) {
        Ok(p) if std::path::Path::new(&p).exists() => format!("local/{p}").into(),
        _ => format!("https://example.invalid/{name}").into(),
    }
}
