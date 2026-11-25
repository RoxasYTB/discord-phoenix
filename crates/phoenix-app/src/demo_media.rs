use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use futures::{AsyncReadExt, future::join_all};
use gpui::http_client::HttpClient;

use crate::{discord_backend::convert::AVATAR_SIZE, media::cache::CachingHttpClient};

pub struct DemoPerson {
    pub name: &'static str,
    pub display: &'static str,
    pub avatar: (u64, &'static str),
    pub tag: Option<(u64, &'static str, &'static str)>,
}

const fn p(name: &'static str, display: &'static str, avatar: (u64, &'static str), tag: Option<(u64, &'static str, &'static str)>) -> DemoPerson {
    DemoPerson { name, display, avatar, tag }
}

pub const PEOPLE: &[DemoPerson] = &[
    p("phoenix", "Phoenix", (724847846897221642, "3fe240e01286b9ddd0c1add42b03325a"), Some((1369382919948210248, "GLAD", "68bd043e2369a73dd25501784a086e5a"))),
    p("wumpus", "Wumpus", (1549105151124701271, "52ff7e1ce55b1fe819e89554b38afa17"), None),
    p("clyde", "Clyde", (1231336466035245140, "fad2c9bdbedfad2291e5c77e6e3b8263"), None),
    p("ferris", "Ferris 🦀", (1161686474094301367, "130959d4b0ec7b37d4ebf4cca55874f8"), Some((399812551963049995, "ARCH", "176416ced6db0ec5ff8014ce2be3297f"))),
    p("nelly", "Nelly", (437297137257152513, "9ca16a6dfbba57be58c955409adb0ccb"), None),
    p("lester", "Lester", (1452229179625181207, "a78fa107df8c6c867e36061f61099d30"), Some((1554164635425443961, "ROOT", "e132c7dbddfd3071bf374d8f428db66c"))),
    p("aiko", "Aiko", (204774577027416064, "e467faa5940feb75c2d2909c48a07caf"), Some((500603666114281473, "SUNY", "ead40c6aa63f3427a25947f36b63bc0c"))),
    p("kaze", "kaze", (1377939901995155547, "b5d9b4eebe4a79dc2d1d0078a4aa4af1"), Some((1573331025076035706, "WM", "dea4f5f8e9aaa736c9b599506226508e"))),
    p("mira", "Mira", (1174259316329566213, "20a615c71568168343d200cbb37378b0"), None),
    p("toast", "toast", (1024742636709486683, "4afeba4b3adad7f07c3409da65817495"), None),
    p("orbit", "Orbit", (1141909667790979082, "00bac4cad20a71164a6bfba3778fe626"), Some((268882317391429632, "MATH", "9b43d5ec59ffd0a187b7d343ac707643"))),
    p("pixel", "pixel.dev", (1054876714301411390, "a_86a829c31cb097eda7dc547bcbc3eb99"), Some((1307018714557841438, "</>", "595e5c37c23a143735d31309a376b490"))),
    p("quartz", "Quartz", (1059125265411870750, "e935f29b87544cb0e315f25934c02f0b"), Some((696767722758864978, "NERD", "ee0e4ebbd87841efb018826a0992eb48"))),
    p("rune", "Rune", (1105952558524149811, "460ba2530a32dde83f71914b746b19f4"), Some((516367331358801950, "D&D", "4ef966b9bdd3ca7155e184e893314cd6"))),
    p("sable", "Sable", (1564412615512555522, "9fc7490f9878725bd84bca26ee77b4ee"), Some((1448911152997335135, "SSH", "04197b582bed6cb17f5c21c83a29a2b4"))),
    p("tidal", "Tidal", (742343474921078814, "d18569377734562eac785cee4cd211dc"), Some((1334818866630627409, "FR", "766f906c6c505e67a4cba98cf00eef31"))),
    p("umbra", "Umbra", (1562534388577734808, "f00950270d9accf6839fb58f2b32db8e"), None),
    p("vexel", "Vexel", (1258452885084049531, "cee2474d85804ea4d39960be51eee039"), Some((1518245134632751124, "DATA", "22957f5661c149eefe1ba25d2bec7060"))),
    p("willow", "Willow", (1147105618642087966, "05772fc7cf1cd9b7604ad3030da77f18"), Some((807537556002308147, "CAT", "683bf9bd8740cfb380134460ddeb754f"))),
    p("xeno", "xeno", (889843803660365864, "310952a63bfff2047ad7e62284791053"), None),
    p("yuzu", "Yuzu", (1231673971142037548, "9e336b2934c536ca9a9b0062ceae8fbf"), Some((779094028327059540, "WUFF", "752517ac009348c6be3e9c4eb004c363"))),
    p("zephyr", "Zephyr", (487572195909042187, "294aa63af63f5e443aa05e4d7ffcbd67"), Some((978336546065383535, "FXDL", "7f968d34689b032120d7a0f2a3d69b83"))),
    p("gpui-bot", "GPUI Bot", (995010152535179314, "a_e64a323cb65ce85ab8bafc70c58e9ed7"), None),
];

impl DemoPerson {
    pub fn avatar_url(&self) -> String {
        let (id, hash) = self.avatar;
        let ext = if hash.starts_with("a_") { "gif" } else { "png" };
        format!("{}/avatars/{id}/{hash}.{ext}?size={AVATAR_SIZE}", phoenix_api::models::CDN_BASE)
    }

    pub fn badge_url(&self) -> Option<String> {
        let (guild, _, badge) = self.tag?;
        let pg = phoenix_api::models::PrimaryGuild {
            identity_guild_id: Some(phoenix_api::models::Snowflake(guild)),
            identity_enabled: Some(true),
            tag: self.tag.map(|t| t.1.to_owned()),
            badge: Some(badge.to_owned()),
        };
        pg.badge_url()
    }
}

pub fn urls() -> Vec<String> {
    PEOPLE.iter().flat_map(|p| std::iter::once(p.avatar_url()).chain(p.badge_url())).collect()
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("phoenix-demo-media")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

pub fn client() -> Arc<dyn HttpClient> {
    let ua = phoenix_api::ClientProperties::default();
    let inner = Arc::new(crate::discord_backend::http::CdnHttpClient::new(runtime().handle().clone(), ua.user_agent()));
    Arc::new(CachingHttpClient::new(inner))
}

pub fn prefetch(http: &Arc<dyn HttpClient>) -> usize {
    let jobs = urls().into_iter().map(|url| {
        let http = http.clone();
        async move {
            let Ok(mut r) = http.get(&url, Default::default(), true).await else { return false };
            let mut bytes = Vec::new();
            r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok() && !bytes.is_empty()
        }
    });
    let all = join_all(jobs);
    let timed = async move { tokio::time::timeout(Duration::from_secs(20), all).await.unwrap_or_default() };
    runtime().block_on(timed).into_iter().filter(|ok| *ok).count()
}

pub fn install(cx: &mut gpui::App) {
    let http = client();
    if std::env::var_os("PHX_DEMO_NO_PREFETCH").is_none() {
        let n = prefetch(&http);
        tracing::info!("demo media: {n}/{} cached", urls().len());
    }
    cx.set_http_client(http);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_demo_tag_has_a_badge() {
        for p in PEOPLE {
            if let Some((g, tag, b)) = p.tag {
                assert!(!tag.is_empty() && b.len() == 32 && g > 0, "{}", p.name);
                assert_eq!(p.badge_url().as_deref(), Some(format!("{}/guild-tag-badges/{g}/{b}.png?size=16", phoenix_api::models::CDN_BASE).as_str()));
            }
            assert!(p.avatar_url().contains(&p.avatar.0.to_string()));
        }
    }

    #[test]
    fn demo_avatars_are_distinct() {
        let mut ids: Vec<u64> = PEOPLE.iter().map(|p| p.avatar.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PEOPLE.len());
    }

    #[test]
    fn demo_urls_pass_the_media_host_filter() {
        assert!(urls().iter().all(|u| crate::discord_backend::http::host_allowed(u)));
    }
}
