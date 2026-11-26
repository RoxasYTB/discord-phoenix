# Parity audit: in-app report modal ("Report Message")

Sources (Nov 24 2026 client, French, page zoom 125 %, CDP on 127.0.0.1:9222):

- Flow: webpack module `422495` (the `InAppReportModal`), read from the live client with
  `webpackChunkdiscord_app`; request helpers in module `369053` (chunk `d65c4148a71425e4.js`).
- Menu: `GET /reporting/menu/{message|user|guild}` (`GET_REPORT_MENU`, optional `?variant=`).
  The server returns the nodes in the account language; there is no `language` field, the
  client falls back to `"en"` in the body.
- Submit: `POST /reporting/{name}` (`SUBMIT_REPORT_MENU`). Never executed from Phoenix or the
  driver: the CDP driver blocked every non-GET `*/reporting*` request with `Fetch.failRequest`
  and refused to click "Envoyer un signalement" or any auto-submit node.
- Dumps: `/mnt/windata/project-phoenix/compare/report/client1` (1536×840) and
  `client-800x600`; menus in `/mnt/windata/project-phoenix/dumps/report`.
- CSS: live `document.styleSheets` rules for `_5tvYxx` (container), `_anhLH-` (child rows),
  `_nlOah7` (links), `_vjZEv2` (header), `_QOd4rY` (message preview), `_jUuvHv`
  (breadcrumbs), `_DUr8vi`/`_vTUVyc` (checkboxes), `_KoJTP` (modal root).

## Flow

| behaviour | client source | Phoenix | status |
|---|---|---|---|
| steps come from the server menu | `li`: `useState(root_node_id)` | `Report::set_menu` | ✅ |
| choosing a child pushes `{nodeRef, destination:[label, target], multiSelect}` | `ln` `ee`/`er` | `Report::go` (`ReportStep`) | ✅ |
| target with a `skip` element and a `next` button is jumped over | `li` `P` | `ReportMenu::resolve` | ✅ |
| Back pops the last step and restores its checkbox picks | `li` `V` | `Report::back` | ✅ |
| `is_auto_submit` nodes submit as soon as they are shown ("Je n’aime pas ça", vulgar language) | `ln` effect on `t.is_auto_submit` | `Report::auto_submit` from `go` | ✅ (was: never sent) |
| submit body: `version`, `variant`, `language ?? "en"`, `breadcrumbs` (every step + current node), `elements` (`{checkbox name: [keys]}`), `name`, target ids | module `369053` `h` | `reporting::report_body` | ✅ (was: `language: ""`, `elements: {}`) |
| user target: `user_id` + `guild_id` | `h` `S.t0.USER` | `ReportTarget::User` | ✅ (was `reported_user_id`) |
| guild target: `guild_id` | `h` `S.t0.GUILD` | `ReportTarget::Guild` | ✅ (new) |
| after a successful submit the success node is shown | `ln` `eu` | `Report::submitted` | ✅ |
| submit error: critical notice under the content, "Une erreur s'est produite…" (`h6D8Vy`), `INVALID_FORM_BODY` → `VjAAuP` | `ln` `ed` | `Report::submitted`, `notice` | ✅ |
| Next disabled while a required checkbox group is empty (`should_submit_data`, `require_all_options`) | module `369053` `ks` | `ReportNode::next_blocked`, opacity .5 | ✅ |
| pre-selected checkbox options (4th tuple value) | `ln` effect on `y.data` | `Report::preselected` | ✅ |
| "Signaler le profil d'utilisateur" / "Signaler le serveur" menu entries | experiments `2023-09_iar_user_reporting`, `2026-11-iar-guild-reporting-desktop` (both off for this account: entries absent from the client menus) | not shown | ✅ (same as the client for this account) |
| own messages have no "Signaler ce message" | live menu on our message | `panels.rs` (others only) | ✅ |

## Layout (1536×840, CSS px)

| element | source | value | Phoenix | status |
|---|---|---|---|---|
| modal root | `root__KoJTP small__KoJTP` | 440 wide, 1px `--border-normal`, radius 12, `--modal-background`, max-height 720 | `report::render` | ✅ |
| modal height | `container_5tvYxx` | max-height 70vh (588 at 840, 420 at 600), header and footer fixed, content scrolls | `max_h = min(720, 0.7 × viewport)` | ✅ (was: no limit, text overflowed) |
| narrow window | 800×600 dump | width stays 440 | `min(440, viewport width)` | ✅ |
| header row | `headerRow_5tvYxx` | padding 24 24 0, heading-lg 20/24 600 `--text-strong`, wraps on the full 392 width | title div `w_full` | ✅ |
| close | `closeButton_5tvYxx` | top/right 8, 40×40, radius 8, XLargeIcon 20 `--control-icon-only-icon-default` | `close` icon | ✅ |
| subheader | `subheader_vjZEv2` | margin-top 8, body-md 16/20 `--text-subtle` | | ✅ |
| submit legal text | `te` (`Q0tSKT`) | body-sm 14/18 `--text-default`, link `--text-link` → discord.com/guidelines | `legal()` | ✅ |
| content | `content__KoJTP contentScrollbarGutter__KoJTP` | padding 24 / 24 inline (16 + 8 gutter), gap 16, no bottom padding with a footer | | ✅ |
| "Message sélectionné" | `header_QOd4rY` | heading-xs/medium 14/18 `--text-default`, mb 8 | `section_title` | ✅ |
| message preview box | `messagePreviewContainer_QOd4rY` | 1px `--border-subtle`, radius 8, padding 16 16 16 0, max-height 100; server avatar, role gradient name, server tag, time | `message_preview` (`member_user`, `role_gradient::for_author`) | ✅ (compared on Clem's message, real account) |
| reason rows | `listElement_5tvYxx`, `childButton_anhLH-` | `--background-mod-subtle`, hover `--background-mod-normal`, active `--background-mod-strong`, 1px `--border-muted` between rows, radius 12 on the first/last, text margin 8 16 + padding 12 (60.8 for one line), body-md 16/20 `--text-default`, wraps | `list_row` | ✅ (was: "›" glyph, no wrap) |
| row chevron | `childIcon_anhLH-` | ChevronSmallRightIcon 24, margin 0 16 0 8, `--interactive-text-default` | `chevron-right` | ✅ |
| external link row | `linkButton_nlOah7` | min-height 64, LinkExternalSmallIcon 18 in a 24 box | `link-external` (new icon mapping) | ✅ |
| "En savoir plus" header | `eQ` | hidden when an element has `is_header_hidden` | `links()` | ✅ |
| checkbox row | `checkboxRow_DUr8vi`, `checkboxOption_vTUVyc` | 20×20 indicator radius 4, gap 12, 16 between rows, label body-md `--text-strong`, CheckmarkSmallBoldIcon 18 `--checkbox-icon-active` | `checkbox_row` | ✅ |
| breadcrumbs | `container_jUuvHv` | header heading-xs/semibold "Signaler la catégorie", dot 4×4 radius 2 `--brand-500` at 0.5em, mr 8, body-md, 2-line clamp, 8 between items | render `breadcrumbs` | ✅ |
| footer | `footer_5tvYxx` | padding 24, `--modal-footer-background`, buttons 40 high, gap 8 | | ✅ |
| footer layout | dumps s4/s6 | side by side (192 + 192) when both labels fit; stacked full width when one does not ("Retour" over "Envoyer un signalement") | measured label width vs half width | ✅ |
| info notice | `tn` (`v.w type info`) | inline info notice | `notice(.., false)` | ✅ |

## Not covered

| element | why | status |
|---|---|---|
| success screen | chunk `0113de44bd0f9760.js` (fetched from the CDN, never opened in the client): `tY` = `successImg_-uSFYN` (margin 0 auto 16, padding-top 24) with the CDN art `9d144133…svg` 288×162 (module 810685/700129), inside `successHeader_5tvYxx` (flex column, align start, mb 8); `body_5tvYxx` has no top padding when it holds the success header (CSS `f5940e3c39b0fcf1.css`) | `report::render` success branch | ✅ code |
| « Davantage de choses que tu peux faire » | `tM` (title `k+QA9N` body-sm/semibold `--text-default` mb 8; `upsellsContainer_oPb-gW` `--background-surface-higher`, 1px `--border-subtle`, `--radius-md`, CSS `b4242ffbaff23a34.css`) holding `J.PQ` rows (module 632738, CSS `row_zgsiD2`: margin 0 16, padding 16 0, gap 16, 1px `--border-muted` between rows; title body-md/semibold `--text-strong`, description body-xs/medium `--text-subtle`, gap 2; sm button). Rows: Ignore `U3yyFs`/`naWE6W`/`ICYEfY`→`nDdxOG` (when the node has `ignore_users`), Block `Q1o/f3`/`G08MKu`/`l+7PZY`→`ot2tSp` critical, Mute `TRp5wR`/`yM/+AJ`/`HITUcR`→`E8x4Nj` (only without `ignore_users`, mutes the DM), Delete `c9BHL9`/`dK8S0w`/`ch2xbt`→`f3pnLL` critical (MANAGE_MESSAGES in a guild channel). The live success nodes of the message and user menus (dumps/report) have `ignore_users`, `block_users`, `delete_message`, `share_with_parents`, `settings_upsells` | `more_you_can_do` (ignore = `toggle_ignore`, block = relationship Block, mute = DM channel override, delete = `delete_message`) | ✅ real_shots 8f945b38 on a sandbox message with `PHX_REPORT_PATH==ok` (illustration, Ignore / Block, thin bar; nothing sent); ⚠ `share_with_parents` (Family Center) and `settings_upsells` rows not built. Traced (js-report `mod422495.js`): `share_with_parents` = `tF` (title `HqyWeO` with the linked parents, description `5l/hlt`, button `sYdX/H` → `ntuuk7` once sent, `tW.Xz()` POST, success toast `wr4IT5`), shown only when the account is a teen with Family Center links (`A.Z()` and `j.GR()` non-empty), so it can't appear on this account; `settings_upsells` = `tV` for message reports, title `1yxTIJ`, rows from `D` filtered by report subtype and channel type: DM spam filter (`vJOqMB`/`43UEUh`, SUB_SPAM in DM/GDM, when the filter is DISABLED → NON_FRIENDS), sexual media (`Gtck/t`/`jcRSp6`, CSAM/LOLI/NCP/degrading/unsolicited porn, when an explicit-content filter is SHOW → BLUR), graphic media (`RVX1zT`/`aqlmp8`, gore/violence, gore filter SHOW → BLUR), then the « `olebGx` » link to MESSAGING_PERMISSIONS; remaining4: `settings_upsells` built (`recommended_settings`): the subtype is `report_type` of the node the report was submitted from (`L(A[e.nodeRef].report_type)` in `li`), eligibility by subtype (`sub_spam`; `sub_csam`, `sub_loli`, `sub_ncp`, `sub_sexually_degrading_content`, `sub_unsolicited_porn`; `sub_gore`, `sub_glorifying_violence`), the DM spam row only in DMs and when the filter is off (`spam` wire 1), the media rows only while one of the three filters is SHOW; Apply (`A8t4Nf` → `h+WsPb`, disabled once applied) writes the same proto fields as the client (spam → NON_FRIENDS, SHOW → BLUR); container `upsellsContainer_xZgbKg` (surface-higher, radius 8, no border), title `1yxTIJ` body-sm/semibold, link `olebGx` text-brand opening Content & Social; test `settings_upsells_follow_subtype_channel_and_filters`. ⚠ `share_with_parents` stays unbuilt (teen account with Family Center links only) |
| content scrollbar | `$m` ModalContent (web bundle module 268814/516521): scroller `thin` by default, no fade, stable gutter, `contentScrollbarGutter__KoJTP` = padding-inline-end 24 − 8 | report content: pl 24, pr 16 + 8px gutter holding `thin_scrollbar_div` | ✅ |

Comparison (private, real account): `/mnt/windata/project-phoenix/compare/report/compare-wide.png`, `compare-narrow.png`, `icons-zoom.png`; renders with `PHX_OPEN=report:<message id>` and `PHX_REPORT_PATH="label|+checkbox key|>|<|=ok"` in `real_shots` (`=ok` shows the success node locally, as after a successful submit, without any request) (steps into auto-submit nodes are refused there and the submit action stays denied).
