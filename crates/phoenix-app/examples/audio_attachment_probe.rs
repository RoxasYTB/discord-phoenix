use std::sync::Arc;
use std::time::{Duration, Instant};

use phoenix_api::{ClientProperties, RestClient};
use phoenix_app::discord_backend::store::{KeyringStore, TokenStore};
use phoenix_app::media::audio_file;

const SANDBOX: u64 = 1574797385463496724;
const TEST_MESSAGE: u64 = 1575263335010799749;

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let (url, name) = rt.block_on(async {
        let token = tokio::task::spawn_blocking(|| KeyringStore.load())
            .await?
            .map_err(anyhow::Error::msg)?
            .ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
        let rest = RestClient::new(token.expose(), ClientProperties::linux_desktop());
        let list = rest.request_json(reqwest::Method::GET, &format!("/channels/{SANDBOX}/messages?around={TEST_MESSAGE}&limit=1"), None).await?;
        let msg = list.as_array().and_then(|a| a.iter().find(|m| m["id"].as_str() == Some(&TEST_MESSAGE.to_string()))).cloned().ok_or_else(|| anyhow::anyhow!("test message not found"))?;
        let att = msg["attachments"].as_array().and_then(|a| a.iter().find(|x| x["content_type"].as_str().is_some_and(|t| t.starts_with("audio/")) || x["filename"].as_str().is_some_and(|n| n.ends_with(".wav")))).cloned().ok_or_else(|| anyhow::anyhow!("no attachment"))?;
        anyhow::Ok((att["url"].as_str().unwrap_or_default().to_string(), att["filename"].as_str().unwrap_or_default().to_string()))
    })?;
    println!("attachment: {name}");
    let (head, full) = rt.block_on(async {
        let http = reqwest::Client::new();
        let head = http.get(&url).header("Range", audio_file::METADATA_RANGE).send().await?.bytes().await?;
        let full = http.get(&url).send().await?.bytes().await?;
        anyhow::Ok((head, full))
    })?;
    let meta = audio_file::probe_duration(&head, &name);
    println!("metadata ({} bytes): {:?} s → {:?}", head.len(), meta, audio_file::duration_text(meta.map(|_| 0.), meta));
    let pcm = audio_file::decode(&full, &name).ok_or_else(|| anyhow::anyhow!("decode failed"))?;
    let secs = pcm.duration().as_secs_f64();
    println!("decoded {} Hz × {} ch, {} samples, {:.3} s", pcm.rate, pcm.channels, pcm.samples.len(), secs);
    let peak = pcm.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    println!("peak {peak:.3}");
    let start = Instant::now();
    let handle = phoenix_voice::audio::effects::play(Arc::new(pcm), 0., false);
    while !handle.is_stopped() && start.elapsed() < Duration::from_secs_f64(secs + 5.) {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("output stream consumed the clip at volume 0 in {:.2} s (stopped: {})", start.elapsed().as_secs_f64(), handle.is_stopped());
    Ok(())
}
