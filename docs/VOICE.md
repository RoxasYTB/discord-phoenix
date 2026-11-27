# Voice: how audio flows

`crates/phoenix-voice` holds the transport, `crates/phoenix-app/src/voice` drives it from the UI.

## Sending

microphone (cpal / PulseAudio) → `audio::device::encode_loop` (input volume, `audio::processing::InputChain`, gate or push-to-talk) → Opus 48 kHz mono 64 kb/s with in-band FEC → `DaveSession::encrypt` → RTP → `TransportCipher` (`*_rtpsize`) → UDP. op 5 SPEAKING is sent when transmission starts and 5 silence frames close it.

## Input processing (`audio::processing`)

Every 20 ms Opus frame goes through `InputChain` as two 10 ms blocks of 480 samples at 48 kHz mono, in this order:

1. high-pass filter + echo cancellation + "Standard" noise suppression: one `sonora::AudioProcessing` (pure Rust port of the WebRTC audio processing module, AEC3, NS level High, internal rate 48 kHz). Built only when echo cancellation or a noise suppressor is on.
2. "Krisp" noise suppression: `nnnoiseless` (pure Rust RNNoise). Its voice probability feeds the Advanced Voice Activity gate.
3. Automatic gain control: a second `sonora::AudioProcessing` with only AGC2 (adaptive digital, default WebRTC parameters, limiter), so it runs after RNNoise like the client's chain runs AGC last.

The echo reference is the mixer output: `mix_loop` pushes each 20 ms frame it actually queues to the speakers into a `RenderTap` (bounded channel, mono downmix), with the queue length ahead of it in ms; the encode thread drains it before each capture block (`process_render_f32`) and sets the stream delay to that queue length + 10 ms. AEC3 also estimates the delay itself. Soundboard sounds and notification sounds (separate streams in `effects.rs`) are not part of the reference.

Latency added (cross-correlation in the self-test): Off 0 ms, Standard 6.9 ms (WebRTC NS overlap + band split), Krisp 10.9 ms (RNNoise overlap-add of one block), echo cancellation adds about 4 ms on top (14.9 ms for AEC3 + RNNoise + AGC2, the Voice Isolation chain).

Measured on 79a47024 (`PHX_AUDIOPROC_SELFTEST=1 int-bin/real_shots`, dev build with dependencies at opt-level 2, synthetic voice at −19 dBFS in pink + white noise at −33 dBFS):

| mode | noise reduction | SNR gain | latency | CPU per 10 ms block |
|---|---|---|---|---|
| off | 0.0 dB | 0.0 dB | 0 ms | 0 µs |
| Standard (WebRTC NS) | 17.2 dB | 16.0 dB | 6.9 ms | 28 µs (0.3 % of a core) |
| Krisp (RNNoise) | 24.8 dB | 24.1 dB | 10.9 ms | 114 µs (1.1 %) |
| AEC3 + RNNoise + AGC2 | 23.5 dB | 23.0 dB | 14.9 ms | 177 µs (1.8 %) |
| AEC3 + NS + AGC2 | 16.1 dB | 19.3 dB | 10.9 ms | 97 µs (1.0 %) |

Echo (60 ms delay, two reflections): echo −28.5 dBFS in, −64.3 dBFS out (35.7 dB removed), near-end talker during double talk −25.8 → −27.0 dBFS. Live in 〢🔊・phoenix-vocal (Voice Isolation, synthetic noisy voice, mixer reference running at 100 blocks/s): about 5 % of one core in the dev build, gate open on ~80 % of frames.

The chain is rebuilt only when a stage is switched on or off. `ChainSettings::from_control` reads the effective flags sent by the app; the synthetic tone input (`SYNTHETIC_INPUT`) bypasses the chain so the transport tests keep a steady tone, the synthetic noisy voice (`SYNTHETIC_VOICE_INPUT`) goes through it.

### Settings, profiles and the client model

Same fields and same override table as the client (`web.b63d846b329d6470.js`, defaults `e5`, profile table `e8`, merge `t3`):

| | stored default | Voice Isolation | Studio | Custom |
|---|---|---|---|---|
| echo cancellation | on | on | off | stored |
| noise suppression (Standard) | off | off | off | stored |
| noise cancellation (Krisp) | on | on | off | stored |
| automatic gain control | on | on | off | stored |
| sensitivity | auto | auto | manual −84 dB | stored |
| Advanced Voice Activity (`vadUseKrisp`) | on | on | off | stored |
| input mode | stored | stored | voice activity | stored |

`VoicePrefs::processing()` applies the table, `controls()` sends the result. Choosing a profile no longer overwrites the stored values (the client keeps them for Custom). The Custom block shows the client's controls: echo cancellation switch, the Noise Suppression select (Krisp / Standard / None, `r6` in `52b6e35ba37f71f0.js`, `setNoiseCancellation` + `setNoiseSuppression`), and in the advanced section Automatic Gain Control (`rD`) and Advanced Voice Activity (`rV`, disabled outside voice activity with automatic sensitivity). The noise button of the voice panel toggles Krisp like the client's popout switch (`setNoiseCancellation(!t)`, which also sets Standard to the opposite) and does nothing outside Custom, where its tooltip gives the client's `/tqmfM` text. Old `voice.json` files: `noise_suppression` without `noise_cancellation` becomes the Krisp flag.

Advanced Voice Activity: with Krisp on, automatic sensitivity and `vad_use_krisp`, the gate opens when the RNNoise voice probability of a frame reaches 0.8 (0.5 without AGC, the client's `vadKrispActivationThreshold` rule) and stays open 12 frames (the client's `vadTrailing` 25 × 10 ms ≈ 240 ms). Otherwise the level gate (`dsp::NoiseGate`) decides.

### Krisp versus RNNoise

Krisp is a proprietary model shipped inside Discord's native voice module; it cannot be reused. Phoenix runs RNNoise (Xiph, BSD, 2017) in its place under the "Krisp" label of the select, so the UI keeps the client's options. Differences to expect: RNNoise is a small GRU on 22 bands with a pitch filter, it removes steady noise (fans, hum, hiss, traffic) well but lets more transient noise through (keyboard clicks, dishes, a dog) and can colour voices slightly; Krisp also removes background voices and transients. The Krisp logo and the "Powered by Krisp" text are not shown.

### Measuring

`cargo test -p phoenix-voice --features audio --lib processing` runs the generated-signal tests (synthetic voice: glottal pulses through three formant resonators, syllables and pauses; pink + white noise; echo path with a 60 ms delay and two reflections plus double talk). `PHX_AUDIOPROC_SELFTEST=1 int-bin/real_shots` prints one line per mode with the noise reduction, the SNR gain, the latency and the CPU time per 10 ms block, then exits. During a call, `PHOENIX_SOUND_LOG=1` prints every 5 s `voice-proc: chain=… in_db=… out_db=… open=… cpu_avg_us=… load=…% latency_ms=… render_blocks=… erle=…`.

Live test with a generated source: `PHX_VOICE=1 PHX_VOICE_TALK=1 PHX_VOICE_SOURCE=voice PHX_VOICE_PROFILE=iso PHX_CHANNEL=1574900893613039696 PHX_VOICE_HOLD=30000 PHOENIX_SOUND_LOG=1 int-bin/real_shots …` joins the test channel unmuted with the synthetic noisy voice (never the microphone), the profile is applied without saving `voice.json`.

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

## Sending video (camera and Go Live)

Capture (test pattern, v4l2, PipeWire portal) → `FfmpegH264` (libx264, Annex B with AUD) → `H264Packetizer::prepare_access_unit` (AUD dropped, SPS/PPS repeated before each IDR, 4-byte start codes) → `DaveSession::encrypt` → `packetize_prepared` (single NAL, STAP-A, FU-A, 1100-byte payloads) → RTP payload type 105 on the `video` SSRC from op 2 READY → transport encryption.

Fixed after the first real call with friends (user log of 8fc6cda5, 27/11):
1. Receiving: the server's op 12 has no `type` in `streams[]` (the client handler does not read a top-level `rtx_ssrc` either). Our decoder required them, so the whole op 12 was dropped (`ignoring undecodable voice gateway frame error=json: missing field type`), the streamer never got a layer, no op 15 asked for its video, and `video_packets` stayed at 0. Every op 12 field now has a default, as in the client's `tr()` (missing type means VIDEO).
2. Sending: DAVE left only the 1-byte NAL header of each H.264 slice in the clear. libdave (`codec_utils.cpp`, `BytesCoveringH264PPS`) keeps the NAL header and the slice header bytes up to the end of `pic_parameter_set_id` (three exp-Golomb values, emulation prevention bytes skipped) in the clear, because WebRTC's H.264 depacketizer reads the PPS id; with it encrypted, viewers dropped every frame and kept asking for keyframes. Only NAL types 1 and 5 are treated as slices, like libdave. The start-code check now covers the DAVE footer too (`ValidateEncryptedFrame` validates ciphertext and supplemental bytes).
3. PLI / FIR for the video SSRC force a new keyframe (`Camera::keyframe_flag`, at most once a second); before, a viewer that joined mid-stream waited for the next periodic keyframe (every 4 s) and a lost keyframe was never answered. NACKs are answered with RTX on the `rtx_ssrc` (payload type 106, original sequence first, original marker and timestamp), from the last 1024 packets.
4. op 12 `streams[].type` follows op 2 READY (the stream server answers `video` for a Go Live connection, the client copies READY's type into its stream parameters).

Stats on the stream connection (`voice-trace: stream(own) Stats(…)`): `video_packets_sent`, `video_keyframes_sent`, `video_keyframe_requests_received`, `video_nacks_received`, `video_rtx_sent`. A working stream shows `video_keyframe_requests_received` that stops growing once viewers have a picture.

Automated tests: `video_rx::tests` (order, NACK, RTX, loss, simulcast, H.264/AV1 frames), `connection::go_live_loop_tests` (generated H.264 → packetizer → DAVE → transport encryption → receiver with a lost packet recovered by NACK/RTX and a PLI → DAVE → ffmpeg), `tests/gateway_mock.rs` (Go Live op 2 / op 12 against a mock voice gateway), `sink_wants::tests` (ladder, focus, 1:1 size, Go Live), `media::ffmpeg::tests` (generated H.264 / VP8 / AV1 frames sent through RTP, the receiver and ffmpeg; ratio kept), `rtp::rtcp::tests` (NACK, FIR), `rtp::av1::tests`.

## Joining, switching and reconnecting

`voice::call::VoiceManager` keeps one target (guild, channel). Own VOICE_STATE_UPDATEs are matched against it (`own_voice_state`): the null state the gateway sends for the old guild when you join a channel of another guild, and the late leave of a quick leave-then-join, are ignored (they used to reset the target, so every later join of the session waited forever for a VOICE_SERVER_UPDATE it then dropped). The connection starts once the target has a session id (own voice state) and a server (VOICE_SERVER_UPDATE with an endpoint); a new endpoint or token restarts it, a null endpoint stops it until the next one.

Voice gateway: close codes other than 1000, 4003-4006, 4009, 4011, 4012, 4014, 4016, 4021, 4022 are resumed (op 7 with `seq_ack`, up to 5 tries). When the connection still fails and the error is transient (`connection::retryable`: no close code, 4009, 4015, socket or IP discovery failure), the manager identifies again with the same session and server after 1, 2 then 4 s, with a new DAVE session. 4014 (moved, channel deleted, server change) waits for the next VOICE_SERVER_UPDATE. An ICMP port unreachable on the media socket is ignored. Every 5 s an RTCP receiver report (one block per heard audio SSRC) keeps the UDP path open while the mic is muted.

DAVE: op 11 adds recognized users, op 13 removes the user and its decryptor; op 27 proposals are committed by every member (op 28), the server announces one commit (op 29) or sends the newcomer a welcome (op 30), each answers op 23, and op 22 installs the new epoch's keys. The previous key of each sender is kept 10 s so frames sent during the transition still open (`dave::session::tests::a_third_member_joins_then_one_leaves`).

## Volumes

Sliders hold the value the client shows (perceptual, 0–200). The engine gets `voice::prefs::amplitude(value)`, the inverse of the client's module 792251: `n<1 ? n^2.8 : 10^((n-1)·6/20)` with `n = value/100`.

## Testing reception live

1. Someone else joins `〢🔊・phoenix-vocal` on Aperture Sciences and speaks (or plays music).
2. Start Phoenix on the account with `PHOENIX_SOUND_LOG=1` (add `PHOENIX_VOICE_TRACE=1` for gateway events), join the same channel, mic muted.
3. You should hear them, see their speaking ring, and stderr should print once per second:
   `voice-recv: user=<id> ssrc=<n> received=… decoded=… fec=… plc=… late=… level_db=-2x.x` and `voice-recv: mixer out_db=… queue_ms=60 underruns=0`.
   `decoded` must grow by ~50 per second while they speak; `peak_db` (loudest frame of the last second) above −60 means real audio, `level_db` is only the last frame.
   The first frame of each sender prints `voice-recv: first frame user=… dave_frame=true bytes=… opus_toc=0x…`; a frame that cannot be opened prints `voice-recv: decrypt failed user=… dave_frame=… keys=[…]: <reason>` (1st, 2nd, 4th, 8th… failure).
4. Check the user menu: the volume slider and "Mute" change only that person; the headset button silences everything (`deafened=true`, `out_db=-100`).

`PHOENIX_SOUND_LOG=1` also prints `voice-audio: engine input=… output=…` when the audio engine starts (or `audio engine failed: …`) and, once per second, `voice-net: … dave=… rtp_audio=… lost=… mapped=[ssrc:user] unmapped=[ssrc:held] decrypt_errors=…`. Reading it: `rtp_audio` stuck at 0 means no audio reaches the socket; `unmapped` growing means the SSRC was never named by op 5 / op 12; `decrypt_errors` growing means DAVE keys are missing; `output=None` means no speakers were opened.

DAVE key packages carry the lifetime the voice gateway requires (`not_before` 0, `not_after` u64 max, protocol.md « Key Packages »); with openmls' default 12-week lifetime the gateway never added Phoenix to the MLS group, so every sender stayed at « no key for sender » (user log of 358d46b7). `RUST_LOG=phoenix_voice=debug` now logs each DAVE opcode (21–30), the key package, merged commits, the welcome join (epoch, members) and each executed transition with the users that have a key; the `voice-net` line shows `dave_keys=[…]`.

The v0.1.0-beta.2 release (4fe939cc) predates the receive work (b647f9a7, 0ad2f8a3, eaf516a3): it drops the packets of every SSRC that op 5 / op 12 had not named yet and has no FEC, so it can stay silent; use a build from main.

Automated tests: `connection::receive_tests`, `audio::mixer::tests`, `rtp::jitter::tests`, `media::opus::tests`, `dave::session::tests`, `crypto::tests`.

Without the full app: `PHX_VOICE=1 PHX_CHANNEL=1574900893613039696 PHX_VOICE_HOLD=30000 PHOENIX_SOUND_LOG=1 int-bin/real_shots …` joins the test channel off screen with the mic muted and a synthetic input, stays 30 s, prints `voice hold t=… others=… speaking=… received=…` each second next to the `voice-recv:` lines, then leaves.
