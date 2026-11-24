use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};

use phoenix_app::discord_backend::{
    convert::{self, Prefs},
    store::{KeyringStore, TokenStore},
};
use phoenix_api::{
    gateway::{GuildSubscription, events},
    models as api,
    rest::{MessageQuery, forum::ThreadSort},
    ClientProperties, Cache, Gateway, GatewayConfig, GatewayEvent, RestClient,
};
use serde_json::Value;

const API: &str = "https://discord.com/api/v10";

macro_rules! bug {
    ($($t:tt)*) => { println!("BUG  {}", format!($($t)*)) };
}
macro_rules! info {
    ($($t:tt)*) => { println!("info {}", format!($($t)*)) };
}

fn explain(name: &str, data: &Value) -> Option<String> {
    use serde_json::from_value as f;
    let d = data.clone();
    let r: Result<(), serde_json::Error> = match name {
        "READY" => f::<events::Ready>(d).map(drop),
        "READY_SUPPLEMENTAL" => f::<events::ReadySupplemental>(d).map(drop),
        "GUILD_CREATE" | "GUILD_UPDATE" => f::<api::Guild>(d).map(drop),
        "GUILD_MEMBER_ADD" | "GUILD_MEMBER_UPDATE" => f::<api::Member>(d).map(drop),
        "GUILD_MEMBER_LIST_UPDATE" => f::<events::GuildMemberListUpdate>(d).map(drop),
        "GUILD_MEMBERS_CHUNK" => f::<events::GuildMembersChunk>(d).map(drop),
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE" | "THREAD_CREATE" | "THREAD_UPDATE" | "THREAD_DELETE" => {
            f::<api::Channel>(d).map(drop)
        }
        "MESSAGE_CREATE" => f::<api::Message>(d).map(drop),
        "MESSAGE_DELETE" => f::<events::MessageDelete>(d).map(drop),
        "MESSAGE_REACTION_ADD" | "MESSAGE_REACTION_REMOVE" => f::<events::ReactionEvent>(d).map(drop),
        "MESSAGE_ACK" => f::<events::MessageAck>(d).map(drop),
        "TYPING_START" => f::<events::TypingStart>(d).map(drop),
        "PRESENCE_UPDATE" => f::<events::PresenceUpdate>(d).map(drop),
        "RELATIONSHIP_ADD" | "RELATIONSHIP_REMOVE" => f::<api::Relationship>(d).map(drop),
        "VOICE_STATE_UPDATE" => f::<api::VoiceState>(d).map(drop),
        "USER_UPDATE" => f::<api::User>(d).map(drop),
        _ => return None,
    };
    r.err().map(|e| e.to_string())
}

fn check_raw_messages(ch: u64, raw: &Value) {
    let Some(arr) = raw.as_array() else {
        bug!("channel {ch}: messages page is not an array");
        return;
    };
    for m in arr {
        match serde_json::from_value::<api::Message>(m.clone()) {
            Err(e) => bug!("channel {ch}: message {} fails to parse: {e}", m["id"]),
            Ok(p) => {
                let n = |k: &str| m[k].as_array().map_or(0, Vec::len);
                if p.attachments.len() != n("attachments") || p.embeds.len() != n("embeds") || p.reactions.len() != n("reactions") {
                    bug!("channel {ch}: message {} lost attachments/embeds/reactions in parse", m["id"]);
                }
                if p.content.is_empty() && m["content"].as_str().is_some_and(|s| !s.is_empty()) {
                    bug!("channel {ch}: message {} lost its content", m["id"]);
                }
                let cm = convert::message(&p);
                if cm.author.0 == 0 {
                    bug!("channel {ch}: message {} has no author after convert", m["id"]);
                }
            }
        }
    }
}

struct Stats {
    events: BTreeMap<String, u32>,
    unknown: BTreeMap<String, u32>,
    parse_fail: BTreeMap<String, (u32, String)>,
}

fn name_of(ev: &GatewayEvent) -> String {
    let s = format!("{ev:?}");
    s.split(['(', ' ', '{']).next().unwrap_or("?").to_owned()
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let secs: u64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(150);
    let live: Option<Vec<Value>> =
        args.get(1).and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok());
    if let Some(p) = args.get(2) {
        // SAFETY: single-threaded at this point.
        unsafe { std::env::set_var("PROBE_DMS", p) };
    }
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    rt.block_on(run(secs, live))
}

async fn run(secs: u64, live: Option<Vec<Value>>) -> anyhow::Result<()> {
    let token = tokio::task::spawn_blocking(|| KeyringStore.load())
        .await?
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
    let props = ClientProperties::linux_desktop();
    let rest = RestClient::new(token.expose(), props.clone());
    let http = reqwest::Client::builder().user_agent(props.user_agent()).build()?;
    let raw_get = |path: String| {
        let http = http.clone();
        let tok = token.expose().to_owned();
        async move {
            let t = Instant::now();
            let r = http.get(format!("{API}{path}")).header("Authorization", tok).send().await?;
            let st = r.status();
            let v: Value = r.json().await.unwrap_or(Value::Null);
            anyhow::Ok((st.as_u16(), v, t.elapsed()))
        }
    };

    let t0 = Instant::now();
    let me = rest.get_me().await?;
    info!("get_me ok in {:?} (id {})", t0.elapsed(), me.id);

    let mut cfg = GatewayConfig::new(token.expose());
    cfg.properties = props.clone();
    if let Ok(c) = std::env::var("PROBE_CAPS") {
        cfg.capabilities = c.parse()?;
    }
    let (gw, mut rx) = Gateway::spawn(cfg);
    let cache = Cache::default();
    let mut st = Stats { events: BTreeMap::new(), unknown: BTreeMap::new(), parse_fail: BTreeMap::new() };

    let t0 = Instant::now();
    let ready = loop {
        let Some(ev) = tokio::time::timeout(Duration::from_secs(40), rx.recv()).await? else {
            anyhow::bail!("gateway closed before READY");
        };
        cache.apply(&ev);
        tally(&mut st, &ev);
        match ev {
            GatewayEvent::Ready(r) => break r,
            GatewayEvent::Fatal { code, reason } => anyhow::bail!("fatal {code}: {reason}"),
            _ => {}
        }
    };
    info!("READY in {:?}: {} guilds, {} private channels, {} relationships", t0.elapsed(), ready.guilds.len(), ready.private_channels.len(), ready.relationships.len());

    for g in &ready.guilds {
        if g.unavailable {
            info!("guild {} unavailable in READY", g.id);
            continue;
        }
        if g.properties.name.is_empty() {
            bug!("guild {} has empty name", g.id);
        }
        if g.channels.is_empty() {
            bug!("guild {} has no channels in READY", g.id);
        }
        if g.roles.is_empty() {
            bug!("guild {} has no roles in READY", g.id);
        }
        if cache.member(g.id, me.id).is_none() {
            bug!("guild {}: own member missing from cache (permissions unknown, all channels shown)", g.id);
        }
    }
    let prefs = Prefs::from_ready(&ready);
    let t = Instant::now();
    let data = convert::build_ready(&cache, &ready, &prefs);
    info!("build_ready {:?}: {} guilds, {} channels, {} dms, {} users", t.elapsed(), data.guilds.len(), data.channels.len(), data.dm_channels.len(), data.users.len());
    if data.guilds.len() != ready.guilds.iter().filter(|g| !g.unavailable).count() {
        bug!("build_ready guild count {} != READY {}", data.guilds.len(), ready.guilds.len());
    }
    for g in &data.guilds {
        if !data.channels.iter().any(|c| c.guild == Some(g.id)) {
            bug!("guild {} shows no channel at all after permission filter", g.id.0);
        }
    }
    for dm in &data.dm_channels {
        if dm.name.is_empty() {
            bug!("DM {} has an empty name", dm.id.0);
        }
    }
    let rs_entries = ready.read_state.as_ref().and_then(|v| v.get("entries").or(Some(v))).and_then(Value::as_array).map_or(0, Vec::len);
    info!("read_state entries {} -> parsed {}", rs_entries, prefs.reads.len());
    if let Some(Value::Object(o)) = ready.read_state.as_ref() {
        info!("read_state keys {:?} version={:?} partial={:?}", o.keys().collect::<Vec<_>>(), o.get("version"), o.get("partial"));
    }
    for e in ready.read_state.as_ref().and_then(|v| v.get("entries")).and_then(Value::as_array).into_iter().flatten() {
        let t = e.get("read_state_type").and_then(Value::as_u64).unwrap_or(0);
        let s = e.to_string();
        if t != 0 || e.get("mention_count").and_then(Value::as_u64).unwrap_or(0) > 0 {
            info!("rs entry type={t} {}", s.chars().take(300).collect::<String>());
        }
    }
    for (cid, r) in &prefs.reads {
        if r.mentions > 0 {
            let c = cache.channel(api::Snowflake(*cid));
            info!("mention entry ch {cid} n={} known={} guild={:?} kind={:?} parent={:?}", r.mentions, c.is_some(), c.as_ref().and_then(|c| c.guild_id), c.as_ref().map(|c| c.kind), c.as_ref().and_then(|c| c.parent_id));
        }
    }
    if rs_entries > 0 && prefs.reads.is_empty() {
        bug!("read states present in READY but none parsed");
    }
    let unread_g = data.guilds.iter().filter(|g| g.unread).count();
    let unread_c = data.channels.iter().filter(|c| c.unread).count();
    info!("unread guilds {unread_g}, unread channels {unread_c}, mentions {}", data.guilds.iter().map(|g| g.mentions).sum::<u32>());
    let gidx: HashMap<u64, usize> = data.guilds.iter().enumerate().map(|(i, g)| (g.id.0, i)).collect();

    if let Some(live) = &live {
        let lids: Vec<u64> = live.iter().filter_map(|v| v["id"].as_str()?.parse().ok()).collect();
        let ours: Vec<u64> = data.guilds.iter().map(|g| g.id.0).filter(|g| lids.contains(g)).collect();
        let theirs: Vec<u64> = lids.iter().copied().filter(|g| gidx.contains_key(g)).collect();
        if ours != theirs {
            let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
            bug!("guild order differs from official client (first diff at {first:?}, {} vs {} guilds)", ours.len(), theirs.len());
        } else {
            info!("guild order matches the official client on {} visible guilds", ours.len());
        }
        for v in live {
            let Some(id) = v["id"].as_str().and_then(|s| s.parse::<u64>().ok()) else { continue };
            let Some(&i) = gidx.get(&id) else { continue };
            let g = &data.guilds[i];
            let pill = v["pill"].as_str().unwrap_or("");
            let their_unread = pill.contains("visible") && !pill.contains("selected");
            let their_ment: u32 = v["badge"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
            let ours_unread = g.unread && !pill.contains("selected");
            if their_unread != ours_unread || their_ment != g.mentions {
                bug!("guild {id}: official unread={their_unread} mentions={their_ment}, ours unread={} mentions={}", g.unread, g.mentions);
                let ugs = ready.user_guild_settings.as_ref().and_then(|v| v.get("entries").or(Some(v))).and_then(Value::as_array);
                if let Some(e) = ugs.and_then(|a| a.iter().find(|e| e["guild_id"].as_str() == Some(&id.to_string()))) {
                    let mut e = e.clone();
                    if let Some(o) = e.get_mut("channel_overrides").and_then(Value::as_array_mut) {
                        o.retain(|c| data.channels.iter().any(|x| x.guild == Some(g.id) && x.unread && c["channel_id"].as_str() == Some(&x.id.0.to_string())));
                    }
                    println!("     settings {e}");
                }
                for c in data.channels.iter().filter(|c| c.guild == Some(g.id) && (c.unread || c.mentions > 0)) {
                    let raw = cache.channel(convert::sf(c.id));
                    let r = prefs.reads.get(&c.id.0);
                    println!(
                        "     channel {} kind={:?} unread={} mentions={} muted={} last_msg={:?} read={:?} parent_muted={}",
                        c.id.0, c.kind, c.unread, c.mentions, c.muted, raw.as_ref().and_then(|c| c.last_message_id), r,
                        c.parent.is_some_and(|p| prefs.mutes.channels.contains(&p.0))
                    );
                    if let Ok((code, v, _)) = raw_get(format!("/channels/{}", c.id.0)).await {
                        println!("     GET channel -> {code} flags={} perms_ow={}", v["flags"], v["permission_overwrites"].as_array().map_or(0, Vec::len));
                    }
                    if let Ok((code, _, _)) = raw_get(format!("/channels/{}/messages?limit=1", c.id.0)).await {
                        println!("     GET messages -> {code}");
                    }
                }
            }
        }
    }

    if let Some(live) = std::env::var("PROBE_DMS").ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()) {
        let theirs: Vec<u64> = live.iter().filter_map(|s| s.parse().ok()).collect();
        let ours: Vec<u64> = data.dm_channels.iter().map(|c| c.id.0).take(theirs.len() + 3).collect();
        let raw: HashMap<u64, Value> = ready.private_channels.iter().map(|c| (c.id.0, serde_json::to_value(c).unwrap_or_default())).collect();
        let desc = |id: &u64| {
            let c = cache.channel(api::Snowflake(*id));
            format!(
                "{id} type={:?} req={:?} last={:?} flags={:?}",
                c.as_ref().map(|c| c.kind), c.as_ref().map(|c| c.is_message_request), c.as_ref().and_then(|c| c.last_message_id),
                raw.get(id).map(|v| v["flags"].clone())
            )
        };
        for (i, t) in theirs.iter().enumerate() {
            if ours.get(i) != Some(t) {
                bug!("DM row {i}: official {} / ours {}", desc(t), ours.get(i).map(desc).unwrap_or_default());
            }
        }
    }

    let reqs = ready.private_channels.iter().filter(|c| c.is_message_request).count();
    let spam = ready.private_channels.iter().filter(|c| serde_json::to_value(c).ok().is_some_and(|v| v["is_spam"] == true)).count();
    let shown_req = data.dm_channels.iter().filter(|d| cache.channel(convert::sf(d.id)).is_some_and(|c| c.is_message_request)).count();
    info!("DMs: {reqs} message requests, {spam} spam; {shown_req} requests shown in our DM list");
    if shown_req > 0 {
        bug!("message requests appear in the DM list ({shown_req})");
    }

    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut probes = probe_targets(&cache, &ready, &data);
    info!("probing {} channels", probes.len());
    let mut opened_guilds = HashSet::new();
    let mut member_list_updates = 0u32;
    loop {
        if let Some(ch) = probes.pop() {
            let c = cache.channel(ch);
            let kind = c.as_ref().map(|c| c.kind);
            if let Some(g) = c.as_ref().and_then(|c| c.guild_id)
                && opened_guilds.insert(g)
            {
                let _ = gw.subscribe_guild(g, GuildSubscription::focused(Some(ch)));
            }
            if matches!(kind, Some(api::ChannelType::GuildForum | api::ChannelType::GuildMedia)) {
                let t = Instant::now();
                for archived in [false, true] {
                    match rest.search_threads(ch, ThreadSort::LastMessage, &[], 0, archived).await {
                        Ok(s) => info!("forum {ch} archived={archived}: {} threads in {:?}", s.threads.len(), t.elapsed()),
                        Err(e) => bug!("forum {ch} archived={archived}: search_threads failed: {e}"),
                    }
                }
            } else {
                let t = Instant::now();
                match rest.get_messages(ch, MessageQuery::latest(50)).await {
                    Ok(ms) => {
                        let el = t.elapsed();
                        if el > Duration::from_secs(2) {
                            bug!("channel {ch}: slow messages page {el:?}");
                        }
                        info!("channel {ch} ({kind:?}): {} messages in {el:?}", ms.len());
                        if !cache.permissions_in(ch).contains(api::Permissions::VIEW_CHANNEL) {
                            bug!("channel {ch}: we hide it (no VIEW_CHANNEL) but REST lets us read it");
                        }
                        if let Ok((200, raw, _)) = raw_get(format!("/channels/{ch}/messages?limit=50")).await {
                            check_raw_messages(ch.0, &raw);
                        }
                        for m in ms.iter().take(3) {
                            for a in &m.attachments {
                                cdn_check(&http, &a.proxy_url).await;
                            }
                        }
                        let mut users = Vec::new();
                        for m in &ms {
                            convert::message_users(m, &mut users);
                        }
                        for u in users.iter().take(2) {
                            if let phoenix_app::model::ImageRef::Url(url) = &u.avatar {
                                cdn_check(&http, url).await;
                            }
                        }
                        if let Some(a) = ms.first().map(|m| m.author.id).filter(|a| *a != me.id) {
                            let gid = c.as_ref().and_then(|c| c.guild_id);
                            match rest.get_user_profile(a, gid).await {
                                Ok(_) => {}
                                Err(e) => {
                                    let (code, raw, _) = raw_get(format!("/users/{a}/profile?with_mutual_guilds=true")).await?;
                                    if code == 200 {
                                        let err = serde_json::from_value::<phoenix_api::rest::profile::UserProfile>(raw).err();
                                        bug!("profile of {a} fails: {e} / {err:?}");
                                    } else {
                                        info!("profile of {a}: HTTP {code}");
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let (code, raw, _) = raw_get(format!("/channels/{ch}/messages?limit=50")).await?;
                        let visible = cache.permissions_in(ch).contains(api::Permissions::VIEW_CHANNEL);
                        if code == 200 {
                            bug!("channel {ch}: get_messages failed to parse: {e}");
                            check_raw_messages(ch.0, &raw);
                        } else if visible {
                            bug!("channel {ch}: shown as visible but REST says HTTP {code}");
                        }
                    }
                }
            }
        }
        let wait = if probes.is_empty() { Duration::from_millis(500) } else { Duration::from_millis(50) };
        while let Ok(Some(ev)) = tokio::time::timeout(wait, rx.recv()).await {
            if matches!(ev, GatewayEvent::GuildMemberListUpdate(_)) {
                member_list_updates += 1;
            }
            let t = Instant::now();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.apply(&ev)));
            if r.is_err() {
                bug!("cache.apply panicked on {}", name_of(&ev));
            }
            if t.elapsed() > Duration::from_millis(20) {
                bug!("cache.apply slow on {}: {:?}", name_of(&ev), t.elapsed());
            }
            tally(&mut st, &ev);
            if let GatewayEvent::Fatal { code, reason } = &ev {
                bug!("fatal {code}: {reason}");
            }
        }
        if Instant::now() > deadline {
            break;
        }
    }
    gw.shutdown();

    info!("member list updates: {member_list_updates}");
    println!("\n== events ==");
    for (k, v) in &st.events {
        println!("  {k}: {v}");
    }
    println!("== unknown dispatches ==");
    for (k, v) in &st.unknown {
        println!("  {k}: {v}");
    }
    println!("== parse failures ==");
    for (k, (n, e)) in &st.parse_fail {
        println!("  {k} x{n}: {e}");
    }
    Ok(())
}

fn tally(st: &mut Stats, ev: &GatewayEvent) {
    if let GatewayEvent::Unknown { name, data } = ev {
        if let Some(e) = explain(name, data) {
            let ent = st.parse_fail.entry(name.clone()).or_insert((0, e));
            ent.0 += 1;
        } else {
            *st.unknown.entry(name.clone()).or_default() += 1;
        }
    } else {
        *st.events.entry(name_of(ev)).or_default() += 1;
    }
}

async fn cdn_check(http: &reqwest::Client, url: &str) {
    if !phoenix_app::discord_backend::http::host_allowed(url) {
        bug!("CDN url not allowed by our http client: host of {}", url.split('/').nth(2).unwrap_or("?"));
        return;
    }
    let t = Instant::now();
    match http.get(url).send().await {
        Ok(r) if r.status().is_success() => {
            if t.elapsed() > Duration::from_secs(3) {
                bug!("slow CDN fetch {:?}", t.elapsed());
            }
        }
        Ok(r) => bug!("CDN {} for {}", r.status(), url.split('?').next().unwrap_or("")),
        Err(e) => bug!("CDN error {e}"),
    }
}

fn probe_targets(cache: &Cache, ready: &events::Ready, data: &phoenix_app::backend::ReadyData) -> Vec<api::ChannelId> {
    let mut out = Vec::new();
    for g in data.guilds.iter().take(8) {
        let gid = convert::sf(g.id);
        let mut n = 0;
        for c in cache.guild_channels(gid) {
            if n >= 2 {
                break;
            }
            let readable = cache.permissions_in(c.id).contains(api::Permissions::VIEW_CHANNEL | api::Permissions::READ_MESSAGE_HISTORY);
            let text = matches!(c.kind, api::ChannelType::GuildText | api::ChannelType::GuildAnnouncement);
            let forum = matches!(c.kind, api::ChannelType::GuildForum | api::ChannelType::GuildMedia);
            if readable && (text || forum) && c.last_message_id.is_some() {
                out.push(c.id);
                n += 1;
            }
        }
    }
    for g in ready.guilds.iter() {
        for c in cache.guild_channels(g.id) {
            let forum = matches!(c.kind, api::ChannelType::GuildForum | api::ChannelType::GuildMedia);
            if forum && !out.contains(&c.id) && cache.permissions_in(c.id).contains(api::Permissions::VIEW_CHANNEL) {
                out.push(c.id);
            }
        }
    }
    for g in ready.guilds.iter().take(8) {
        if let Some(c) = cache.guild_channels(g.id).into_iter().find(|c| {
            c.kind == api::ChannelType::GuildText && !cache.permissions_in(c.id).contains(api::Permissions::VIEW_CHANNEL)
        }) {
            out.push(c.id);
        }
    }
    let mut dms = cache.private_channels();
    dms.sort_by_key(|c| std::cmp::Reverse(c.last_message_id.unwrap_or(c.id).0));
    out.extend(dms.iter().take(5).map(|c| c.id));
    out.reverse();
    out
}
