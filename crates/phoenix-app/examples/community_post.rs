use phoenix_api::{
    ClientProperties, RestClient,
    models::Snowflake,
    rest::{CreateMessage, FileUpload},
};
use phoenix_app::discord_backend::store::{KeyringStore, TokenStore};
use serde_json::{Value, json};

const GUILD: u64 = 1574895284901777448;
const GUILD_NAME: &str = "Discord | Project Phoenix EARLY ACCESS & BETA [UNOFFICIAL]";
const CHANNELS: &[(&str, u64, &str)] = &[
    ("annonces", 1574895965171617982, "📢・annonces"),
    ("captures", 1574895971513532477, "📸・captures"),
    ("avances", 1574898034859581631, "⚙️・avancés"),
    ("changelogs", 1575141380785836133, "📋・changelogs"),
];
const USAGE: &str = "usage: community_post check|send|edit|pin <annonces|captures|avances|changelogs> [<text file> [images...] | <message id> [<text file>]]";

fn channel(key: &str) -> anyhow::Result<(u64, &'static str)> {
    CHANNELS
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, id, name)| (*id, *name))
        .ok_or_else(|| anyhow::anyhow!("refusing: channel {key} is not on the allow list"))
}

fn text(path: &str) -> anyhow::Result<String> {
    let t = std::fs::read_to_string(path)?.trim_end().to_owned();
    anyhow::ensure!(!t.is_empty(), "empty text");
    anyhow::ensure!(t.chars().count() <= 2000, "text is {} chars, over 2000", t.chars().count());
    anyhow::ensure!(!t.contains("@everyone") && !t.contains("@here"), "refusing: @everyone / @here");
    anyhow::ensure!(!t.contains("<@"), "refusing: mentions");
    Ok(t)
}

fn image(path: &str) -> anyhow::Result<FileUpload> {
    anyhow::ensure!(
        (path.contains("docs/screenshots/") || path.contains("/reviewed-screenshots/")) && path.ends_with(".png"),
        "refusing: {path} is not a reviewed screenshot"
    );
    Ok(FileUpload {
        filename: std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy().into_owned(),
        bytes: std::fs::read(path)?,
        content_type: Some("image/png".into()),
        ..Default::default()
    })
}

async fn own_message(rest: &RestClient, ch: u64, id: u64, me: &str) -> anyhow::Result<Value> {
    let v = rest.request_json(reqwest::Method::GET, &format!("/channels/{ch}/messages?around={id}&limit=1"), None).await?;
    let m = v
        .as_array()
        .and_then(|a| a.iter().find(|m| m["id"].as_str() == Some(&id.to_string())))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("message {id} not found in the channel"))?;
    anyhow::ensure!(m["author"]["id"].as_str() == Some(me), "refusing: message {id} is not the account's own");
    Ok(m)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() >= 2, "{USAGE}");
    let op = args[0].clone();
    let (ch, expect) = channel(&args[1])?;
    let rest_args = args[2..].to_vec();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let token =
            tokio::task::spawn_blocking(|| KeyringStore.load()).await?.map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
        let rest = RestClient::new(token.expose(), ClientProperties::linux_desktop());
        let guild = rest.request_json(reqwest::Method::GET, &format!("/guilds/{GUILD}"), None).await?;
        let chan = rest.get_channel(Snowflake(ch)).await?;
        let me = rest.request_json(reqwest::Method::GET, "/users/@me", None).await?;
        let me = me["id"].as_str().unwrap_or_default().to_owned();
        anyhow::ensure!(
            guild["name"].as_str() == Some(GUILD_NAME) && chan.guild_id == Some(Snowflake(GUILD)) && chan.name.as_deref() == Some(expect),
            "refusing: guild or channel name does not match"
        );
        println!("guard ok: {GUILD_NAME} / {expect}");
        match op.as_str() {
            "check" => {
                if let Some(path) = rest_args.first() {
                    let t = text(path)?;
                    for p in &rest_args[1..] {
                        let f = image(p)?;
                        println!("image {} {} bytes", f.filename, f.bytes.len());
                    }
                    println!("text ok, {} chars", t.chars().count());
                }
            }
            "send" => {
                let path = rest_args.first().ok_or_else(|| anyhow::anyhow!("{USAGE}"))?;
                let t = text(path)?;
                let files = rest_args[1..].iter().map(|p| image(p)).collect::<anyhow::Result<Vec<_>>>()?;
                anyhow::ensure!(files.len() <= 10, "at most 10 images");
                let mut msg = CreateMessage::text(t);
                msg.allowed_mentions = Some(json!({ "parse": [] }));
                let m = if files.is_empty() {
                    rest.send_message(Snowflake(ch), msg).await?
                } else {
                    rest.send_message_with_files(Snowflake(ch), msg, files).await?
                };
                println!("sent https://discord.com/channels/{GUILD}/{ch}/{}", m.id);
            }
            "edit" => {
                anyhow::ensure!(rest_args.len() == 2, "{USAGE}");
                let id: u64 = rest_args[0].parse()?;
                let t = text(&rest_args[1])?;
                own_message(&rest, ch, id, &me).await?;
                let m = rest.edit_message(Snowflake(ch), Snowflake(id), &t).await?;
                println!("edited https://discord.com/channels/{GUILD}/{ch}/{}", m.id);
            }
            "pin" => {
                anyhow::ensure!(args[1] == "avances" && rest_args.len() == 1, "pins only in avances");
                let id: u64 = rest_args[0].parse()?;
                let m = own_message(&rest, ch, id, &me).await?;
                if m["pinned"].as_bool() == Some(true) {
                    println!("already pinned");
                } else {
                    rest.pin_message(Snowflake(ch), Snowflake(id)).await?;
                    println!("pinned https://discord.com/channels/{GUILD}/{ch}/{id}");
                }
            }
            _ => anyhow::bail!("{USAGE}"),
        }
        anyhow::Ok(())
    })
}
