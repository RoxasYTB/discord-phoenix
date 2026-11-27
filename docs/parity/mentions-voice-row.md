# Mentions and voice channel member rows: states

Sources: ~/discord-phoenix-ref/current-client/css + live vanilla client (2026-11-24), cached in
ref-cache/mention-pill-default.json, voice-user-row-default.json, voice-user-row-click.json.

| element / state | CSS source | property | CSS value | our code | status |
|---|---|---|---|---|---|
| mention pill default | `.mention` / `wrapper_NDaLCQ` | bg / color / radius / padding / weight / box | --mention-background / --mention-foreground / 3px / 0 2px / 500 / content area 1.3em (20.8 at 16px, live rect 552,659 173.8x20.8) | ui/chat/message_ext.rs `mention_box` | ✅ one pill per mention with radius and padding (was a bare run background, cut around emoji images) |
| mention across lines | `box-decoration-break: slice` (computed) | edges | radius and padding only on the first and last fragment | message_ext.rs `pill` (first/last cell) | ✅ |
| emoji inside a mention | live DOM `@🌸˖𓂃❀𝐍𝐲𝐱𝐢𝐞❀𓂃˖🌸`: plain text, no `<img class=emoji>`; CDP platform fonts: Noto Color Emoji | rendering | font glyphs | richtext.rs `rich_inline` skips emoji ranges inside mentions | ✅ (were 22px images outside the pill) |
| mention hover target | `.interactive:hover` on the whole span | hover | every fragment of the mention highlights together | ui/chat/mention.rs `hover_pill` / `pill_runs` | ✅ |
| mention pill hover | `.interactive:hover` | bg / color | --brand-500 / --white (50ms) | ui/chat/mention.rs hovered_runs | ✅ CSS (no transition) |
| role mention | `roleMention_ku1OHH wrapper_NDaLCQ interactive` + inline style | color / bg | role color / role color 10 %; a role without colour has no inline style and looks like a user mention | richtext.rs color_role_mentions | ✅ live (inbox Mentions tab, 2026-11-24): radius 3, padding 0 2, 500 16/22 |
| role mention hover | same, transition background-color/color 0.05s | bg / color | coloured: role color 30 %, text unchanged; uncoloured: --brand-500 / white | ui/chat/mention.rs hovered_runs | ✅ live (used to skip role mentions) |
| channel mention icon | `icon__RYHZ5` | size / margin-inline-end | 1em / --space-4 | ui/chat/message_ext.rs CHANNEL_ICON_TOKEN | ✅ CSS; always hash icon (no voice/thread variant) |
| user mention click | live click | opens | profile popout | mention.rs click | ✅ |
| mention of a user first seen in the page | live #général 25/11: `Glados ban <@1574934402113478824>` shows `@catapulteur2daronnes` (user in the message's `mentions`) | label | resolved name | app.rs `rerender_mentions_of` on AppEvent::Users | ✅ (the page was rendered before the Users event and kept "@Utilisateur inconnu") |
| channel mention click | live behaviour | opens | channel | mention.rs click (text chat, never voice) | ✅ |
| mentioned message | `mentioned_SY_P2T` | bg / hover / bar | --message-mentioned-background-default / -hover / 2px --icon-feedback-warning | ui/chat.rs | ✅ |
| role-mention highlight | same | trigger | my role mentioned | richtext.rs build has_role | ✅ (needs my member entry loaded) |
| voice row hover | `listDefault_ .clickable_:hover .content_` | bg | --interactive-background-hover | voice_panel.rs voice_user_row | ✅ |
| voice row active | `:active .content_` | bg | --interactive-background-active | voice_panel.rs | ✅ |
| voice row selected (popout open) | `.selected_ .content_` / `.username_` | bg / color | --interactive-background-selected / --interactive-text-active | voice_panel.rs | ✅ live class seen |
| voice row username | `usernameFont_ body-sm/medium`, hover | font / hover color | 14px 500 18px / --interactive-text-hover | voice_panel.rs | ✅ |
| voice row options button | `optionsButton_YUoOxm` | size / radius / hover | 24px / 4px / --interactive-background-active, hidden unless row hovered | voice_row.rs options_button | ✅ CSS; icon (more-horizontal) not verified live |
| voice row click | live | popout position | left = row right + 8, top = row top | voice_row.rs popout_anchor | ✅ measured (415,291 vs row 118..407,290) |
| voice row right-click | live | menu | voice menu in call, user menu otherwise | voice_panel.rs | ✅ |
| popout "In a voice channel" card | `clickableContainer_Bu4Al8` | card metrics | activity-card padding 8 / --radius-sm / header body-xs/medium / heading-xs/semibold | voice_row.rs voice_activity_card | ✅ (wt/home-now, from JS + CSS) the popout card is the same component as the modal (module 287746 `q`, theme POPOUT): `card_Bu4Al8` padding 8, content gap 8, header body-xs/medium `text-strong` (`msxteM`), thumbnail 60×60 (`container_vg4rr1`, radius-sm, `--card-background-default` + `--background-mod-subtle`) with the POPOUT slots `b` (28 / 20+26 / 18+22+24 / 22+18+18+22) and a 2 px outline, channel heading-xs/semibold `text-default` with the 12 px voice icon, « dans {guild} » body-xs, the voice status (`parseVoiceChannelStatus`, `105065`: pencil on hover), then the full-width Join `96ANUN` / Open Voice `BXxdl7` button under the body (`actions_Bu4Al8` mt 4); the whole card is a `clickableContainer` (hover `--user-profile-overlay-background-hover`) that opens the profile modal on the Activity tab (`584904`). `profile_voice::card` with `Colors::popout`. ⚠ not measured live: on 27/11 the client was on a DM with nobody in voice; the header context menu (`285909`) and the DM / stage header strings are not drawn |

Compare score: not available, the 2026-11-24 01:15 run was discarded (an overlay was open in Discord).

| voice row speaking ring | live `avatarSmall_YUoOxm` 24x24 (vanilla-voice-sidebar #avatar, name at x+32) | box | ring drawn inside the 24px avatar, 2px --green-360 | voice_panel.rs speaking_ring | ✅ box matches dump (was 28px, shifted the name 4px) |
| voice row options container | `voiceUser_:not(:hover) .optionsButtonContainer_{display:none}` | layout | takes no room when not hovered | voice_panel.rs voice_user_row | ✅ icons end at row-10 like `icons_YUoOxm` 361..397 |
| LIVE badge | `liveIcon_YUoOxm base_lMvXrQ baseShapeRound_` | height / radius / bg | 16 / --radius-round / --background-feedback-notification | voice_panel.rs | ✅ CSS (font not in CSS bundle, not verified live) |
| voice row state icons | live `icons_YUoOxm` / `iconGroup_YUoOxm` / `icon_YUoOxm` / `iconServer_YUoOxm` (2026-11-24) | size / gap / color / order | 16×16, gap 4, ends at row − 10; --channels-default, server mute/deaf --icon-feedback-critical; "Muet" before "En sourdine"; no hover change | voice_panel.rs voice_user_row | ✅ flags now sent for GUILD_CREATE / cached voice states, own row from the gateway unless Phoenix holds the call |
