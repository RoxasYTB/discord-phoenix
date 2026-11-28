const A: &str = "https://discord.com/assets/";
const C: &str = "https://cdn.discordapp.com/assets/content/";

fn a(f: &str) -> String {
    format!("{A}{f}")
}

pub fn connections() -> Vec<(&'static str, String, bool)> {
    let app = |n, u: &str| (n, format!("https://cdn.discordapp.com/app-icons/{u}.png?size=240"), true);
    let svg = |n, f: &str| (n, a(&format!("{f}.svg")), false);
    vec![
        app("Riot Games", "1443033465766281327/b69039088ad141a9c036f7f4f247f6ba"),
        app("Battle.net", "1356665549089800303/e9cb3d82f173c6a131fd08bbfaab9a65"),
        svg("Bluesky", "2709f058378a099d"),
        svg("Letterboxd", "6b6632ec799ccf64"),
        ("PayPal", paypal(), false),
        svg("Reddit", "adfd927dcc2049a5"),
        svg("Steam", "1f7ec18f3695d4cf"),
        svg("X", "a61999ae9bfb9658"),
        svg("eBay", "b28a7a265581b6e3"),
        svg("Crunchyroll", "94ef3e8b7fa85a2e"),
        svg("PlayStation Network", "eed203a7ec517e23"),
        svg("Spotify", "d5719388ffc613da"),
        svg("Xbox", "c4f09fda61827e19"),
        svg("Amazon Music", "e3c4aacc1a54395d"),
        svg("Bungie.net", "099cd81cf3b4cc98"),
        svg("Domain", "b4376756bcbbf1ef"),
        svg("Epic Games", "199eceff4fca1a0c"),
        svg("Facebook", "17be29f77bee4405"),
        svg("GitHub", "a35ff3e86ffa1eb2"),
        svg("Roblox", "a4d8e9b0404a2d00"),
        svg("Twitch", "4fda00c96319c8ae"),
        svg("YouTube", "0fa530ba9c04ac32"),
    ]
}

pub fn paypal() -> String {
    a("dcb64a4ff8f61b2c.svg")
}

pub fn connection_icon(kind: &str) -> Option<(String, bool)> {
    let label = super::connections::platform_label(kind);
    connections().into_iter().find(|(n, _, _)| *n == label).map(|(_, u, app)| (u, app))
}

pub fn app_icons() -> Vec<String> {
    [
        "5926a37cee0904ce", "e8396e79419904ca", "2d62663c4b493f8e", "28f89b5f78dd6f80", "96f9a2942150ba17", "22bb7108ad1c466f",
        "9af4b996c897420f", "9ad8ec824f71bd05", "5bf3279039efa66c", "3a2d57c0bde9b218", "181e40b5ea872cdb", "1e208c512641758d",
        "e14578cc054054c0", "5e5cc9b058e63987", "82c7a89d15622af4", "441d164362b711b1", "e5f2a55421a6dc7e", "9b50f7ba00ee941f",
        "a629a2a3fe1ccbcc", "685b2900a9423534", "10cd104567438715", "7ebbc1790864207a", "7ce5a942a5cc7cba",
    ]
    .iter()
    .map(|h| a(&format!("{h}.webp")))
    .collect()
}

pub fn limited_app_icon() -> String {
    a("236f9310543771ae.webp")
}

pub fn theme_banner() -> String {
    format!("{C}568728735ebc2ee3eb3a7ab29d37c1f698f413d29cee50f9a19ed2d2eefea27f.svg")
}

pub fn nitro_tier() -> String {
    a("3aaa2c2d1874c196.svg")
}
pub fn nitro_gift() -> String {
    a("9f5bdd034cc313ae.svg")
}

pub fn boost_gem() -> String {
    a("263e4cc9043cab70.svg")
}

pub fn boost_video_still() -> String {
    format!("{C}7b8d6c9c9f8bc8092089a2f55081695d40f7f303f053fcc6f944bf4b38403573.png")
}
