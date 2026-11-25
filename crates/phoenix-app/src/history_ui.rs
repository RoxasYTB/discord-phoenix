use gpui::{Context, ListOffset};

use crate::{
    app::PhoenixApp,
    model::{Id, Message},
};

pub fn on_refresh(app: &mut PhoenixApp, channel: Id, messages: Vec<Message>, cx: &mut Context<PhoenixApp>) {
    if !app.messages.get(&channel).is_some_and(|e| e.loaded_once && !e.loading && e.msgs.len() <= 50) {
        return;
    }
    let anchor = (app.selected_channel == Some(channel) && !app.chat.at_bottom).then(|| {
        let top = app.message_list.logical_scroll_top();
        let id = top.item_ix.checked_sub(1).and_then(|i| app.messages.get(&channel)?.msgs.get(i).map(|m| m.id));
        (id, top.offset_in_item)
    });
    app.messages.remove(&channel);
    app.insert_history(channel, messages, false, cx);
    if let Some((Some(id), offset_in_item)) = anchor
        && let Some(ix) = app.messages.get(&channel).and_then(|e| e.index_of(id))
    {
        app.message_list.scroll_to(ListOffset { item_ix: ix + 1, offset_in_item });
    }
}

use std::collections::{HashMap, HashSet};

use crate::{grouping, markdown::{MentionResolver, SoundboardMention}, richtext::{self, RenderedMessage}};

#[derive(Default)]
pub struct PageResolver {
    users: HashMap<u64, String>,
    channels: HashMap<u64, String>,
    roles: HashMap<u64, (String, Option<u32>, bool)>,
    sounds: HashMap<(u64, u64), SoundboardMention>,
}

impl MentionResolver for PageResolver {
    fn user(&self, id: u64) -> Option<String> {
        self.users.get(&id).cloned()
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.channels.get(&id).cloned()
    }
    fn role(&self, id: u64) -> Option<String> {
        self.roles.get(&id).map(|r| r.0.clone())
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.roles.get(&id).and_then(|r| r.1)
    }
    fn has_role(&self, id: u64) -> bool {
        self.roles.get(&id).is_some_and(|r| r.2)
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
        self.sounds.get(&(guild_id, sound_id)).or_else(|| self.sounds.get(&(0, sound_id))).cloned()
    }
}

fn mention_ids(text: &str, users: &mut HashSet<u64>, roles: &mut HashSet<u64>, channels: &mut HashSet<u64>) {
    for (i, _) in text.match_indices('<') {
        let rest = &text[i + 1..];
        let (set, body) = if let Some(b) = rest.strip_prefix("@&") {
            (&mut *roles, b)
        } else if let Some(b) = rest.strip_prefix("@!") {
            (&mut *users, b)
        } else if let Some(b) = rest.strip_prefix('@') {
            (&mut *users, b)
        } else if let Some(b) = rest.strip_prefix('#') {
            (&mut *channels, b)
        } else {
            continue;
        };
        let digits: &str = &body[..body.find(|c: char| !c.is_ascii_digit()).unwrap_or(body.len())];
        if body[digits.len()..].starts_with('>')
            && let Ok(id) = digits.parse()
        {
            set.insert(id);
        }
    }
}

pub fn page_resolver(app: &PhoenixApp, msgs: &[Message]) -> PageResolver {
    let (mut u, mut r, mut c) = (HashSet::new(), HashSet::new(), HashSet::new());
    for m in msgs {
        mention_ids(&m.content, &mut u, &mut r, &mut c);
    }
    let mut out = PageResolver::default();
    for id in u {
        if let Some(user) = app.users.get(&Id(id)) {
            out.users.insert(id, user.display_name.to_string());
        }
    }
    for id in c {
        if let Some(ch) = app.channels.get(&Id(id)) {
            out.channels.insert(id, ch.name.to_string());
        }
    }
    for id in r {
        let Some((g, role)) = app.roles.iter().find_map(|(g, rs)| rs.iter().find(|x| x.id.0 == id).map(|x| (*g, x))) else {
            continue;
        };
        let mine = app
            .members
            .get(&g)
            .and_then(|ms| ms.iter().find(|m| m.user == app.me.id))
            .is_some_and(|m| m.roles.contains(&Id(id)));
        out.roles.insert(id, (role.name.to_string(), role.color, mine));
    }
    out
}

struct MessagePageResolver<'a> {
    base: &'a PageResolver,
    sounds: &'a [crate::model::SoundboardSound],
}

impl MentionResolver for MessagePageResolver<'_> {
    fn user(&self, id: u64) -> Option<String> {
        self.base.user(id)
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.base.channel(id)
    }
    fn role(&self, id: u64) -> Option<String> {
        self.base.role(id)
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.base.role_color(id)
    }
    fn has_role(&self, id: u64) -> bool {
        self.base.has_role(id)
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
        self.sounds
            .iter()
            .find(|s| s.id.0 == sound_id && (guild_id == 0 || s.guild.is_none_or(|g| g.0 == guild_id)))
            .map(|sound| SoundboardMention {
                name: sound.name.as_ref().map(ToString::to_string).filter(|n| !n.is_empty()).unwrap_or_else(|| "Unknown".into()),
                emoji_name: sound.emoji_name.as_ref().map(ToString::to_string),
            })
            .or_else(|| self.base.soundboard(guild_id, sound_id))
    }
}

pub fn render_page(
    msgs: &[Message],
    resolver: &PageResolver,
    me: Id,
    utc_offset: i64,
    revealed: &HashSet<(Id, usize)>,
) -> Vec<RenderedMessage> {
    (0..msgs.len())
        .map(|i| {
            let (m, prev) = (&msgs[i], i.checked_sub(1).map(|p| &msgs[p]));
            let message_resolver = MessagePageResolver {
                base: resolver,
                sounds: &m.soundboard_sounds,
            };
            let rev = |s: usize| revealed.contains(&(m.id, s));
            let (parsed, blocks, mentions_me, preview, preview_mentions) = richtext::build(&m.content, &message_resolver, me, &rev);
            RenderedMessage {
                group_start: grouping::starts_group(prev, m, utc_offset),
                day_divider: (prev.is_none() || grouping::day_divider(prev, m, utc_offset))
                    .then(|| crate::app::format_day(m.timestamp)),
                time_full: crate::app::format_time_full(m.timestamp),
                time_short: crate::app::format_time_short(m.timestamp),
                blocks,
                mentions_me,
                preview,
                preview_mentions,
                parsed,
            }
        })
        .collect()
}

pub fn load_first_page(app: &PhoenixApp, id: Id, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.load_messages(id, None, 50);
    cx.spawn(async move |this, cx| {
        let msgs = fut.await.unwrap_or_default();
        let Ok((resolver, me, off, revealed)) = this.update(cx, |a, _| {
            let rev: HashSet<(Id, usize)> = a.revealed.iter().filter(|(m, _)| msgs.iter().any(|x| x.id == *m)).copied().collect();
            (page_resolver(a, &msgs), a.me.id, a.utc_offset, rev)
        }) else {
            return;
        };
        let t0 = std::time::Instant::now();
        let on_screen = this.update(cx, |a, _| a.selected_channel == Some(id)).unwrap_or(false);
        if cfg!(test) || on_screen {
            let rendered = render_page(&msgs, &resolver, me, off, &revealed);
            let _ = this.update(cx, |a, cx| {
                insert_first_page(a, id, msgs, rendered, cx);
                crate::commands::state::apply_read_mark(a, id);
            });
            return;
        }
        let (tx, rx) = futures::channel::oneshot::channel();
        let job = move || {
            let r = render_page(&msgs, &resolver, me, off, &revealed);
            let _ = tx.send((msgs, r));
        };
        if std::thread::Builder::new().name("phoenix-layout".into()).spawn(job).is_err() {
            return;
        }
        let Ok((msgs, rendered)) = rx.await else { return };
        crate::perf::span("page_layout_bg", t0);
        let _ = this.update(cx, |a, cx| {
            insert_first_page(a, id, msgs, rendered, cx);
            crate::commands::state::apply_read_mark(a, id);
        });
    })
    .detach();
}

fn insert_first_page(app: &mut PhoenixApp, ch: Id, page: Vec<Message>, rendered: Vec<RenderedMessage>, cx: &mut Context<PhoenixApp>) {
    crate::perf::channel_messages_ready(ch.0, "load");
    let mut entry = app.messages.remove(&ch).unwrap_or_default();
    entry.loading = false;
    entry.loaded_once = true;
    entry.reached_start = page.len() < 50;
    entry.msgs = page;
    entry.rendered = rendered;
    let len = entry.msgs.len();
    app.messages.insert(ch, entry);
    if app.selected_channel == Some(ch) {
        app.message_list.reset(len + 1);
    }
    cx.notify();
}
