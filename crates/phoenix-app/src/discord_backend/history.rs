use std::{
    sync::{Arc, LazyLock},
    time::Instant,
};

use phoenix_api::{RestClient, models as api, rest::MessageQuery};
use tokio::sync::Semaphore;

use super::{DiscordBackend, Inner, Session, api_err, convert, convert::sf, msg_cache};
use crate::{
    backend::{AppEvent, BackendError, BoxFuture},
    model::{Id, Message},
};

const PAGE: u32 = 50;
static BG: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

async fn fetch(rest: &RestClient, channel: Id, before: Option<Id>, limit: u32) -> Result<Vec<api::Message>, BackendError> {
    let t0 = Instant::now();
    let limit = limit.clamp(1, 100) as u8;
    let q = match before {
        Some(b) => MessageQuery::before(sf(b), limit),
        None => MessageQuery::latest(limit),
    };
    let mut msgs = rest.get_messages(sf(channel), q).await.map_err(api_err)?;
    crate::perf::span(if before.is_none() { "rest_messages_latest" } else { "rest_messages_older" }, t0);
    msgs.sort_by_key(|m| m.id);
    Ok(msgs)
}

fn emit_users(inner: &Inner, msgs: &[api::Message]) {
    let mut users = Vec::new();
    for m in msgs {
        convert::message_users(m, &mut users);
    }
    inner.emit(AppEvent::Users(users));
}

fn after_page(inner: &Inner, s: &Session, channel: Id, msgs: &[api::Message], latest: bool) {
    emit_users(inner, msgs);
    super::ensure_message_authors(inner, s, msgs);
    if let Some(g) = s.cache.channel(sf(channel)).and_then(|c| c.guild_id) {
        let voice: Vec<api::Snowflake> = s.cache.voice_states(Some(g)).iter().map(|v| v.user_id).collect();
        super::ensure_authors(inner, s, g, voice);
    }
    if latest {
        let ev = s.adapter.lock().recount_mentions(&s.cache, channel.0, msgs);
        if let Some(ev) = ev {
            inner.emit(ev);
        }
    }
}

async fn refresh(inner: Arc<Inner>, s: Arc<Session>, channel: Id) {
    let Ok(_permit) = BG.acquire().await else { return };
    if !msg_cache::begin_fetch(channel.0) {
        return;
    }
    let old = msg_cache::peek(channel.0);
    let res = fetch(&s.rest, channel, None, PAGE).await;
    msg_cache::end_fetch(channel.0);
    let Ok(msgs) = res else { return };
    msg_cache::put(channel.0, &msgs);
    if old.as_ref().is_some_and(|o| *o != msgs) {
        after_page(&inner, &s, channel, &msgs, true);
        inner.emit(AppEvent::HistoryRefresh { channel, messages: msgs.iter().map(convert::message).collect() });
    }
    s.cache.insert_messages(sf(channel), msgs);
}

pub(super) fn warm_recent(inner: &Arc<Inner>, s: &Arc<Session>) {
    let (inner, s) = (inner.clone(), s.clone());
    tokio::spawn(async move {
        let mut todo: Vec<u64> = {
            let a = s.adapter.lock();
            let mut m: Vec<(u64, u64)> =
                a.prefs.reads.iter().filter(|(_, r)| r.mentions > 0).map(|(c, _)| (*c, s.cache.channel(api::Snowflake(*c)).and_then(|ch| ch.last_message_id).map_or(0, |l| l.0))).collect();
            m.sort_by_key(|x| std::cmp::Reverse(x.1));
            m.into_iter().take(8).map(|x| x.0).collect()
        };
        let recent = tokio::task::spawn_blocking(|| msg_cache::recent_on_disk(6)).await.unwrap_or_default();
        for c in recent {
            if !todo.contains(&c) {
                todo.push(c);
            }
        }
        crate::perf::mark(&format!("warm_channels {}", todo.len()));
        for c in todo {
            if s.cache.channel(api::Snowflake(c)).is_some() && !msg_cache::has_fresh(c) {
                refresh(inner.clone(), s.clone(), Id(c)).await;
            }
        }
    });
}

async fn pre_session(inner: &Arc<Inner>, channel: Id, before: Option<Id>, limit: u32) -> Option<Vec<Message>> {
    let mut rest = None;
    for _ in 0..100 {
        if inner.session.lock().is_some() {
            break;
        }
        rest = inner.pre_rest.lock().clone();
        if rest.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let rest = match rest {
        Some(r) => r,
        None => inner.session().ok()?.rest.clone(),
    };
    let msgs = fetch(&rest, channel, before, limit).await.ok()?;
    crate::perf::mark("messages_pre_session");
    emit_users(inner, &msgs);
    inner.note_pre_session(&msgs);
    if before.is_none() && limit == PAGE {
        msg_cache::put_with(channel.0, &msgs, false);
    }
    Some(msgs.iter().map(convert::message).collect())
}

impl DiscordBackend {
    pub(super) fn load_messages_impl(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.spawn(move |inner| async move {
            let t0 = Instant::now();
            if before.is_none() && limit == PAGE {
                let c = channel.0;
                let cached = tokio::task::spawn_blocking(move || msg_cache::get(c)).await.ok().flatten();
                if let Some((msgs, fresh)) = cached {
                    crate::perf::span(if fresh { "messages_cache_fresh" } else { "messages_cache_stale" }, t0);
                    match inner.session() {
                        Ok(s) => {
                            after_page(&inner, &s, channel, &msgs, true);
                            if !fresh {
                                tokio::spawn(refresh(inner.clone(), s, channel));
                            }
                        }
                        Err(_) => {
                            emit_users(&inner, &msgs);
                            inner.note_pre_session(&msgs);
                        }
                    }
                    return Ok(msgs.iter().map(convert::message).collect::<Vec<Message>>());
                }
            }
            let s = match inner.session() {
                Ok(s) => s,
                Err(e) => return pre_session(&inner, channel, before, limit).await.ok_or(e),
            };
            let msgs = fetch(&s.rest, channel, before, limit).await?;
            after_page(&inner, &s, channel, &msgs, before.is_none());
            if before.is_none() && limit == PAGE {
                msg_cache::put(channel.0, &msgs);
            }
            let out: Vec<Message> = msgs.iter().map(convert::message).collect();
            s.cache.insert_messages(sf(channel), msgs);
            Ok(out)
        })
    }

    pub(super) fn load_messages_at_impl(&self, channel: Id, at: crate::backend::HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.spawn(move |inner| async move {
            let s = inner.session()?;
            let limit = limit.clamp(1, 100) as u8;
            let q = match at {
                crate::backend::HistoryAt::Around(m) => MessageQuery::around(sf(m), limit),
                crate::backend::HistoryAt::After(m) => MessageQuery::after(sf(m), limit),
            };
            let mut msgs = s.rest.get_messages(sf(channel), q).await.map_err(api_err)?;
            msgs.sort_by_key(|m| m.id);
            after_page(&inner, &s, channel, &msgs, false);
            let out: Vec<Message> = msgs.iter().map(convert::message).collect();
            s.cache.insert_messages(sf(channel), msgs);
            Ok(out)
        })
    }

    pub(super) fn prefetch_impl(&self, channel: Id) {
        let Ok(s) = self.inner.session() else { return };
        if msg_cache::has_fresh(channel.0) || BG.available_permits() == 0 {
            return;
        }
        self.inner.rt.spawn(refresh(self.inner.clone(), s, channel));
    }
}
