# Composer parity

Sources: the 2026 client CSS (`~/discord-phoenix-ref/current-client/css`, file prefix + hashed
class) and its JS bundle (behaviour). The base composer box, buttons, placeholder, reply bar and
typing indicator were audited in `chat.md` (rows "composer", "reply bar", "typing").

Nothing was typed into the live client (typing would broadcast "is typing"). Popups that only open
from typed text are built from the CSS rules alone, marked **CSS only**: no live dump, no
side-by-side compare.

The official August 11, 2026 changelog describes desktop game mentions: type `@game`, search games
with a Discord profile, and insert a game embed (examples: Palworld and Sunderfolk). It does not
specify the suggestion row layout, keyboard behavior, exact inserted token payload, or search
endpoint. Phoenix's `GET /games/{id}` use is only for known profile-widget IDs and is not a search
API. `ac-games` remains stored but unwired until a read-only catalog/search route and response
shape are established. Do not substitute the user's current activity list: that is not documented
as the searchable catalog. Evidence: [Discord Update: August 11, 2026](https://discord.com/blog/discord-update-august-11-2026-changelog).

The `send-btn` preference is wired locally in Phoenix. A temporary visual-only capture of the
official client at 1920×1050 showed the regular composer actions, with no Send Message control in
that observed state. The preference state was not changed, so this does **not** establish how the
composer looks when Send Message is enabled. No screenshot or private account content was retained.
Compare enabled and disabled states before upgrading the row below from ⚠.

Renders: `cargo run -p phoenix-app --example shortcuts_shot -- out.png <mention|emoji|channel|long|upload>` (mock data).

| element | CSS / JS source | property | value | our code | status |
|---|---|---|---|---|---|
| autocomplete container | 116815 `autocompleteAttached_rg6La1` | position | absolute, bottom calc(100% + 8px) of channelTextArea_, inset-inline 8 / 16 | ui/autocomplete.rs render | ✅ CSS only |
| autocomplete container | 116815 `autocomplete_rg6La1`, 790340 `autocomplete_CAMiwx` | look | radius 5, --shadow-border + --shadow-high, --background-surface-high | autocomplete.rs | ✅ CSS only |
| autocomplete list | 790340 `autocompleteInner_` | padding | bottom 8 | autocomplete.rs | ✅ CSS only |
| autocomplete row | 790340 `autocompleteRow_` / `base_` | box | row padding 0 8; base padding 8, radius 3; 14/16 medium --interactive-text-default | autocomplete.rs | ✅ CSS only (was 36px rows, 15px, radius 4) |
| row selected / hover | 790340 `[aria-selected=true]>.base_`, `:hover` | bg | --interactive-background-hover | autocomplete.rs | ✅ CSS only |
| row icon | 790340 `autocompleteRowIcon_`, `icon_` | size / gap | 16px icon (channels), margin-inline-end 8; emoji 20px (`emojiImage_`) | autocomplete.rs | ✅ CSS only |
| row secondary text | 790340 `autocompleteRowContentSecondary_` | layout | flex-grow 1, margin-inline-start 16, text-align end, ellipsis | autocomplete.rs | ✅ CSS only |
| section title | 790340 `contentTitle_` | text | 14px medium --interactive-text-default, padding 4px 0, no uppercase | autocomplete.rs | ✅ (was 12px uppercase). JS Autocomplete.Title = div.base_ (padding 8, no row wrapper) > contentTitle_: 8px inline, 12px above and below |
| user row avatar | JS Autocomplete.User: Avatar `SIZE_24`, `getAvatarURL(guild,24)`, `status` | size | 24px with status | autocomplete.rs | ✅ 24px + status cut-out |
| / slash commands popup | 116815/790340 container + rows; 358931 `optionPill_`, `optionPillKey_`, `optionPillValue_` | look | shared autocomplete container and rows; option pills: radius 4, 1px --background-base-lowest, --background-mod-normal, key padding 1 8 on --background-surface-high | ui/chat/slash.rs | ✅ CSS + JS (ApplicationCommandBrowser): wrapper_ row 420px; rail wrapper_mTknAg 48px --background-base-lowest, listPadding [16,8,8,8], one 32px app icon per section 8 apart; list_ listPadding [8,8,0,8], 32px categoryHeader_ (wrapper_kSdzCr: surface-high, padding-inline 8 4, 16px icon + 14px semibold label), rows 56px, sections 16 apart. App icons from the command index's `applications[].icon` |
| sticker suggestions | JS emoji autocomplete `queryResults` / `renderResults` + Autocomplete.Sticker; 790340 `autocompleteRowVerticalSmall_` | rows | after the emojis: Divider, title "Stickers matching q" (`uferGG`), up to max(4, 8 − emojis) sendable stickers (emojis cut to max(6, 10 − stickers)); row = 40px sticker, name, "Similar to: tag" (`PAutaQ`) when matched on a tag, pack/server name at the end; padding 4 8; picking sends the sticker | autocomplete.rs sticker_matches / with_stickers | ✅ (`stickerAutoComplete_` grid is the old 116px layout, not used by the current emoji autocomplete). Emoji rows also get the 4 8 padding |
| upload tray | 331249 `channelAttachmentArea_iH8bsl` | layout | flex, gap 24, padding 20 10 10, overflow-x auto | ui/chat/composer.rs upload_tray | ✅ (was padding 16, divider) |
| upload card | 331249 `upload_JyxBSo` | box | 200x200, padding 8, --background-surface-high, 1px --border-normal, radius --radius-sm (12), --shadow-low | composer.rs | ✅ (was 216, radius 4) |
| upload preview | 331249 `media_sBdlOA` | radius | --radius-sm | composer.rs | ✅ |
| upload filename | 331249 `filename_sBdlOA` | text | margin-top 8, one line, ellipsis | composer.rs | ✅ |
| upload spoiler | 331249 `spoiler_sBdlOA` | filter | blur(--custom-channel-attachment-upload-spoiler-blur-radius 44px) | ui/chat/spoiler.rs (image crate: downscale to 64px, fast_blur, PNG) | ✅ (blur done once, off the UI thread; dimmed until ready) |
| upload action bar | 331249 `actionBar_JyxBSo` | position / look | top-right, translate(25%, -25%); hover-bar popover (surface-high, 1px border-muted, radius 8, padding 2); 20px icons | composer.rs | ✅ |
| upload action: edit | client action bar (Spoiler, Edit, Remove) | — | rename the file | composer.rs open_upload_edit | ✅ "Modify Attachment" modal (JS onSubmit {name, description, spoiler}; locale `Y8ujqr`, `ILJuBq`, `eOB2eR`, `RNH1jn`, `gsI+xC`, `TY+auE`): filename, description sent as `attachments[].description`, spoiler checkbox (checkboxOption_vTUVyc). The modal's own layout chunk (669865) isn't in the local cache: generic form card |
| character counter | JS CharacterCount; 331249 `characterCount_A7cPID` | rule / place | shown when remaining <= max/10 (200 of 2000); absolute bottom 10, inset-inline-end 14; body-sm/semibold, --text-default / --text-feedback-critical | composer.rs | ✅ (was < 200, 12px, below the box) |
| Nitro 4000 limit | JS CharacterCount: max = 4000 with `canUseIncreasedMessageLength`, visible when remaining <= max/10 or (Nitro and length > 2000) | rule | — | composer.rs char_count (+ test) | ✅ |
| edit-in-place hints | 775417/8e8ecc `operations_h_zE4S` | text | 12px medium --text-default, padding 7px 0; links --text-link | ui/chat/message_ext.rs | ✅ (was padding 4, regular) |
| drag-and-drop overlay | 865429 `uploadArea_TLgD1o` | bg | --opacity-black-80 | ui/chat/panels.rs drop_overlay | ✅ (was 70% black) |
| drop modal | 865429 `uploadDropModal_`, `bgScale_`, `inner_` | box | 310x170; --brand-500 radius 10; inner 2px dashed --opacity-white-40 radius 6; title 22px bold; instructions 12/16, mt 4 | panels.rs | ✅ (was 20/14px, no fixed height) |
| drop modal icons | 865429 `icons_`, `icon_ one/two/three` | art / pose | default icons [document, image, code] (JS `ir:[1,0,2]`), 100x130 `icon_` in a 100x60 `icons_`, wraps -70/-80/-70, final keyframe translate(∓44,-6) rotate(∓30deg) | panels.rs upload_icons, icons/art.rs | ✅ the client's own art, read at runtime from the local cache (background-image of `.document_/.image_/.code_<hash>`), posed at the final keyframe. Each new drag replays the enter keyframes (`panels::track_drop`): icon translate/opacity (.8s ease, per-keyframe), bgScale_ bounce, title/instructions fade-in from -4px (.175s/.185s delays). The tilt animates too: side icons are pre-posed at every whole degree 0–36 (`art::UploadArt`) and picked per frame from `icon_pose`, whose tilt stops (0, 0, -36, -28, -32, -30.5, -30) fit the live client sampled every 50ms with the animation paused (overshoot -35.9° at 250ms, within 0.2°; `panels::tests::upload_icon_pose_matches_live_samples`). The middle icon carries the live `drop-shadow(rgba(88,101,242,.5) 0 0 48px)` as an SVG `feDropShadow` (σ 24) on a canvas 60px wider each side (2026-11-24, CDP computed styles) |
| slowmode | d53e40 `cooldownWrapper_bbCgC2`, `slowModeIcon_bbCgC2` | state | right of the typing row: margin-inline-start auto, padding 3 8 / inline-end 16, gap 4; countdown text then icon (margin-inline-start 4); bypass with MANAGE_MESSAGES/MANAGE_CHANNELS | ChannelExtra.slowmode/bypass_slowmode (rate_limit_per_user, cache permissions), composer.rs cooldown(), send held while counting down | ✅ CSS only. The icon (clock) and the mm:ss format are assumed |
| no-permission composer | client notice | state | disabled text area with the notice, no buttons | ChannelExtra.send_blocked (SEND_MESSAGES / SEND_MESSAGES_IN_THREADS from the cache's overwrite computation), composer.rs | ✅ wording = locale key `IYKTTc` of the client's fr/en bundles (fr: "Tu n'as pas la permission…") |
| Send Message preference | official composer screenshot, 1920×1050; enabled state not observed | visibility / interaction | visible control when setting enabled; empty-draft and no-permission behavior still unmeasured | composer.rs `send-btn` → `should_show_send_button`; reuses `submit_composer` | ➖ **besoin utilisateur**: wired; the enabled state only exists after turning the synced Send Message setting on, which the campaign forbids on the real account (it would also sync to the user's other clients). Needs the user to enable it once (or a throwaway account) for a CDP measure of the button, its empty-draft and no-permission states |
| multiline input | 962953 `scrollableContainer_2x8-Qn`, `buttons_2x8-Qn`, `textArea_2x8-Qn` | layout | grows with the text up to --custom-channel-textarea-text-area-max-height (50vh) then scrolls; buttons are a 56px row pinned to the top; text padding 17 0, 16/22; Enter sends, Shift+Enter newline | app.rs composer (TextareaState auto_grow + submit_on_enter), composer.rs | ✅ (was a single-line input). 22px lines: `.line_height` on the Textarea overrides the Input's 1.25rem (its refinement is applied last), padding back to 17 |
| typing indicator | 56501 `base_U9Tvvz` | text / dots | see chat.md | composer.rs typing | ✅ (chat.md) |
| reply bar | 841136 `replyBar_` | see chat.md | — | composer.rs reply_bar | ✅ (chat.md) |

## App launcher (Applications button)

Measured 2026-11-24 on the live client (1536x788, Onyx, French), plus the older dump
`vanilla-apps-launcher`. Code: `ui/chat/app_launcher.rs`.

> **Do not open the launcher on the real account.** Opening it makes the official client send
> `PATCH /users/@me/settings-proto/1` (plus `POST /science`), seen with the CDP Network domain.
> It was opened four times while measuring before this was noticed. Measure it from the dump or
> a throwaway account from now on.

| Element | Live value | Phoenix | Status |
|---|---|---|---|
| popout | `contentWrapper_` 500x680, right edge = button right, bottom 8px above the button, radius 8, `--shadow-border` + `--shadow-high`, bg black in Onyx (`--background-base-lower` family) | `app_launcher::render`, anchor from a probe painted in the button | ✅ geometry; ⚠ bg token chosen among several equal ones in Onyx, other themes not compared |
| search | `searchBarContainer_` padding 16, 468x40 field, radius 8, `--input-background-default`, 1px `--input-border-active` (focused on open), 30px icon box, 16/20 text | reuses the expression picker's `search_box`, focused on open | ✅ |
| sections | `scrollableContent_` padding 0 16 16, 32px between sections, header 24px (pt 4, 16/20 medium `--text-strong`), 8px to the content | same | ✅ |
| Promoted (`type` 5) | two columns of 224x180 cards, gap 12, `--card-background-default` radius 12, 126px banner, details padding 8: 38px icon radius 12 with 1px `--border-subtle`, gap 12, name 16/20 600 `--text-strong`, one line 14/18 `--text-subtle` | `card` | ✅ layout; ⚠ banner uses the app `cover_image`, not checked against the client's banner source |
| row sections (`type` 4) | 460x64 rows, padding 12, 40px icon, 12px radius on the outer corners, 1px `--background-mod-subtle` divider, four rows, "Voir plus" 16/20 medium `--text-brand` on the right of the header | `rows`, "Voir plus" expands the section in place | ✅ rows; ⚠ what "Voir plus" opens in the client was not checked (can't reopen the launcher) |
| data | `GET /application-directory-static/collections?surface=2&active_state=1&platform=1&locale=…`: public, read-only, returns Promoted / Puzzle Games / Strategy… in `position` order, items in `position` order | `Backend::app_directory_collections` | ✅ same order as the live popout (Wordle, Magic Garden, Farm Merge Valley, Watch Together; Daily Word Wheel first in Puzzle Games) |
| Récentes | 48px icon cards, radius 12, gap 12: the account's recently used apps | not shown | ❌ needs the application frecency from the settings protobuf; not parsed yet |
| search results | client searches apps and commands | Phoenix filters the loaded apps by name | ⚠ commands not searched |
| hover / pressed | not measured | none | ⚠ can't reopen the launcher on the real account |
| click an app | opens the app's page (launch / authorise flows) | shows the "not available" notice only | ➖ launching, authorising or installing an app writes to the account |
| window height | only 788px measured | shrinks to keep 8px from the top, 200px minimum | ⚠ |
