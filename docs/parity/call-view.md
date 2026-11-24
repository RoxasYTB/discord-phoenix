# Voice call view (2026 vanilla client)

Reference dump: `dumps/vanilla-call-view.json` (live vanilla client, focused Go Live + participant strip, chat closed).
Code: `crates/phoenix-app/src/ui/call_view.rs`. Offscreen render: `cargo run -p phoenix-app --example call_view -- out.png [chat]` (mock voice state, never joins voice).

| element | CSS source (file + selector prefix) | property | CSS value | our code | status |
|---|---|---|---|---|---|
| call container | callContainer_hbKzX7 | background / border-top | --black / 1px --app-frame-border | call_view.rs `render` | ✅ |
| top bar | upperContainer_ (dump #1252) | height / padding-inline | 33px / 24px | `top_bar` | ✅ |
| top bar title | title_ h1 body-md/medium (#1255) | font | 16px / 500 / 20px, --text-strong | `top_bar` | ✅ |
| channel icon | channelIcon_ svg (#1257) | size / colour | 20px, --channel-icon | `top_bar` | ✅ |
| chat toggle | chatIcon_Fy2P0m (#1288) | size | 24px, toolbar gap 20 | `top_bar` | ✅ |
| tile | tile_p87Rny (#1405) | radius / overflow | --radius-sm 8px / hidden | `render_tile` | ✅ |
| tile gap | tileSizer_ (#1403, #1448) | spacing | 8px | `grid`, `render` | ✅ |
| tile background | background_ (#1456) | background | avatar dominant colour | `tile_color` | ✅ |
| avatar | avatarWrapper_ (#1436) | size | 80px circle | `render_tile` | ✅ |
| speaking ring | border_p87Rny (#1447) | box-shadow | inset 2px --status-positive + inset 3px --black | `render_tile` | ✅ |
| name / mute pill | experimentOverlayTitle_7NOr2f (#1504) | bg / radius / padding / gap / font | --control-overlay-secondary-background-default / 8 / 6 8 / 4 / 14/18 | `render_tile` pill | ✅ |
| mute/deaf icon | titleIcon_ (#1505) | size | 16px | `render_tile` | ✅ |
| stream placeholder tile | emptyPreviewContainer_ (#1408) | bg | --background-base-lower | `render_tile` stream | ✅ |
| LIVE pill | liveIndicator_ (#1413) | font / radius / bg | 12px 600 / 4px / --status-danger | `render_tile` | ✅ |
| grid layout (no focus) | - | columns | biggest 16:9 tile | `grid` | ✅ (click focused tile → grid) |
| spotlight focused tile | videoSizer_QKgWuD / focusedVideo_QKgWuD (#1372) | box | 684x387 16:9, 72px below top bar, 8 above strip | `render` focus branch | ✅ |
| participants strip | root_coTPKH / tileSizer_coTPKH (#1402-#1514) | height / tile / gap | 112 / (W-gaps)/n ≈135 / 8 | `render` STRIP_H | ✅ |
| button section | buttonSection_9bvEB0 (#1298) | bg / border / radius / gap / padding | --background-surface-high / 1px --border-muted / 12 / 8 / 4 | `section` | ✅ |
| center button | centerButton_K9Xq13 (#1301) | size / radius | 40x40, 8 (attached 8 0 0 8) | `center_button` | ✅ |
| caret nub | contextMenuNub_.attachedCaret_ (#1306) | size / radius / margin | 20x40 / 0 8 8 0 / 1px | `center_button` | ✅ |
| green state | greenGlow_K9Xq13 (#1322) | bg / colour | --opacity-green-12 / --green-300 | `Look::Green` | ✅ |
| red state | redGlow_K9Xq13 | bg / colour | --opacity-red-12 / --red-400 | `Look::Red` | ✅ |
| section gap | wrapper_ (#1297) | gap | 12px | `bottom_controls` | ✅ |
| disconnect | disconnect_K9Xq13.fullRegionButton_ (#1354) | size / radius / bg | 65x49 / 12 / --control-critical-primary-background-default | `bottom_controls` | ✅ |
| edge controls | edgeControls_ (#1291, #1358) | width / gap / icon | 160 / 24 / 24px | `bottom_controls` | ✅ |
| bottom bar | bottomControls_ (#1290) | height / padding / bottom margin | 49 / 20 / 12 | `bottom_controls` | ✅ |
| side chat panel | channelChatWrapper_hbKzX7 + JS store `callChatSidebarWidth ?? 480` (min 450) | bg / width | --background-base-lowest / 480 | `render` CHAT_W=480, no member list, call column min_w_0 | ✅ |
| hover-only name pills (idle opacity 0) | .idle_7NOr2f{opacity:0}, JS IDLE_TIMEOUT:2e3 | opacity | 0 after 2000 ms without mouse move | `idle()` / `on_mouse_move` | ✅ |
| tray / camera icons | icon provider WindowLaunchIcon, FullscreenEnterIcon, VideoSlashIcon (path data matches dump #1362/#1366) | - | - | icons/names.rs `call/*` | ✅ |
| tray responsive collapse | call controls module `bf(width)`: <=650 NO_INVITE, <=550 MINIMAL; bottomControls_rTsZa_ grid minmax(max-content,1fr) auto minmax(max-content,1fr) | invite / 2nd section hidden | 650 / 550 px | `collapse()` / `bottom_controls` | ✅ 1280 px + chat: hang-up fully visible (call_view example) |
| overlay fade | overlayContainer_7NOr2f, actionRow_FowdXv | transition | opacity .2s ease-in-out | `overlay_opacity()` (smoothstep, raf while fading) | ✅ |
| participants strip toggle | participantsButton_1uD6uZ (#1394-#1400) | box / bg / radius / shadow | 60x32, pad 4, --primary-630 (hover 700), r48, --shadow-border+--shadow-high, caret+members 24 | `strip_button` | ✅ |
| mic / camera icon | lottieIcon_69Onuk (#1303, #1312) | toggle animation | Lottie animation on toggle, static at rest | static end-frame icon | ⛔ rest state matches; animation needs a Lottie player. Checked the client's animations in the local cache (Lottie_HoverOnOff_Camera, Lottie_HoverOnOff_Microphone): 4–5 precomp layers, masks, 3 trim paths, 10–17 animated properties each. A subset renderer would need precomps, masks, trim paths (path length) and bezier easing, plus runtime extraction from the cache (the JSON can't be committed). That is not cheap for two icons |

## Soundboard (lot 1, wt/voice, 2026-11-24)

Live reference: official client joined to the test voice channel `〢🔊・phoenix-vocal` (`1574900893613039696`, Aperture Sciences), mic muted, Onyx theme, 1536x840 CSS px at 125 % zoom. Dumps (private) in `/mnt/windata/project-phoenix/compare/voice/pass{1,2,3}`. JS: `web.b63d846b329d6470.js` (SoundboardManager `_playSound`, picker categories, `SEND_SOUNDBOARD_SOUND`); CSS: `449933…` (`picker_1lN7Eh`, `_-SUGNi`, `_6EKaag`), `967850…` (`_N99eHz`), `916885…` (`_X9_kMu`), `655282…` (`border_7NOr2f`).

| element | source | value | Phoenix | status |
|---|---|---|---|---|
| send | `SEND_SOUNDBOARD_SOUND` | `POST /channels/{id}/send-soundboard-sound` `{sound_id, emoji_id, emoji_name, source_guild_id (not for "0")}` | `RestClient::send_soundboard_sound` | ✅ request shape from the bundle; live POST seen from the official client (pass3) |
| receive | gateway `VOICE_CHANNEL_EFFECT_SEND` (`sound_id`, `sound_volume`, `user_id`, `channel_id`, `emoji`, `animation_type/id`) | played when `user_id != me` and the channel is ours, never while deafened | `GatewayEvent::VoiceChannelEffectSend` → `voice::soundboard::on_effect` | ✅ code + parse test |
| local playback | `_playSound`: own sound played locally on click (`GUILD_SOUNDBOARD_SOUND_PLAY_LOCALLY`) with the sound's own volume | `cdn.discordapp.com/soundboard-sounds/{id}` (guild sounds are Ogg Opus, defaults mp3) | `voice::soundboard::play`, `audio::ogg_opus` demuxer + Opus decode | ✅ |
| volume | module 904054: `min(soundVolume * M(setting)/100 * min(outputVolume/100, 1), 1)`, M = 792251 (`n<1 ? n^(1/2.8) : 20·log10(n)/6+1`) ×100, setting `voice_and_video.soundboard_settings.volume` (proto 5.9.1 float, default 100) | | `playback_volume`, `perceptual`, synced entry `sb-volume` | ✅ unit test |
| cooldown | no client-side cooldown in the bundle; the server rate-limits the POST | | none | ✅ (same as client) |
| stop on leave | `VOICE_CHANNEL_SELECT` → `_stopAndClearSounds` | | `app.join_voice` → `soundboard::stop_all` | ✅ |
| disabled | `selfDeaf‖mute‖suppress`; aria "Soundboard est désactivé lorsqu'il est mis en sourdine" / "…lorsque le serveur est muet" / "…lorsqu'il est interdit"; centerButton opacity .5, cursor not-allowed (pass1 #1363) | | `pickers::soundboard::call_button`, `disabled_reason` | ✅ |
| button open | `.active_K9Xq13` primaryDark: `hsla(0,0%,100%,.1)` (pass2) | | same as hover | ✅ |
| popout box | `picker_1lN7Eh` live 531x520, `--background-base-low`, radius 8, `--shadow-border`+`--shadow-high`; centred on the button, bottom 16px above it (pass2: 772,239 for the button at 1017,775) | | `geometry` (+ test) | ✅ |
| header | `header_6EKaag` 64px, padding 12, `--elevation-low`; search `container_0PLWhS md` (placeholder "Trouve le son parfait"); volume `settingsClickArea_-SUGNi` 24px VoiceNormalIcon, margin-left 12 | | `render` | ✅ layout, ⚠ the volume popout itself (would write the synced setting) is not opened |
| category rail | `wrapper_6AvvhR categoryList_X9_kMu` 48px, top 65; items 32px, padding 4, radius 4, margin-bottom 8; pill 4x24; icons TrophyIcon/StarIcon/ClockIcon, guild icons 32 | | `rail`, `grid::rail_item` | ✅ |
| sections | order (picker categories fn): top 3 of the voice guild (`GET /guilds/{id}/top-soundboard-sounds`, `items[].sound_rank`), Favorites (`settings-proto/2` field 8 `FavoriteSoundboardSounds.sound_ids`), Frequently used (field 11 `PlayedSoundFrecency`, 3 not favourited), current guild, defaults before other guilds without Nitro (after with Nitro), other guilds (locked without Nitro) | live order matches (pass2: metal pipe / fahhhhhh / niqué, Rizz, SNCF / RATP / cricket) | `build_sections` | ✅ |
| section header | `sectionContainer_-SUGNi` 32px, padding 0 4 0 8, `--background-base-low`; 16px icon, title body-sm/semibold 14/18 `--interactive-text-default` (hover active), margin 0 4, chevron 16 | | `header_row` | ✅, collapse kept in memory only (the client syncs `soundboardPickerCollapsedSections`, a settings write) |
| sound row | `soundRow_-SUGNi` gap 8, padding 0 0 8 8; 3 per row | | `build_rows` (+ test) | ✅ |
| sound button | `soundButton_N99eHz` 148x40, radius 8, `--background-mod-subtle`, 1px transparent (HC `--border-muted`), hover/active `--border-subtle`; wrapper `--background-base-lower`; info padding 8 gap 8; emoji 20; name body-xs/medium 12/16 `--text-strong`, 2-line clamp | | `sound_button` | ✅ |
| hover overlay | `buttonOverlay_` opacity .2s; background `--background-surface-highest` .8; actions padding 0 8: preview VoiceNormalIcon 16 / PlayIcon 20 `--interactive-text-active` / favourite StarOutlineIcon 16 (StarIcon `--text-feedback-warning` when favourite) | | `sound_button` | ✅ layout, ⚠ no fade; favourite toggle not wired (settings-proto/2 write) |
| playing | `.soundButtonInteractive.playing` border `--brand-500` | | `is_sound_playing` | ✅ |
| locked | other guild without Nitro: lock icon 20 over the overlay background | | `locked` | ✅ |
| tile while playing | `border_7NOr2f.voiceChannelEffect_7NOr2f`: inset 2px `--brand-500` + inset 3px `--black`, `transition: box-shadow .1s ease-out` (pass3 #1505) | | `render_tile` `sb` ring | ✅ |
| emoji effect on the tile | `VOICE_CHANNEL_EFFECT_SEND` with `animation_type/id` (enum 194486: PREMIUM 0, BASIC 1) → module 716365 `Bf`: BASIC `[FrameEffect_10-1_Out]`, PREMIUM 21 `FrameEffect_*`; the emoji (`getEmojiURL` / twemoji, 32·dpr) replaces `assets[0].p`; lottie-web svg, `preserveAspectRatio: xMidYMax slice`, PREMIUM `setSpeed(.8)`, at most 50 at once, nothing with reduced motion | seen in pass3 (emoji rises over the avatar ~0.4–1.4 s) | `ui::voice_effects` (animations extracted at runtime from the cached `web.*.js`, never committed), `media::lottie` image layers (ty 2) + `rasterize_speed` | ⚠ implemented, waiting for a live check |
| voice panel button | the sidebar panel's Soundboard button | | still a stub | ❌ next lot |

## Solo call tile

`root_vXFZs9 singleUserRoot_vXFZs9` (pass1 #1425): when you are alone the grid gets a second 16:9 tile: `--background-base-lower` under `radial-gradient(110.79% 100% at 50% 100%, --background-base-lower 60%, transparent)` and `linear-gradient(270deg, --background-tile-gradient-pink-end 0, --background-tile-gradient-pink-start 80%)`, padding 16, gap 16; art `https://discord.com/assets/664390de11a80444.svg` (max 400x210, margin-bottom 24, loaded at runtime); `stack_TWWX0S` gap 8 with two secondary md buttons (padding 7 15, radius 8, 20px icon + 4 gap, 16/500/20): UserPlusIcon "Inviter dans le salon vocal", AppsIcon "Choisis une Activité". Phoenix: `call_view::solo_tile`; the radial gradient is approximated by a vertical fade (gpui has no radial gradient) ⚠.

## Camera (lot 2, in progress)

| element | source | Phoenix | status |
|---|---|---|---|
| send | official client frames (CDP, cam2, 2026-11-24): op 12 `{audio_ssrc, video_ssrc, rtx_ssrc, streams:[{type:"video", rid:"100", ssrc, active:true, quality:100, rtx_ssrc, max_bitrate:2500000, max_framerate:20, max_resolution:{type:"fixed",width:1280,height:720}}]}`; camera off sends `video_ssrc:0, rtx_ssrc:0, active:false` with the same limits. Select Protocol codecs: opus 120, AV1 101/102, H265 103/104, H264 105/106, VP8 107/108. Main gateway op 4 `self_video: true`, H.264 RTP through DAVE | `connection::VideoOut`, `media::camera` (test pattern or v4l2 through ffmpeg), `media::ffmpeg::FfmpegH264` (libx264 baseline, zerolatency, AUD-split access units), `voice::call` `SetCamera` | ✅ live 2026-11-25 (real_shots `PHX_VOICE_CAMERA=1`, test pattern): the gateway echoes `self_video: true`, video RTP leaves and the SFU sends RTCP for our video SSRC; op 12 matches the official client's frames |
| own preview | own tile shows the local camera | `call_view::camera_image` (half-size BGRA RenderImage) | ✅ live |
| receive | other members' video RTP | not decoded yet | ❌ only one account is available: the official client and Phoenix are the same user, so nobody else can send video into the test channel |
| test sources | brief: never the real webcam | `phoenix-test-pattern` (colour bars, moving square, frame counter); `/dev/video0` is a v4l2loopback (OBS Virtual Camera) fed by `~/discord-phoenix-ref/voice/mire.sh` for the official client | ✅ |
| own preview mirrored | `media-engine-video video_rfWwnu videoContain_rfWwnu mirror_rfWwnu`: `transform: matrix(-1,0,0,1,0,0)`, `object-fit: contain` (cam2 #1403) | `camera_image` flips X, `ObjectFit::Contain` | ✅ |
| panel title | "Vidéo connectée" (`HtVOdd`) instead of "Voix connectée" while the camera is on (cam2) | `voice_panel` | ✅ |

## Screen share (Go Live) and Activities: status and limits

- Go Live is a second RTC connection, not op 12 on the voice connection. Bundle (`web.b63d846b329d6470.js`, GatewayConnectionStore): `STREAM_START` → main gateway `streamCreate(type, guildId, channelId, region)` (op 18), then dispatches `STREAM_CREATE` (`stream_key`, `rtc_server_id`, `viewer_ids`, `paused`) and `STREAM_SERVER_UPDATE` (`stream_key`, `endpoint`, `token`); viewers send `streamWatch(streamKey)` (op 20), owners `streamSetPaused` (op 22) and `streamDelete` (op 19). The stream connection is a voice gateway session with `server_id = rtc_server_id` and a `screen` video stream. Phoenix has no Go Live connection yet ❌ (next lot: reuse `connection::run` with the stream server and the synthetic `TestPattern` as the source).
- The official client cannot be measured sharing: on Linux it goes through the xdg-desktop-portal ScreenCast picker, which needs the user to choose a source on their screen (forbidden by the brief). Only the stream tile UI already in Phoenix (LIVE pill, placeholder) is measured (`vanilla-call-view.json`).
- Activities: opening the launcher in the official client sends `PATCH /users/@me/settings-proto/1` (brief), so it was not opened. An Activity is a web app (`<app_id>.discordsays.com`) running in an iframe with the Embedded App SDK talking to the client over postMessage RPC (auth, voice state, participants). GPUI has no web engine, so running an Activity natively is not feasible; the realistic option is opening the Activity URL in the system browser, which loses the SDK bridge ❌. The call view's "Choisis une Activité" button is drawn (solo tile) but launches nothing.

## Voice panel and call tray icons (2026-11-25, live CDP icons1)

Order in the "Voix connectée" panel and in the call tray: camera, screen share, activity, soundboard (tray: mic, camera, screen, activity, soundboard, more, disconnect). The client draws these as Lottie icons; their rest frames match the icon components: camera off = VideoSlashIcon (camera = VideoIcon), screen = ScreenArrowIcon, activity = AppsIcon (rounded square, triangle, flower, sparkle; Phoenix used RocketIcon ❌ → fixed), soundboard = SoundboardIcon. Tooltips: "Allumer la caméra" / "Éteindre la caméra", "Partage ton écran", "Commencer une activité", "Ouvrir Soundboard" (deafened: "Soundboard est désactivé lorsqu'il est mis en sourdine", panel opacity .3, tray .5). The panel camera button now shows the slashed camera when off and the green state when on, and toggles the camera.

## Live checks (real_shots on the real account, test voice channel only)

- `PHX_VOICE=1 PHX_GUILD=1369382919948210248 PHX_CHANNEL=1574900893613039696` joins the test channel only (any other channel is refused in `ReadOnly::join_voice`), with the synthetic tone as microphone and always muted; `PHX_VOICE_OPEN=soundboard`, `PHX_VOICE_SOUND=<name>`, `PHX_VOICE_CAMERA=1`, `PHOENIX_VOICE_TRACE=1`, `PHOENIX_SOUND_LOG=1`. It leaves the channel before exiting.
- Voice gateway: Ready → UDP discovery → Select Protocol → Session Description (`aead_aes256_gcm_rtpsize`) → Connected ✅. One early run stayed on "Connexion RTC" for 20 s without a trace; with the trace on, later runs connected every time (keep an eye on it).
- Soundboard: playing "Bonsoir" sent the POST, played locally, gave the tile the brand ring and the button the playing border; the gateway echoed `VOICE_CHANNEL_EFFECT_SEND` with `animation_type: 1` (BASIC) and the sound's emoji ✅. The emoji animation itself is drawn under the open picker in that capture, so it is not visually confirmed yet ⚠.
