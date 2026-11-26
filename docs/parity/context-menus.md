# Context menus — parity audit

Reference: vanilla dumps `/mnt/windata/project-phoenix/dumps/vanilla-menu-*.json` (private) and
`menu_M3qk4j` in current-client/css; positioning from ReferencePositionLayer in web.*.js.
Renders: `PHX_OPEN=guildmenu|chanmenu[:n]|catmenu[:n]|usermenu[:n]|msgmenu int-bin/real_shots out.png`.

| menu | dump | items / order / separators | status |
|---|---|---|---|
| server | vanilla-menu-guild | Mark As Read (disabled when read) / Invite / Mute >, Notification Settings > (subtext), Hide Muted ☐, Show All ☐ / Server Settings >, Privacy, Per-server Profile / Create Channel, Category, Event / Move To… > / Security Actions, Leave (danger) / Copy ID | ✅ (render matches) |
| folder | vanilla-menu-folder | Mark Folder As Read / Move To… > / Folder Settings, Close All Folders | ✅ |
| text channel | vanilla-menu-channel | Mark As Read / Favorites > / Invite, Pin to Top, Copy Link / Mute >, Notification > / Move To… > / Edit, Clone, Create Text, Delete (danger) / Copy ID | ✅ (render matches) |
| notification submenu | vanilla-menu-channel-notify | Use Category Default (subtext) ◉, All, @mentions, Nothing | ✅ (fr label "@mentions" fixed) |
| category | vanilla-menu-category | Mark As Read / Collapse ☐, Collapse All / Mute >, Notification > / Move To… > / Edit, Delete / Copy ID | ✅ |
| DM | vanilla-menu-dm | Mark As Read / Pin, Favorites > / Profile, Call, Add Note (subtext), Friend Nickname, Close DM / Apps >, Invite >, Remove Friend, Ignore, Block / Mute @x > / Copy User ID, Copy Channel ID | ✅ |
| group DM | vanilla-menu-group-dm | Mark As Read / Pin, Favorites > / Invites, Edit Group / Mute > / Leave (danger) / Copy ID | ✅ |
| member | vanilla-menu-member-2/-5 | Profile, Mention, Send Message, Call, Add/Edit Note, Friend Nickname / Change Nickname, Apps >, Invite >, Remove Friend, Ignore, Block / Roles > (checkboxes) / Mod View, Timeout, Kick, Ban / Copy ID | ✅ ("Envoyer un message", Edit Note fixed) |
| member in voice | vanilla-menu-member | … Watch Stream … / Roles >, Move To > / Mod View, Server Mute ☐, Server Deafen ☐, Disconnect | ✅ |
| own user | vanilla-menu-member-1 | Profile, Mention / Per-server Profile, Apps > / Roles > / Mod View / Copy ID (no note row) | ✅ (note row removed) |
| message | vanilla-menu-message | 4 quick reactions / Add Reaction, View Reactions / Reply, Forward, Thread / Copy Text, Pin (perm), Bookmark, Remind Me > (30m,1h,4h,tomorrow,Monday,custom), Apps >, Mark Unread, Copy Link, Speak / Remove Reactions >, Remove All, Delete (own or Manage Messages), Report (others) / Copy ID | ✅ except below |
| message: Add Reaction > | vanilla-menu-message-addreaction (current-client/dumps) | 12 most used emoji (20px emoji + :name:) / View More (picker) | ✅ |
| message: View Interaction Info (app messages) | vanilla-menu-message | `ui/chat/panels.rs` + `commands/store.rs` `interaction_info` (shown on application/interaction/bot messages, opens info toast with app name, app ID and message ID) | ✅ |
| image (right-click on an image) | vanilla-menu-image (current-client/dumps) | message menu + / Copy Image, Save Image / Copy Image Link, Open Image Link | ✅ (copy and save image fetch the file: `PhoenixApp::copy_image` / `save_image`) |
| forum post (thread) | vanilla-menu-thread (current-client/dumps, 2026-11-24) | Mark As Read (disabled) / Favorites > / Follow, Close, Lock-Unlock, Edit, Edit Tags >, Copy Link / Mute (disabled), Notification Settings (disabled) / Pin-Unpin, Delete (danger) / Copy Thread ID | ✅ forum post cards (`ui/thread_menu.rs`) + sidebar thread rows (`nav_menus::Target::Thread` wired on `sidebar::thread_row`) |
| custom emoji in a message | vanilla-menu-custom-emoji | the plain message menu, nothing emoji-specific | ✅ (same handler as the message) |
| link in a message | vanilla-menu-link | message menu + / Copy Link, Open Link (ids `message-copy-native-link`, `message-open-native-link`) | ✅ (ui/chat.rs rich: right-click on a text link arms `panels::set_menu_link`; icons chat/link, menu/open-link) |
| panel | menu_M3qk4j | radius 8, 1px --border-subtle, --background-surface-higher, --shadow-high, min 188 / max 320, scroller 8px | ✅ |
| rows | item_M3qk4j | 34 (text) / 36 (icon) / 52 (subtext), padding 8, 14/18 medium, icon 20 + mr 8 | ✅ |
| edge nudging | web.*.js nudgeTopAlignment | 12px window margin, max-height = window − top − 12 (was 16) | ✅ |
| keyboard | web.*.js list navigators | Up/Down wrap, Home/End, Right/Left submenu, Enter/Space, Esc | ✅ (type-ahead not seen in the client) |

## Button pass (wt/btn-home, 26/11)

| entry | behaviour now | source | status |
|---|---|---|---|
| server: Server Insights | opens Server Settings on Analytics, shown with the same rule as the settings page (community + Manage Server) | chunk `6c30816cda95e0b0.js` id `insights` → `en.A.open(id, BEX.ANALYTICS)` | ✅ |
| server: Report Raid | shown with Ban, Kick, Timeout or Manage Server on a community server; opens the module 935074 modal (title `uYPGsS`, subtitle `Hg8Ee7` with the help link to article 10989121220631, checkbox group of the five raid types in the client order, gap 16 / padding 8 0 4 from `group_Si_H15`, Cancel / Submit); Submit sends `POST /guilds/{id}/auto-moderation/report-raid` through the guard, which refuses it with the blocked-write toast | `6c30816cda95e0b0.js` (`report-raid`, `$.ic` module 974103), CDN chunk `5ef1ef49ddc769e5.js` (935074), `aed76361ef0ef614.js` (859706 `B`), CSS `3942.05b22d8cf5539c36.css` | ✅ |
| user / DM / message: Apps > | disabled "No Commands Available" row (`YSNlV2`), as the client shows when the app list is empty | `b1ab6c34147db858.js` `menu-commands-empty` | ✅ (Phoenix does not list user-installed app commands yet) |

## Message Details (wt/polish-mix, 26/11)

| item | client | Phoenix | status |
|---|---|---|---|
| « Détails du message » | chunk `15299af603b47329.js` `tZ`: with Developer Mode, clipboard support and a right-click on an emoji (with an id) or a sticker, the « Copier l'identifiant du message » row becomes the submenu `IqqJNI` (CircleInformationIcon): Copy Message ID (`zBoHlf`), Copy Emoji / Sticker ID (`Ap2oVy` / `SJ3249`), and with an image Copy / Open Emoji / Sticker Image Link (`cIoudn` / `B1ubHx`, `gDAM2n` / `qAEi+C`, LinkIcon / BrowserIcon); the link group `tJ` is hidden while the submenu shows | `panels::details_items`, target set on right mouse down by the custom emoji cell (`message_ext`) and the sticker attachment (`chat::attachment`), image = CDN emoji `size=44` / sticker URL | ✅ ⚠ the trailing LinkExternalSmall indicator on « Ouvrir » is not drawn (menu rows have no trailing icon) |
