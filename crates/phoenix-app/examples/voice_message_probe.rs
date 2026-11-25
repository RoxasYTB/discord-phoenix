use std::sync::Arc;

use phoenix_api::{
    ClientProperties, RestClient,
    models::Snowflake,
    rest::{CreateMessage, FileUpload, uploads},
};
use phoenix_app::discord_backend::store::{KeyringStore, TokenStore};
use phoenix_voice::audio::{
    AudioControl,
    voice_message::{self as vm, Recorder, Source},
};

const TEST_GUILD: u64 = 1369382919948210248;
const SANDBOX: (u64, &str) = (1574797385463496724, "phoenix");

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let secs: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4.5);
    let hz: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(440.);
    let tone = vm::tone(48_000, secs.clamp(1., 30.), hz, 0.4);
    println!("recording a generated {hz} Hz tone for {secs} s (no microphone)");
    let rec = Recorder::start(Arc::new(AudioControl::default()), Source::Pcm { rate: 48_000, samples: tone })?;
    std::thread::sleep(std::time::Duration::from_secs_f32(secs + 0.3));
    let rec = rec.stop()?;
    let waveform = phoenix_app::ui::chat::voice_message::waveform_b64(&rec.samples);
    println!("ogg {} bytes, {:.2} s, {} level samples, waveform {} chars", rec.ogg.len(), rec.duration_secs, rec.samples.len(), waveform.len());
    if std::env::var_os("PROBE_DRY").is_some() {
        if let Some(p) = args.get(3) {
            std::fs::write(p, &rec.ogg)?;
        }
        return Ok(());
    }
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let token =
            tokio::task::spawn_blocking(|| KeyringStore.load()).await?.map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
        let rest = RestClient::new(token.expose(), ClientProperties::linux_desktop());
        let (channel, expect) = SANDBOX;
        let ch = rest.get_channel(Snowflake(channel)).await?;
        let guild = rest.request_json(reqwest::Method::GET, &format!("/guilds/{TEST_GUILD}"), None).await?;
        let gname = guild["name"].as_str().unwrap_or_default();
        let cname = ch.name.clone().unwrap_or_default();
        anyhow::ensure!(
            ch.id == Snowflake(channel) && ch.guild_id == Some(Snowflake(TEST_GUILD)) && gname == "Aperture Sciences" && cname.contains(expect),
            "refusing: channel/guild names do not match the sandbox channel"
        );
        println!("guard ok: {gname} / {cname}");
        let intro = rest
            .send_message(
                Snowflake(channel),
                CreateMessage::text(format!(
                    "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — message vocal en réponse (tonalité {hz} Hz générée, aucun micro, {:.1} s)",
                    rec.duration_secs
                )),
            )
            .await?;
        let files = vec![FileUpload {
            filename: vm::FILENAME.into(),
            bytes: rec.ogg,
            content_type: Some(vm::CONTENT_TYPE.into()),
            duration_secs: Some(rec.duration_secs),
            waveform: Some(waveform),
            ..Default::default()
        }];
        let slots = rest.upload_files(Snowflake(channel), &files, None, uploads::UPLOAD_CHUNK).await?;
        let mut msg = CreateMessage::text(String::new()).reply_to(Snowflake(channel), intro.id, false);
        msg.flags = Some(vm::FLAG_IS_VOICE_MESSAGE);
        msg.attachments = uploads::message_attachments(&files, &slots).as_array().cloned().unwrap_or_default();
        let m = rest.send_message(Snowflake(channel), msg).await?;
        println!("voice message {} flags {}", m.id, m.flags);
        for a in &m.attachments {
            println!(
                "  {} {} bytes {:?} duration {:?} waveform {}",
                a.filename,
                a.size,
                a.content_type,
                a.duration_secs,
                a.waveform.as_deref().map_or(0, str::len)
            );
        }
        anyhow::Ok(())
    })
}
