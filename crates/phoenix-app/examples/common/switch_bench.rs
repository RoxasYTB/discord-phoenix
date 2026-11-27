use std::time::{Duration, Instant};

use gpui::{AnyWindowHandle, Context, Entity, HeadlessAppContext};
use phoenix_app::{app::PhoenixApp, model::{ChannelKind, Id}};

#[derive(Clone, Copy)]
enum Target {
    Guild(Id),
    Channel(Id),
}

struct Sample {
    first_us: u128,
    total_us: u128,
    frames: u32,
}

fn draw(cx: &mut HeadlessAppContext, win: AnyWindowHandle) {
    cx.update_window(win, |_, w, _| w.refresh()).ok();
    cx.run_until_parked();
}

fn loaded(cx: &mut HeadlessAppContext, v: &Entity<PhoenixApp>) -> bool {
    cx.update(|cx| {
        let a = v.read(cx);
        a.selected_channel.and_then(|c| a.messages.get(&c)).is_some_and(|e| e.loaded_once && !e.rendered.is_empty())
    })
}

fn act(v: &Entity<PhoenixApp>, cx: &mut HeadlessAppContext, f: impl FnOnce(&mut PhoenixApp, &mut Context<PhoenixApp>)) {
    cx.update(|cx| v.update(cx, f));
}

fn hover(cx: &mut HeadlessAppContext, win: AnyWindowHandle, v: &Entity<PhoenixApp>, t: Target, ms: u64) {
    act(v, cx, |a, cx| match t {
        Target::Guild(g) => phoenix_app::history_ui::hover_guild(a, g, cx),
        Target::Channel(c) => phoenix_app::history_ui::hover_channel(a, c, cx),
    });
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        std::thread::sleep(Duration::from_millis(16));
        draw(cx, win);
    }
    if let Target::Channel(c) = t {
        act(v, cx, |a, cx| phoenix_app::history_ui::preload(a, c, cx));
        std::thread::sleep(Duration::from_millis(16));
        draw(cx, win);
    }
}

fn switch(cx: &mut HeadlessAppContext, win: AnyWindowHandle, v: &Entity<PhoenixApp>, t: Target) -> Sample {
    let t0 = Instant::now();
    act(v, cx, |a, cx| match t {
        Target::Guild(g) => a.select_guild(Some(g), cx),
        Target::Channel(c) => a.select_channel(c, cx),
    });
    draw(cx, win);
    let first_us = t0.elapsed().as_micros();
    let mut frames = 1;
    while !loaded(cx, v) && t0.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(1));
        draw(cx, win);
        frames += 1;
    }
    Sample { first_us, total_us: t0.elapsed().as_micros(), frames }
}

fn report(name: &str, s: &[Sample]) {
    if s.is_empty() {
        return;
    }
    let instant = s.iter().filter(|x| x.frames == 1).count();
    let mut first: Vec<u128> = s.iter().map(|x| x.first_us).collect();
    let mut total: Vec<u128> = s.iter().map(|x| x.total_us).collect();
    first.sort_unstable();
    total.sort_unstable();
    let n = s.len();
    println!(
        "switch_bench {name} n={n} instant={instant}/{n} first_frame_ms p50={:.1} max={:.1} to_messages_ms p50={:.1} max={:.1}",
        first[n / 2] as f64 / 1000.,
        first[n - 1] as f64 / 1000.,
        total[n / 2] as f64 / 1000.,
        total[n - 1] as f64 / 1000.
    );
}

pub fn run(cx: &mut HeadlessAppContext, win: AnyWindowHandle, v: &Entity<PhoenixApp>, n: usize) {
    let hover_ms: u64 = std::env::var("PHX_SWITCH_HOVER").ok().and_then(|x| x.parse().ok()).unwrap_or(0);
    let (guilds, channels, dms) = cx.update(|cx| {
        let a = v.read(cx);
        let guilds: Vec<Id> = a.guilds.iter().map(|g| g.id).filter(|g| Some(*g) != a.selected_guild && a.guild_channels.contains_key(g)).take(n).collect();
        let home = guilds.first().copied();
        let channels: Vec<Id> = home
            .and_then(|g| a.guild_channels.get(&g))
            .into_iter()
            .flatten()
            .copied()
            .filter(|c| !a.is_trap_channel(*c) && a.channels.get(c).is_some_and(|ch| matches!(ch.kind, ChannelKind::Text | ChannelKind::Announcement)))
            .skip(1)
            .take(n)
            .collect();
        let dms: Vec<Id> = a.dms.iter().copied().filter(|d| a.channels.contains_key(d)).take(n).collect();
        (guilds, channels, dms)
    });
    let mut plan: Vec<(&str, Target)> = guilds.iter().map(|g| ("guild", Target::Guild(*g))).collect();
    plan.extend(channels.iter().map(|c| ("channel", Target::Channel(*c))));
    plan.extend(dms.iter().map(|d| ("dm", Target::Channel(*d))));
    for pass in ["first", "again"] {
        let mut by: Vec<(&str, Vec<Sample>)> = vec![("guild", Vec::new()), ("channel", Vec::new()), ("dm", Vec::new())];
        for (kind, t) in &plan {
            if pass == "first" && hover_ms > 0 {
                hover(cx, win, v, *t, hover_ms);
            }
            let s = switch(cx, win, v, *t);
            println!("switch {pass} {kind} first_frame_ms={:.1} to_messages_ms={:.1} frames={}", s.first_us as f64 / 1000., s.total_us as f64 / 1000., s.frames);
            if let Some((_, l)) = by.iter_mut().find(|(k, _)| *k == *kind) {
                l.push(s);
            }
            let settle = Instant::now();
            while settle.elapsed() < Duration::from_millis(400) {
                std::thread::sleep(Duration::from_millis(16));
                draw(cx, win);
            }
        }
        for (kind, l) in &by {
            report(&format!("{pass}_{kind}_hover{hover_ms}"), l);
        }
    }
}
