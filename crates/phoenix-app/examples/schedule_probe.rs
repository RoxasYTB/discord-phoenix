use phoenix_api::{
    ClientProperties, RestClient,
    models::Snowflake,
    rest::{CreateMessage, guild_manage::AdminCall},
};
use phoenix_app::{
    discord_backend::store::{KeyringStore, TokenStore},
    model::Id,
    ui::chat::scheduled,
};

const TEST_GUILD: u64 = 1369382919948210248;
const SANDBOX: u64 = 1574797385463496724;

fn main() -> anyhow::Result<()> {
    let what = std::env::args().nth(1).unwrap_or_else(|| "message programmé".into());
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let token =
            tokio::task::spawn_blocking(|| KeyringStore.load()).await?.map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("no token in keyring"))?;
        let rest = RestClient::new(token.expose(), ClientProperties::linux_desktop());
        let ch = rest.get_channel(Snowflake(SANDBOX)).await?;
        let guild = rest.request_json(reqwest::Method::GET, &format!("/guilds/{TEST_GUILD}"), None).await?;
        let gname = guild["name"].as_str().unwrap_or_default();
        let cname = ch.name.clone().unwrap_or_default();
        anyhow::ensure!(
            ch.guild_id == Some(Snowflake(TEST_GUILD)) && gname == "Aperture Sciences" && cname.contains("phoenix"),
            "refusing: channel/guild names do not match the sandbox channel"
        );
        println!("guard ok: {gname} / {cname}");
        let now = chrono::Local::now().timestamp();
        let at = now + 15 * 60;
        let msg = CreateMessage::text(format!("[Test Phoenix] envoyé depuis le client Rust (Phoenix) — {what}"));
        let created = rest.admin(&scheduled::create_call(Id(SANDBOX), at, &msg)).await?;
        let keys: Vec<&String> = created.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        println!("created, keys {keys:?}");
        let list = rest.get_scheduled_messages().await?;
        let mine = list.iter().find(|s| s.channel_id() == Snowflake(SANDBOX)).ok_or_else(|| anyhow::anyhow!("not listed"))?;
        println!("listed: {} at {} state {}", mine.id, mine.scheduled_timestamp, mine.state);
        anyhow::ensure!(mine.content().starts_with("[Test Phoenix]"), "content mismatch");
        let id = mine.id;
        rest.admin(&AdminCall::EditScheduled { id, at: Some(scheduled::iso(now + 20 * 60)), content: None, flags: None }).await?;
        println!("rescheduled");
        let edited = format!("{} (modifié)", mine.content());
        rest.admin(&scheduled::edit_content_call(Id(id.0), edited.clone(), mine.create_args.flags)).await?;
        let list = rest.get_scheduled_messages().await?;
        let after = list.iter().find(|s| s.id == id).ok_or_else(|| anyhow::anyhow!("edited message not listed"))?;
        anyhow::ensure!(after.content() == edited, "content edit not applied");
        println!("edited: at {} content ok", after.scheduled_timestamp);
        if std::env::var("PROBE_SEND_NOW").as_deref() == Ok("1") {
            rest.admin(&AdminCall::SendScheduledNow { id }).await?;
            println!("sent now");
        } else {
            rest.admin(&AdminCall::DeleteScheduled { id }).await?;
            println!("cancelled");
        }
        anyhow::Ok(())
    })
}
