# Voice messages

Reference: the 2026 desktop client, read from its local bundles on 2026-11-24
(`web.b63d846b329d6470.js`, CSS `550929.23708a7f085f3437.css` recorder,
`967062.e9e61e85cf579a8c.css` player). Code: `crates/phoenix-voice/src/audio/{ogg,voice_message}.rs`,
`crates/phoenix-app/src/ui/chat/voice_message.rs`.

Desktop recording sits behind the user experiment `2026-09-desktop-voice-message-recording`
(default off). The measured account does not have it: the live composer has no voice
entry point and Alt+R does nothing (CDP, sandbox channel, 2026-11-24). Phoenix implements the
flow the client runs when the experiment is on. The recorder was never opened on the live
client (it would capture the real microphone).

## Flow

| Step | Client source | Phoenix | Status |
|---|---|---|---|
| Entry points | `RECORD_VOICE_MESSAGE` (Alt+R, module 87207 `canShowVoiceMessageButton`: DM or `SEND_VOICE_MESSAGES`, not forum, no pending upload/sticker) and the "+" menu row `RECORD_VOICE_MESSAGE` (MicrophoneIcon, "Enregistrer un message vocal") | `shortcuts.rs` Alt+R, `composer.rs` attach menu row, both call `voice_message::start` | ✅ (permission bit not checked, the send-blocked notice is) |
| Phases | store 709372: `requesting` → `recording` → `recorded`; `finalizingByChannel` | `Phase::{Requesting, Recording, Finalizing, Recorded}` | ✅ |
| Capture | `startLocalAudioRecording` with the user's echo cancellation / noise suppression / AGC | cpal capture through the call `Processor` (same switches as Voice & Video) | ✅ |
| Level samples | `VoiceActivity` dB → `round((clamp(db,-100,0)+100)/100*255)` | `level_sample`, one per 20 ms Opus frame | ✅ mapping; ⚠ native event rate not measured |
| Waveform field | `ei()`: downsample to 256 (934581 bucket average), round, min 255, base64 | `vm::waveform` + `waveform_b64` | ✅ |
| Limits | `518950`: max 1 199 500 ms (auto stop to `recorded`), blink at 1 169 500 ms (500 ms) and 1 184 500 ms (250 ms), 1000 ms with reduced motion; under 0.5 s nothing is sent | `vm::MAX_DURATION`, `WARN_*`, `MIN_DURATION_SECS` | ✅ |
| Switching channel while recording | unmount: `recording` → stop to `recorded` + toast "Enregistrement du message vocal interrompu." + sound `voice_message_recording_stopped` at 0.4; `requesting` → cancel | `on_channel_change` | ✅ |
| Second draft in another channel | modal "Tu as un message vocal non envoyé" (return / discard) | Alt+R is ignored while another channel holds a recorded draft | ❌ modal not reproduced |
| File | native recorder file (`audio/ogg` unless the extension says otherwise) | `voice-message.ogg`, Ogg Opus 48 kHz mono, 64 kb/s VoIP, pre-skip 312, Ogg CRC per page | ✅ format; ⚠ client bitrate not measured |
| Upload | `CloudUpload` with `durationSecs`/`waveform`; message attachments get `duration_secs` (seconds, float) and `waveform`; message `flags: IS_VOICE_MESSAGE` (8192), `content: ""`, pending reply kept | `Backend::send_voice_message` → `upload_files` + `message_attachments` | ✅ sent in the sandbox (see below) |
| Failure | toast "Impossible d'envoyer ton message vocal. Merci de réessayer." / "Le début de l'enregistrement a échoué" | `parity_toast` | ✅ |

## Recorder bar (`bar_bWT6VE`)

Replaces the whole inner row of the text area (attach button, editor and buttons, `tY ? e9 : …`).

* `bar_`: flex, gap `--space-8`, min-height `--custom-channel-textarea-text-area-height` (56),
  `padding-inline-end: 6px`, `0` under `.has-webkit-scrollbar` (the Electron root has it) → 0.
* `capture_`: flex 1, gap `--space-16`; stop button (StopIcon, tooltip "Arrêter") only while recording.
* live canvas (`eq`): color `--interactive-text-active`; bars 2 px wide every 6 px, height
  `v·(24−2)+2`, vertically centred, right-aligned; a new bar every 48 ms smoothed with
  `d += (last − d)·0.45`; the row slides left by one pitch per bar (still with reduced motion).
* `timerGroup_` gap 4: `recordDot_` 4×4 `--text-feedback-critical` (transparent once stopped),
  `timer_` body-sm 14/18 `--text-default`, min-width 32, `--text-feedback-critical` while blinking.
  Format `m:ss`, minutes unpadded (`rB`).
* `divider_` 1 px × `--chat-markup-line-height` (22), `--border-subtle`.
* Trash (TrashIcon, "Supprimer"), Send (`sendButton_`: 32×32, padding 4, SendMessageIcon 20 px,
  `--text-brand`, hover `--brand-430`, disabled `--icon-muted` + not-allowed while requesting).
* Recorded: the capture area becomes the preview `e8` (play/pause chat button, fluid waveform
  with neutral colours, timer = `ceil(duration − position)` once played).
* Enter sends while recording (stops first) or recorded.

Not measured on the live client (recorder never opened): the exact x positions inside the row.

## Player (`container_seLrDE`)

| Part | CSS / JS | Phoenix |
|---|---|---|
| Container | `--background-mod-subtle`, radius 24, padding 12, gap `--space-12`, row | ✅ |
| Ripple | `rippleContainer_` 1 px `--border-subtle`, radius 24; `ripple_` `--brand-530` grows to 200% on play (.2 s) | ✅ colour, ⚠ no grow animation (instant) |
| Play button | 32×32 circle, margin −4 (start/top/bottom); dark: `linear-gradient(151.11deg, --brand-460 16.55%, --brand-560 104.36%)`, hover `--brand-400 17.78% → --brand-460 82.22%`; playing: `--white` bg, icon `--brand-500`; Play/Pause icon `sm` (18 px) `--white` | ✅ (no :active scale .875) |
| Waveform | canvas 32 px high (`canvas_CNmkpo` margin −4px 0, so the player is 48 px tall), width `6·floor((w+4)/6)−4` with `w` = 40 (≤0.5 s) … 294 (≥45 s) linear; bars 2 px / pitch 6, height `22v+2`; fewer samples than bars → padded with 0 (non fluid) or interpolated (fluid); more → bucket average | ✅ |
| Waveform colours | idle: all `--interactive-text-default`; paused after play: played `--interactive-text-active`, rest `--background-mod-muted`; playing: played `--white`, rest `--brand-430` (`408822`) | ✅ (no easeOutBack bar animation) |
| Duration | body-sm 14/18 `--interactive-text-default`, `--white` while playing; `ceil(duration)` then `ceil(duration − position)` | ✅ |
| Speed | `CA = [.75, 1, 1.5, 2]` cycled on click, label `${rate}X` without the leading 0 (".75X"), shared by every voice message (`MEDIA_PLAYBACK_RATE_UPDATE`); `playbackRate_`: width 32, radius 4, body-xs/semibold, `--background-mod-subtle` / `--interactive-text-default`, hover `--interactive-background-hover` / `--interactive-text-active`, playing `--interactive-background-active` / `--brand-200` | ✅; time-stretch keeps the pitch (WSOLA, `vm::time_stretch`) like `preservesPitch` |
| Volume | VoiceNormalIcon (VoiceXIcon muted) 24 px `--interactive-text-default`, playing `--brand-200` | ✅ mute toggle; ❌ hover volume slider |
| One at a time | `VOICE_MESSAGE_PLAYBACK_STARTED` pauses the others | ✅ |
| End | position back to 0, played reset after 500 ms | ✅ (immediately) |
| Seek | mousedown/drag on the canvas | ✅ click; ❌ drag |

## Tests

* Unit: Ogg CRC (reference value), Ogg write/read round trip, encode → decode of a tone,
  level mapping, waveform downsampling, time-stretch length for each speed, timer format,
  canvas width, speed labels, attachment JSON (`duration_secs`, `waveform`).
* Real send: `examples/voice_message_probe.rs` (hard-locked to the sandbox `〢💬・phoenix`,
  names checked by GET first) records a generated tone through `Recorder` (no microphone),
  posts a `[Test Phoenix]` line and the voice message as a reply to it.

## Live check (2026-11-24, sandbox, 4.5 s tone sent by `voice_message_probe`)

Discord accepted the upload (`flags` 8192, `duration_secs` 4.5, 300-char waveform). Official
client at zoom 1.25: container 246.6×48 at x 496.6; play button x 504.6 (32, margin −4),
waveform x 548.6 width 62 (= `canvas_width(4.5)`), duration x 622.6 (28.6 wide), speed
x 663.2 (32×16 inside a 4 px padded box), volume x 707.2 (24). Phoenix matches every x; its
height was 56 until the −4 px canvas margin was added, and it stretched to the message width
until it got `align-self: flex-start`. After both fixes (c4f9a001) the real-account render matches
the client crop (246.6×48).
