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
| DND, quiet mode, @silent, blocked | `K`: status DND, `quiet_mode` proto (7.5), `SUPPRESS_NOTIFICATIONS` (1<<12), blocked/ignored | `message_sounds` | ✅ unit test (quiet mode reads the new `quiet-mode` proto entry) |
| Deafen / mute | `Nc` | `watch::cues` | ✅ unit test |
| Connect / disconnect | `N_`: RTC connected → `user_join`, lost → `disconnect`, not stages | `watch::cues` | ✅ unit test |
| Others join / leave, streams | `Np` (same channel, previous count ≤ 25, not stages) | `watch::cues` (guild voice channels) | ✅ unit test; ⚠ AFK channel not excluded (no afk id in the UI model); ⚠ DM-call participants not counted |
| Moved | `Nh` `user_moved` | channel change we did not request | ✅ unit test |
| Camera | `Nu` | own `VoiceFlags.video` | ✅ unit test |
| PTT | `NE`: push-to-talk mode, not muted | `watch::on_ptt` from the PTT key hooks | ✅ |
| Incoming call | `Gc`: `call_ringing` looped, classic pack 1/1000 `call_ringing_beat` / `call_ringing_snow_halation` | `watch` loop while `incoming_call` | ✅ |
| Outgoing call | module 413528: `call_calling` loop while our call rings someone and fewer than 2 are connected | loop while a CALL_* for our DM call rings others | ✅ (no count check in DMs) |
| Activity / stage cues | `Nm`, `NI` | — | ❌ Phoenix has no activities or stage request-to-speak |
| Settings rows | Notifications > Sounds (`message1`, selected channel, `call_ringing`, disable all) and Voice & Video > Sounds (22 rows, `collapseAfter: 4`, "Show N more sounds"), "Preview Sound" plays the sound | pages.rs `B::Sound` (link plays `sounds::preview`), `B::Collapse` (4 visible + expandable "Afficher N sons de plus" with rotating 24px chevron), voice.rs `voice_sounds` (`useInlineNotice` 8px radius, 1px `--border-subtle`, 14/18 `--text-default`); rows dimmed and off while "disable all" is on | ✅ |
