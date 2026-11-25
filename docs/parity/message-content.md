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
| Masked link parsing | client `link` rule (web.b63d846b329d6470.js module 694403 + simple-markdown regex in 361020) | nearest `[` whose label has only balanced brackets / lone `]` followed by another `]`; lazy href with paren groups, `<…>`, title; bracket counters; rejected match = whole text literal (mentions, URLs, emoji outside embeds/webhooks, blank label, bad protocol / host) | markdown/link.rs | ✅ (#phoenix 24/11: `[Test Phoenix] … [phxsurligne](…)` linked the whole first line from the first `[`; unit tests on that message; webhook `allowEmojiLinks` not wired: no webhook flag at parse time) |
| `<t:…>` timestamps | `timestamp_ku1OHH` --background-mod-normal, radius 3, padding 0 2 | | markdown.rs `format_timestamp`, richtext.rs | ✅ (CSS; radius/padding approximated by the text run background) |
| Soundboard mention in message | local client bundle `current-client/js/22fbc347793fb277.js` parses `<sound:guildId:soundId>`; message renderer resolves message-local sounds first and otherwise consults the Soundboard store; another renderer branch labels unresolved IDs `Unknown` | message metadata includes sound ID, guild ID, name, optional emoji ID/name; active click uses channel and voice context to play | API model and backend conversion preserve message-local sound metadata; markdown resolves token name and optional emoji; normal and history-page text rendering use the resolver; playback deliberately not wired | ➖ **besoin utilisateur**: static fixture implementation. Measuring the chip needs a real message containing a `<sound:…>` token in a channel the client can show; sending one is forbidden and none is known in the account's channels. Playback (click) needs a voice connection, also forbidden. Spacing, colour, hover and click stay unmeasured until the user points to such a message |
| Inline code | `markup_ code.inline` --font-code 85%, padding 0 .2em, margin -.2em 0, radius 4; `markup_ code` --background-code, 1px --border-normal; box-decoration-break slice | live #phoenix 24/11: 13.6px gg mono, 96.8×19.2 box, border 0.8px (1px at dpr 1.25) | ui/chat/message_ext.rs `code_box` (each word a slice of the box, edges on the first / last) | ✅ (was a bare text-run background: no border, 16px) |
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
| Image description (ALT) | module 959760 `mediaMosaicAltText_j9Vvzj`: shown only when Accessibility > "Afficher les descriptions d'image" (`text_and_images.view_image_descriptions`, proto 6.7) is on, the description is non-empty and the spoiler is revealed; button bottom 4 / inset-inline-start 4.5, rgba(0,0,0,.6) (.8 hover, --background-mod-normal active), radius 4, 14px semibold / 18, padding 7 8, white, text "ALT" (`jCV1Tz`) | | ui/chat/alt_text.rs (single image + mosaic cells) | ✅ code from CSS/JS; ⚠ the live account has the setting unset (= off), so the client shows no badge and the ON state is only compared to the CSS (real_shots `PHX_PROTO=alt-text=1`) |
| ALT popout | `mediaMosaicAltTextPopout_j9Vvzj` surface-high, radius 8, `--shadow-border, --shadow-high`, padding 13.5 12, max-width 360; title `fSiQ3A` "Description de l'image (alternative textuelle)" body-xs/semibold, margin-bottom 9.5; text body-md/normal, word-break; `Popout` defaults: position right, align top, spacing 8, nudged into the viewport | | ui/chat/alt_text.rs `popout` | ✅ CSS/JS; videos: `mediaMosaicVideoAltTextContainer_` top 4 / start 4.5, button `position/bottom/inset-inline-start: inherit` (so 4.5 more), rendered while `controlsVisible` or the button is hovered (JS `tg`), i.e. on hover of the video like its controls (ui/chat/alt_text.rs `video_badge`, single video and mosaic cells) ✅ |
| Upload description field | "Modifier la pièce jointe" modal (live, sandbox 2026-11-24, 1280×800): card 480 wide; image preview 16px under the header, 40px from the card edge, object-fit contain, max-height 150; Nom de fichier input 38.4; Description (texte alternatif) 3-row textarea 86.4 high, padding 12 10, placeholder "Ajouter une description", maxLength 1024; "Définir comme spoiler"; Annuler / Sauvegarder full width, gap 8 | | ui/chat/composer.rs `open_upload_edit`, ui/modals/mod.rs `Form.preview` / `Field::rows` | ✅ layout and strings; sent as `attachments[].description` (upload_probe test message 1574904977028030487). the 1024 `maxLength` is enforced like a textarea (the inserted text is cut to fit, in UTF-16 units; `modals::clamp_insert`, unit test) ✅; ⚠ labels stay 14/18 (client body-md/medium 16/20, shared by every Form) |
| Multi-image posts (X/Twitter, Bluesky…) | live client embed utils: embeds sharing a `url` merge into the first, later ones only add their `image` to its `images`; `renderImages` draws them with the attachment media mosaic | | model.rs `merge_embeds_on_url` (convert.rs), ui/chat.rs `embed` gallery | ✅ (rule and mosaic verified against client code; unit-tested in `model.rs` and rendered via `mosaic::render` in `chat.rs`) |
| Spotify / Apple Music / Amazon Music | `embedSpotify_` etc. are iframes (the provider's own player, opacity .8 → 1 on hover) | | ui/chat.rs `embed` rich card | ➖ third-party web player, not reproducible natively: Phoenix keeps the embed's rich card |
| Spoiler image | `spoilerContainer_0_7_Rr.attachmentContainer_` radius-sm 8, bg --__spoiler-background-color--hidden; `hiddenSpoiler_` blur 44px (no dark overlay); `spoilerWarning_` r20 15px semibold uppercase, bg --opacity-black-60 → black-88 on hover, text --primary-200 → white | | ui/chat.rs `spoiler_cover` | ✅ (CSS; blur approximated by an upscaled 1/24 thumbnail; no live example found to dump) |
| Video attachment chrome | `wrapper_FJ99sr` radius, `playPausePop_` --opacity-black-60 p12 icon 24, `videoControls_` 32px --opacity-black-60 on hover | | ui/chat/attach.rs `video_card` | ✅ |
| Audio player | `wrapperAudio_ newMosaicStyle_` 432, p16, radius 8, --background-base-lower, 1px --border-subtle; `audioControls_` --opacity-black-28 radius-sm mt 4; `controlIcon_` 24+4; duration --font-code 12/12 500; seek `mediaBarWrapper_` 6px | | ui/chat/attach.rs `audio_card` | ✅ |
| Generic file card | `fileWrapper_` 432; `file_` --background-surface-high, 1px --border-subtle, radius 8, --shadow-low, p16; icon 30x40 mr 8; name 16/20 --text-link hover underline; size body-xs 12/16 --text-muted (dump vanilla-thread-channel node fileWrapper_) | | ui/chat/attach.rs `file_card` | ✅ |
| File download hover button | `hoverButtonGroup_ nonMediaMosaicItem_` -8/-8, --background-base-low, radius 5; `hoverButton_` p6, 20px, hover --interactive-background-hover | | ui/chat/attach.rs `hover_download` | ✅ |
| Stickers | `assetWrapper_1399Om` / `stickerAsset_` 160x160 | | ui/chat.rs `attachment` | ✅ (live dump: 160x160) |
| Poll container | `container_HOJNvw` --background-surface-high, 1px --border-subtle, radius-sm, p16, max 472 / min 302 | | ui/components.rs `render_poll` | ✅ |
| Poll question / prompt | body-md/medium (16/20 500) --text-strong, header_ grid `1fr 24px` gap 8 (the question wraps 32px short) / prompt body-sm/normal --text-muted mt 4, only while the poll can be answered (`promptLabel` undefined once expired, live JS 1b027c… 2026-11-24) | | ui/components.rs | ✅ (fixed: the expired poll showed "Sélectionne une réponse") |
| Poll answers | `answersContainer_` gap 8, margin 8 0 16; `answerInner_` min-h 50, p 8 16, gap 8, --background-mod-muted; **outline** 1px var(--custom-poll-style-border) (outside the box: rows stay 50px); styles from the view model: victorSelected / victorNotSelected → victorStyles_ (--status-positive), voted (open poll) → votedStyles_ (--control-brand-foreground), loserSelected / notVoted / selected / normalVote → transparent; selected while voting --control-brand-foreground, hover --border-strong; label body-sm/semibold | | ui/components.rs `answer_style` (test) | ✅ (fixed: rows were 52px with a 1px border, and an expired losing vote kept the brand outline) |
| Poll selection icon | F: RADIO_BUTTONS k$ 24 (r10 ring 2px --text-default, r5 dot --control-brand-foreground), CHECKBOXES e5 20 (2px --text-default, r3; selected --brand-500 + white check .85×), LIST + own vote Vx 24 CircleCheckIcon: expired victor --control-connected-background-default on white, expired loser --icon-strong on --background-mod-muted, else --brand-500 on white | | ui/components.rs `voted_check`, icons `circle-check(-bg)` (extracted at runtime) | ✅ (was always a blue disc) |
| Poll results bar | `votePercentageBar_` --interactive-background-active / --polls-voted-fill / --polls-victor-fill; `votesData_` gap 8: count body-xs/semibold --text-strong (secondary textButton) + % body-md/semibold | | ui/components.rs | ✅ |
| Poll bottom bar | `bottomBar_` wrap, gap 16, min-h --custom-button-button-md-height; detailsText_: "N votes" textButton secondary body-sm/medium (--text-strong), then the expiration label body-sm/normal --text-muted after `::before "∙"` margin 0 .5rem; tertiary "Afficher les résultats" (/KHAUF) while unanswered; primary Voter (JwkNU4, disabled without a pick) / Supprimer le vote (XhQEh8) / Revenir au vote (gNj6In), none once expired | | ui/components.rs, `parity.poll_show_results` | ✅ (fixed: everything was muted with "•"; Show results / Go back to vote were missing) |
| Poll result line (POLL_RESULT 46) | the message carries `mentions: [poll author]` (live store) → `.mentioned_` row for the author (bg --message-mentioned-background-default, 2px --icon-feedback-warning bar, padding 2 0); card container_D5ZFqQ max-width 376 + padding (8 16 8 8 with the 40px --background-mod-subtle emoji box, emoji 22px), as wide as allowed so "Voir le sondage" sits at the right (402px live); description gap 2, "Réponse gagnante • N%" body-xs/medium --text-muted, Vx 16 | | ui/chat/system.rs `poll_result` | ✅ |
| Container spoiler (components v2) | `spoilerContainer_.attachmentContainer_.hidden_` radius 8, bg --__spoiler-background-color--hidden (hover --hidden--hover), shadow .5 .5 1 1; the card under it `container_rc97-L.isHidden_` is blurred 44px (live centre 61,61,68 on a 53px card); `spoilerWarning_` 15/15 semibold uppercase, padding 8 12 → 87×31 | | ui/components.rs `spoiler_cover` + `blurred_veil` (gaussian coverage grid, test) | ✅ (was square, flat 89,90,99, pill 38px tall). The "red bar" above it in the 17h compare was the bottom of the 256px red gallery image of the same message, not a Phoenix artefact |
| Forwarded messages | `container_4dy_KE` | | ui/chat/forward.rs | ✅ (forward agent) |
| System: join, boost, pin, thread, call | `systemMessage_SY_P2T` content at +80, `iconContainer_bLMKT5` 72x22 at +8, text 16/22 | | ui/chat/system.rs | ✅ (live dump; the join welcome card `systemMessageContainer_g2fiji` with its wave button is not done) |
| `(edited)` label | `edited_2i8Bfm` .625rem, line-height 1, normal weight, --text-muted | | ui/chat.rs `edited_marker` | ✅ (live dump: 10px / 10px, 400) |

Coverage: 38 ✅ / 1 ⚠ / 0 ❌ / 2 ➖ (besoin utilisateur, iframe players) of 41 rows (spoiler image from CSS only: none found in 16 scanned channels). Soundboard static mention rendering is implemented in code, but its visuals are provisional and no playback behavior is claimed. The join welcome CTA ("Wave to say hi!", welcomeCTA_eZQ1Gc, secondary sm button with the greeting sticker, client sticker pick) is done in `chat::system::welcome_cta`.
Live dumps (2026-11-24, user asleep, read-only, view restored): ref-cache/message-content-<channel>.json.
Colours are not compared from the dumps: the user's theme differs from the generated dark tokens.
Still "✅ (CSS)": `<t:…>` timestamps and inline code (no example in the scanned channels). No real-data compare score for these
components yet (none of them is on the live channel the user shows).

## Message components (buttons, selects, Components V2)

Sources: the component renderer of the 2026 client (`current-client/js/e940c8e032f54075.js`:
`lS` root, `lE` per type, `V` button, `ep`/`na` selects, `lA` section, `lC` container, `nQ`
thumbnail, `nw` gallery, `ng` file, `nF` separator, `n1` unknown) and its CSS modules; checked on
live DOM dumps (2026-11-24): search results of real GLaDOS / bot messages in Aperture Sciences
(sections with green/red buttons, a primary button with an animated emoji, a transcript file,
a one-image gallery) and, in the #phoenix sandbox, one test message posted through a temporary
webhook (deleted right after) that holds every non-interactive type at full width. Parsing is in
`phoenix-api/src/models/component.rs` (unit tests on anonymised shapes of the real payloads),
drawing in `ui/components.rs`.

| feature | CSS / JS source | values | status |
|---|---|---|---|
| Root | `container_uEFz2z` / `.isComponentsV2_` | column, row-gap 6px (v1) / 8px + max-width min(600px, 100%) + width fit-content (v2) | ✅ (live: 600 / 469 / 276 wide) |
| Action row | `container_J1RNcC` > `children_J1RNcC` | flex wrap, gap 8 | ✅ (live) |
| Button | `button_9OQtot sizeSmall_ grow_` | 32px, 1px border, radius 8, padding 3 11, min-width 60, 14/18 medium; disabled opacity .5 | ✅ (live) |
| Button colours | `V` colour switch + `lookFilled_.color*_` | primary + premium → `--control-primary-*`, success → `--control-connected-*` (white text), danger → `--control-critical-primary-*`, secondary + link → `--control-secondary-*` with `--border-muted`; hover / active backgrounds | ✅ (live: rgb(88,101,242) / (0,133,69) / (210,45,57) / secondary) |
| Button content | `content_ZMR-xe` | emoji 19.25px + 4px before a label; label ellipsis; link: WindowLaunchIcon 16px, 8px left; premium: ShopIcon 16px white + 4px | ✅ (live: link, emoji); premium ⚠ see below |
| Premium button | `V` sku branch | label "{sku} — {price}" from the store listing, else disabled "Product Unavailable" | ⚠ Phoenix has no store: always the disabled "Produit indisponible" (no premium button found to compare) |
| Button click | `V` / `component_body` | interaction `type: 3` (POST /interactions) with the message flags; link: opens the URL (the client first asks for untrusted links) | ✅ implemented, never fired on a real message (unit test checks the link URL / custom id); link buttons and masked links go through the client's "Tu quittes Discord…" modal (see the Links section of chat.md) |
| Selects | shared Select (`selectField_vWC4bk`), `ep` for user/role/mentionable/channel | 40px field, 1px --input-border, radius 8, 16/20 value, 20px chevron; menu of ≤ 5 rows; multi-select sends on close | ✅ (CSS + `ui/components.rs` `select` verified: `h(40)`, `r(8)`, 1px input border, 16/20 text, 20px chevron, `max_h(5 * 40 + 8)`) |
| Section | `section_5m-AKz`, `children_`, `textChildren_`, `accessory_` | row gap 12, texts column gap 4 (centred next to a button), accessory align start, `restrictWidth_` max calc(50% − 12px) | ✅ (live); restrictWidth drawn as 50% |
| Text display | `markup_ markdownContainer_ root_KNAhcP` (`.sizeReduced_` in containers) | 16/22, in a container 14/19.25 (h1 24/33, h2 20/27.5, small 12/16.5) | ✅ (live) |
| Thumbnail | `imgContainer_TZN6d8` | 85px square, --radius-sm 8, cover | ✅ (live, sandbox) |
| Media gallery | mosaic with `isInAppComponentsV2_` | the attachments' mosaic, max-width 600; one item = oneByOneGrid_ at its own size (never upscaled) + 2.4px line box | ✅ (live: 3 items 1×2 at 350 high, 1 item 256×256 in 258.4) |
| File | `fileDisplayContainer_aMNXbv` + attachment card | 432px card (name link, size, download) | ✅ (live) |
| Separator | `divider_pdtj88` / `spacingLarge_` | 1px --background-mod-strong; large: margin 8 0 | ✅ (live) |
| Container | `container_rc97-L` | --background-surface-high, 1px --border-subtle, radius 8, padding 16, gap 8; 4px accent bar `:before` | ✅ (live) |
| Spoiler container / media | `spoilerContainer_0_7_Rr` + `isHidden_` blur | hidden spoiler background, SPOILER pill (padding 8 12, radius 20, 15px semibold) | ✅ layout; the blur itself is a flat cover (GPUI has no backdrop blur) |
| Unknown type | `n1` `container_xAKhZr` | 1px --status-warning, radius 12, padding 16, body-sm/medium --text-subtle "Merci de mettre à jour Discord pour afficher ce message" | ✅ (CSS + string) |
| Search results | `shouldDisableInteractiveComponents` | same drawing, no interaction, no highlight inside components | ✅ (live) |

## Invite cards (wt/invites, 2026-11-25)

Sources: the 2026 invite embed in `current-client/js/2420b0c09d1ff9a5.js` (the switch on the
resolved invite: `dK` guild profile card v2, `dG` legacy guild card, `dJ` invalid / expired,
`dL` group DM, `dk` friend link, `dX` stream, `d7`/`cf` voice v2, `dc.Ay` scheduled event) and the
profile card modules of `816d6850742d01a6.js` (`42780` banner and avatar, `86376` header,
`136696` games and traits, `123213` + `461888` call to action) and `d40616fab4aa4a98.js`
(`649640` banner background). CSS: `845486.*.css` (`guildInviteContainer_fknPy8`),
`179301.*.css` (`*_Xweo-z`), `10886.*.css` (`*_E7xgJz`), `653294.*.css` (`wrapper_jsxF_b`),
`435860.*.css` (`inviteEmbed_aoI0WQ`). Live DOM and computed styles of the #général pins popout
(1536×840 @1.5, page zoom 125 %): one invalid invite and two guild invites without banner or icon.
Data: `GET /invites/{code}?with_counts=true&with_expiration=true` (plus
`&guild_scheduled_event_id=` for `?event=` links), read only. Code: `ui/chat/invite_embed.rs`,
parsing in `discord_backend/nav.rs` `invite_preview`.

| feature | source | values | status |
|---|---|---|---|
| Which card | resolved invite switch | expired or banned → invalid; group DM (`type` 1 or channel type 3) → group card; friend (`type` 2, or no guild with an inviter) → friend card; stream target → stream card; event → event card; voice / stage / thread channel or HUB → legacy card; any other guild invite (`da.v`) → profile card v2 | ✅ guild / legacy / invalid / group / friend; ❌ stream, event and voice v2 cards (see below) |
| Link detection | message `codedLinks` | `discord.gg/`, `discord.com/invite/`, `discordapp.com/invite/` (with `www.`, `ptb.`, `canary.`), code `[A-Za-z0-9-]`, optional `?event=`; not inside inline or fenced code; one card per distinct link, in text order; the link stays in the text | ✅ unit tests |
| Where | message accessories | chat, pins popout, inbox (mentions / saved), search results | ✅ |
| Resolving | `iW` | Phoenix: legacy wrapper with the "Résolution de l'invitation" title and 48px / bar placeholders | ⚠ the client draws a masked shimmer (`resolving_jsxF_b`); not compared |
| Profile card frame | `.guildInviteContainer_fknPy8` | 300px wide, 1px --border-subtle, radius 16, --background-surface-high, overflow hidden; 2px top padding of the accessory grid | ✅ live 301.6 × 237.2 |
| Banner | `.cardHeightMeasure_ .banner_` + `649640` | 72px; DISCOVERABLE with `custom_banner_hash` → discovery splash cover; else `radial-gradient(105.43% 127.05% at 50.1% 127.05%, brighten(c, 1.75) 20.65%, c 85.16%)` with c = brand colour, else the icon's dominant colour, else `--guild-profile-banner-background-default` | ✅ live rgb(74,74,74) → black for a server without icon (baked image: GPUI has no radial gradient); icon colour uses the color-thief port of `avatar_color` on the 128px icon (client: 64px) |
| Avatar | `.avatarContainer_Xweo-z`, `.avatarWrapper_Xweo-z`, `145497` | margin-top −34, padding 0 16; 70px clan-icon squircle in --background-surface-highest, 64px icon inside; acronym `.acronym_sX5_6T` 14px 500 --text-strong on --background-base-lowest | ✅ live box 70 at x+16; squircle drawn as radius 22 / 19 |
| Header | `.header_E7xgJz` | margin 8 0, padding 0 16; name heading-sm/semibold 16/20 --text-strong ellipsis, gap 4 with the server tag; members row gap 0 8: 8px dot (--text-status-online / --text-status-offline) + 4 + body-sm 14/18 --text-subtle "1 en ligne", "12 membres" (ICU plural, nbsp); "depuis oct. 2025" from the snowflake, `toLocaleDateString(year numeric, month short)` | ✅ live |
| Body | `136696` `V` | description body-sm --text-subtle, games row, traits (max 2 lines); nothing → 8px `emptyBody_` | ✅ empty case live; description / traits not seen live; games row ❌ |
| Button | `123213` + `461888` | full width sm (32, radius 8, 14/18 medium), `active` = --control-connected-*; member → "Aller sur le serveur"; member verification + manual approval → "Postule pour rejoindre"; valid invite → "Rejoindre"; missing roles → "Accepter" | ✅ Rejoindre live (rgb(0,133,69)); "Aller sur le serveur" opens the server and channel; "Accepter" and the application status ❌ (no invite roles / applications data) |
| Long cards | `dK` | over 292px the card collapses with "Afficher plus" / "Dissimuler les détails" | ❌ not implemented |
| Legacy card | `wrapper_jsxF_b` | padding 16, 1px --border-subtle, radius 8, --background-surface-high, max 432 / min 160; title body-sm/medium --text-muted mb 12; content wrap gap 16; 48px icon; title 16/20, detail 14/16 --interactive-text-default; splash 64px | ⚠ from CSS, no live sample (needs a voice / hub invite) |
| Invalid card | `dJ` | title "Tu as été invité(e), mais" (own message: "Tu as envoyé une invitation, mais"), 48px box radius 12 --opacity-red-12 bg + border, 26px EnvelopeIcon --icon-feedback-critical; "Lien d'invitation invalide" heading-sm/medium --text-feedback-critical, "Demander une nouvelle invitation" body-sm 14/16 --interactive-text-default | ✅ live 302.4 × 111.6 |
| Group DM card | `dL` | "Tu as été invité(e) à" (own: "Tu as envoyé une invitation pour"), icon only when the group has one, name = group name or recipients ("a, b", "a, b, et c", "a et b autres", "Sans nom"), "N membres", full-width "Rejoindre le groupe privé" or disabled secondary "Rejoint" | ⚠ from JS, no live sample; the 3 recipient avatars ❌ |
| Friend card | `dk` | "Tu as reçu un lien d'ami" (own: "Tu as envoyé un lien d'ami"), 48px avatar, username twice, "Ajouter" (active) / "Envoyer un message" (secondary, friends) / disabled own | ⚠ from JS, no live sample |
| Join actions | `acceptInviteAndTransitionToInviteChannel` | `POST /invites/{code}` then open the server | ✅ coded, never run on the real account (`real_shots` denies it) |
| Stream, voice v2, event, application cards | `dX`, `d7`/`cf`, `dc.Ay`, `d_` | | ❌ a voice invite of a server you can see uses the voice v2 card in the client, Phoenix draws the legacy card; event invites draw the guild card |

Real comparison (2026-11-25, int-bin 4ad5b6ae, 1536×840 @1.5, private captures in
`/mnt/windata/project-phoenix/compare/invites/cmp/`): the #phoenix test message (one guild invite,
one dead invite, posted from the official client) and the #général pins popout scrolled to its
two guild invites. Cards match in size, banner gradient, icon, counts, date and button; the chat
cards only appeared once `invite_embed` remeasured the message list after resolving (GPUI list
rows are not redrawn by `refresh_windows`) and retried requests that fail before the session is
ready.
