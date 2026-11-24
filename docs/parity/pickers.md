# Expression pickers (emoji / GIF / sticker)

Source: dumps `vanilla-picker-emoji`, `vanilla-picker-gif`, `vanilla-picker-sticker` (1280x700 viewport) + CSS bundle 569603.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| picker body | emojiPicker_-sZhZn (#2502) | size | 497x427 | pickers/mod.rs PICKER_W | ✅ |
| nav tabs | nav_58b0l2 (#2495) | height / padding | 30 / 0 16 | pickers/mod.rs tab_button | ✅ |
| search | container_VhFsFg (#2504) | size | 272x40 | pickers/mod.rs search_box | ✅ |
| diversity | diversitySelector_eHzdRB (#2511) | size | 38 | grid.rs tone_button | ✅ |
| emoji cell | emojiItemMedium_6OcK5D | size / img | 48 / 40 | grid.rs render_row | ✅ |
| rail | wrapper_6AvvhR (#2697) | width | 48 | grid.rs rail | ✅ |
| rail Nitro row | row_3MXiRB (#2701) | border-bottom / pb / mb | 1px --border-subtle / 8 / 8 | grid.rs render_body | ✅ (non-Nitro only) |
| rail pill | .small__o8_Xq .item.selected | height | 24px, on default categories too | grid.rs rail_pill | ✅ |
| frequently used | categorySection (#2525), 42 items | source | settings-proto/2 emoji frecency | grid.rs build + rest/frecency.rs | ✅ |
| inspector | inspector_1bDrGE (#2687) | height / graphic | 48 / 28 | grid.rs inspector | ✅ |
| GIF tiles | result_H8nOze | size / gaps | 231x110, 11 / 12 | gif.rs | ✅ |
| GIF favorites tile | img gif_H8nOze under categoryFadeBlurple | image | newest favorite GIF | gif.rs (favorite_gifs) | ✅ |
| GIF trending tile | img under categoryFade | image | first trending GIF | gif.rs | ✅ cause: `/gifs/trending` returns the preview as a raw `static.klipy.com` URL (not proxied), blocked by the image host allowlist until 24dd2a3; checked 2026-11-24 with `PHX_DEBUG_GIF` on real data |
| GIF requests | bundle 3ad442 `GIFS_TRENDING` / `GIFS_SEARCH` / `GIFS_SUGGEST` | query | `media_format` + `locale` (+ `q`, `limit`), no `provider` since Klipy | rest/search.rs gif_query | ✅ |
| GIF search placeholder | intl `T1Frnm` | text | "Search Klipy" / "Rechercher sur Klipy" | picker.search_klipy | ✅ |
| HC borders | `.high-contrast-mode .navButton_58b0l2` / `.navButtonActive_` / `.result_H8nOze` / `.wrapper_6AvvhR` / bare `.wrapper_-sZhZn` | border | 1px --border-subtle / --border-normal / --border-strong / inline-end --border-subtle / --border-normal | mod.rs tab_button, gif.rs hc_outline, grid.rs rail_shell | ✅ CDP computed (iframe) |
| Light selected emoji | `.theme-light .emojiItemSelected_6OcK5D` | bg / shadow | --background-base-low, inset 1px --border-subtle | grid.rs render_row | ✅ CDP computed |
| locked sticker | `.stickerLocked_5z8c5P` | filter | brightness(.4); Light opacity .4 | sticker.rs cell (dark: opacity .3 approximation, GPUI has no filter) | ✅ |
| sticker cell | sticker_5z8c5P | size / gap | 98 / 13 | sticker.rs | ✅ |
| sticker frequent | stickerCategoryGeneric (#2731) | rail + section | clock, first | sticker.rs | ✅ |
| soundboard | official guide + local client bundle `current-client/js/web.b63d846b329d6470.js`; UI still needs a safe visual reference | read path | default sounds: `GET /soundboard-default-sounds`; guild sounds: Gateway `REQUEST_SOUNDBOARD_SOUNDS {guild_ids}` → `SOUNDBOARD_SOUNDS {guild_id, soundboard_sounds[]}`. Observed sound fields: `name`, `sound_id`, `emoji_name`, `emoji_id`, `user_id`, `volume`, `available`; REST route registry also names `/guilds/{id}/soundboard-sounds`, top sounds, and sound metadata routes but method/use not yet proven here | no backend/model/picker | ⚠ read contract partly traced; no voice call, sound playback, upload, or account setting mutation. Picker UI/geometry and send payload remain unmeasured |

# Search filters: date rows and calendar

Source: live dumps (private) `search-date`, `search-date-operator`, `search-date-calendar`
(2026-11-24, opened from the search popout's "Plus de filtres" row and closed without applying)
+ CSS `calendarPicker_7yAVe6`, `dateFilter_R726Px`. Note: the older `search-add-date` dump
shows the quick switcher, not this modal.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| add date button | button_F7qvhY md secondary, full width | content | 20px plus + "Ajouter une date", 8px under the label, 12px under a row, stays for more dates | search_filters.rs date_section | ✅ |
| date row | dateFilter_R726Px | layout | 40px, gap 8: 120px type select, date field (254 in a 430 column), 40px remove | date_row | ✅ |
| type select | selectField_vWC4bk + listBox_b5PEJH | options | "Avant le…" / "Après le…" / "Le…" (en Before / After / During) → `before:` / `after:` / `during:` | DateOp | ✅ |
| date field | container_tafmhI | style | padding 8 12, gap 8, "24 nov. 2026" 16/20 --text-default, 18px calendar glyph | date_row | ✅ |
| remove | removeDateFilterButton_R726Px | icon | 24px trash, --icon-muted | date_row | ✅ |
| clear count | textButton_nJgowo | text | "Effacer les filtres (n)" once something is set | render | ✅ |
| calendar shell | .calendarPicker_7yAVe6 .react-datepicker | box | 322 wide (280 + 2×20 + border), --background-surface-higher, 1px --border-subtle, radius .3rem, legacy elevation border + high, 8px under the field | calendar | ✅ |
| month title | __current-month | text | uppercase 13/20 semibold --text-strong, padding-bottom 20 + 1px rule (Dark --primary-800 30%), margin-bottom 10 | calendar | ✅ |
| nav | __navigation--previous / --next | box | 18x18 at 20,20, 1px --primary-800, radius 2; no "next" on the current month (future days disabled) | calendar | ✅ (chevron glyph instead of the 6x12 background arrow) |
| day names | __day-name | text | 13px medium uppercase, --primary-100 60%, 40 wide, 10px under; Monday first (Sunday first in English) | calendar | ✅ |
| day cells | __day | box | 40x40, 1px --border-subtle left/top, last column/row closed in --primary-800, corner radius 3 | calendar | ✅ |
| day states | __day--outside-month / --disabled / --today / --selected / :hover | colour | 500 at 30%; disabled on --primary-630; today bold; selected 3px --brand-500 bar; hover --brand-500 + white | calendar | ✅ |
| Escape | — | behaviour | closes the calendar / type list before the modal | search.rs on_key | ✅ |

Open points (2026-11-24): the calendar is not flipped or clamped at the window edge (the client scrolls
the date row towards the middle first, so it rarely overflows); the search popout stays open behind
the modal because the modal is a child of the popout; the De / Dans / Contient / Mentions / Type
d'auteur / Épinglé comboboxes are still free-text inputs without their option lists.
Implementation note: popovers inside a `modal::frame` must not use a nested `deferred` (it paints
under the card's footer); search_filters.rs places them beside the card from canvas probes.
