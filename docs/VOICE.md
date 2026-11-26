# Voice: how audio flows

`crates/phoenix-voice` holds the transport, `crates/phoenix-app/src/voice` drives it from the UI.

## Sending

microphone (cpal / PulseAudio) → `audio::device::encode_loop` (high-pass, noise suppression, gain, gate or push-to-talk, AGC) → Opus 48 kHz mono 64 kb/s with in-band FEC → `DaveSession::encrypt` → RTP → `TransportCipher` (`*_rtpsize`) → UDP. op 5 SPEAKING is sent when transmission starts and 5 silence frames close it.

## Receiving

UDP → `udp::classify_and_decrypt` → `rtp::parse_packet` → `connection::Receiver`:

1. the SSRC is mapped to a user from op 5 SPEAKING or op 12 VIDEO; packets of an unknown SSRC wait (25 per SSRC) until it is named;
2. `DaveSession::decrypt` removes the end-to-end layer with that sender's key ratchet (passthrough while DAVE is off, or 10 s after a transition);
3. the first non-silence frame raises `VoiceEvent::Speaking`, 300 ms without audio clears it;
4. the Opus frame goes to `audio::mixer::Mixer`: per-user jitter buffer, Opus decoder, FEC for a single loss, PLC for short bursts, `user volume × output volume`, deafen;
5. `audio::device::mix_loop` keeps ~60 ms queued on the output device chosen in the settings.

Go Live audio from a watched stream uses the same mixer, keyed `owner | STREAM_AUDIO_BIT`.

## Receiving video (cameras and Go Live)

UDP → transport decrypt → `video_rx::VideoReceiver` → `connection::open_video_frame` (DAVE) → `VoiceEvent::Video { codec, keyframe }` → `media::ffmpeg::FfmpegDecoder` (one ffmpeg per sender) → BGRA frame → `call_view::remote_image` / `stream_image` / PiP.

1. op 12 VIDEO gives each sender's layers (`streams[]`: `ssrc`, `rtx_ssrc`, `quality`, `active`; falls back to `video_ssrc`/`rtx_ssrc` when `streams` is empty). RTX packets (payload type + 1, 2-byte original sequence first) are put back in place.
2. Packets are ordered by extended sequence number; a frame is the run of packets that ends with the marker bit, all with the same timestamp. A gap sends a generic NACK (RTCP 205/1) at once and every 100 ms; after 400 ms the frame is dropped, the receiver skips to the next frame and asks for a keyframe. Delta frames are held back until a keyframe; while waiting, a PLI (RTCP 206/1) goes out every 200 ms. These are libwebrtc's receive defaults (NACK on gap, RTT 100 ms by default, keyframe request repeat 200 ms); the client's native media engine is not visible in JS.
3. Depacketizers: H.264 (single NAL, STAP-A, FU-A → Annex B), VP8 (descriptor stripped), AV1 (aggregation header → OBUs with size fields). DAVE is removed on the whole frame; for AV1 the sender encrypted the frame with the last OBU unsized, so a second try drops that size field.
4. Simulcast: frames of another layer of the same sender are only shown from a keyframe on, so a layer switch never shows a broken picture.
5. Decoding: ffmpeg reads IVF from stdin (`H264`, `VP80`, `AV01` fourccs; AV1 gets a temporal delimiter and size fields), writes PAM `rgba` frames that are turned into BGRA. Output keeps the sender's ratio, scaled down to fit 1280x720 (camera) or 1920x1080 (stream). Hardware decoding is asked with `-hwaccel`: `videotoolbox` on macOS, `d3d11va` on Windows, `auto` on Linux (VAAPI, VDPAU or CUDA, software if none works); `PHOENIX_VIDEO_HWACCEL=none` turns it off, another value is passed to ffmpeg as is. If a hardware decoder dies, the next ones are software. The codecs offered in Select Protocol are the decoders `ffmpeg -decoders` lists (H.264, VP8, AV1).
6. op 15 MEDIA_SINK_WANTS like the client (`MediaSinkWantsManager` and `GoLiveQualityManager` in `web.b63d846b329d6470.js`): see `sink_wants.rs` and `docs/parity/call-view.md`.

Go Live: « Regarder le stream » sends op 20 STREAM_WATCH only; STREAM_CREATE and STREAM_SERVER_UPDATE start a second voice connection on the stream's RTC server; its video and audio use the same path, audio as the mixer track `owner | STREAM_AUDIO_BIT`.

Testing video live: someone joins `〢🔊・phoenix-vocal` with their camera on (or streams a test window), Phoenix joins with `PHOENIX_SOUND_LOG=1 PHOENIX_VOICE_TRACE=1`. stderr prints `voice-trace: op 12 user=… layers=[…]`, `voice-trace: op 15 media sink wants …` and once per second `voice-net: … video_packets=… rtx=… frames=… shown=… dropped=… nacks=… plis=… video_decrypt_errors=…`. `shown` must grow at the sender's frame rate and `video_decrypt_errors` stay at 0.

Automated tests: `video_rx::tests` (order, NACK, RTX, loss, simulcast, H.264/AV1 frames), `sink_wants::tests` (ladder, focus, 1:1 size, Go Live), `media::ffmpeg::tests` (generated H.264 / VP8 / AV1 frames sent through RTP, the receiver and ffmpeg; ratio kept), `rtp::rtcp::tests` (NACK, FIR), `rtp::av1::tests`.

## Volumes

Sliders hold the value the client shows (perceptual, 0–200). The engine gets `voice::prefs::amplitude(value)`, the inverse of the client's module 792251: `n<1 ? n^2.8 : 10^((n-1)·6/20)` with `n = value/100`.

## Testing reception live

1. Someone else joins `〢🔊・phoenix-vocal` on Aperture Sciences and speaks (or plays music).
2. Start Phoenix on the account with `PHOENIX_SOUND_LOG=1` (add `PHOENIX_VOICE_TRACE=1` for gateway events), join the same channel, mic muted.
3. You should hear them, see their speaking ring, and stderr should print once per second:
   `voice-recv: user=<id> ssrc=<n> received=… decoded=… fec=… plc=… late=… level_db=-2x.x` and `voice-recv: mixer out_db=… queue_ms=60 underruns=0`.
   `decoded` must grow by ~50 per second while they speak; `level_db` above −60 means real audio.
4. Check the user menu: the volume slider and "Mute" change only that person; the headset button silences everything (`deafened=true`, `out_db=-100`).

`PHOENIX_SOUND_LOG=1` also prints `voice-audio: engine input=… output=…` when the audio engine starts (or `audio engine failed: …`) and, once per second, `voice-net: … dave=… rtp_audio=… lost=… mapped=[ssrc:user] unmapped=[ssrc:held] decrypt_errors=…`. Reading it: `rtp_audio` stuck at 0 means no audio reaches the socket; `unmapped` growing means the SSRC was never named by op 5 / op 12; `decrypt_errors` growing means DAVE keys are missing; `output=None` means no speakers were opened.

The v0.1.0-beta.2 release (4fe939cc) predates the receive work (b647f9a7, 0ad2f8a3, eaf516a3): it drops the packets of every SSRC that op 5 / op 12 had not named yet and has no FEC, so it can stay silent; use a build from main.

Automated tests: `connection::receive_tests`, `audio::mixer::tests`, `rtp::jitter::tests`, `media::opus::tests`, `dave::session::tests`, `crypto::tests`.

Without the full app: `PHX_VOICE=1 PHX_CHANNEL=1574900893613039696 PHX_VOICE_HOLD=30000 PHOENIX_SOUND_LOG=1 int-bin/real_shots …` joins the test channel off screen with the mic muted and a synthetic input, stays 30 s, prints `voice hold t=… others=… speaking=… received=…` each second next to the `voice-recv:` lines, then leaves.
