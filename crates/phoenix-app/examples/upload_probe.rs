use std::sync::Arc;

use phoenix_api::{
    ClientProperties, RestClient,
    models::Snowflake,
    rest::{CreateMessage, FileUpload},
};
use phoenix_app::discord_backend::store::{KeyringStore, TokenStore};

const TEST_GUILD: u64 = 1369382919948210248;
const GENERAL: (u64, &str) = (1403447573389381652, "général");
const SANDBOX: (u64, &str) = (1574797385463496724, "phoenix");

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 3, "usage: upload_probe \"<what is tested>\" <file>[:spoiler][:alt=text] ...");
    let what = args[1].clone();
    let (channel, expect) = match std::env::var("PROBE_CHANNEL").as_deref() {
        Ok("general") => GENERAL,
        _ => SANDBOX,
    };
    let chunk = std::env::var("PROBE_CHUNK_KIB")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
        .map_or(phoenix_api::rest::uploads::UPLOAD_CHUNK, |k| k * 1024);
    anyhow::ensure!(chunk.is_multiple_of(256 * 1024), "chunk must be a multiple of 256 KiB");
    let mut files = Vec::new();
    for spec in &args[2..] {
        let mut parts = spec.split(':');
        let path = parts.next().unwrap_or_default();
        let mut f = FileUpload {
            filename: std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy().into_owned(),
            bytes: std::fs::read(path)?,
            content_type: None,
            description: None,
            spoiler: false,
        };
        for p in parts {
            if p == "spoiler" {
                f.spoiler = true;
            } else if let Some(a) = p.strip_prefix("alt=") {
                f.description = Some(a.to_owned());
            }
        }
        files.push(f);
    }
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let token = tokio::task::spawn_blocking(|| KeyringStore.load())
            .await?
            .map_err(anyhow::Error::msg)?
            .ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
        let rest = RestClient::new(token.expose(), ClientProperties::linux_desktop());
        let ch = rest.get_channel(Snowflake(channel)).await?;
        let guild = rest.request_json(reqwest::Method::GET, &format!("/guilds/{TEST_GUILD}"), None).await?;
        let gname = guild["name"].as_str().unwrap_or_default();
        let cname = ch.name.clone().unwrap_or_default();
        anyhow::ensure!(
            ch.guild_id == Some(Snowflake(TEST_GUILD)) && gname == "Aperture Sciences" && cname.contains(expect),
            "refusing: channel/guild names do not match the test channel"
        );
        println!("guard ok: {gname} / {cname}");
        let content = format!("[Test Phoenix] envoyé depuis le client Rust (Phoenix) — {what}");
        let progress: phoenix_api::rest::uploads::Progress = Arc::new(|a, b| {
            println!("progress {a}/{b}");
            true
        });
        let mut msg = CreateMessage::text(content);
        let t = std::time::Instant::now();
        let m = rest.send_message_with_uploads_chunked(Snowflake(channel), &mut msg, files, Some(progress), chunk).await?;
        println!("sent in {:?}, message {}", t.elapsed(), m.id);
        for a in &m.attachments {
            println!("  {} {} bytes {:?} alt={:?}", a.filename, a.size, a.content_type, a.description);
        }
        anyhow::Ok(())
    })
}
