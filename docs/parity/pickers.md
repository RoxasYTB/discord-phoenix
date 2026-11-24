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

## Search filters: option lists (live CDP 2026-11-24, 1536×840 @1.25, Onyx)

Measured on the client by opening De / Dans / Contient (screens + `selectDropdown_vWC4bk`
of the date type list in dump `search-date-operator`), closed without applying.

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| list box | selectDropdown_vWC4bk > listBox_b5PEJH | box | field width, 4px under it, 1px --border-subtle, radius 8, --background-surface-higher, 0 2 8 black 16%, padding 4 | search_filters.rs option_list | ✅ |
| list height | screen | rows | 40px rows, five visible then a scroller | option_list / list_h | ✅ |
| De / Mentions rows | screen | content | 24px avatar, gap 8, display name 16/20, username on the right (muted), 20px checkbox; you first | options() users | ✅ (order after you: recent authors then members, the client's exact ranking is not known) |
| Dans rows | screen | content | channel glyph, name, checkbox; open channel first | options() "in:" | ✅ (the rest in sidebar order; the client's order looks frecency-based) |
| Contient rows | screen | content | glyph + image, vidéo, lien, fichier, intégration (more under the scroll) | HAS | ✅ first five; son / autocollant / sondage and the glyphs are ours |
| Type d'auteur / Épinglé | dump search-more-filters | kind | Type d'auteur is a combobox (input), Épinglé a plain select (`selectButton_`) | field(), options() | ✅ Type d'auteur combobox (Utilisateur / Bot / Webhook) + Épinglé select (Vrai / Faux) wired with option lists |
| open field | screen | style | --input-border-active border, chevron up | field() | ✅ |
| autocomplete | comboBoxInput_vWC4bk | behaviour | typing filters the list (exact, prefix, contains) | options() score | ✅ |
| picked values | — | style | pills before the text | field() | ✅ removable pills (`×`) rendered before the text input |
| popout | CDP | behaviour | "Plus de filtres" hides the search popout (bar back to idle); closing the modal (Escape) brings the popout back with the bar focused | search.rs render | ✅ |
| calendar / type list / option list | — | position | flipped above the field when they would pass the window bottom | render | ✅ |

⚠ During this pass the client sent one `POST /channels/<id>/messages/<id>/ack` (new message
in the open, focused channel while the modal was up). Following the brief the modal was closed
and not reopened, so Type d'auteur / Épinglé were not measured open.

Implementation note: popovers inside a `modal::frame` must not use a nested `deferred` (it paints
under the card's footer); search_filters.rs places them beside the card from canvas probes.

# Search results panel (live CDP 2026-11-24, 1536×840 @1.25, Onyx)

Searches run on the client ("merci", page 2, an empty query `zqxjvkwq`), each cleared afterwards
with the bar's clear button (route and bar text checked unchanged). No non-GET request went out.

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| sizes | compare 1536×788 and 1280×720 @1.25 (client emulated in DIP: w×1.25, dsf 1, its page zoom is 125%) | — | header, groups, cards and pager line up | — | ✅ |
| panel | searchResultsWrap_ | box | 419 wide under the header, black in Onyx (all base tokens are black there) | search.rs results_panel (--background-base-lower, 1px --border-subtle left) | ⚠ token not separable in Onyx |
| header | searchHeader_ | layout | 48px, padding 8 16; total body-md/medium --text-strong ("4 083 résultats", "Pas de résultat") | results_panel, group_digits | ✅ |
| header buttons | button_ sm secondary hasText | style | 32px, radius 8, 1px border, --background-mod-subtle, padding 3 11, 16px glyph + 14/18 medium, gap 8: "Filtres" (opens the modal), "Trier" | sm_secondary | ✅ |
| sort menu | menu_ flexible_ | box / rows | 144 wide under "Trier", right-aligned, 1px --border-subtle, radius 8; 36px radio rows (padding 8): Plus récents / Date / Pertinence | sort_popover, SortOrder → `sort_by=timestamp|relevance` | ✅ |
| scroller | scroller_ thin | padding | 16 16 0 | results_panel | ✅ |
| channel group | searchResultGroup_ / channelNameContainer_ | layout | group margin-bottom 24; 22px header (16px glyph + padding 4, name 16/20 medium --text-strong, margin-bottom 8); right: folder glyph + category 12/16 medium --text-default | group_header | ✅ |
| hit card | searchResult_ | box | 1px --border-subtle, radius 8, margin-bottom 8; cozy message: 40px avatar at 20, text at 80, name 16/22 medium + tag / role icon / APP, time 12 medium --text-muted ("14:58", "Hier à 13:23", "19/11/2026 18:52") | result_card | ✅ |
| match highlight | span.highlight | colour / words | `.highlight` hsl(--yellow-300 / .3) = rgba(253, 184, 51, .3). Words: the client's highlighter (js 698890330839a6c6 module 697530): query split on `/\W+/` (non-ASCII letters split words: "règlement" lights "r" + "glement", seen live), possessive `'s` dropped, lowercase, English stop words out, Porter2 stems (module 83956) compared token by token; applied to every markdown node with string content but code blocks (bold, italic, underline, strike, spoiler, links, inline code, headings, lists, quotes, and user / role / channel mentions, whose parse rules carry a `{type: text, content: "@name"}` child); not emoji, custom emoji, timestamps or sounds (no string content); the group's channel name too (`channelNameText_`, seen live); never inside Components V2 text (live: no `.highlight` under `isComponentsV2_`) | search_highlight.rs, richtext::apply_highlights, search.rs highlight_plain | ✅ |
| reply | repliedMessage_ messageSpine_ | layout | spine + 16px avatar + name + preview, or the italic "Le message n'a pas pu être chargé" placeholder | reply_line | ✅ |
| components v2 | container isComponentsV2_ | content | bot cards drawn like the chat, interactions disabled (`shouldDisableInteractiveComponents`, live: `cursor: not-allowed` inside the buttons) | components::render_static | ✅ (live search dumps: sections + green/red buttons, file, gallery, primary button with animated emoji) |
| header bar | searchBar_ | border | idle border while results show, active while typing | live_bar | ✅ |
| Jump | buttonsContainer_ > button_ | hover | "Y aller", 24px, padding 4, radius 3, 12/16 medium, top-right on hover; jumps to the message | result_card (jump_to_message) | ✅ |
| pager | paginationDock_ / pageControl_ | layout | 56px dock, padding 0 16; 40px "Précédent" / "Suivant" (caret 16, 16/18 semibold, radius 8); 28px round pages 8px apart, current --brand-500 + white; "1 2 3 … 164" | pager, page_items | ✅ |
| empty | emptyResultsWrap_ | layout | 280px column centred, 160px illustration, text 16/24 medium --text-default | empty_state | ✅ layout (our own illustration and wording) |
| error | — | — | not reproducible on the client with a GET | empty_state(error) | ⚠ |
