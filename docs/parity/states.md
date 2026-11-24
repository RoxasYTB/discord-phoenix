# Interactive states parity (hover / active / selected / tooltip)

Source: live vanilla client (CDP, CSSOM rules matched against the on-screen elements with
`:hover`/`:active` stripped, 2026-11-24), the current-client CSS bundles, and the
`vanilla-tooltip-guild` / `vanilla-tooltip-header` dumps. Values only, no CSS copied.

Legend: ✅ implemented and traced to the CSS rule, ❌ missing or wrong, ➖ the state does not exist in the client.

## Guild rail

| component | state | client (CSS source) | ours | status |
|---|---|---|---|---|
| pill `item__o8_Xq` | rest / unread | 8px, opacity 0 → 1 with `.visible`, translateX(-4px → 0) | server_bar.rs `pill` | ✅ |
| pill | hover | height 20px | `pill` | ✅ |
| pill | selected | height 40px | `pill` | ✅ |
| pill | transition | height/opacity/transform .2s ease-out | `PILL_MS` 200 | ✅ |
| guild icon | squircle | fixed squircle mask path (`svg_hqZPsU`), no radius morph on hover in the refresh | `ICON_RADIUS` 12 | ✅ |
| home / add / discover tile `childWrapper_JJ-hgR` | hover, selected | `--background-brand` bg, `--white` glyph, scale 1.1 (`childWrapperHoverScale`), .15s ease-out | brand bg + white glyph, `states::fade_to` 150ms; live DOM: no rail item carries `childWrapperHoverScale`, so nothing scales | ✅ |
| folder `folderIcon_RTMSuZ` | hover | `--background-mod-subtle` bg | `--background-mod-subtle` | ✅ |
| rail tooltip `tooltip_ZDdyhE` | position | card starts 12px right of the tile (x 72 for a 20..60 tile), vertically centred, caret left | anchor at tile right + 12 − 5 | ✅ (fixed) |
| rail tooltip | style | `--background-surface-high`, 8px radius, inset 1px `--border-subtle` + `--shadow-high`, padding 8 12, body-md/semibold 16/20 | tokens | ✅ (fixed: was old palette) |
| rail tooltip | appear | no delay (~60ms), opacity 1, slides 4px leftwards over ~100ms (ref-cache tooltip-rail-appear.json) | 100ms 4px slide + `zoom_card_sized` 0.95 → 1, no fade | ✅ |

## Channel list

| component | state | client | ours | status |
|---|---|---|---|---|
| channel row `wrapper_TNyK-N .link` | hover | `--interactive-background-hover` bg, name `--interactive-text-hover` | sidebar.rs channel row | ✅ |
| channel row | active (pressed) | `--background-modifier-active` → `--interactive-background-active` | `.active()` | ✅ (added) |
| channel row | selected | `--interactive-background-selected` | ✅ | ✅ |
| channel row muted `modeMuted_` | hover | name `--interactive-text-hover` | opacity 1 + hover colour | ✅ |
| thread row | hover / active | same as channel row | `.active()` | ✅ (added) |
| DM home nav rows (Friends, Nitro, Shop, Quests) | hover / active | same link rules | `.active()` | ✅ (added) |
| row action icons `actionIcon_Wx6SnQ` (invite, settings) | reveal on hover | `iconVisibility:hover .iconItem` display block, `.channelInfo` hidden | sidebar.rs `invite_hover` group_hover | ✅ |
| row action icons | hover / active | `--icon-muted` → `--interactive-text-hover`; active `--interactive-text-active` + translateY(.5px) | group_hover colour; group_active colour + 0.5px offset | ✅ |
| category `wrapper_yOdhm_` | hover | name + chevron `--interactive-text-hover` | sidebar.rs:408 | ✅ |

## Member list

| component | state | client | ours | status |
|---|---|---|---|---|
| member row `clickable_J1Adu_ .childContainer` | hover | `--interactive-background-hover`, avatar opacity 1 | members.rs | ✅ (now token) |
| member row | active | `--interactive-background-active`, text `--interactive-text-active` | `.active()` | ✅ (added bg) |
| offline member | hover | opacity back to 1 | ✅ | ✅ |

## Messages

| component | state | client | ours | status |
|---|---|---|---|---|
| avatar `avatar_2i8Bfm.clickable` | hover | drop-shadow `--elevation-medium` (0 4px 4px black 16%) | chat.rs msg-avatar `.hover(shadow)` | ✅ |
| avatar | active | translateY(1px) (decoration too) | `.active(mt 1px)` on the avatar box, which now also holds the decoration (chat.rs msg-avatar) | ✅ (decoration moves with it) |
| username `.clickable` | hover | underline | chat.rs author name `.hover(underline)` | ✅ |
| reaction `reaction_MmKct8` | hover / active | `--reaction-background-hover/-active`, border, count `--reaction-text-hover/-active` | message_ext.rs `reactions` | ✅ (added active + count colours) |
| add-reaction button `reactionBtn_` | reveal / hover / active | `forceShow` variant (padding 5 7, opacity 1), hover/active reaction tokens | message_ext.rs | ✅ |
| reply preview `repliedTextPreview_` | hover | `--text-strong` | chat.rs reply line `.hover` | ✅ (added) |
| reply spine | hover | border `--interactive-text-default` | chat.rs spine `.hover` | ✅ (added) |
| hover toolbar `buttonsInner_` | reveal | right 14px, top −16px, 34px tall, padding 2, radius 8, `--background-surface-high` (ref-cache message-hover-toolbar.json) | chat/message_ext.rs `toolbar` | ✅ |
| toolbar button `button_Z0MHyc` | hover | `--interactive-background-hover`, glyph `--interactive-text-hover`, icon scale 1.1 (.4s cubic-bezier(.16,1,.3,1)) | group_hover colour; 20 → 22px glyph eased both ways over 400ms with `states::cubic_bezier(.16,1,.3,1)` (`message_ext::tool_button`) | ✅ |
| toolbar button | active | `--interactive-background-active`, glyph `--interactive-text-active`, icon scale 1 | `.active()` + group_active | ✅ (added) |

## Header, user panel, composer

| component | state | client | ours | status |
|---|---|---|---|---|
| header icon `clickable_ybUK7Z .icon` | hover / selected | no background; glyph `--icon-muted` → `--icon-subtle`, selected `--icon-strong` | states.rs `header_button` | ✅ (removed the old hover tile) |
| header tooltip | position | shows ~50ms after hover (no delay), centred, card 11px below the button, caret up, body-sm/medium 14/18, scale-in ~100ms (ref-cache tooltip-header-appear.json) | states.rs `below_tip`: centred, card top 11px below, no delay (offscreen: card y 84 = live 84) | ✅ (enter zoom 0.95 → 1 in 100ms via `states::zoom_card`) |
| user panel button `button_Y-8oZW` | hover | `--control-secondary-background-hover`, glyph `--interactive-text-hover`, background .2s ease (`states::fade_layer`) | bg + glyph via `panel-btn` group | ✅ (added) |
| user panel red (muted/deafened) | rest / hover | `--opacity-red-12` → `--opacity-red-24` | voice_panel.rs | ✅ |
| user panel | active | none in CSS | | ➖ |
| audio button pair `audioButtonParent_` | hover | pair `--background-mod-muted`, hovered half `--control-secondary-background-hover` | voice_panel.rs | ✅ |
| composer buttons `button_hfM7yP` (attach, gift, GIF, sticker, emoji) | hover | glyph `--interactive-text-hover`, 8px tile `--interactive-background-selected`, .2s | states.rs `composer_button` | ✅ (added) |
| composer buttons | active | glyph `--interactive-text-active` | `group_active` | ✅ (added) |
| app launcher button | hover / active | same rules | composer.rs `apps` | ✅ (added) |
| shared tooltip (`tooltip.rs`) | style | `tooltip_ZDdyhE` tokens, 14/18 medium | ✅ | ✅ |

## Context menus (`context_menu.rs`, `menu_M3qk4j`)

Source: CSS `_M3qk4j` rules + dumps vanilla-menu-{channel,guild,dm,member,message,…}.

| component | state | client | ours | status |
|---|---|---|---|---|
| menu item | hover (mouse) | `item_:hover` `--background-mod-subtle`, radius 2px | `row` `.hover` | ✅ |
| menu item | focused (keyboard / highlighted) | `colorDefault_.focused_` `--interactive-background-hover`, radius 4px, `--text-strong`; caret/subtext recoloured | `row(lit)` | ✅ |
| menu item | active (pressed) | `colorDefault_:active` `--background-mod-subtle` | `.active()` | ✅ (added) |
| danger item | rest / focused / active | `--text-feedback-critical`; `--background-feedback-mod-critical` | `row(danger)` + `.active()` | ✅ |
| disabled item | rest | `disabled_M3qk4j` opacity .5, no pointer events (live: "Mark As Read" with nothing unread, 34px) | `MenuItem::Disabled`; channel + guild "Mark As Read" | ✅ (added) |
| submenu row | hover opens, caret 20px `--text-muted` → white when focused | `Submenu` | ✅ |
| checkbox / radio | checked colours `--control-brand-foreground-new` | `Check` | ✅ |

## Header labels (DM)

| component | state | client (dump aria-label) | ours | status |
|---|---|---|---|---|
| DM toolbar tooltips | label | vanilla-dm: Démarrer un appel vocal / vidéo, Messages épinglés, Ajouter au MP, Masquer le profil d'utilisateur | fr.yml `header.*` | ✅ (relabelled from the dump) |
| group DM toolbar tooltips | label | vanilla-group-dm: … Inviter dans le groupe privé, Masquer la liste des membres | `gdm-add` / `gdm-members` | ✅ (added) |

## Popouts and settings

| component | state | client | ours | status |
|---|---|---|---|---|
| emoji picker cell `emojiItem_6OcK5D` | hovered / selected | `--interactive-background-selected` | pickers/grid.rs | ✅ |
| picker category rail `categoryItemDefaultCategory_` | hover / selected | hover `--interactive-background-hover`; selected `--background-base-low` | grid.rs `rail_item` | ✅ |
| picker tabs (GIF / Stickers / Emoji) | hover / active | `--control-secondary-background-hover`; active `--background-mod-normal` (+ strong on hover) | pickers/mod.rs | ✅ |
| settings sidebar `item_xyJHva` | hover | `--background-mod-muted`, label + glyph `--text-strong` | settings/view.rs `nav_item` | ✅ (glyph + label tint added) |
| settings sidebar | selected | `--background-mod-normal`, background-color .3s ease on selection | `states::fade_to` 300ms | ✅ (added) |
| settings sidebar destructive (Log Out) | hover | `--background-feedback-mod-critical` | ✅ | ✅ |

## Context menu entry order (dumps vanilla-menu-*, mock renders via `states_shots x,y,right`)

| menu | client order (dump) | ours | status |
|---|---|---|---|
| text channel | Mark As Read (disabled) \| Favorites > \| Invite, Pin to Top, Copy Link \| Mute >, Notification Settings > \| Move To > \| Edit, Duplicate, Create Text Channel, Delete \| Copy ID | same (admin rows behind permissions) | ✅ |
| category | Mark As Read \| Collapse, Collapse All \| Mute >, Notifications > \| Move To > \| Edit, Delete \| Copy ID | same | ✅ |
| guild | Mark As Read \| Invite \| Mute >, Notifications >, Hide Muted, Show All \| Server Settings >, Privacy, Per-server Profile \| Create Channel/Category/Event \| Move To > \| Security Actions, Leave \| Copy ID | same (render: non-admin mock) | ✅ |
| folder | Mark Folder As Read \| Move To > \| Folder Settings, Close All Folders | same | ✅ |
| DM / group DM | as in nav_menus.rs header comment | same | ✅ |
| member (friend) | Profile, Mention, Message, Call, Add Note, Friend Nickname \| Apps >, Invite to Server >, Remove Friend, Ignore, Block \| Roles > \| moderator rows \| Copy ID | same (render) | ✅ |
| message | quick reactions, Add Reaction >, View Reactions \| Reply, Forward, Create Thread \| Copy Text, Pin, Bookmark, Remind Me >, Apps >, Mark Unread, Copy Link, Speak \| delete/report \| Copy ID | same (render) | ✅ |

## Buttons, modals, search

| component | state | client | ours | status |
|---|---|---|---|---|
| Mana button `button_F7qvhY` (secondary / primary / critical-primary) | hover | background + text + border `--control-*-hover` | controls.rs `button` | ✅ (text + border added) |
| Mana button | active | `--control-*-background/-text/-border-active` | `.active()` | ✅ (added) |
| confirm modal (leave server, delete…) `container_dvN-MP` | layout / close hover | 400px, header 16 24 8, close 40px `--control-icon-only-background-hover`, equal-width footer buttons (ref-cache modal-confirm-leave.json) | modal.rs `confirm` | ✅ |
| search result `searchResult_N3Anbl` | rest / hover | `--background-base-lower`, 1px `--border-subtle`, radius 8, no hover tint; hover shows the Jump button top-right | search.rs | ✅ (was the old palette + hover tint) |
| search Jump `button_N3Anbl` | hover / active | 24px, `--background-base-lowest`, `--text-default` → `--interactive-text-hover`, active 1px down | search.rs | ✅ (added) |
| pinned message card `messageGroupWrapper_6bgemo` | rest / hover | `--background-surface-higher`, 1px `--border-subtle`, radius 8; hover shows Jump + unpin (top/right 8) | inbox.rs `pins_popout` | ✅ |
| popout sm buttons (Jump, Mark As Read, icon buttons) | hover / active | `secondary_`/`primary_F7qvhY` hover + active background, border, text | inbox.rs `sm_text_button` / `sm_icon_button` | ✅ (active + border/text added) |
| inbox tabs `item_6zZYhF brand_` (top) | rest / hover / active / selected | `--text-subtle`; hover + active `--text-strong`; selected `--text-brand` + 2px bar; no hover bar (`border-bottom: none !important`) | inbox.rs `tab_bar` | ✅ |
| thread card `container_yRVHqK` | rest / hover | 72px, padding 15, radius 8, `--background-surface-higher`, 1px `--border-subtle`; hover `--background-surface-highest` + `--border-normal` (live dump ref-cache threads-popout-live.json, 3 cards) | inbox.rs thread rows | ✅ |

## Group DM picker (`inbox.rs` group DM create)

| component | state | client | ours | status |
|---|---|---|---|---|
| friend row `friend_45OTue` | rest | 48px, padding 8, `--radius-sm`, gap 8, `--text-muted`; discordTag `--text-muted` | inbox.rs gdm-friend | ✅ (was the old palette, 6/8px padding) |
| friend row | highlighted (`friendSelected_`, follows the mouse) | `--background-mod-subtle`, `--text-strong`, background-color .2s ease | `states::fade_layer` 200ms + hover text | ✅ (added) |
| friend checkbox | checked | brand fill | `--control-brand-foreground` | ✅ |
| moderation radio rows (timeout / ban) | rest / hover / selected | not inventoried (no dump) | old palette | ➖ no reference |

Mock data: `MockBackend` now returns 3 pins per channel, 3 archived threads and substring search results, so
`PHX_OPEN=pins|threads|search` renders filled popouts.

## Transitions

`states.rs` has a small colour-fade helper: `track` + `fade_layer` (a bg layer animated
with gpui `with_animation`, keyed on the last hover change, so first paint and remounts
don't flash) and `fade_to` for elements that already track hover.
Per CSS: channel rows, member rows, header icons and toolbar buttons have NO transition
(instant is correct). Faded: user panel buttons .2s, composer buttons .2s, rail tiles .15s,
pill .2s, reactions .1s (with the pressed colour over the fading layer).

Offscreen check: `phoenix-build <out> build -p phoenix-app --example states_shots`, then `<out>/states_shots <dir> x,y[,down|,right] ...`
(`PHX_SETTINGS=1` for settings, `PHX_OPEN=pins|inbox|threads|search[:q]` for popouts)
renders the mock app with the mouse at each point (pressed with `,down`).

## Focus rings (Tab navigation)

Live client, 2026-11-24, Tab from the chat (focus only, nothing opened). `.focus-rings-ring` is an
absolute box over the focused element: `box-shadow: 0 0 0 4px` `--focus-primary` (unset in the 2026
themes: fallback `#00b0f4`), radius = the element's radius (4px when it has none). Shown only after
keyboard input. Phoenix: `ui/focus_ring.rs` (Tab / Shift+Tab at the root, after the composer,
pickers and menus had their turn; Enter / Space click through GPUI's keyboard click).

| element | client ring box (live) | Phoenix | status |
|---|---|---|---|
| composer text area | the 830x57 input box, radius 8 | gpui-component input (own focus, no ring) | ❌ no ring on the text field yet |
| composer buttons (gift, GIF, sticker) | 32x24, 4px inside the 32px button vertically, radius 8 (gift 4) | `states::composer_button_ext` `ring_inset(0, 4)` | ✅ |
| emoji / apps buttons | 40x24 (4px wider than the button on each side) | emoji: same helper with `dx -4`; apps: own button, no ring yet | ✅ emoji / ❌ apps |
| member rows | 244x36, 4px inside the 252x44 row, radius 8 | `members.rs` `ring_inset(4, 3)` | ✅ |
| channel rows | `3d994642c869c81b.js` `FocusRing` `offset: { top: 2, bottom: 2, right: 4 }` on `link_TNyK-N` (radius 8) | `sidebar.rs` chrow `ring_inset_edges(0, 2, 4, 2)` | ✅ |
| rail tiles | 40px tile bounds, radius 12 | `server_bar.rs` `row`, around the 40px tile, radius 12 | ✅ |
| header / user panel buttons | header `114640` (`web.b63d846b329d6470.js`): `Clickable` on 32px `iconWrapper_ybUK7Z`, no offset, radius 4; user panel `484555` (`web.b63d846b329d6470.js`): `button_Y-8oZW` `focusProps: { offset: { left: -1, top: -1, right: 1, bottom: 1 } }`, radius 8 | `states::header_button` (`ring` radius 4), `voice_panel::panel_button` (`ring_inset_edges(-1, -1, 1, 1)` radius 8) | ✅ |
| modal focus trap | client keeps Tab inside an open modal (focus lock) | not done: Tab walks the whole window | ❌ |

## Coverage

81 rows: 79 ✅, 2 ➖, 0 ❌.
Transitions are instant in our app apart from the pill and the rail tooltip; the client uses
.1s–.2s ease-out colour fades.

| shared tooltip (`tooltip::Tipped`) | position / delay / flip | client TooltipLayer: centred on the element, card 11px away, caret toward it, top by default, flipped below near the top edge, nudged 12px inside the window, shown on the first hover frame, 100ms zoom-in, wraps at 200px (ref-cache tooltip-*-appear) | ui/tooltip.rs `Tipped::tip` (21 call sites moved off gpui's cursor tooltip) | ✅ (offscreen check pending) |
