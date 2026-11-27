# Emoji and animated media parity

Measured on the live client through CDP (2026-11-24, read only) and on the public
CDNs. ✅ = matches the measurement, ⚠ = implemented but not compared live, ❌ = missing.

## Unicode emoji

| feature | client (measured) | Phoenix | status |
|---|---|---|---|
| Image set | The client bundles the Twemoji library pointed at `jdecked/twemoji@16.0.1`; message emoji are `img.emoji` SVGs from its own assets | `media::emoji::TWEMOJI_BASE` = jsDelivr `jdecked/twemoji@16.0.1` 72×72 PNG (was 15.1.0: the 8 Emoji 16.0 emoji 🫩 🫆 🪾 🫜 🪉 🪏 🫟 🇨🇶 were missing) | ✅ |
| Which sequences become emoji | The client's emoji table has 4027 surrogate keys: the Emoji 16.0 set fully qualified, the same sequences without U+FE0F when they have no ZWJ (`☺` and `☺️`, `#⃣`), the 26 lone regional indicators, the 5 skin tones and `👁‍🗨` | `media::emoji_seqs` (generated from Unicode `emoji-test.txt` 16.0 by `tools/gen_emoji.py`) + the same FE0F-less / lone-indicator / tone rules. Checked offline against the client's 4027 keys: all 4027 found and all map to an existing Twemoji 16.0.1 file; Phoenix adds only `©️ ®️ ™️ ⚧️ 👁️‍🗨️` (the fully qualified forms of keys the client lists without FE0F) | ✅ |
| Text symbols stay text | `→ ★ ∞ ✓ ■ ♡` are not in the table: plain text | Before: every code point in U+2190–2BFF / U+2600–27BF was taken for an emoji (2843 false positives: arrows, maths, box drawing, stars…), drew a 404 image with its text fallback at 0.9× and a lone `→` went jumbo. Now longest match against the table only | ✅ |
| Segmentation | Longest match (ZWJ families, skin tones, tag flags 🏴󠁧󠁢󠁳󠁣󠁴󠁿, keycaps); an unknown ZWJ pair splits into its parts | `media::emoji::unicode_emoji_at`, a trailing FE0F stays with the emoji; `richtext::emoji_ranges` | ✅ (unit tests) |
| Twemoji file names | FE0F dropped unless the sequence has a ZWJ; the eye in a speech bubble is `1f441-200d-1f5e8` | `twemoji_code` on the fully qualified form, same exception | ✅ |
| Names, channel names, role group headers | Text in `"gg sans","Noto Sans",…`; Chromium draws the emoji with the system colour font (`fc-match emoji`) | `ui::emoji_text::label` (Noto Color Emoji fallback) | ✅ (unchanged) |
| Custom status and activity line (member list, user panel) | `customStatusContent_` and the activity text hold `img.emoji` 14px Twemoji, not font glyphs | `emoji_text::status_label`: Unicode and custom emoji as images at 14px | ✅ member list (live DOM); ➖ user panel: the account's own status has no emoji and setting one is a write to the account (same `status_label` code as the member list) |
| Profile status bubble | CSS (`rmCN9_`): `.statusEmoji_rmCN9_:not(:has(~.statusText_rmCN9_))` 32×32px (emoji-only); `.statusEmoji_rmCN9_:has(~.statusText_rmCN9_)` 16×16px, `margin-inline-end: 4px` (emoji + text) | `emoji_text::status_wrapped`: 32px for emoji-only, 16px + 4px right margin when paired with text (was 19.25px = 1.375em) | ✅ (CSS verified) |
| Picker data | Emoji 16.0, client database `vnd-emoji.*.js` module 13474 (`emojis`, `emojisByCategory` ranges, `names`, `hasDiversity`): 1932 picker entries | `ui/pickers/emoji_data.rs` generated from that module by `scripts/emoji/gen_emoji_data.mjs <vnd-emoji.js> <out>`: same order and categories, `names[0]` is the shortcode (`:grinning:`, `:thumbsup:`), the other names are aliases, tone support = `hasDiversity` | ✅ |

## Animation

GPUI only advances the frames of an `img` that has an element id: the frame index and
its timer live in the element state, and an id-less `img` of a GIF or animated WebP
stays on frame 0 forever (`vendor/gpui-pre/src/elements/img.rs`, `global_id.is_some()`
gates both the state and `request_animation_frame`). Offscreen proof with
`anim_emoji_shot` (control row, same 4-frame 100 ms GIF, 3 s sampled every 20 ms on the
headless clock): bare `img` without id 0 frame changes, with an id 62–64. Every image that can animate now carries an id (`media::anim_id`).

| media | client rule | Phoenix | status |
|---|---|---|---|
| Custom emoji (messages, reactions, statuses) | Animated URL `…webp?size=N&animated=true` when the window is focused and Play animated emoji is on, or hovered | `CustomEmojiImage`: inner `img` gets an id (was missing: animated emoji never moved) | ✅ |
| Emoji picker | Plays only the hovered emoji | same component with `auto = false` | ✅ |
| GIF uploads | Animated in place; with "Play GIFs when Discord is focused" off: first frame, played on hover | `media_image` (already had an id) + `gif_hover`; off = proxy `format=webp` still until hovered | ✅ on / ➖ off: comparing it would mean turning "Play GIFs when Discord is focused" off on the account (a settings write); covered by `gif_plays` / `gif_hover` |
| GIF uploads in a mosaic | same | the mosaic `img` had no id: now `frames` | ✅ |
| Tenor gifv | mp4 `<video>` | Tenor gif rendition (unchanged) | ✅ |
| Klipy gifv (the 2026 provider) | mp4 `<video>`, poster = Klipy `.webp` | The Klipy thumbnail is an animated WebP (measured: 72 frames, 33/66 ms, 2.4 s loop ≈ 30 fps, 160 KB, 286×300): played directly from `static.klipy.com`. Before: no rendition known, still frame only | ✅ |
| GIF autoplay off | Still, plays on hover | `media::gif_plays` / `gif_hover` (thread-local hovered key, re-render on change) | ➖ not compared live: the setting would have to be changed on the account (settings write) |
| Window not focused | GIFs and emoji pause | GPUI only advances frames while `is_window_active()`; emoji switch to the still URL (`sync_motion`) | ✅ |
| Animated avatars | Static until hover | `avatar_image`: the hover `img` had no id, so it showed frame 0: now plays on hover | ✅ hover on the avatar itself; message rows and member rows play it while the row is hovered, member rows also while selected (`9fb052bb9a0957b3.js` member item: `animateOnHover: !(selected || isHovering)`; `ui::avatar_hover`, keyed like the avatar decorations: `avatar_deco::hover_key(2, message)` / `(1, user)`) |
| Animated server icons (rail) | Static until hover | Was the `.gif` frame 0 forever: now still PNG + playing GIF on hover | ✅ |
| Animated server banner (channel list) | Plays on hover | the hover `img` now has an id | ✅ |
| Avatar decorations | Static, animated on row hover; always in popouts | `avatar_deco::layer` and the message decorations get ids (animated WebP already requested) | ✅ |
| APNG stickers | APNG | GPUI decodes APNG as one frame: messages now ask the CDN for `stickers/<id>.webp?size=320&animated=true` (measured: 25 frames, 78 ms ≈ 12.8 fps, 349 KB at 320px; still = same URL without `animated=true`) | ✅ |
| GIF stickers | GIF | id added | ✅ |
| Lottie stickers | Lottie | `media::lottie` (unchanged) | ✅ |
| Sticker animations setting | Always / on hover / never | Always (0): plays; On interaction (1): first frame until hovered (`media::sticker_plays` / `sticker_hover`, APNG/GIF + Lottie); Never (2): first frame | ✅ |
| GIF picker tiles | autoplay | the tile `img`s get ids (picker search itself belongs to another branch) | ✅ |

### Frame timing and loading (27/11, wt/upload-gif)

| case | client | Phoenix | status |
|---|---|---|---|
| Frames of 10 ms or less | Chromium plays them for 100 ms (`deferred_image_decoder.cc` `FrameDurationAtIndex`: "duration <= 10 ms → 100 ms", all formats) | GPUI advanced such a frame on every redraw. `media::frame_delay::normalize` rewrites the GIF Graphic Control Extension delays (≤ 1 cs → 10 cs) and the WebP `ANMF` durations (≤ 10 ms → 100 ms) of every image served by `CachingHttpClient` (disk and network) | ✅ measured offscreen on the real account (#phoenix, `real_shots` + 1×1 `px:` probes decoded back to the frame colour): the 100 ms test GIF advanced 92 frames in 9.2 s (exact); the 0 ms one advanced about 7 frames per 0.17 s probe (render rate) before the fix |
| Playing GIF uploads | proxy `…/x.gif?…&format=webp&animated=true&width&height` (chunk `efc34e4425fa2af5.js`: `.gif` on the media proxy → `format=webp`, `animated=true`); still = `format=webp` without `animated` (fetched: 1.3 KB single frame); the proxy keeps the 0 ms durations in the WebP | `media::animated_media`; still unchanged (`video_poster`) | ✅ |
| GIF uploads in a mosaic | proxy size ≈ the tile (`273×210` for a 166×128 tile in the live DOM) | was `1100×700` for every tile (whole frames decoded at that size); now twice the cover size of the tile, never above the original (`mosaic::request_size`); the tile also follows "Play GIFs" and hover like single uploads | ✅ |
| Still → animated switch (hover, focus back, autoplay) | the still stays until the animation is decoded | avatars, server icons, custom emoji, single and mosaic GIF uploads keep the still under the animated image until `lru::ready` (decoded in the chat LRU cache), instead of an empty box for 200 ms then a grey placeholder | ✅ |
| Animated server icons not hovered | not decoded or animated | the hidden `invisible()` GIF was decoded and requested an animation frame (full redraw) every frame while the rail was on screen; now only loaded while hovered (`media::hover_animated`) | ✅ |
| Frame catch-up after a hitch | Chromium skips to the frame of the current time | GPUI plays the missed frames one per redraw (`img.rs` keeps `last_frame_time` debt) | ❌ needs a GPUI patch (vendor rev shared with the scroll work) |

## Account settings read (not changed)

From the cached READY `user_settings_proto` (text_and_images): `gif_auto_play` = true,
`animate_emoji` absent (default true), `animate_stickers` = 0 (Always). Phoenix's
`effects::sync` reads the same fields.

## Animation numbers

`int-bin/anim_emoji_shot <prefix> <dir> 111` at `58273ab`, test art = 4-frame 96×96 GIF,
100 ms per frame (no real emoji), headless renderer, 2026-11-24:

| measure | value |
|---|---|
| Inline animated emoji, before the fix | 0 frame changes (still on frame 0) |
| Inline animated emoji, after | 42 frame changes in 3.0 s sampled every 20 ms on the headless clock (14 per second; the GIF asks for 10, the virtual clock of the sampling loop runs a little ahead) |
| Control `img` without id / with id | 0 / 62 frame changes in 3 s |
| CPU, 5 s of 60 Hz redraws | 2.2 % of one core playing, 1.8 % with Play animated emoji off (≈ 0.4 % for decoding/painting one animated emoji) |

Decoded frames are kept in memory as RGBA for the whole loop (GPUI decodes every frame
when the image loads):

| asset (measured on the CDN) | frames | timing | decoded RGBA |
|---|---|---|---|
| Klipy gifv thumbnail, 286×300 WebP | 72 | 33/66 ms, 2.4 s loop ≈ 30 fps | 24.7 MB |
| APNG sticker as `.webp?size=320&animated=true` | 25 | 78 ms ≈ 12.8 fps | 10.2 MB |

Frames only advance while the image is painted (virtualised list: off-screen rows stop)
and the window is active, so an unfocused or idle window costs nothing.

## Not done / open

- Animated avatars play on hover of the avatar itself; the client also plays them on
  message and member row hover.
- The DM profile panel of the real account showed no custom status bubble in the offscreen
  render (client: "🔨 …" bubble); profile loading is outside this branch.
- Picker shortcodes are CLDR names, not the client's.
