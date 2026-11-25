use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Subscription, Window,
    div, prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok},
    ui::{avatar, home::data::RelationshipKind, icon, sidebar::channel_icon},
};

pub const HISTORY: usize = 8;
const LIMIT: usize = 5;
const MODE_LIMIT: usize = 100;
const REFETCH_LIMIT: usize = 100;
const FUZZY_TAIL: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Channel(Id),
    Dm(Id),
    Guild(Id),
    User(Id),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Header(SharedString),
    Hit(Target),
}

impl Row {
    pub fn target(&self) -> Option<Target> {
        match self {
            Row::Hit(t) => Some(*t),
            Row::Header(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    User,
    UserGlobal,
    Text,
    Voice,
    Guild,
    Game,
}

pub struct Switcher {
    pub input: gpui::Entity<InputState>,
    pub results: Vec<Row>,
    pub selected: usize,
    pub _sub: Subscription,
}

pub fn parse(raw: &str) -> (String, Option<Mode>) {
    if let Some(rest) = raw.strip_prefix("@@") {
        return (rest.to_string(), Some(Mode::UserGlobal));
    }
    let mode = match raw.chars().next() {
        Some('@') => Some(Mode::User),
        Some('#') => Some(Mode::Text),
        Some('!') => Some(Mode::Voice),
        Some('*') => Some(Mode::Guild),
        Some('$') => Some(Mode::Game),
        _ => None,
    };
    let mut q = raw.to_string();
    if raw.starts_with('@') {
        q.remove(0);
    } else if let Some(i) = raw.find(['#', '!', '*', '$']) {
        q.remove(i);
    }
    (q, mode)
}

pub fn step(rows: &[Row], from: Option<usize>, down: bool) -> usize {
    let n = rows.len() as isize;
    if n == 0 {
        return 0;
    }
    let mut i = from.map_or(-1, |f| f as isize);
    for _ in 0..n {
        i += if down { 1 } else { -1 };
        if i < 0 {
            i = n - 1;
        } else if i >= n {
            i = 0;
        }
        if matches!(rows[i as usize], Row::Hit(_)) {
            return i as usize;
        }
    }
    from.unwrap_or(0)
}

pub fn visit(app: &mut PhoenixApp, id: Id) {
    let h = &mut app.nav.switcher_history;
    h.retain(|x| *x != id);
    h.insert(0, id);
    h.truncate(HISTORY);
}

pub fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|ch| !('\u{300}'..='\u{36f}').contains(ch))
        .map(|ch| match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
            'ď' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
            'ĥ' => 'h',
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
            'ĵ' => 'j',
            'ķ' => 'k',
            'ĺ' | 'ļ' | 'ľ' => 'l',
            'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
            'ŕ' | 'ŗ' | 'ř' => 'r',
            'ś' | 'ŝ' | 'ş' | 'š' => 's',
            'ţ' | 'ť' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
            'ŵ' => 'w',
            'ý' | 'ÿ' | 'ŷ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            other => other,
        })
        .collect()
}

pub fn subseq(needle: &str, hay: &str) -> bool {
    let n: Vec<char> = needle.chars().collect();
    let h: Vec<char> = hay.chars().collect();
    if n.len() > h.len() {
        return false;
    }
    if n.len() == h.len() {
        return n == h;
    }
    let mut j = 0;
    'outer: for nc in n {
        while j < h.len() {
            j += 1;
            if h[j - 1] == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

#[derive(Clone, Debug)]
pub struct Tok {
    lower: String,
    dashed: Option<String>,
    full: bool,
}

impl Tok {
    fn new(q: &str) -> Self {
        Self { lower: q.to_lowercase(), dashed: None, full: false }
    }
}

pub fn tokens(q: &str, allow_empty: bool) -> Vec<Tok> {
    let mut out: Vec<Tok> = q.split(' ').filter(|w| !w.is_empty() || allow_empty).map(Tok::new).collect();
    if q.contains(' ') {
        let lower = q.to_lowercase();
        out.insert(0, Tok { dashed: Some(lower.replacen(' ', "-", 1)), lower, full: true });
    }
    out
}

pub fn eg(e: &str, t: &Tok, fuzzy: bool) -> i32 {
    let alt = t.dashed.as_deref();
    if e.starts_with(t.lower.as_str()) || alt.is_some_and(|a| e.starts_with(a)) {
        return if e == t.lower { 10 } else { 7 };
    }
    if e.contains(t.lower.as_str()) || alt.is_some_and(|a| e.contains(a)) {
        return 5;
    }
    if t.lower.split([',', ' ']).all(|w| e.contains(w)) {
        return 3;
    }
    if fuzzy && subseq(&t.lower, e) {
        return 1;
    }
    0
}

fn best(e: &str, toks: &mut Vec<Tok>, fuzzy: bool) -> i32 {
    let mut top = 0;
    let mut at = None;
    for (i, t) in toks.iter().enumerate() {
        let s = eg(e, t, fuzzy);
        if s > top {
            top = s;
            at = Some(i);
        }
    }
    if let Some(i) = at {
        if toks[i].full {
            toks.clear();
        } else {
            toks.remove(i);
        }
    }
    top
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    target: Target,
    score: f64,
}

pub struct Boost {
    places: HashMap<u64, i64>,
    max: f64,
    users: HashMap<Id, f64>,
}

fn is_voice(k: ChannelKind) -> bool {
    matches!(k, ChannelKind::Voice | ChannelKind::Stage)
}

fn dm_recipient(app: &PhoenixApp, ch: Id) -> Option<Id> {
    let c = app.channels.get(&ch)?;
    if c.guild.is_some() || is_group(app, ch) {
        return None;
    }
    c.voice_users.first().copied()
}

fn is_group(app: &PhoenixApp, ch: Id) -> bool {
    app.nav.extras.get(&ch).is_some_and(|e| e.recipients > 1)
}

impl Boost {
    pub fn new(app: &PhoenixApp, places: HashMap<u64, i64>) -> Self {
        let max = places.values().copied().max().unwrap_or(0) as f64;
        let mut users: HashMap<Id, f64> = HashMap::new();
        if max > 0. {
            for d in &app.dms {
                let Some(f) = places.get(&d.0) else { continue };
                let share = *f as f64 / max;
                if is_group(app, *d) {
                    let rs = &app.channels[d].voice_users;
                    for r in rs {
                        users.insert(*r, 1. + share / rs.len().max(1) as f64);
                    }
                } else if let Some(r) = dm_recipient(app, *d) {
                    users.insert(r, 1. + share);
                }
            }
        }
        for r in app.nav.relationships.iter().filter(|r| r.kind == RelationshipKind::Friend) {
            *users.entry(r.user).or_insert(1.) += 0.2;
        }
        for d in &app.dms {
            if let Some(r) = dm_recipient(app, *d) {
                *users.entry(r).or_insert(1.) += 0.1;
            }
        }
        Self { places, max, users }
    }

    fn frecency(&self, id: Id) -> i64 {
        self.places.get(&id.0).copied().unwrap_or(0)
    }

    fn place(&self, id: Id) -> f64 {
        match self.places.get(&id.0) {
            Some(f) if self.max > 0. => 1. + *f as f64 / self.max,
            _ => 1.,
        }
    }
}

pub fn places(cx: &gpui::App) -> HashMap<u64, i64> {
    cx.try_global::<crate::ui::pickers::Pickers>().and_then(|p| p.frecency.as_ref()).map(|f| f.places.clone()).unwrap_or_default()
}

fn guild_name(app: &PhoenixApp, g: Option<Id>) -> Option<SharedString> {
    let g = g?;
    app.guilds.iter().find(|x| x.id == g).map(|x| x.name.clone())
}

fn rail_rank(app: &PhoenixApp, ch: &crate::model::Channel) -> (i64, usize) {
    let g = ch.guild.and_then(|g| app.guilds.iter().position(|x| x.id == g)).unwrap_or(usize::MAX);
    let parent = ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.position);
    let pos = match parent {
        None if ch.kind == ChannelKind::Category => (i64::from(ch.position) + 1) * 1000,
        None => i64::from(ch.position),
        Some(pp) => (i64::from(pp) + 1) * 1000 + i64::from(ch.position) + if is_voice(ch.kind) { 500 } else { 0 },
    };
    (pos, g)
}

fn text_like(k: ChannelKind) -> bool {
    matches!(k, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum)
}

fn channels(app: &PhoenixApp, b: &Boost, q: &str, guild: Option<Id>, voice_only: bool, limit: usize) -> Vec<Hit> {
    let toks = tokens(q, q.is_empty());
    let mut hits: Vec<(Hit, (usize, i64))> = Vec::new();
    let pool: Vec<&crate::model::Channel> = match guild {
        Some(g) => app.guild_channels.get(&g).into_iter().flatten().filter_map(|id| app.channels.get(id)).collect(),
        None => app.channels.values().filter(|c| c.guild.is_some()).collect(),
    };
    for ch in pool {
        let voice = is_voice(ch.kind);
        if ch.kind == ChannelKind::Category || (voice_only && !voice) || (!voice_only && !voice && !text_like(ch.kind)) {
            continue;
        }
        if guild.is_some() && !voice_only && voice {
            continue;
        }
        let mut r = toks.clone();
        let mut s = f64::from(best(&ch.name.to_lowercase(), &mut r, true));
        if s == 0. {
            continue;
        }
        if !r.is_empty() {
            let gname = guild_name(app, ch.guild).map(|n| n.to_lowercase());
            let pname = ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.name.to_lowercase());
            for t in [gname, pname].into_iter().flatten() {
                if t.is_empty() {
                    continue;
                }
                let e = best(&t, &mut r, false);
                if e != 0 {
                    s += 0.5 * f64::from(e);
                }
            }
            s = s.min(6.);
        }
        if s == 0. || r.len() > 1 || (r.len() == 1 && !r[0].full) {
            continue;
        }
        if !voice_only && voice {
            s = (s - 1.).max(0.5);
        }
        let cap = if s >= 7. { 10. } else { 7. };
        s = (s + if b.frecency(ch.id) >= 1 { 3. } else { 0. }).min(cap);
        let booster = if voice && !voice_only { 1. } else { b.place(ch.id) };
        hits.push((Hit { target: Target::Channel(ch.id), score: 1000. * s * booster }, rail_rank(app, ch)));
    }
    hits.sort_by(|x, y| y.0.score.total_cmp(&x.0.score).then_with(|| x.1.cmp(&y.1)));
    hits.truncate(limit);
    hits.into_iter().map(|(h, _)| h).collect()
}

fn guilds(app: &PhoenixApp, b: &Boost, q: &str, skip: Option<Id>, limit: usize) -> Vec<Hit> {
    let t = Tok::new(q);
    let mut hits: Vec<Hit> = app
        .guilds
        .iter()
        .filter(|g| Some(g.id) != skip)
        .filter_map(|g| {
            let s = eg(&g.name.to_lowercase(), &t, true);
            (s > 0).then(|| Hit { target: Target::Guild(g.id), score: 1000. * f64::from(s) * b.place(g.id) })
        })
        .collect();
    hits.sort_by(|x, y| y.score.total_cmp(&x.score));
    hits.truncate(limit);
    hits
}

fn nick(app: &PhoenixApp, guild: Option<Id>, user: Id) -> Option<String> {
    let g = guild?;
    crate::member_index::find(app.members.get(&g)?, g, user)?.nick.as_ref().map(|n| n.to_lowercase())
}

fn blocked(app: &PhoenixApp, u: Id) -> bool {
    app.nav.relationships.iter().any(|r| r.user == u && (r.kind == RelationshipKind::Blocked || r.ignored))
}

fn users(app: &PhoenixApp, b: &Boost, q: &str, pool: &[Id], limit: usize) -> Vec<Hit> {
    let u = q.to_lowercase();
    let un = fold(&u);
    let mut strong: Vec<(Hit, String)> = Vec::new();
    let mut weak: Vec<(Hit, String)> = Vec::new();
    for id in pool {
        let Some(user) = app.users.get(id) else { continue };
        if user.id == app.me.id || blocked(app, user.id) {
            continue;
        }
        let n = user.name.to_lowercase();
        let r = fold(&n);
        let e = nick(app, app.selected_guild, user.id);
        let d = e.as_deref().map(fold);
        let f = (user.display_name != user.name).then(|| user.display_name.to_lowercase());
        let h = f.as_deref().map(fold);
        let sortable = h.clone().or_else(|| d.clone()).unwrap_or_else(|| r.clone());
        let booster = b.users.get(&user.id).copied().unwrap_or(1.);
        let pre = |x: Option<&str>| x.is_some_and(|x| x.starts_with(u.as_str()));
        let pre_n = |x: Option<&str>| x.is_some_and(|x| x.starts_with(un.as_str()));
        let hit = |s: f64| Hit { target: Target::User(user.id), score: 1000. * s * booster };
        if q == user.id.0.to_string() || [Some(n.as_str()), Some(r.as_str()), e.as_deref(), d.as_deref(), f.as_deref(), h.as_deref()].into_iter().any(pre) {
            strong.push((hit(10.), sortable));
        } else if [Some(r.as_str()), d.as_deref(), h.as_deref()].into_iter().any(pre_n) {
            strong.push((hit(1.), sortable));
        } else if weak.len() < FUZZY_TAIL
            && [r.as_str(), d.as_deref().unwrap_or(""), h.as_deref().unwrap_or("")]
                .into_iter()
                .filter(|x| !x.is_empty())
                .any(|x| subseq(&u, x) || subseq(&un, x))
        {
            weak.push((hit(1.), sortable));
        }
    }
    let order = |v: &mut Vec<(Hit, String)>| v.sort_by(|x, y| y.0.score.total_cmp(&x.0.score).then_with(|| x.1.cmp(&y.1)));
    order(&mut strong);
    if strong.len() < limit {
        order(&mut weak);
        let room = limit - strong.len();
        strong.extend(weak.into_iter().take(room));
    }
    strong.truncate(limit);
    strong.into_iter().map(|(h, _)| h).collect()
}

fn group_dms(app: &PhoenixApp, b: &Boost, q: &str, limit: usize) -> Vec<Hit> {
    let t = Tok::new(&fold(q));
    let mut hits: Vec<Hit> = Vec::new();
    for id in app.dms.iter().filter(|d| is_group(app, **d)) {
        let Some(ch) = app.channels.get(id) else { continue };
        let mut s = eg(&fold(&ch.name), &t, true);
        for r in &ch.voice_users {
            let Some(u) = app.users.get(r) else { continue };
            for name in [&u.name, &u.display_name] {
                s = s.max(eg(&fold(name), &t, true).min(5));
            }
        }
        if s > 0 {
            hits.push(Hit { target: Target::Dm(*id), score: 1000. * f64::from(s) * b.place(*id) });
        }
    }
    hits.sort_by(|x, y| y.score.total_cmp(&x.score));
    hits.truncate(limit);
    hits
}

fn all_users(app: &PhoenixApp) -> Vec<Id> {
    let mut v: Vec<Id> = app.users.keys().copied().collect();
    v.sort_by_key(|i| i.0);
    v
}

fn guild_or_friends(app: &PhoenixApp, g: Id) -> Vec<Id> {
    let mut seen = HashSet::new();
    let mut v: Vec<Id> = app.members.get(&g).into_iter().flatten().map(|m| m.user).filter(|u| seen.insert(*u)).collect();
    for r in app.nav.relationships.iter().filter(|r| r.kind == RelationshipKind::Friend) {
        if seen.insert(r.user) {
            v.push(r.user);
        }
    }
    v
}

fn recently_talked(app: &PhoenixApp) -> Vec<Target> {
    let Some(ch) = app.selected_channel else { return Vec::new() };
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in app.messages.get(&ch).into_iter().flat_map(|m| m.msgs.iter().rev()) {
        if m.author == app.me.id || !seen.insert(m.author) {
            continue;
        }
        if let Some(g) = guild
            && app.members.get(&g).is_none_or(|ms| crate::member_index::find(ms, g, m.author).is_none())
        {
            continue;
        }
        if app.users.contains_key(&m.author) {
            out.push(Target::User(m.author));
        }
        if out.len() == MODE_LIMIT {
            break;
        }
    }
    out
}

fn as_target(app: &PhoenixApp, id: Id, mode: Option<Mode>) -> Option<Target> {
    let ch = app.channels.get(&id)?;
    let t = if ch.guild.is_some() { Target::Channel(id) } else { Target::Dm(id) };
    let ok = match mode {
        None => true,
        Some(Mode::Text) => ch.guild.is_some() && !is_voice(ch.kind),
        Some(Mode::Voice) => ch.guild.is_some() && is_voice(ch.kind),
        _ => false,
    };
    ok.then_some(t)
}

fn sections(app: &PhoenixApp) -> Vec<Row> {
    let current = app.selected_channel;
    let mut taken: HashSet<Id> = HashSet::new();
    let mut previous: Vec<Row> = Vec::new();
    for id in &app.nav.switcher_history {
        if Some(*id) == current {
            continue;
        }
        if let Some(t) = as_target(app, *id, None) {
            previous.push(Row::Hit(t));
            if taken.len() < 3 {
                taken.insert(*id);
            }
        }
    }
    let mut rest: Vec<Row> = Vec::new();
    let mut mentioned: Vec<Id> = app.dms.iter().copied().filter(|d| app.channels.get(d).is_some_and(|c| c.mentions > 0)).collect();
    for g in &app.guilds {
        mentioned.extend(app.guild_channels.get(&g.id).into_iter().flatten().copied().filter(|c| app.channels.get(c).is_some_and(|c| c.mentions > 0)));
    }
    let mentioned: Vec<Target> = mentioned
        .into_iter()
        .filter(|id| Some(*id) != current && !taken.contains(id))
        .filter_map(|id| as_target(app, id, None))
        .collect();
    if !mentioned.is_empty() {
        rest.push(Row::Header(crate::t!("switcher.mentions")));
        for t in mentioned {
            if let Target::Channel(id) | Target::Dm(id) = t {
                taken.insert(id);
            }
            rest.push(Row::Hit(t));
        }
    }
    if let Some(g) = app.selected_guild {
        let unread: Vec<Target> = app
            .guild_channels
            .get(&g)
            .into_iter()
            .flatten()
            .filter_map(|id| app.channels.get(id))
            .filter(|c| {
                text_like(c.kind)
                    && c.unread
                    && !c.muted
                    && Some(c.id) != current
                    && !taken.contains(&c.id)
                    && !c.parent.and_then(|p| app.channels.get(&p)).is_some_and(|p| p.muted)
            })
            .map(|c| Target::Channel(c.id))
            .collect();
        if !unread.is_empty() {
            rest.push(Row::Header(crate::t!("switcher.unread")));
            rest.extend(unread.into_iter().map(Row::Hit));
        }
    }
    previous.truncate(if rest.is_empty() { 7 } else { 3 });
    if previous.is_empty() {
        return rest;
    }
    let mut out = vec![Row::Header(crate::t!("switcher.previous"))];
    out.extend(previous);
    out.extend(rest);
    out
}

fn header(app: &PhoenixApp, mode: Mode) -> SharedString {
    match mode {
        Mode::User => match guild_name(app, app.selected_guild) {
            Some(name) => crate::t!("switcher.search_users_in", name = name),
            None => crate::t!("switcher.search_users"),
        },
        Mode::UserGlobal => crate::t!("switcher.search_users"),
        Mode::Text => crate::t!("switcher.search_text"),
        Mode::Voice => crate::t!("switcher.search_voice"),
        Mode::Guild => crate::t!("switcher.search_guilds"),
        Mode::Game => crate::t!("switcher.search_games"),
    }
}

fn targets(hits: Vec<Hit>) -> Vec<Row> {
    let mut seen = HashSet::new();
    hits.into_iter().filter(|h| seen.insert(h.target)).map(|h| Row::Hit(h.target)).collect()
}

pub fn compute(app: &PhoenixApp, places: HashMap<u64, i64>, raw: &str) -> Vec<Row> {
    compute_with(app, &Boost::new(app, places), raw)
}

pub fn compute_with(app: &PhoenixApp, b: &Boost, raw: &str) -> Vec<Row> {
    let (q, mode) = parse(raw);
    let q = q.trim();
    let Some(mode) = mode else {
        if q.is_empty() {
            return sections(app);
        }
        return merged(app, b, q);
    };
    let here = app.selected_guild;
    let rows: Vec<Row> = if q.is_empty() {
        match mode {
            Mode::User | Mode::UserGlobal => recently_talked(app).into_iter().map(Row::Hit).collect(),
            Mode::Guild => targets(guilds(app, b, "", None, MODE_LIMIT)),
            Mode::Text => targets(channels(app, b, "", here, false, MODE_LIMIT)),
            Mode::Voice => targets(channels(app, b, "", here, true, MODE_LIMIT)),
            Mode::Game => Vec::new(),
        }
    } else {
        match mode {
            Mode::User => {
                let pool = match here {
                    Some(g) => guild_or_friends(app, g),
                    None => all_users(app),
                };
                targets(users(app, b, q, &pool, MODE_LIMIT))
            }
            Mode::UserGlobal => targets(users(app, b, q, &all_users(app), MODE_LIMIT)),
            Mode::Guild => targets(guilds(app, b, q, None, MODE_LIMIT)),
            Mode::Text => targets(channels(app, b, q, None, false, MODE_LIMIT)),
            Mode::Voice => targets(channels(app, b, q, None, true, MODE_LIMIT)),
            Mode::Game => Vec::new(),
        }
    };
    let mut out = vec![Row::Header(header(app, mode))];
    out.extend(rows);
    out
}

fn merged(app: &PhoenixApp, b: &Boost, q: &str) -> Vec<Row> {
    let pool = all_users(app);
    let skip = app.selected_guild;
    let run = |limit: usize| -> Vec<Vec<Hit>> {
        vec![users(app, b, q, &pool, limit), group_dms(app, b, q, limit), channels(app, b, q, None, false, limit), guilds(app, b, q, skip, limit)]
    };
    let mut cats = run(LIMIT);
    let filled: Vec<usize> = (0..cats.len()).filter(|i| !cats[*i].is_empty()).collect();
    if let [only] = filled.as_slice() {
        cats[*only] = run(REFETCH_LIMIT).swap_remove(*only);
    }
    let mut all: Vec<Hit> = cats.into_iter().flatten().collect();
    all.sort_by(|x, y| y.score.total_cmp(&x.score));
    targets(all)
}

pub fn open(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if app.screen != crate::app::Screen::Main {
        return;
    }
    crate::ui::pickers::load_frecency(app, cx);
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("switcher.placeholder")));
    let sub = cx.subscribe_in(&input, window, |app, _, ev: &InputEvent, window, cx| match ev {
        InputEvent::Change => update(app, cx),
        InputEvent::PressEnter { .. } => {
            if let Some(t) = app.switcher.as_ref().and_then(|s| s.results.get(s.selected).and_then(Row::target)) {
                activate(app, t, window, cx);
            }
        }
        _ => {}
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    app.switcher = Some(Switcher { input, results: Vec::new(), selected: 0, _sub: sub });
    update(app, cx);
}

pub fn update(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(sw) = &app.switcher else { return };
    let q = sw.input.read(cx).value().to_string();
    let res = compute(app, places(cx), &q);
    if let Some(sw) = &mut app.switcher {
        sw.selected = step(&res, None, true);
        sw.results = res;
    }
    cx.notify();
}

pub fn activate(app: &mut PhoenixApp, t: Target, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    app.close_switcher(window, cx);
    match t {
        Target::Channel(id) | Target::Dm(id) => app.select_channel(id, cx),
        Target::Guild(id) => app.select_guild(Some(id), cx),
        Target::User(user) => app.open_dm_with(user, cx),
    }
}

pub fn on_key(app: &mut PhoenixApp, key: &str, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(sw) = app.switcher.as_mut() else { return false };
    match key {
        "up" | "down" => {
            if sw.results.iter().any(|r| matches!(r, Row::Hit(_))) {
                sw.selected = step(&sw.results, Some(sw.selected), key == "down");
                cx.notify();
            }
            true
        }
        "tab" => true,
        _ => false,
    }
}

const W: f32 = 679.;

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

type Line = (AnyElement, SharedString, Option<SharedString>, Option<SharedString>, Option<SharedString>);

pub fn render(app: &PhoenixApp, sw: &Switcher, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let fg = tk(idx::INTERACTIVE_TEXT_DEFAULT);
    let mut rows = div().id("switcher-results").mt(px(16.)).h(px(236.)).overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("switcher-results")).flex().flex_col();
    if sw.results.is_empty() {
        rows = rows.child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_MUTED))
                .child(crate::t!("switcher.cant_find"))
                .child(div().text_color(tk(idx::TEXT_LINK)).child(crate::t!("switcher.learn_more_qs"))),
        );
    }
    for (i, row) in sw.results.iter().enumerate() {
        let t = match row {
            Row::Header(text) => {
                rows = rows.child(
                    div()
                        .h(px(34.))
                        .flex_none()
                        .px(px(10.))
                        .pt(px(4.))
                        .text_size(px(14.))
                        .line_height(px(30.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(fg)
                        .child(text.clone()),
                );
                continue;
            }
            Row::Hit(t) => *t,
        };
        let unread = match t {
            Target::Channel(id) | Target::Dm(id) => app.channels.get(&id).is_some_and(|c| c.unread),
            Target::Guild(id) => app.guilds.iter().any(|g| g.id == id && g.unread),
            Target::User(_) => false,
        };
        let (lead, name, user, note, misc): Line = match t {
            Target::Channel(id) => {
                let Some(ch) = app.channels.get(&id) else { continue };
                let guild = ch.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone());
                let note = ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
                let lead = div().w(px(20.)).h(px(16.)).flex().justify_center().child(icon(channel_icon(ch.kind), 16., fg)).into_any_element();
                (lead, ch.name.clone(), None, note, guild)
            }
            Target::Dm(id) => {
                let Some(ch) = app.channels.get(&id) else { continue };
                let user = app.users.values().find(|u| u.display_name == ch.name || u.name == ch.name);
                (avatar(user, 20.), ch.name.clone(), user.map(|u| u.name.clone()), None, None)
            }
            Target::User(id) => {
                let u = app.users.get(&id);
                (avatar(u, 20.), u.map(|u| u.display_name.clone()).unwrap_or_default(), u.map(|u| u.name.clone()), None, None)
            }
            Target::Guild(id) => {
                let Some(g) = app.guilds.iter().find(|g| g.id == id) else { continue };
                let lead = match &g.icon {
                    crate::model::ImageRef::Url(u) => gpui::img(u.clone()).size(px(20.)).rounded(px(3.)).into_any_element(),
                    _ => div()
                        .size(px(20.))
                        .rounded(px(3.))
                        .bg(tk(idx::BACKGROUND_BASE_LOW))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(8.))
                        .child(g.acronym.clone())
                        .into_any_element(),
                };
                (lead, g.name.clone(), None, Some(crate::t!("switcher.guild")), None)
            }
        };
        let selected = i == sw.selected;
        rows = rows.child(
            div()
                .id(("sw", i))
                .h(px(34.))
                .flex_none()
                .px(px(10.))
                .rounded(px(3.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if unread { tk(idx::TEXT_STRONG) } else { fg })
                .when(selected, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .on_mouse_move(cx.listener(move |a, _, _, cx| {
                    if let Some(sw) = a.switcher.as_mut()
                        && sw.selected != i
                    {
                        sw.selected = i;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |a, _, w, cx| activate(a, t, w, cx)))
                .child(div().flex_none().mr(px(5.)).child(lead))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(div().mr(px(4.)).flex_shrink(1.).min_w_0().overflow_hidden().text_ellipsis().child(name))
                        .children(user.map(|u| div().flex_none().font_weight(FontWeight::NORMAL).opacity(0.6).child(u)))
                        .children(note.map(|n| {
                            div()
                                .flex_shrink(1.)
                                .min_w_0()
                                .mt(px(1.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(px(10.))
                                .line_height(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tk(idx::TEXT_MUTED))
                                .child(n)
                        })),
                )
                .children(misc.map(|m| {
                    div()
                        .ml(px(4.))
                        .flex_none()
                        .max_w(px(140.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .opacity(0.6)
                        .child(m)
                })),
        );
    }
    let sym = |s: &'static str| {
        div()
            .mx(px(1.))
            .px(px(2.))
            .rounded(px(4.))
            .bg(tk(idx::BACKGROUND_BASE_LOWEST))
            .font_family(crate::theme::FONT_MONO())
            .child(s)
    };
    let protip = div()
        .pt(px(10.))
        .flex()
        .items_center()
        .flex_wrap()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .child(
            div()
                .mr(px(3.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_FEEDBACK_POSITIVE))
                .child(crate::t!("switcher.protip_label")),
        )
        .child(div().mr(px(3.)).child(crate::t!("switcher.protip")))
        .children(["@", "#", "!", "*", "$"].map(sym))
        .child(div().ml(px(3.)).mr(px(3.)).child(crate::t!("switcher.protip_after")))
        .child(div().text_color(tk(idx::TEXT_LINK)).child(crate::t!("switcher.learn_more")));
    div()
        .id("switcher-backdrop")
        .absolute()
        .inset_0()
        .bg(tk(idx::BACKGROUND_SCRIM))
        .flex()
        .justify_center()
        .items_center()
        .on_click(cx.listener(|a, _, w, cx| a.close_switcher(w, cx)))
        .child(
            div()
                .id("switcher")
                .w(px(W))
                .pt(px(12.))
                .px(px(20.))
                .rounded(px(8.))
                .bg(tk(idx::MODAL_BACKGROUND))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .shadow_lg()
                .text_color(tk(idx::TEXT_DEFAULT))
                .flex()
                .flex_col()
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .h(px(70.))
                        .flex_none()
                        .px(px(12.))
                        .rounded(px(8.))
                        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                        .border_1()
                        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .text_size(px(22.))
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .child(Input::new(&sw.input).appearance(false).bordered(false).text_size(px(22.))),
                )
                .child(div().h(px(262.)).flex_none().child(crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Auto).child(rows).children(crate::ui::chat::scrollbar::bar_for("switcher-results", crate::ui::chat::scrollbar::Bar::Auto))))
                .child(protip.pb(px(1.))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_follow_the_client_regex() {
        assert_eq!(parse("#gen"), ("gen".into(), Some(Mode::Text)));
        assert_eq!(parse("@bob"), ("bob".into(), Some(Mode::User)));
        assert_eq!(parse("@@bob"), ("bob".into(), Some(Mode::UserGlobal)));
        assert_eq!(parse("!vc"), ("vc".into(), Some(Mode::Voice)));
        assert_eq!(parse("*srv"), ("srv".into(), Some(Mode::Guild)));
        assert_eq!(parse("$game"), ("game".into(), Some(Mode::Game)));
        assert_eq!(parse("gen#x"), ("genx".into(), None));
        assert_eq!(parse("a@b"), ("a@b".into(), None));
    }

    #[test]
    fn arrows_wrap_and_skip_headers() {
        let h = || Row::Header("h".into());
        let t = |n| Row::Hit(Target::Guild(Id(n)));
        let rows = vec![h(), t(1), t(2), h(), t(3)];
        assert_eq!(step(&rows, None, true), 1);
        assert_eq!(step(&rows, Some(2), true), 4);
        assert_eq!(step(&rows, Some(4), true), 1);
        assert_eq!(step(&rows, Some(1), false), 4);
        assert_eq!(step(&[h()], None, true), 0);
    }

    #[test]
    fn match_scores_follow_the_client() {
        let t = Tok::new("gen");
        assert_eq!(eg("gen", &t, true), 10);
        assert_eq!(eg("general", &t, true), 7);
        assert_eq!(eg("off-general", &t, true), 5);
        assert_eq!(eg("g-e-n", &t, true), 1);
        assert_eq!(eg("g-e-n", &t, false), 0);
        assert_eq!(eg("dev general", &Tok::new("general,dev"), true), 3);
        let full = tokens("gen chat", false);
        assert!(full[0].full && full.len() == 3);
        assert_eq!(eg("gen-chat", &full[0], true), 7);
        assert!(subseq("gnr", "gaming-news-release") && !subseq("xyz", "random"));
        assert_eq!(fold("Général"), "general");
    }

    #[test]
    fn best_token_consumes_what_it_matched() {
        let mut toks = tokens("gen dev", false);
        assert_eq!(best("general", &mut toks, true), 7);
        assert_eq!(toks.len(), 2);
        assert_eq!(best("dev", &mut toks, false), 10);
        assert_eq!(toks.len(), 1);
        assert!(toks[0].full);
    }
}
