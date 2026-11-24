# Message content parity

Values come from `~/discord-phoenix-ref/current-client/css/` (2026 client) and the Text
variants named in the client JS bundles of the local Discord cache (only values are recorded).
Mock render: `content_shots out.png dark` (generates local gradient PNGs and points the mock media at them via `PHX_MOCK_MEDIA_DIR`, so images load offline) (mock `#showcase` channel of the first guild, filled
by `src/mock_showcase.rs` with one message of each kind). ✅ = matches CSS/JS values,
⚠ = rendered but not yet checked against a live dump, ❌ = missing.

| feature | CSS / JS source | values | our code | status |
|---|---|---|---|---|
| Bold / italic / underline / strike | `markup_` | strong 700, em italic, u, s | markdown.rs `DELIMS` | ✅ |
| `***bold italic***`, `__*u+i*__` | – | nested emphasis | markdown.rs `find_close` (run end) | ✅ (was rendering `*…*` literally) |
| Headers `#`/`##`/`###`, `-#` subtext | `markup_ h1/h2/h3`, `small` | 24/20/16, subtext 12 --text-subtle | markdown.rs `line_block`, ui/components.rs | ✅ (see text.md) |
| Lists (nested, ordered) | `markup_ ul/ol` margin-inline 16, nested `ul ul` circle; `li` margin-bottom 4 | | markdown.rs `ListItem`, ui/chat.rs `block` | ✅ (live dump: ul margin 4 0 0 16, li margin-bottom 4) |
| Block quotes | `blockquoteContainer_` margin-block --space-4; `blockquoteDivider_` 4px radius 4 --spine-default; `blockquote` padding-inline 12 8, --text-subtle, max 90% | | ui/chat.rs `block`, richtext.rs | ✅ (live dump: divider 4 wide radius 4, blockquote padding 0 8 0 12) |
| Spoilers (text, click to reveal) | `spoilerContent_` hidden: --spoiler-hidden-background (text + bg); revealed --spoiler-revealed-background | | richtext.rs | ✅ (live dump: radius 4, hidden bg = --spoiler-hidden-background) |
| Masked links + hover underline | `anchor_ anchorUnderlineOnHover_` | --text-link, underline on hover | richtext.rs | ✅ (live dump: anchor 16/22 --text-link) |
| `<t:…>` timestamps | `timestamp_ku1OHH` --background-mod-normal, radius 3, padding 0 2 | | markdown.rs `format_timestamp`, richtext.rs | ✅ (CSS; radius/padding approximated by the text run background) |
| Soundboard mention in message | local client bundle `current-client/js/22fbc347793fb277.js`: inline token `soundboard`, raw form `<sound:guildId:soundId>`; renderer resolves visible name from the Soundboard store by `soundId`; gateway message model also has `soundboard_sounds` | token contains guild + sound ID; sound metadata carries name/emoji/volume/availability | no parse/render path in `markdown.rs`, `richtext.rs`, `ui/chat.rs`; API message model currently drops `soundboard_sounds` | ❌ unsupported; cosmetic message display can be implemented separately from voice playback, but exact visual card and click behavior need a reference |
| Inline code | `markup_ code.inline` --font-code 85%, --background-code, radius 4 | | richtext.rs | ✅ (CSS) |
| Code block + highlighting | `markup_ pre` mt 6 radius 4 max 90%; `code` .875rem/1.125rem 1px --border-normal; `.hljs` padding .5em --background-base-lower --text-code | | ui/chat.rs `block`, richtext/highlight.rs | ✅ (live dump: pre mt 6 radius 4; code.hljs padding 7, 14/18) |
| Rich embed frame | `embedFull_ grid_` | 4px start border, radius 4, padding 2 16 16 12, max 516 | ui/chat.rs `embed` | ✅ |
| Embed provider | `embedProvider_` body-xs/normal | 12/16 --interactive-text-default | ui/chat.rs `embed` | ✅ |
| Embed author | `embedAuthorName_` .875rem semibold --text-strong; icon 24 + 8 | | ui/chat.rs `embed` | ✅ |
| Embed title | `embedTitle_` 1rem semibold --text-strong / link --text-link | | ui/chat.rs `embed` | ✅ |
| Embed description | `embedDescription_` body-sm/normal --text-default | 14/18 | ui/chat.rs `embed_markdown` | ✅ |
| Embed fields grid | `embedFields_` grid gap 8, mt 8; name body-sm/semibold --text-strong mb 2; value body-sm/normal | inline ≤3 per row (2 with thumb) | ui/chat.rs `embed` | ✅ |
| Embed footer | `embedFooterText_` body-xs/medium --text-default; icon 20 + 8 | | ui/chat.rs `embed` | ✅ |
| Embed thumbnail / media | `embedThumbnail_` ml 16 mt 8; `embedMedia_` mt 16 radius 4 | | ui/chat.rs `embed` | ✅ |
| Link preview (article) | same grid, side thumbnail 80 | | ui/chat.rs `embed` | ✅ |
| Image / gifv embeds | bare media, gifv max-h 300 | | ui/chat.rs `embed` | ✅ (real compare earlier) |
| Video embed (YouTube) | `embedVideoActions_` centred; `wrapper_lj7V-K` --opacity-black-60, radius 24, h 48, p 12; `iconWrapper_` opacity .6 → 1, 24px play + external | | ui/chat/attach.rs `video_embed_actions` | ✅ |
| Image attachment | `visualMediaItemContainer_` max 550x350, radius 8 | | ui/chat.rs `media_image` | ✅ |
| Image mosaic (2–10) | `mosaicItem_` rows | | ui/chat/mosaic.rs | ✅ |
| Spoiler image | `spoilerContainer_0_7_Rr.attachmentContainer_` radius-sm 8, bg --__spoiler-background-color--hidden; `hiddenSpoiler_` blur 44px (no dark overlay); `spoilerWarning_` r20 15px semibold uppercase, bg --opacity-black-60 → black-88 on hover, text --primary-200 → white | | ui/chat.rs `spoiler_cover` | ✅ (CSS; blur approximated by an upscaled 1/24 thumbnail; no live example found to dump) |
| Video attachment chrome | `wrapper_FJ99sr` radius, `playPausePop_` --opacity-black-60 p12 icon 24, `videoControls_` 32px --opacity-black-60 on hover | | ui/chat/attach.rs `video_card` | ✅ |
| Audio player | `wrapperAudio_ newMosaicStyle_` 432, p16, radius 8, --background-base-lower, 1px --border-subtle; `audioControls_` --opacity-black-28 radius-sm mt 4; `controlIcon_` 24+4; duration --font-code 12/12 500; seek `mediaBarWrapper_` 6px | | ui/chat/attach.rs `audio_card` | ✅ |
| Generic file card | `fileWrapper_` 432; `file_` --background-surface-high, 1px --border-subtle, radius 8, --shadow-low, p16; icon 30x40 mr 8; name 16/20 --text-link hover underline; size body-xs 12/16 --text-muted (dump vanilla-thread-channel node fileWrapper_) | | ui/chat/attach.rs `file_card` | ✅ |
| File download hover button | `hoverButtonGroup_ nonMediaMosaicItem_` -8/-8, --background-base-low, radius 5; `hoverButton_` p6, 20px, hover --interactive-background-hover | | ui/chat/attach.rs `hover_download` | ✅ |
| Stickers | `assetWrapper_1399Om` / `stickerAsset_` 160x160 | | ui/chat.rs `attachment` | ✅ (live dump: 160x160) |
| Poll container | `container_HOJNvw` --background-surface-high, 1px --border-subtle, radius-sm, p16, max 472 / min 302 | | ui/components.rs `render_poll` | ✅ |
| Poll question / prompt | body-md/medium (16/20 500) --text-strong / body-sm/normal --text-muted mt 4 | | ui/components.rs | ✅ |
| Poll answers | `answersContainer_` gap 8, margin 8 0 16; `answerInner_` min-h 50, p 8 16, gap 8, --background-mod-muted, outline (voted --control-brand-foreground, victor --status-positive, hover --border-strong); label body-sm/semibold | | ui/components.rs | ✅ |
| Poll results bar | `votePercentageBar_` --interactive-background-active / --polls-voted-fill / --polls-victor-fill; `votesData_` gap 8: count body-xs/semibold + % body-md/semibold | | ui/components.rs | ✅ |
| Poll bottom bar | `bottomBar_` gap 16, min-h --custom-button-button-md-height; details body-sm/normal muted | | ui/components.rs | ✅ |
| Forwarded messages | `container_4dy_KE` | | ui/chat/forward.rs | ✅ (forward agent) |
| System: join, boost, pin, thread, call | `systemMessage_SY_P2T` content at +80, `iconContainer_bLMKT5` 72x22 at +8, text 16/22 | | ui/chat/system.rs | ✅ (live dump; the join welcome card `systemMessageContainer_g2fiji` with its wave button is not done) |
| `(edited)` label | `edited_2i8Bfm` .625rem, line-height 1, normal weight, --text-muted | | ui/chat.rs `edited_marker` | ✅ (live dump: 10px / 10px, 400) |

Coverage: 38 ✅ / 0 ⚠ / 1 ❌ of 39 rows (spoiler image from CSS only: none found in 16 scanned channels). The join welcome CTA ("Wave to say hi!", welcomeCTA_eZQ1Gc, secondary sm button with the greeting sticker, client sticker pick) is done in `chat::system::welcome_cta`.
Live dumps (2026-11-24, user asleep, read-only, view restored): ref-cache/message-content-<channel>.json.
Colours are not compared from the dumps: the user's theme differs from the generated dark tokens.
Still "✅ (CSS)": `<t:…>` timestamps and inline code (no example in the scanned channels). No real-data compare score for these
components yet (none of them is on the live channel the user shows).
