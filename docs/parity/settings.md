# Settings pages parity audit

Source: CDP outlines of each page of the live 2026 client (user settings modal, `panel_nNq2Vb` content, 2026-11-23) and the CSS bundles in `~/discord-phoenix-ref/current-client/css/`. Code: `crates/phoenix-app/src/ui/settings/` (`pages.rs` holds the page contents as blocks).

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| sidebar sections | vanilla-settings-open sidebar | order | (General) Account, Data & Privacy, Message Permissions, Notifications / Billing: Nitro, Server Boost, Subscriptions, Gift Inventory, Billing / Experience: Voice & Video, Appearance, Accessibility, System, Language & Time / Games & Apps: Registered Games, Activity Privacy, Connected Apps / (Other) Developer, Log Out — no Vencord/Plugins/Themes/Updater | `mod.rs` `NAV` | ✅ (Billing and Registered Games added; billing pages point to the web) |
| developer page | vanilla-settings-15 | rows | Developer Mode, Application Test Mode (switches) | `pages.rs` `DEVELOPER` | ✅ (extra "User ID" row removed) |
| registered games | vanilla-settings-12 | rows | Current Game ("No game detected", "Not seeing your game? Add it!"), Added Games + IGDB note, Related → Activity Privacy | `pages.rs` `GAMES` | ✅ #2686 `notDetected_` box (bg --background-mod-subtle, r12, p16, 16/22 600 + 14 muted + --text-brand "Add it!"), content mismatch 3.7% (game list: no process scan, empty like a fresh client) |
| sidebar sub-nav | `subnav_8qMUr2` under the active item | rows | the page's category titles | `view.rs` `subnav` / `pages::titles` | ✅ |
| sidebar search | `searchBar` input (40px, 16px text) | behaviour | filters pages whose texts match, hides empty sections, the sub-nav and Log Out | `view.rs` `nav`, `pages::matches` | ✅ |
| category title | `heading-xl/medium_RAOoio` | font | 24/32, 500, --text-strong | `controls::section_title` | ✅ |
| category lead | `body-sm/normal_RAOoio` | font | 14/18, --text-subtle, 4px under the title (14b #3019→#3021; was 14px, fixed) | `pages.rs` `B::Lead` | ✅ |
| row | `baseControlItem_FiiQ6I` | box | padding/gap --space-12, margin-inline -12, min-height 48, --radius-lg | `pages.rs` `item` | ✅ |
| clickable row hover | `.clickable_FiiQ6I` | background | --interactive-background-hover | `pages.rs` `related`, `B::Expand` | ✅ |
| row title | `label.body-md/medium_RAOoio` | font | 16/20, 500, --text-strong | `pages.rs` `title_text` | ✅ |
| switch | `switchIndicator_9AraPt` | position | top-aligned with the title, trailing | `pages.rs` `item(.., top)` | ✅ |
| legend | `legend.body-md/semibold_RAOoio` | font | 16/20, 600, 30px box | `pages.rs` `B::Legend` | ✅ |
| radio group | `group_oJuRbG` | options | 20px indicator, text at 32px, 36px stride, optional body-sm description | `pages.rs` `radio_group` | ✅ |
| select | `selectButton_vWC4bk` + `listBox_b5PEJH` | box | 40px field, list padding 4, 40px options, radius 4 | `pages.rs` `dropdown` | ✅ |
| related settings | `heading-sm/semibold` "Related Settings" + `baseControlItem` with 48px leading icon box and trailing chevron | layout | 16/20 600 --text-muted header | `pages.rs` `related` | ✅ |
| divider | `divider_ZaYWLb` | box | 1px --border-subtle, margin 50px 0 | `pages.rs` `B::Divider` | ✅ |
| keybind key cap | `key_dMUH0f` | box | --background-mod-muted, 1px --border-strong, radius 4, h 23, padding 3 6 4, 12/600, 3px apart | `pages.rs` `key_cap` | ✅ |
| keybind group legend | `legend.body-lg/medium_RAOoio` | font | 20/24, 500 | `pages.rs` `key_group` | ✅ |
| keybind rows | default keybinds list | row | 16/20 --text-default, 1px separators, 56px rows | `pages.rs` `key_group` | ✅ |
| slider markers | `markValue_OnqacX` (vanilla-settings-09) | position | proportional to value (12/14/15/16/18/20/24px) | `controls::marker_slider` (numeric labels → proportional, midpoint hit zones) | ✅ side-by-side match, content mismatch 7.0% |
| profiles page | profile editor modal (`focusLock`, styles panel + preview) | layout | full-screen editor | `profile_editor.rs` `render` (full-screen layer over the app, see the *layer / root* row below) | ✅ layer 1184 wide, 48 each side, styles panel + right column |
| voice & video | vanilla-settings-07 #2702–#2777 | top block | "Micro"/"Haut-parleur" 16/500 labels (340px columns, gap 16), "Volume du micro" sliders (bar 4px, 16px round grabber #2737), "Test du micro" + meter, "Besoin d'aide ? … guide de dépannage." 14/18 | `voice.rs` `render`/`slider`/`mic_test` | ✅ top block 4.4% (07); lower half 07b (#2816 label 180, radios 215/273/331, PTT #2842 410, advanced #2855 471, divider then Caméra #2919): 4.0% → 3.2%; 07c backgrounds 24.9% (font + the 4 animated mp4 tiles stay plain, stills load at runtime) |

### Screen-by-screen pass (2026-11-23, vanilla dumps `vanilla-settings-01..15`, 1280x700 @1.5x, onyx/midnight, FR)

Scores = `% pixels |Δ|>24` on the content panel (dump png vs `dialog_shots settings:<page>` upscaled). Residuals come from the font (gg sans vs Noto Sans wraps differently), the user's real data, and dumps scrolled a few px (e.g. 01 is scrolled 38px: `panel_nNq2Vb` at y=115 instead of 153).

| page | dump node | fix | content before → after | status |
|---|---|---|---|---|
| Compte | #2684–#2729 | no "Display Name" row (only Username/E-mail/Phone) | 4.2% → 4.1% | ✅ |
| sidebar profile tab | #2489/#2490 | "Modifier les profils" + 12px pencil, 4px gap | — | ✅ |
| sidebar search | #2498 `input_0PLWhS` | text starts right after the 30px icon box (no input padding), 16px | — | ✅ |
| sidebar sub-nav | #2510/#2518–#2527 | first row 40px under the item label; Compte adds "Centre familial"; rows wrap instead of overflowing | side 10.0% → 9.8% | ✅ |
| switch rows | #2688 `baseSettingWrapper_4Z`, #2700 `description_VhFsFg` | padding 10 0, description max-width 572 | notifications 8.0% → 7.1%, data 12.4% → 12.6% (font wrap) | ✅ |
| related settings | #2733 `icon_W6yko_` | 48px --background-mod-muted tile behind the 24px glyph | activity 7.7% → 7.5% | ✅ |
| Apparence "Thèmes par défaut" | #2721 heading-sm/semibold | 600 weight | 7.8% | ✅ (08b) 5th "Synchroniser avec l'ordinateur" swatch (current theme bg + 24px RefreshIcon), `displayContainer_yH_g_W` banner (117x69 art, 20/24 title, primary button), limited-time theme (countdown pill + Vice City 135deg swatch), app icons (23 + limited) moved under the theme category with the client's art at runtime: 7.8% → 7.0% |
| Connexions lead | #2685-ish body-sm | full FR sentence | 14.4% → 14.6% | ✅ text + 22 `accountButtonInner_yX-kzH` tiles (44px r8, 52/53px pitch, logos fetched at runtime from the URLs the client uses; 28px app icons for Riot/Battle.net); the Nitro rewards promo card (dynamic offer) is not reproduced |
| Voix & Vidéo breadcrumb | #2671 | "Voix & Vidéo" | — | ✅ |
| Nitro / Facturation | vanilla-settings-05b (re-dumped after the spinner) | `tierCard_20JRSQ` (95deg 3-stop gradient as two halves, r16, perks 32px pitch, overlay buttons) + `giftCard_` with runtime art | 49% → 4.3% (Nitro) | ✅ Nitro. The account is a Nitro Squad member, so the non-subscriber variant couldn't be observed; title is generic "NITRO" (no plan data). Facturation: vanilla-settings-06c (re-dumped on the real Billing page) #3454-#3517 — "Modes de paiement" + gift card row (32px brand card glyph, sm secondary "Utiliser"), dividers, right-aligned primary "Ajouter un mode de paiement", 50px divider, "Historique des transactions" pane (r8, 1px --border-subtle, --background-base-lower, 53px Date/Description/Montant header 12/12 600) ✅ structure; ⛔ blocked: PayPal/card rows, country line, balance amount and transaction rows need the billing REST API (payment-sources, payments), which Phoenix doesn't call |
| Système | vanilla-settings-10 | #2728 `warning_vb4RFD` banner (p8, --radius-xs, 20px icon), #2764 `info_0v8Ae1` notices + sm "Modifier dans les paramètres système", secondary "+ Ajouter" button | 6.7% → 6.4% | ✅ |
| Accessibilité | vanilla-settings-09 | #2690 `preview_TpS9Es` (--background-base-low, --border-normal, p24, 175px max), user's messages + reactions + link, 3 status avatars + sm primary button | 7.0% → 5.9% | ✅ |

## Profile editor (full screen, "Modifier les profils")

Dump `vanilla-settings-profile-editor` (+ `-scrolled`, `-switcher`), 2026-11-23, vanilla client, 1280x700 CSS px. Code: `crates/phoenix-app/src/ui/settings/profile_editor.rs`; offscreen `--example profile_editor -- out.png [dirty]`.

| element | dump node | property | CSS value | our code | status |
|---|---|---|---|---|---|
| layer / root | #3153 `layer_C_WHpX`, #3161 `layoutContainer_s3DB6Y` | layout | 1184 wide, 48 each side, full height | `render` max_w 1184 mx 48 | ✅ |
| styles panel | #3170 `panel_BfN7RR editingPanel_` | box | 242 wide, padding-top 32, --background-base-lowest, 1px --border-subtle left/right | `styles_panel` | ✅ |
| profile switcher | #3178 `header_cuaHiV`, #3185 `selectButton_VrghH6` | header | padding 4 8, border-bottom; button padding 8 4 4 8, r8, 16/20 500 | `styles_panel` header | ✅ |
| switcher popout | `-switcher` dump | rows | main profile (Discord glyph + check) then one row per server with the per-server name | `switcher` (painted by `render` so it overflows the panel) | ✅ structure; ✅ box: #3188 `selectDropdown_vWC4bk` 264x221 at panel (9,80), --background-surface-higher, 1px --border-subtle, r8, shadow 0 2 8 --opacity-black-16; #3189 listbox p4; #3193 `listBoxItem_` r4 hover --background-mod-normal, 40 single / 44 two-line rows; `guildOptionRow_` p 4 4 4 8, leading 32 (ml -8), check 20 + 4 inline. Offscreen `PHX_SWITCHER=1`: popout bg box 57..320 x 80..301 = dump (colour differs only by theme: dump is midnight) |
| groups | #3189 `content_UqVm6V`, #3192 `group_Z5MObs` | spacing | padding 16, gap 24, label h3 14/18 500 --text-strong, gap 8 | `group_label` / `content` | ✅ |
| tiles | #3195/#3231 `tile_HhFoZE` | box | bar 208x42, square 100x100, gap 8, r8, --background-mod-subtle | `tile` | ✅ |
| theme tile | #3281 `gradient_Z2TFt5`, #3284 `swatchInner_` | swatches | two 24px swatches, 2px white border, r8, on a primary→accent gradient | `theme_tile` | ✅ |
| card | #3330 `profile_s3DB6Y` | box | 400 wide, r 16 16 0 0, banner 140, avatar 120 at (32,80) | `card` | ✅ |
| card body | #3373 `profileBody_` | spacing | padding 4 24 32 32, gap 24; name 24/32 600; username 14/18 + 4px dot + pronouns | `card` body | ✅ |
| inline fields | #3383/#3405/#3443 `container_CJLDJI` / `editable_IEutSJ` | editing | display name, pronouns, bio edited in place on the card | `inline_field` + Input/Textarea | ✅ |
| tabs | #3479 `tabBarItem_y4ZEM5` | layout | Board / Activity / Wishlist, padding-bottom 16, 30 apart; "Your widgets" 14/18 600 + "Add widget" | `tabs` | ✅ structure, ✅ widget cards from CSS (`widgetContainer_zEJODa`, see *app widget* row; contents are user data) |
| close | #3164 `modalHeaderButtons_` | box | 31px secondary button, top 40, right 8 | `close` | ✅ |
| unsaved bar | #3327 `profileCardToastContainer_` | content | not dumped (typing into a field is a real edit, skipped as unsafe); reproduced from CSS: `profileCardToastContainer_s3DB6Y` left --custom-modal-v2-padding 48, top 48 + max(system bar 0, private banner 0) + --space-8 = 56, width --custom-modal-v2-profile-width 400, z 7; toast `container_aTnRjo` --background-surface-highest, 1px --border-subtle, --radius-sm 8, padding 10 / 16 10, `actions_aTnRjo` gap 10, `message_aTnRjo` --text-default ellipsis, `resetButton_` padding-inline 0 | `unsaved_bar` | ✅ from CSS (offscreen `dirty`) |
| Nitro locks | n/a (account has Nitro) | locked tiles | `tile_HhFoZE` has no lock badge in the CSS: Nitro-only groups carry the Nitro glyph on the label, `disabled_HhFoZE` opacity .5 (content .6); `tileInner_:after` inset 1px --border-muted. Collectible tiles (nameplate, decoration, effect, frame) stay enabled (shop items) | `tile(…, locked)` on name style + banner, theme tile .5 | ✅ from CSS (mock `PHX_NO_PREMIUM=1`) |

Scores (`% pixels |Δ|>24`, dump png vs offscreen mock render, 1280x700): styles panel 22.3%, card 95.4%, tabs 59.2%, overall 59.2%. The card and right column are dominated by the user's real data (Nitro gradient theme, image banner, custom profile background, widgets) versus mock data, so they are not a layout measure; the styles panel is the comparable region (residuals: avatar/banner images, gg sans vs Noto Sans).

Real-data mode (`PHX_REAL=1 --example profile_editor`, user's own account read-only, same 1280x700 regions):

| run | styles panel | card | tabs | board | right column | overall |
|---|---|---|---|---|---|---|
| mock (e1e62f9) | 22.3% | 95.4% | 59.2% | — | — | 59.2% |
| real data, before fixes | 14.3% | 79.7% | 99.5% | 94.1% | 70.2% | 55.0% |
| + themed card/backdrop, tab geometry, app widget | 14.3% | 12.2% | 8.9% | 18.0% | 12.5% | 12.7% |
| + hero asset size fix, status dot, custom status bubble | 13.4% | 8.9% | 8.0% | 9.2% | 9.2% | 10.0% |
| + name style font, tag select, badges, bio links/offset, note, button colour, banner crop (9565842) | 7.6% | 8.4% | 7.0% | — | 5.4% | 8.8% |

Added rows:

| element | dump node | property | CSS value | our code | status |
|---|---|---|---|---|---|
| themed card | #3330 `profile_s3DB6Y` + `:before` | background | gradient primary→140px→secondary, overlay black/.6 | `card` `card_layers` | ✅ |
| themed backdrop | #3324-#3328 `outer_P1YEjQ`/`inner_`/`backgroundImage_s3DB6Y` | layers | modal colour, black .48→.12 @300, banner .3 masked 280 | `render` `backdrop` | ✅ (no 15px blur) |
| tabs | #3476-#3485 | position | x+32, y+20; item lh 14 pb 16; panel +20 | `tabs` | ✅ 8.9% |
| Activity tab | `vanilla-settings-profile-editor-activity` #4567-#4665 `card_Bu4Al8`, `badgesContainer_CTWncy` | layout | sections gap 24, cards p12 r12 gap 8, 60px image r8, name heading-xs/semibold, badges 12px icon + body-xs | `profile_activity.rs` (`GET /content-inventory/users/{id}/outbox` + `/applications/public`) | ✅ recent activity (12.9% overall), ✅ time rule = client `aJ` (entry snowflake; <12h hours, <9d rounded days, weeks, 30-day months), ✅ NewUserSimpleIcon / FlashIcon, ⛔ "current activity" voice card (needs the user in a voice channel; joining voice is forbidden, so not dumpable) |
| collectible pickers | `vanilla-profile-picker-{decoration,effect,frame,nameplate}` `modal_UlS6S9`, `decorationGridItem_hxzEt-`, `effectGridItem_bYuL-p`, `frameGridItem_EVjahG`, `previewContainer_-JS7Po` | layout | 603/666 wide r12, tiles 75x80 / 83x80 / nameplate 105x80 + 224x48 rows, 12 apart, 2px brand ring; effect tile = presetEffectBackground_ skeleton + thumbnail `?width=100&height=195` filling the width from the top; frame tile = 52x56 sample profile + layer; effect/frame preview = full profile card 304x333 (outer_P1YEjQ p2 r8 theme gradient, banner 105, avatar 80, name/tag/badges, bio 3 lines, "Bouton d'exemple" sm) | `collectible_picker.rs` (`profile_card`, `sample_profile`; owned items `GET /users/@me/collectibles-purchases`, catalogue `GET /collectibles-categories/v2`, Shop order `GET /storefront/recommended-items`) | ✅ modal crop mismatch: decoration 4.3%, effect 21.2% (was 33.7/38.7), frame 12.5% (was 31.1), nameplate ~20.8% (unchanged code; scorer noise vs 18.1). Frames come back from the shop only since the X-Super-Properties advertise the current build (679055 / 1.0.170): 41 frames. Nitro section unlocked for Nitro users. Frame tiles now draw every layer (back, sample profile, front) through `profile_fx::frame`, like the preview card. Shop order uses the client's own endpoint (`/storefront/recommended-items`), which changes daily, so a different order from the dump is server data, not a bug. ⛔ Apply only changes the preview: equipping isn't saved because `ProfilePatch` has no collectible fields, and saving would PATCH the user's real profile (deliberately left out) |
| app widget | #3498-#3539 `widgetContainer_zEJODa` | layout | r16, header 24, content p32 0, hero 204x148 fade, 2x2 items 48px gap 16 | `profile_widgets.rs` | ✅ structure, ✅ hero fade = `imageFadeToLeftMask_sIDBpr` 16-stop eased ramp (GPUI has no masks: fade colour painted at 1-alpha per segment, `FADE_MASK`) |
| card name/tag/badges | #3387 `dnsFont_`, #3408 `selectButtonPill_C3qxaU`, #3412 `container_IqWmhA` | font, layout | name style font; pill 24h p0 6 0 8 r8 + chevron; badges p1 3 gap 2 | `card`, `tag_select`, `badges_row` | ✅ |
| card bio / note | #3443-#3447, #3462-#3475 | links, offset | --text-link, editable at heading bottom +5; note -4 -7 | `bio_markup`, `card` | ✅ |
| Send message colour | #3421 | background | --profile-gradient-button-color = primary walked in Lab to contrast 7-10 vs white (module 498596 `OB`) | `gradient_button_color` (test: #00A7FF → #004D9A ±1) | ✅ |

## Control states (live client, ref-cache `settings-switch-states`, `settings-slider-select-states`, 2026-11-24)

| control | state | CSS value | our code | status |
|---|---|---|---|---|
| switch on | hover | bg #5865F2 → #4452BB (--switch-background-selected-hover) | `controls::switch` `.hover` | ✅ states_shots |
| switch off | hover | border alpha .2 → .4 (--switch-border-hover) | `controls::switch` `.hover` | ✅ states_shots |
| switch | hover target | only the indicator itself (hovering the row label changes nothing) | `controls::switch` | ✅ |
| slider grabber | hover / drag | 16px → 20px, 1px --border-normal, shadow 0 1 4 .14 | `voice::slider` grabber | ✅ |
| select field | hover | no visual change (transition border-color .1s only on focus) | `pages::dropdown` | ✅ |
| marker slider grabber | hover | 16 → 20 | `controls::marker_slider` (grabber painted above the hit zones, non-blocking hitbox) | ✅ |
| select field | open / focus | `selectField_vWC4bk:focus-within` 2px outline --input-border-active, offset -2 | `pages::dropdown` overlay ring | ✅ |
| input / select | hover | `wrapper_UcGqzv:hover` border --input-border-hover | `controls::select_field` `.hover` | ✅ |
| switch / button | pressed | not measured (pressing a switch would toggle it); buttons use the CSS `:active` tokens; the switch uses `--switch-background-active` (blurple-65, set from JS while the pointer is down) | `controls::button`, `controls::switch(id, on)` with `.active` + `group_active(SWITCH_ROW)` on the 7 call sites | ✅ |

## Billing pages (vanilla-settings-16-1/2/3, 2026-11-24)

The live sidebar (FR) matches `NAV` exactly: Compte, Données et confidentialité, Permissions de messagerie, Notifications / Nitro, Boost de serveur, Abonnements, Inventaire des cadeaux, Facturation / Voix & Vidéo, Apparence, Accessibilité, Système, Langue et heure / Jeux enregistrés, Confidentialité des activités, Applications connectées / Développeur, Déconnexion. Clips, Keybinds, Streamer mode, What's New, Merch, HypeSquad, Family Centre and Authorized apps are no longer top-level pages in the 2026 client.

| page | dump node | element | CSS | our code | status |
|---|---|---|---|---|---|
| Boost de serveur | #2727 `header_xDn2Ui` | header | gap 32, mb 32; title heading-xl + gem, lead body-sm --text-subtle; 300x169 video card r10 | `boost::header` | ✅ structure (video thumbnail art is the client's asset: plain surface + title + play button) |
| Boost de serveur | #2742 `guildBoostUnavailableNotice_Jru8ph` | notice pill | white/.1, 2px white, r9999, p 8 16 | `boost::notice` | ✅ (shown because the account is in a Nitro Squad) |
| Boost de serveur | #2749 `card_rH8mBz` | guild cards | --background-surface-high, 1px --border-muted, r16, p12, 40px icon, sm secondary button | `boost::guild_cards` | ✅ layout + statsRow_rH8mBz (14px gem, body-xs --text-muted count, 4px dot, level; `Guild.boosts` / `boost_tier` from `premium_subscription_count` / `premium_tier`) |
| Boost de serveur | #2799 `table_twYJsJ` | perks table | rows p16 0, 1px --border-normal, 4 flex cells p0 16, check 24px --text-strong | `boost::perks_table` | ✅ |
| Boost de serveur | #2923 `card_dgf5AJ` | community cards | 2x2, r16, p12, 42px --icon-muted glyph | `boost::community` | ✅ (our glyphs) |
| Boost de serveur | #2938 `listItem_eICevE` | FAQ | r8 p12 gap 4, 18px chevron | `boost::faq` | ✅ collapsed + expanded (`listItemExpanded_` surface-highest / border-normal, `answer_` pt 8, chevron up; answers from the client strings, each row toggles independently) |
| Abonnements | `content_NedsCW` | title, description, credit fieldset | heading-lg, 16/20, legend body-lg/medium | `gift::subscriptions` | ✅ static text |
| Abonnements | `container_pmhtZL nitro-pink_pmhtZL container_xFL7Bm` (live client 2026-11-24) | Nitro Squad card | --background-surface-high r8 p 4 24 0; 270deg wash hsl(305 76% 31%/.3)→hsl(240 82% 44%/.3) under `radial-gradient(100% 100% at 50% 100%, transparent 60%, #000)`; header pt/pb 12 + 1px --border-muted, wordmark 16px + Beta pill (white, #2f3035 12/16 600, p 0 8); 2-col grid min-h 320: left pr 32 py 24 border-right, "Membre Nitro Squad" 24/32 600, body-sm/medium --text-muted + link, secondary md "Quitter l'abonnement"; right pl 32, header 14/18 500 p 12 4 0 mb 10, 40px rows (p 8 8 8 4, 24px avatar +12, name --text-strong · role --text-subtle) | `gift::squad_card`, data `subscriptions.rs` (GET `/users/@me/premium-group/membership`, `/users/@me/billing/subscriptions/{id}/members`) | ✅ structure + data; wordmark set in type (client asset not copied); radial mask sampled vertically (no GPUI masks); "Quitter" inert |
| Abonnements | `accountCreditRow_ysRrD_` | credit rows | --background-base-lower r5, p16; 32px tile r4 tier gradient (tier0 90deg --premium-tier-0-purple→blue), 24px white wheel; title 16/20 600 --text-strong "Crédit de {plan}", subtitle 14/18 --text-default "Les crédits sont suspendus…" while in a Squad, count 16/20 600 --interactive-text-default "1 mois" | `gift::credit_row`, GET `/users/@me/applications/521842831262875670/entitlements?exclude_consumed=true` (unconsumed entitlements with `subscription_plan`, grouped per plan) | ✅ (live: 1 Nitro Basic monthly credit) |
| Inventaire des cadeaux | `container_J9WkLo` | gift card promo | 90deg --expressive-gradient-blue, r16, min-h 180, p 16 16 16 28, overlay-primary sm buttons | `gift::promo` | ✅ (lottie replaced by our gift glyph) |
| Inventaire des cadeaux | `container_VhFsFg` | redeem codes | label + description, 40px input, md primary | `gift::redeem` | ✅ (field is inert) |
| Inventaire des cadeaux | `container_qZFuGA` | gifting badge | content p32 r12 top --background-mod-subtle, 6 x 96px badge items, footer p24 surface-high + "Send a gift" | `gift::badges` | ✅ layout (badge art = our glyph) |
| Inventaire des cadeaux | — | gifts purchased | "No gifts." + line | `gift::purchased` | ✅ |

## Server settings visible to members (guild menu, vanilla-guild-privacy-modal / vanilla-guild-notifications-modal, 2026-11-24)

| modal | dump node | element | CSS value | our code | status |
|---|---|---|---|---|---|
| Privacy Settings | `container_dvN-MP size-sm` | box | 400 wide, r12, p 8 0 16, header 16 24 8, title "Privacy Settings—<server>" heading-lg/semibold | `modals::privacy::render_privacy` + `card(400.)` | ✅ |
| Privacy Settings | `container_VhFsFg` | rows | DMs, Message Requests, Share my activity, Activity Joining; 16 apart, label body-md/medium, description body-sm --text-muted, switch top-aligned, gap 24 | `privacy::row` | ✅ (two activity rows added, keys `activity_restricted_guild_ids` / `activity_joining_restricted_guild_ids`) |
| Privacy Settings | label/description opacity | requests disabled | .5 while DMs are off | `render_privacy` | ✅ |
| Privacy Settings | `actionBar_dvN-MP` | footer | full-width md primary "Done" | `render_privacy` | ✅ |
| Notification Settings | `container_dvN-MP` 480px | sections | mute (saved), server level radios (saved), community alerts, suppress @everyone / roles / highlights / new events / mobile push (session), override picker label | `modals::guild_notify` (replaces the menu stub) | ✅ structure + override table (`channelNotificationHeaders_Onug95` headers, `override_Onug95` cards with all / mentions / nothing / mute checkboxes, saved through `NavRequest`); ⚠ add-override channel picker is a label only, highlights / events / mobile push not persisted |
