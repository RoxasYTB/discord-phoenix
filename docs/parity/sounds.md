# Sounds

Source: the cached 2026-11-24 client bundle (`web.5233bd6fd7ed31db.js` and the settings chunk),
read locally. No sound file is committed: `crates/phoenix-app/src/sounds/assets.rs` takes each
mp3 from the local Discord cache (sparse `_s` range entries) or from `discord.com/assets/<file>`
(the URL the client loads), into `~/.cache/discord-phoenix/sounds/`. The name -> file table is
parsed from the cached main bundle (webpack context module 696354), with the same table from
this build as a fallback. Playback: `phoenix-voice` `audio::effects` (one cpal stream on the
default output, closed after 3 s idle); decoding: symphonia (mp3 only).

`PHOENIX_SOUND_LOG=1` prints every cue (`phoenix-sound: <name> volume=<v> loop=<bool>`).

| Rule | Client source | Phoenix | State |
|---|---|---|---|
| Volume | `WebAudioSound`: `min(outputVolume/100 × volume, 1)`, cues pass `.4` | `sounds::effective_volume`, voice output volume | ✅ unit test |
| Output | `setSinkId("default")` for the DEFAULT channel | system default output | ✅ |
| Streamer mode | `playSound` returns when `StreamerModeStore.disableSounds` (enabled && "Disable sounds", default on) | `streamer_mutes` | ✅ unit test |
| Disabled sounds | `isSoundDisabled(n) = disableAllSounds \|\| disabledSounds.includes(n)` | `LocalPrefs` `notification_sounds` + `snd-<name>` switches | ✅ unit test |
| Message throttle | names containing "message": `throttle(…, 1000, {leading: true})` | `throttled` | ✅ |
| New message | NotificationStore MESSAGE_CREATE: `shouldNotify(msg, ch, !focused)` → `message1` .4 | `sounds::on_message` (gateway pump) | ✅ unit test; live check below |
| Viewed channel | not when it is the selected channel and the window has focus | `MessageCtx.focused/selected` (focus read at render) | ✅ unit test |
| Selected channel option | `notifyMessagesInSelectedChannel` → `message3` | `snd-msg-cur` | ✅ unit test |
| Mutes | `allowNoMessages` (guild / category / channel muted, "Nothing"), mentions-only levels | `notifications::should_notify` / `allow_no_messages` (wt/mute) | ✅ |
| DND, quiet mode, @silent, blocked | `K`: status DND, `quiet_mode` proto (7.5), `SUPPRESS_NOTIFICATIONS` (1<<12), blocked/ignored | `message_sounds` (uses `f.blocked` which covers blocked + ignored users) | ✅ unit test |
| Deafen / mute | `Nc` | `watch::cues` | ✅ unit test |
| Connect / disconnect / reconnect | `N_`: RTC connected → `user_join`, reconnected → `reconnect`, lost → `disconnect`, not stages | `watch::cues` (`Snap.reconnecting`) | ✅ unit test |
| Others join / leave, streams | `Np` (same channel, previous count ≤ 25, not stages) | `watch::cues` (guild voice channels) | ✅ unit test; AFK channel excluded like the client (`Nd`: `guild.afkChannelId === channel.id` returns before the join / leave / stream cues; `model::Guild.afk_channel`, `Snap.afk`); DM calls counted like the client (`countVoiceStatesForChannel(call) − isInChannel`, no guild check): DM voice states are kept in `ParityUi::call_users` (seeded by `CALL_CREATE.voice_states`, moved by `VOICE_STATE_UPDATE` on a DM / group DM, cleared by `CALL_DELETE`) instead of the DM's `voice_users`, which holds the recipient for the avatar |
| Moved | `Nh` `user_moved` | channel change we did not request | ✅ unit test |
| Camera | `Nu` | own `VoiceFlags.video` | ✅ unit test |
| PTT | `NE`: push-to-talk mode, not muted | `watch::on_ptt` from the PTT key hooks | ✅ |
| Incoming call | `Gc`: `call_ringing` looped, classic pack 1/1000 `call_ringing_beat` / `call_ringing_snow_halation`, muted in DND / quiet mode | `watch` loop while `incoming_call` and not DND/quiet | ✅ |
| Outgoing call | module 413528: `call_calling` loop while our call rings someone and fewer than 2 are connected | loop while a CALL_* for our DM call rings others and fewer than 2 voice states are in the call | ✅ |
| Connected sound (`discodo`) | connecting screen `CF.componentDidUpdate`: on `connected` while its video is mounted, only when the `discodoEasterEgg` storage key is on (volume 1, no disabled-sound check); the key is toggled by 16 Home clicks less than 500 ms apart (`nH`), which play `discodo` (on) or `user_leave` (off) through `playSound` and show `nkq1l+` / `Be8Q5E` in the Home tooltip for 1 s | `ui::boot::render` plays `discodo` when the connecting screen starts to fade on the main screen and `discodo-easter-egg` is on; `server_bar::discodo_click` / `on_home_click` toggle it | ✅ unit test (`sixteen_quick_home_clicks_toggle_discodo`); off by default like the client, so a normal start stays silent. The `custom_notification_sounds_discodo` key the client also sets only unlocks a soundpack, and Phoenix has no soundpack picker |
| Activity / stage cues | `Nm`, `NI` | — | ❌ Phoenix has no activities or stage request-to-speak |
| Settings rows | Notifications > Sounds (`message1`, selected channel, `call_ringing`, disable all) and Voice & Video > Sounds (22 rows, `collapseAfter: 4`, "Show N more sounds"), "Preview Sound" plays the sound | pages.rs `B::Sound` (link plays `sounds::preview`), `B::Collapse` (4 visible + expandable "Afficher N sons de plus" with rotating 24px chevron), voice.rs `voice_sounds` (`useInlineNotice` 8px radius, 1px `--border-subtle`, 14/18 `--text-default`); rows dimmed and off while "disable all" is on | ✅ |
