# Text rendering parity

Values come from the live vanilla client (CDP computed styles, cached in
`/mnt/windata/project-phoenix/ref-cache/`) and the CSS bundles in
`~/discord-phoenix-ref/current-client/css/`. Scores come from `scripts/compare/compare.py`
(mismatch %, lower is better). The dumps and renders are private and not in the repo.

| element | CSS source | property | CSS value | our code | status |
|---|---|---|---|---|---|
| UI font | `--font-primary` | font-family | gg sans (from the local Discord cache) | fonts/mod.rs `ui_family` | ✅ |
| Container text display | `root_ sizeReduced_` | font-size / line-height | .875rem / calc(--chat-markup-line-height * .875) = 14 / 19.25 | ui/components.rs `markdown_sized` | ✅ (messages 8.9 → 8.0) |
| Container subtext `-#` | `sizeReduced_ small` | font-size / line-height | .75rem / 16.5px | ui/components.rs `markdown_sized` | ✅ |
| Subtext outside containers | `markup_ small` | size / line-height / color | .875rem / 1.20313rem / --text-subtle | ui/components.rs, ui/chat.rs | ✅ |
| Heading in container | `markup_ h2` (not reduced) | size / line-height / margin | 20 / 27.5 / 0 | ui/components.rs | ✅ |
| Container width | `container_rc97-L` in a flex row | width | content width | ui/components.rs `Comp::Container` | ✅ |
| Emoji in container text | `markup_ .emoji` | size | 1.375em (19.25 at 14px) | ui/chat/message_ext.rs `rich_with_emoji_scaled` | ✅ |
| Activity card name | `userName_g7esM5` | color / font / max-width | --channels-default / 14px, 1.2857, 500 / 85% | ui/members.rs activity_section | ✅ |
| Styled / gradient-role name | `name_Wo4EpK` + font class + `twoColorGradient_ usernameGradient_` | font, letter-spacing, gradient | style font, .01-.04em, (c1, c2, c1) tiled every 100px | fonts::name_paint `NameText` (per-glyph colour, gradient from role.colors) | ✅ |
| Member row height | `member_Iy0c7A` | height | 42 (one line) / 44 (with sub line) | ui/members.rs `member_row_in` | ✅ |
| Member list content width | `content_tqapBM` inside `members_86es0b` | width | 260 of 268 (thin scroller gutter) | ui/members.rs list `.pr(8)` | ✅ |
| Member group header | `membersGroup_86es0b` | padding / height | 24 4 4 20 / 46 | ui/members.rs `group_header` | ✅ |
| Letter-spacing in chat, sidebar, members, headers | – | letter-spacing | normal everywhere on the main screen (live scan); only styled display names have tracking | fonts::name_text | ✅ |
| Ellipsis of letter-spaced styled names | `userName_` text-overflow | ellipsis | "…" | fonts::name_paint `NameText` measures and ends with "…" | ✅ |
| Underline (hovered names, links, mentions) | Blink `text-decoration: underline` auto, headless Chrome 158 probes with gg sans / Journal (72 points, scale 1–2) | position / thickness | one line per box from the primary font: round(top + floor(half leading) + A + max(1, ceil(size/20))), thickness max(1, floor(size/10)) dp | GPUI patch 8 `underline_geometry`, fonts::name_paint `NameText` | ✅ (was one line per glyph group, at the height of its fallback font, 1 dp) |
| Fallback for 𝐍𝐲𝐱𝐢𝐞 (U+1D400 math bold) | CDP `CSS.getPlatformFontsForNode` on the live mention | family | FreeSans (client) | fonts/fallback.rs chain (fc-match sans-serif) | ❌ Phoenix picks a serif math font |
| Channel list label baseline | `name_` 16/20 | baseline | – | side-by-side crop: same baseline | ✅ |

Scores on the same guild/channel:
- First session: messages 8.9 → 8.0, channel list 7.5 → 6.5, overall 11.0 → 10.4.
- Second session: member list 26.6 → 21.5.
| Avatar status glyphs | `svg-mask-status-*` | shapes | online circle; idle minus (.25,.25) r.375; dnd minus rect .75x.25; offline minus r.25 | ui/status_svg.rs | ✅ |
| Avatar status cut-out | `svg-mask-avatar-status-round-<size>` + status rect | position / size | 32: rect 22,22 10x10; 80: rect 60,60 16x16 | ui/status_svg.rs `geometry` | ✅ |
| Cut-out on hover / selected rows | real mask (transparent) | background | the row background shows through | `avatar_with_status_overlay` in members, DM list, user panel, friends | ✅ (GPUI paints the matching colour) |
| Ellipsis (`text-overflow: ellipsis`) | Chromium 152 (headless probes, 2026-11-24) | cut point | longest prefix of whole graphemes whose shaped width + "…" fits; spaces and punctuation kept ("Happy 03/1…", "Hello …", "ab, …") | GPUI patch 4 `truncate_line_end_shaped` (docs/gpui-patches.md) | ✅ (live 1280×720: "Responsable du Dév…", "Hopinett…" identical; was one glyph short with the old per-char sum and its punctuation trim) |
| Ellipsis with `-webkit-line-clamp` | Chromium 152 (headless probes) | cut point | last line ends at its own soft wrap, collapsible space dropped, then "…"; cuts inside a word only when no break fits; punctuation kept ("bbbb…", "cccc, dddd.…") | GPUI patch 4 `truncate_wrapped_line` | ✅ (unit test `test_multiline_truncation_keeps_last_line_word_wrap`) |
| Thin hover scrollbar | `thin_tqapBM fade_tqapBM` on `members_`, channel / DM `scroller_`, rail `stack_` (CDP 2026-11-24) | width / thumb / visibility | 8px stable gutter; thumb `--scrollbar-thin-thumb` (#595a63 in dark/onyx), `border: 2px solid transparent` + `background-clip: padding-box` → 4px bar inset 2px, radius 4 (2 inner), min-height 40; track `--scrollbar-thin-track` transparent; hidden until `:hover` | ui/chat/scrollbar.rs `thin_fade_scrollbar` (members, sidebar list, rail) | ✅ (live 1280×720 member list hovered: thumb x 1274–1278, 50px at the top; rail and DM list show it on hover only). The thumb length uses the average measured row height × the row count, since GPUI only knows the heights of rows it has laid out (the lazy member list showed a 412px thumb instead of the 40px minimum) |
| Chat scrollbar | `scroller_8AE1Qq auto_tqapBM` (CDP `CSS.getMatchedStylesForNode`, 2026-11-24) | width / track / thumb | `overflow-y: scroll`, 16px bar; track and thumb `border: 4px solid transparent` + `background-clip: padding-box` → 8px wide inset 4px, radius 8 (4 inner); track `--scrollbar-auto-track`, `margin-bottom: 36px`, always drawn; thumb `--scrollbar-auto-thumb`, min-height 40, `background: transparent` while `.scroller_8AE1Qq:not(:hover)` | ui/chat/scrollbar.rs `auto_scrollbar` (`AUTO_HOVER_GROUP` on the message list) | ✅ (was inset 1px with the track to the bottom and the thumb always visible) |
| Friends list scrollbar | `peopleList_wVFOQW` + `auto_` (CSS bundle) | bar | same auto bar, thumb always visible, track always drawn (`overflow: scroll`); rows end 36px from the edge (16 bar + 20 margin, compare/home-friends) | ui/home/friends.rs `list` (`.pr(16)`) + `auto_scrollbar_div` | ✅ code from CSS; Phoenix rows ended 20px from the edge before |
| Active Now scrollbar | `scroller_GNEh_C thin_` (no `fade_`), `scrollbarGutter_` = `stable both-edges`, `padding-inline: 8px` | bar | thin bar always visible: 8px, thumb 4px inset 2px, radius 4 (2 inner), track `--scrollbar-thin-track` | ui/home/friends.rs `active_now` + `thin_scrollbar_div` | ✅ code from CSS |
| Inbox popout scrollbar | `scroller_0JZW4Z thin_tqapBM scrollbarGutterStable_` (live CDP, popout opened and closed, no write) | bar | thin bar always visible | ui/inbox.rs + `thin_scrollbar_div` | ✅ |
| Popout and modal scrollbars | DOM class lists of the existing vanilla dumps (`/mnt/windata/project-phoenix/dumps/*.json`, scroller classes `thin_` / `auto_` / `none_` + `fade_` of module 6095 `_tqapBM`) and a static scan of the client chunks (scroller factories 95362 `Ip` thin / `Ar` auto / `d_` none, 516521 `Ch`/`Gt`/`zC`, list scrollers 475825 `OZ` thin / `Ei` auto; `fade` defaults to false; ModalContent `$m` defaults to thin; Menu = `hideScroller ? none : thin`) | variant | see the list below | `scrollbar::{host, handle, bar_for}` + `thin_list_scrollbar` | ✅ |

Variants per view (✅ = Phoenix draws the same bar):
- thin: context menus `scroller_M3qk4j scrollerWithScrollbar_M3qk4j` (padding-inline-end 0, the 8px gutter replaces it) ✅, pins `messagesPopout_6bgemo` ✅, account popout `body_nitlyF` ✅, slash/mention autocomplete `scroller_rg6La1` (JS) ✅, events modal ModalContent ✅, report modal ModalContent ✅, emoji picker list `scroller_EwKtHG list_6c9R7U` ✅, sticker picker `scroller_EwKtHG` ✅, shop `shop_XBXDcd` ✅, inbox ✅ (before).
- thin + fade: threads browser `list_6_wQXw` ✅, profile popout `body_NOlkhE` ✅, apps launcher `scrollableContent_OfZRPz` ✅, Channels & Roles `scroller_pHGfKY` ✅, GIF results `results_H8nOze` (JS `fade:!0`) ✅, user/server settings sidebar `navScroller_cR4ic2` / `sidebarRegionScroller` ✅, profile modal body/tabs ✅ (before).
- auto: quick switcher `scroller_a1DYes` ✅, guild notifications / privacy / topic / more-filters modals `body_dvN-MP` ✅ (notifications, topic, more filters), forum list `list_BkGrJJ` ✅, Browse Channels `browser_fTKmdS` ✅, Server Guide ✅, Nitro home `mainPageScroller_IRnLy8` ✅, settings content `scroller_nNq2Vb` / `contentRegionScroller` ✅ (user settings, server settings, channel settings layout).
- none: search filter popout `container_cbVzxY`, status submenu (Menu `hideScroller`), emoji/sticker category rail `scroller_EwKtHG`, DM profile panel and lightbox `scrollerFade_S_xUx6`, sort-and-view menu (`hideScroller:!0`): Phoenix draws nothing ✅.
- not measured (no dump, no class found in the scan): reactions modal, forward / add-to-favorites modal, search results panel, soundboard picker, group DM popout, thread create panel ⚠.
| Combining marks (cluster font) | Chromium shapes a grapheme cluster with the first font of the stack that covers all of it (CDP `CSS.getPlatformFontsForNode`) | font / width | "ღ🌸~͓̽ǤŁa̠̠ĐØS~🌸ღ" 16px/500 = 140.01px: ღ 12.59 (DejaVu Sans), 🌸 19.96 (Noto Color Emoji), ~͓̽ 8.15 and a̠̠ 8.09 (Noto Sans Medium, web), Ǥ 10.35 (Noto Sans), Ł Đ Ø S ~ in gg sans | GPUI patches 5, 6 and 7 (docs/gpui-patches.md) | ✅ real_shots 1280×720 against the client capture of the same view: every glyph within 1 device px, marks on their base (was 141.8px, marks one letter to the right, ellipsis). Patch 7: text boxes round up to 1/64 px, not to a whole px ("✓ APP" badge 46 → 45.2px, client 45.16) |

- Third session: member list 21.5 → 20.6, messages 8.6 → 3.3, overall 9.8 → 7.3.
- On current main (with the GPUI subpixel patch d25d2ca), rebuilt with phoenix-build: guild rail 3.2 %. The 14.8 % measured earlier came from a binary built before that patch.
