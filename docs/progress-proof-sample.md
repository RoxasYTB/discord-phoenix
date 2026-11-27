# Échantillon du niveau de preuve des lignes ✅

Annexe de [PROGRESS.md](PROGRESS.md). Tirage stratifié de 183 lignes ✅ sur 2465 dans `docs/parity/*.md` (origin/main du 27/11, 8c7b358b), `random.Random(20261127)`, n = max(3, round(160 × ✅ du doc / ✅ total)) par doc. Classement à la main d'après le texte de la ligne, l'en-tête de section et le préambule du doc.

Catégories : A1 = mesure dans le client + comparaison avec un rendu Phoenix (ou fonction exécutée en vrai) ; A2 = mesure dans le client seule (dump, CDP) ; B = tiré du CSS ou du JS sans comparaison ; C = test unitaire, démo ou rendu sans comparaison ; D = sans source.

## Total de ✅ par doc

| doc | ✅ | échantillon | A1 | A2 | B | C | D |
|---|---|---|---|---|---|---|---|
| popouts.md | 292 | 19 | 1 | 10 | 8 | 0 | 0 |
| settings.md | 207 | 13 | 3 | 4 | 4 | 1 | 1 |
| navigation.md | 195 | 13 | 0 | 5 | 5 | 1 | 2 |
| chat.md | 173 | 11 | 2 | 3 | 6 | 0 | 0 |
| call-view.md | 151 | 10 | 0 | 4 | 3 | 2 | 1 |
| message-content.md | 147 | 10 | 1 | 2 | 5 | 2 | 0 |
| inventory.md | 112 | 7 | 0 | 0 | 3 | 0 | 4 |
| settings-wiring.md | 94 | 6 | 0 | 0 | 2 | 3 | 1 |
| real-compare.md | 87 | 6 | 4 | 2 | 0 | 0 | 0 |
| icons.md | 86 | 6 | 0 | 5 | 1 | 0 | 0 |
| states.md | 84 | 5 | 0 | 3 | 2 | 0 | 0 |
| pickers.md | 75 | 5 | 0 | 1 | 3 | 1 | 0 |
| composer.md | 72 | 5 | 0 | 1 | 4 | 0 | 0 |
| forum.md | 72 | 5 | 1 | 1 | 3 | 0 | 0 |
| home.md | 68 | 4 | 0 | 3 | 1 | 0 | 0 |
| shortcuts.md | 66 | 4 | 0 | 2 | 2 | 0 | 0 |
| channel-settings.md | 39 | 3 | 1 | 2 | 0 | 0 | 0 |
| responsive.md | 37 | 3 | 0 | 2 | 1 | 0 | 0 |
| report.md | 36 | 3 | 0 | 2 | 1 | 0 | 0 |
| connection.md | 33 | 3 | 0 | 1 | 1 | 1 | 0 |
| text.md | 32 | 3 | 1 | 1 | 0 | 0 | 1 |
| window-chrome.md | 32 | 3 | 0 | 1 | 2 | 0 | 0 |
| notifications.md | 31 | 3 | 0 | 0 | 0 | 3 | 0 |
| voice-messages.md | 29 | 3 | 0 | 1 | 2 | 0 | 0 |
| themes.md | 27 | 3 | 0 | 1 | 2 | 0 | 0 |
| loading.md | 26 | 3 | 0 | 0 | 2 | 0 | 1 |
| context-menus.md | 25 | 3 | 0 | 3 | 0 | 0 | 0 |
| emoji.md | 25 | 3 | 0 | 1 | 1 | 0 | 1 |
| mentions-voice-row.md | 25 | 3 | 0 | 2 | 1 | 0 | 0 |
| typing.md | 25 | 3 | 0 | 0 | 3 | 0 | 0 |
| sounds.md | 20 | 3 | 0 | 0 | 1 | 2 | 0 |
| search.md | 18 | 3 | 0 | 0 | 3 | 0 | 0 |
| hover-animations.md | 17 | 3 | 0 | 0 | 3 | 0 | 0 |
| fonts.md | 7 | 3 | 0 | 0 | 2 | 0 | 1 |
| **total** | **2465** | **183** | 14 | 63 | 77 | 16 | 13 |

## Proportions

| catégorie | brut (n) | brut % | pondéré % (extrapolé) | ≈ lignes ✅ |
|---|---|---|---|---|
| A1 | 14 | 8 % | 8 % | 205 |
| A2 | 63 | 34 % | 36 % | 884 |
| B | 77 | 42 % | 40 % | 997 |
| C | 16 | 9 % | 8 % | 208 |
| D | 13 | 7 % | 7 % | 171 |
| A (A1+A2) | 77 | 42 % | 44 % | 1089 |

### Par type (pondéré, % de la ligne)

| type | n | part des ✅ | A1 | A2 | B | C | D |
|---|---|---|---|---|---|---|---|
| visuel | 99 | 56 % | 11 % | 45 % | 35 % | 4 % | 4 % |
| interaction | 46 | 25 % | 5 % | 28 % | 56 % | 0 % | 11 % |
| fonction | 38 | 19 % | 6 % | 20 % | 35 % | 31 % | 8 % |

### Docs les plus faibles (part B+C+D dans l'échantillon du doc)

- fonts.md : 3/3 sans mesure client (D 1, C 0, B 2)
- hover-animations.md : 3/3 sans mesure client (D 0, C 0, B 3)
- inventory.md : 7/7 sans mesure client (D 4, C 0, B 3)
- loading.md : 3/3 sans mesure client (D 1, C 0, B 2)
- notifications.md : 3/3 sans mesure client (D 0, C 3, B 0)
- search.md : 3/3 sans mesure client (D 0, C 0, B 3)
- settings-wiring.md : 6/6 sans mesure client (D 1, C 3, B 2)
- sounds.md : 3/3 sans mesure client (D 0, C 2, B 1)
- typing.md : 3/3 sans mesure client (D 0, C 0, B 3)
- composer.md : 4/5 sans mesure client (D 0, C 0, B 4)

## Échantillon

| # | doc | ligne | extrait | cat. | type |
|---|---|---|---|---|---|
| 0 | call-view.md | L17 | speaking ring : border_p87Rny (#1447) → ✅ | A2 | visuel |
| 1 | call-view.md | L53 | volume : module 904054: min(soundVolume * M(setting)/100 * mi… → ✅ unit test | C | fonction |
| 2 | call-view.md | L58 | popout box : picker_1lN7Eh live 531x520, --background-base-low, radius … → ✅ | A2 | visuel |
| 3 | call-view.md | L86 | media sink wants (op 15) : MediaSinkWantsManager t… → ✅ unit tests ➖ offscr… | C | fonction |
| 4 | call-view.md | L92 | test sources : brief: never the real webcam → ✅ | D | fonction |
| 5 | call-view.md | L94 | panel title : "Vidéo connectée" (HtVOdd) instead of "Voix connectée" wh… → ✅ | A2 | visuel |
| 6 | call-view.md | L105 | stop : STREAM_STOP → streamDelete op 19; viewers too (e9) → ✅ | B | fonction |
| 7 | call-view.md | L110 | presets : 476697: Gaming (PRESET_VIDEO) = first al… → ✅ (kept for the sessi… | B | fonction |
| 8 | call-view.md | L154 | Options menu : menu_M3qk4j 225 wide, above the cog… → ✅; "Avancés" ✅ wt/row… | A2 | visuel |
| 9 | call-view.md | L245 | width used : grid: tile width; focused: the call width; strip: min((W−8… → ✅ | B | visuel |
| 10 | channel-settings.md | L23 | separator : separator_6zZYhF → ✅ | A2 | visuel |
| 11 | channel-settings.md | L33 | topic / post guidelines : text, news, forum → ✅ toolbar wraps the s… | A2 | interaction |
| 12 | channel-settings.md | L62 | add members modal : container_dvN-MP size-md 480: channel line, search … → ✅ | A1 | visuel |
| 13 | chat.md | L14 | grouping rules : 4d795811fbaaab27.js module 857069 u → ✅ (#phoenix 24/11: l'… | A2 | fonction |
| 14 | chat.md | L15 | avatar : 738015 avatar_ → ✅ (top was 2) | B | visuel |
| 15 | chat.md | L17 | timestamp cozy : 738015 cozy_ .timestamp_, timestampInline_ → ✅ (ml was 8) | B | visuel |
| 16 | chat.md | L20 | reply line : 738015 repliedMessage_ → ✅ | B | visuel |
| 17 | chat.md | L45 | mention pill : 738015 .mention → ✅ (radius not possibl… | B | visuel |
| 18 | chat.md | L48 | embed parts : 198496 embedAuthorIcon_/FooterIcon_/Thumbnail_/Media_ → ✅ | B | visuel |
| 19 | chat.md | L59 | welcome : 274698 container_/emptyChannelIcon_/description_ → ✅ | B | visuel |
| 20 | chat.md | L149 | thread toolbar order : dump header toolbar 856/898/940/982 → ✅ (structure) | A1 | visuel |
| 21 | chat.md | L175 | optimistic copy : inserted at Enter, id = the send nonce → ✅ | A2 | fonction |
| 22 | chat.md | L214 | Poll display / vote : live poll : answers 50px r8 --background-mod-mute… → ✅ | A1 | fonction |
| 23 | chat.md | L243 | track click : page of 0.875 × view height (490 px for 560), animated ~9… → ✅ | A2 | interaction |
| 24 | composer.md | L33 | row selected / hover : 790340 [aria-selected=true]>.base_, :hover → ✅ CSS only | B | interaction |
| 25 | composer.md | L64 | drop modal icons : 865429 icons_, icon_ one/two/three → ✅ the client's own ar… | A2 | interaction |
| 26 | composer.md | L123 | headings, lists : JS: heading_marker, list_marker … → ✅ (large headings wou… | B | visuel |
| 27 | composer.md | L153 | body : same editor, placeholder of the normal composer, Enter adds a li… → ✅ | B | interaction |
| 28 | composer.md | L156 | send : primary sm "Envoyer" 14 semibold text-overlay-light + SendMessag… → ✅ | B | visuel |
| 29 | connection.md | L19 | first beat : random() × interval after HELLO, then every interval → ✅ | B | fonction |
| 30 | connection.md | L32 | zlib-stream : new inflater per socket → ✅ | C | fonction |
| 31 | connection.md | L89 | op 3 timing : after READY only if the presence dif… → ✅ op3_is_deduplicated… | A2 | fonction |
| 32 | context-menus.md | L14 | DM : vanilla-menu-dm → ✅ | A2 | visuel |
| 33 | context-menus.md | L21 | message: View Interaction Info (app messages) : vanilla-menu-message → ✅ | A2 | interaction |
| 34 | context-menus.md | L25 | link in a message : vanilla-menu-link → ✅ (ui/chat.rs rich: r… | A2 | interaction |
| 35 | emoji.md | L14 | Twemoji file names : FE0F dropped unless the sequence has a ZWJ; the ey… → ✅ | A2 | fonction |
| 36 | emoji.md | L18 | Picker data : Emoji 16.0, client database vnd-emoji.*.js module 13474 (… → ✅ | B | fonction |
| 37 | emoji.md | L45 | Lottie stickers : Lottie → ✅ | D | interaction |
| 38 | fonts.md | L27 | faces declared in the CSS but absent from the cache (gg sans 800 / 800 … → ✅ | B | fonction |
| 39 | fonts.md | L29 | gpui-component Root surface and its tooltips use Theme::font_family (.S… → ✅ | D | visuel |
| 40 | fonts.md | L30 | Nitro titles (nitro-xs/sm/md, nitro.hello, Nitro home section titles, N… → ✅ | B | visuel |
| 41 | forum.md | L46 | Event card actions : chunk bbc562afbe18acf5.js mod… → ✅ JS; ❌ the « … » con… | B | interaction |
| 42 | forum.md | L65 | sidebar row "Server Guide" : vanilla-server-guide #643–#647 → ✅ (dump) | A2 | visuel |
| 43 | forum.md | L105 | spoiler channel toggle in thread settings : IS_SPO… → ✅ unit test + live AP… | A1 | fonction |
| 44 | forum.md | L134 | gallery view : forum chunk 10923e5c961551e9.js: Ma… → ✅ bundle + CSS (unit … | B | visuel |
| 45 | forum.md | L136 | media channel (type 16) : getDefaultLayout() (web … → ✅ JS; ⚠ the client dr… | B | visuel |
| 46 | home.md | L23 | Close button, plated : opacity 1, plate behind the glyph, glyph white .9 → ✅ | B | visuel |
| 47 | home.md | L24 | Close tooltip "Close DM" / "Leave Group" : hover → ✅ | A2 | interaction |
| 48 | home.md | L33 | Online / All / Pending tabs : idle --text-subtle, … → ✅ (this pass, it used… | A2 | visuel |
| 49 | home.md | L55 | Stream card with "LIVE" badge and preview : rest → ✅ | A2 | visuel |
| 50 | hover-animations.md | L23 | Panneau utilisateur : Paramètres (roue) → ✅ sidebar (panel_butt… | B | interaction |
| 51 | hover-animations.md | L28 | Panneau vocal : Activités → ✅ voice-activity | B | interaction |
| 52 | hover-animations.md | L31 | Compositeur : Cadeau → ✅ composer_button_ext… | B | interaction |
| 53 | icons.md | L19 | Boutique : ShopIcon 20 px, « Boutique » (pWG4ze) → ✅ corrigé | A2 | visuel |
| 54 | icons.md | L25 | Sélectionné : fond --interactive-background-selected, texte --t… → ✅ corrigé | A2 | visuel |
| 55 | icons.md | L26 | En-tête « Messages privés » + : PlusMediumIcon 16 px → ✅ corrigé | A2 | visuel |
| 56 | icons.md | L47 | Ajouter un serveur : CirclePlusIcon, .circleIcon_g8YUxD 20 px → ✅ corrigé | A2 | visuel |
| 57 | icons.md | L159 | Onglets : Applications AppsIcon, Serveurs ServerIc… → ✅ (deux onglets dans … | A2 | visuel |
| 58 | icons.md | L176 | dm.title : « Messages privés » → ✅ | B | visuel |
| 59 | inventory.md | L28 | user-context (vocal) : membre en vocal → ✅ | B | interaction |
| 60 | inventory.md | L56 | switch-accounts-submenu : popout du compte → ✅ inventory2 (compte … | B | interaction |
| 61 | inventory.md | L66 | wishlist-overflow-menu : liste de souhaits du profil → ✅ menus-account (uf, … | B | interaction |
| 62 | inventory.md | L115 | Sélecteur rapide : Ctrl+K → ✅ | D | interaction |
| 63 | inventory.md | L136 | Boîte de réception (Pour toi, Non lus, Mentions) : en-tête → ✅ | D | visuel |
| 64 | inventory.md | L152 | Paramètres : Accessibilité → ✅ | D | visuel |
| 65 | inventory.md | L154 | Paramètres : Langue et heure → ✅ | D | visuel |
| 66 | loading.md | L29 | title : "Did you know" (v0R1Lh), 16px, medium, --text-subtle, 8px below… → ✅ | B | visuel |
| 67 | loading.md | L31 | text block : top:-20px → ✅ | B | visuel |
| 68 | loading.md | L36 | when it hides : the client hides it on gateway connect → ✅ | D | fonction |
| 69 | mentions-voice-row.md | L15 | channel mention icon : icon__RYHZ5 → ✅ CSS; always hash ic… | B | visuel |
| 70 | mentions-voice-row.md | L16 | user mention click : live click → ✅ | A2 | interaction |
| 71 | mentions-voice-row.md | L27 | voice row right-click : live → ✅ | A2 | interaction |
| 72 | message-content.md | L14 | em / strong / u arbitration : simple-markdown parser: same order, highe… → ✅ | B | fonction |
| 73 | message-content.md | L35 | Embed description : embedDescription_ body-sm/normal --text-default → ✅ | B | visuel |
| 74 | message-content.md | L40 | Image / gifv embeds : bare media, gifv max-h 300 → ✅ (real compare earli… | A1 | visuel |
| 75 | message-content.md | L57 | Audio playback : es controls: PauseIcon while play… → ✅ (nothing plays unle… | B | fonction |
| 76 | message-content.md | L101 | Channel follow (12) : iR: asset 617184 /assets/8b9… → ✅ unit test; ➖ the an… | C | visuel |
| 77 | message-content.md | L106 | App premium (32) : TicketIcon md, dXdgno (with application.n… → ✅ unit tests | C | visuel |
| 78 | message-content.md | L181 | Profile card frame : .guildInviteContainer_fknPy8 → ✅ live 301.6 × 237.2 | A2 | visuel |
| 79 | message-content.md | L185 | Body : 136696 V → ✅ live (wt/rows2, #ph… | A2 | visuel |
| 80 | message-content.md | L195 | Event card : dc.Ay (module 427080) + header 557582… → ✅ from JS (invite_car… | B | visuel |
| 81 | message-content.md | L215 | Message headings and subtext : message rules → ✅ (a heading with an … | B | visuel |
| 82 | navigation.md | L22 | folder header : #78 → ✅ | A2 | visuel |
| 83 | navigation.md | L31 | channel name : #667 → ✅ | A2 | visuel |
| 84 | navigation.md | L38 | user panel : #990/#991 → ✅ | A2 | visuel |
| 85 | navigation.md | L64 | channel name : N-N #643 → ✅ | A2 | visuel |
| 86 | navigation.md | L76 | DM nav rows : me #597..#625 channel_zln_tS → ✅ | A2 | visuel |
| 87 | navigation.md | L91 | title bar : 300559 bar_3zSH5d → ✅ | B | visuel |
| 88 | navigation.md | L99 | guild list : 703298 scroller_5dqHTG>[hidden] → ✅ | B | visuel |
| 89 | navigation.md | L109 | pill : 441674 .full-motion .item → ✅ | B | interaction |
| 90 | navigation.md | L136 | category : 65a wrapperCommon → ✅ | B | interaction |
| 91 | navigation.md | L178 | action buttons : 593586 actionButtons_9U6LUN → ✅ | B | visuel |
| 92 | navigation.md | L196 | write : j() → updateAsync("favorites", …) → ✅ unit tests, never s… | C | fonction |
| 93 | navigation.md | L223 | bouton Inviter de l'en-tête du serveur : ouvre la … → ✅ (n'avait aucun hand… | D | interaction |
| 94 | navigation.md | L229 | liste des salons : Inviter / Paramètres au survol … → ✅ (le clic sélectionn… | D | interaction |
| 95 | notifications.md | L21 | Do Not Disturb, quiet mode, restricted hours : tE(… → ✅ unit test (schedule… | C | fonction |
| 96 | notifications.md | L41 | Threads created : THREAD_CREATE (998e3527691a7525.… → ✅ unit test (thread_c… | C | fonction |
| 97 | notifications.md | L43 | Stage started : STAGE_INSTANCE_UPDATE: send_start_… → ✅ unit test (stage_in… | C | fonction |
| 98 | pickers.md | L16 | frequently used : categorySection (#2525), 42 items → ✅ order: see below | A2 | fonction |
| 99 | pickers.md | L77 | type select : selectField_vWC4bk + listBox_b5PEJH → ✅ | B | interaction |
| 100 | pickers.md | L83 | nav : __navigation--previous / --next → ✅ (chevron glyph inst… | B | visuel |
| 101 | pickers.md | L155 | gear menu action : transitionStateToSearchContext → ✅ (never clicked on t… | B | interaction |
| 102 | pickers.md | L157 | all DMs : SearchFetcher.DX + createRequestPayload → ✅ unit tests; not run… | C | fonction |
| 103 | popouts.md | L25 | server tag chip : chipletContainerInner_k8jyN7, clickable_k8jyN7 (23401… → ✅ | B | interaction |
| 104 | popouts.md | L35 | menu item focused : colorDefault_M3qk4j.focused_ → ✅ (was blurple + whit… | B | interaction |
| 105 | popouts.md | L38 | menu separator : separator_M3qk4j → ✅ (was 4px) | B | visuel |
| 106 | popouts.md | L42 | profile popout : 253781 outer_P1YEjQ.user-profile-… → ✅ (was 300 / floating… | B | visuel |
| 107 | popouts.md | L59 | server tag pill : 866212 guildTagPill_C2XpHA → ✅ (new) | B | visuel |
| 108 | popouts.md | L83 | popout stream card : live popout of un membre stre… → ✅ from the live dump;… | A2 | visuel |
| 109 | popouts.md | L108 | Activity header : N-N #1503 headerContainer_HxSvbL → ✅ | A2 | visuel |
| 110 | popouts.md | L161 | DM member since : CDP headings_NcvRRg → ✅ | A2 | visuel |
| 111 | popouts.md | L166 | DM footer : CDP redesignFooter_YBm90d / button_F7qvhY → ✅ | A2 | visuel |
| 112 | popouts.md | L187 | left icon : iconContainerLeft_M3qk4j → ✅ (was on the right) | A2 | visuel |
| 113 | popouts.md | L189 | caret : svg.caret_M3qk4j → ✅ (was --text-header) | A2 | visuel |
| 114 | popouts.md | L190 | separator : div.separator_M3qk4j → ✅ (was 8 all, divider… | A2 | visuel |
| 115 | popouts.md | L248 | threads "older threads" : vanilla-threads-lrer #13… → ✅ facepile from the t… | A1 | visuel |
| 116 | popouts.md | L250 | thread section : #1432 sectionHeader_6_wQXw → ✅ "N fils rejoints" (… | A2 | visuel |
| 117 | popouts.md | L255 | Friends title : friends-all #41, pending #41 → ✅ (was em dash, Pendi… | A2 | visuel |
| 118 | popouts.md | L272 | text channel : vanilla-menu-channel → ✅ (admin rows gated o… | A2 | visuel |
| 119 | popouts.md | L333 | ack fallback : never read: the guild joined_at as a snowflake; no join … → ✅ | B | fonction |
| 120 | popouts.md | L336 | section dividers : before the first loaded card of sortOrd… → ✅ (wt/unreads) | B | visuel |
| 121 | popouts.md | L408 | popout SCALE : scale(.01) + opacity:0 → scale(1), .12s ease-out, origin… → ✅ | B | interaction |
| 122 | real-compare.md | L202 | Threads popout 480 px instead of 537.6 at 1536 (me… → ✅ inbox.rs (+ title_b… | A1 | visuel |
| 123 | real-compare.md | L204 | "+" menu: upload and thread icons (small) : client… → ✅ composer.rs, icons/… | A1 | visuel |
| 124 | real-compare.md | L381 | ✅ (wt/sweep3-fixes) **High** — settings sidebar: i… → ui/settings/view.rs, … | A2 | visuel |
| 125 | real-compare.md | L390 | ✅ (wt/sweep3-fixes) **Small** — pins: own server t… → ui/inbox.rs cozy_mess… | A2 | fonction |
| 126 | real-compare.md | L537 | User panel hover roll : hoverRoll_Dq3ch1: on hover .default fades out a… → ✅ | A1 | interaction |
| 127 | real-compare.md | L597 | Hover roll slide : .full-motion .default/.hovered … → ✅ closed below (state… | A1 | interaction |
| 128 | report.md | L56 | checkbox row : checkboxRow_DUr8vi, checkboxOption_vTUVyc → ✅ | A2 | visuel |
| 129 | report.md | L58 | footer : footer_5tvYxx → ✅ | A2 | visuel |
| 130 | report.md | L68 | content scrollbar : $m ModalContent (web bundle module 268814/516521): … → ✅ | B | visuel |
| 131 | responsive.md | L79 | Avatar : 40, margin-top 2 → ✅ (was always 40 at 2… | A2 | visuel |
| 132 | responsive.md | L101 | Friends: Active Now column : 599618.*.css @media (max-width:1200px){.no… → ✅ | B | visuel |
| 133 | responsive.md | L107 | Call PiP : live 2026-11-25: [class*=pictureInPictu… → ✅ (the voice card cov… | A2 | fonction |
| 134 | search.md | L15 | filter keys : localized (1TUdFo from/de, i96lO+ mentions, CqCvir has/co… → ✅ | B | fonction |
| 135 | search.md | L19 | dates (j()) : today, yesterday, week, month, year (localized), month na… → ✅ | B | fonction |
| 136 | search.md | L31 | group dividers : eE draws divider_lm2_KW (1px border-subtle, margin 8 -… → ✅ | B | visuel |
| 137 | settings-wiring.md | L51 | desktop-notif : Enable Desktop Notifications → ✅ | D | fonction |
| 138 | settings-wiring.md | L57 | n-react : Someone reacts to my messages → ✅ | B | fonction |
| 139 | settings-wiring.md | L59 | badge : Enable Unread Message Badge → ✅ (tray.rs, unit test… | C | fonction |
| 140 | settings-wiring.md | L64 | e-recs : Recommendations emails → ✅ | B | fonction |
| 141 | settings-wiring.md | L92 | name-styles : Display name styles → ✅ (real_shots PHX_LOC… | C | visuel |
| 142 | settings-wiring.md | L95 | sat-custom : Apply saturation to custom colors → ✅ (theme::user_color,… | C | visuel |
| 143 | settings.md | L24 | keybind key cap : key_dMUH0f → ✅ | B | visuel |
| 144 | settings.md | L29 | voice & video : vanilla-settings-07 #2702–#2777 → ✅ top block 4.4% (07)… | A1 | visuel |
| 145 | settings.md | L37 | Compte : #2684–#2729 → ✅ | A1 | visuel |
| 146 | settings.md | L57 | styles panel : #3170 panel_BfN7RR editingPanel_ → ✅ | A2 | visuel |
| 147 | settings.md | L59 | switcher popout : -switcher dump → ✅ structure; ✅ box: #… | A1 | visuel |
| 148 | settings.md | L205 | Accès (édition) : barre : Ir/Is: default bar; ELIGIBLE_DISABLED « Rempl… → ✅ | B | visuel |
| 149 | settings.md | L207 | Accès (édition) : Postule pour rejoindre : AU/AG/A… → ✅ ; ❌ the profile pre… | B | interaction |
| 150 | settings.md | L218 | role editor : role_tab_display/perms/links.json, r… → ✅ structure and data;… | A2 | visuel |
| 151 | settings.md | L221 | Soundboard : soundboard.json → ✅; ✅ Add Sound modal:… | A2 | visuel |
| 152 | settings.md | L249 | Centre familial : ACCOUNT_FAMILY_CENTER_NESTED_PAN… → ✅ three tabs rebuilt … | A2 | visuel |
| 153 | settings.md | L296 | Bannissements → Rechercher : filtre déjà en direct → ✅ ; ⚠ la modale de dé… | D | interaction |
| 154 | settings.md | L297 | Rôles → Création de rôle : hb.A.createRole (web) : POST /guilds/… → ✅ refusé | B | fonction |
| 155 | settings.md | L334 | Modération ActionRequest::Admin (Kick, Ban, Unban,… → ✅ tests moderation_is… | C | fonction |
| 156 | shortcuts.md | L29 | Ctrl+Shift+M : Toggle mute → ✅ | A2 | interaction |
| 157 | shortcuts.md | L49 | Shift+Page Up : Jump to the oldest unread message → ✅ | A2 | interaction |
| 158 | shortcuts.md | L62 | 1 : CALL_DECLINE → ✅ | B | interaction |
| 159 | shortcuts.md | L63 | 2 : context menu close() → ✅ | B | interaction |
| 160 | sounds.md | L26 | Connect / disconnect : N_: RTC connected → user_join, lost → … → ✅ unit test | C | fonction |
| 161 | sounds.md | L33 | Connected sound (discodo) : connecting screen CF.c… → ✅ unit test (sixteen_… | C | fonction |
| 162 | sounds.md | L35 | Settings rows : Notifications > Sounds (message1, selected channel, cal… → ✅ | B | visuel |
| 163 | states.md | L73 | app launcher button : hover / active → ✅ (added) | B | interaction |
| 164 | states.md | L113 | category : Mark As Read \ → ✅ | A2 | visuel |
| 165 | states.md | L130 | popout sm buttons (Jump, Mark As Read, icon button… → ✅ (active + border/te… | B | interaction |
| 166 | states.md | L132 | thread card container_yRVHqK : rest / hover → ✅ | A2 | interaction |
| 167 | states.md | L175 | header / user panel buttons : header 114640 (web.b63d846b329d6470.js): … → ✅ | A2 | interaction |
| 168 | text.md | L11 | Container text display : root_ sizeReduced_ → ✅ (messages 8.9 → 8.0) | A1 | visuel |
| 169 | text.md | L34 | Cut-out on hover / selected rows : real mask (tran… → ✅ (GPUI paints the ma… | D | visuel |
| 170 | text.md | L41 | Inbox popout scrollbar : scroller_0JZW4Z thin_tqapBM scrollbarGutterSta… → ✅ | A2 | visuel |
| 171 | themes.md | L16 | call container : .theme-midnight:not(.in-popout) .… → ✅ CDP computed; was d… | A2 | visuel |
| 172 | themes.md | L88 | backdrop : --custom-theme-background: linear-gradient(angle, stops) pai… → ✅ | B | visuel |
| 173 | themes.md | L90 | token mix : 952007.8c2302e28f050501.css @supports (color:color-mix(in l… → ✅ | B | visuel |
| 174 | typing.md | L10 | row start : inTextChannel_ / typingDots_ → ✅ | B | visuel |
| 175 | typing.md | L13 | dot animation, reduced motion : .reduce-motion .do… → ✅ sweep5, test reduce… | B | interaction |
| 176 | typing.md | L28 | Open / Unlock buttons : 0dvvEi / zA9d1J → ✅ | B | fonction |
| 177 | voice-messages.md | L73 | Speed : CA = [.75, 1, 1.5, 2] cycled on click, lab… → ✅; time-stretch keeps… | B | interaction |
| 178 | voice-messages.md | L74 | Volume : 20504: VoiceNormalIcon, VoiceLowIcon under half, VoiceXIcon mu… → ✅ | B | interaction |
| 179 | voice-messages.md | L75 | Volume slider : shown on hover of the button or th… → ✅ volume_slider; the … | A2 | interaction |
| 180 | window-chrome.md | L9 | top bar : bar_3zSH5d → ✅ | B | interaction |
| 181 | window-chrome.md | L15 | minimize glyph : dump #48 → ✅ | A2 | visuel |
| 182 | window-chrome.md | L21 | close hover : winButtonClose_:hover → ✅ | B | interaction |

## Exemples typiques par catégorie

**A1**

- #144 settings.md : voice & video : vanilla-settings-07 #2702–#2777 → ✅ top block 4.4% (07)… (dump settings-07 + score de diff 4,4 % puis 4,0 → 3,2 %)
- #145 settings.md : Compte : #2684–#2729 → ✅ (page Compte, diff 4,2 → 4,1 %)
- #22 chat.md : Poll display / vote : live poll : answers 50px r8 --background-mod-mute… → ✅ (sondage du salon de test, vote et retrait faits par Phoenix en vrai)
- #115 popouts.md : threads "older threads" : vanilla-threads-lrer #13… → ✅ facepile from the t… (rendu threads_shots aligné au px sur le nœud #1392 du dump)
- #147 settings.md : switcher popout : -switcher dump → ✅ structure; ✅ box: #… (rendu offscreen PHX_SWITCHER=1 : boîte = dump)

**A2**

- #0 call-view.md : speaking ring : border_p87Rny (#1447) → ✅ (nœud #1447 du dump vanilla-call-view, pas de rendu comparé)
- #82 navigation.md : folder header : #78 → ✅ (dump vanilla #78 : 48×48)
- #78 message-content.md : Profile card frame : .guildInviteContainer_fknPy8 → ✅ live 301.6 × 237.2 (« live 301.6 × 237.2 » relevé dans le client)
- #179 voice-messages.md : Volume slider : shown on hover of the button or th… → ✅ volume_slider; the … (géométrie mesurée par CDP au zoom 1)
- #23 chat.md : track click : page of 0.875 × view height (490 px for 560), animated ~9… → ✅ (pas de page et répétition mesurés (Chrome 158))

**B**

- #14 chat.md : avatar : 738015 avatar_ → ✅ (top was 2) (classe avatar_ du chunk 738015, valeur codée)
- #104 popouts.md : menu item focused : colorDefault_M3qk4j.focused_ → ✅ (was blurple + whit… (règle .focused_ du menu)
- #41 forum.md : Event card actions : chunk bbc562afbe18acf5.js mod… → ✅ JS; ❌ the « … » con… (« ✅ JS » : chunks 428978/482857, rien de vu)
- #24 composer.md : row selected / hover : 790340 [aria-selected=true]>.base_, :hover → ✅ CSS only (« ✅ CSS only » écrit dans la ligne)
- #58 icons.md : dm.title : « Messages privés » → ✅ (clé de traduction /U8Iwa)

**C**

- #1 call-view.md : volume : module 904054: min(soundVolume * M(setting)/100 * mi… → ✅ unit test (formule du module 904054, preuve = « ✅ unit test »)
- #96 notifications.md : Threads created : THREAD_CREATE (998e3527691a7525.… → ✅ unit test (thread_c… (règles THREAD_CREATE, preuve = test thread_create_rules)
- #92 navigation.md : write : j() → updateAsync("favorites", …) → ✅ unit tests, never s… (« unit tests, never sent »)
- #141 settings-wiring.md : name-styles : Display name styles → ✅ (real_shots PHX_LOC… (rendu real_shots Phoenix seul, pas de capture client)
- #76 message-content.md : Channel follow (12) : iR: asset 617184 /assets/8b9… → ✅ unit test; ➖ the an… (message système, « ✅ unit test »)

**D**

- #62 inventory.md : Sélecteur rapide : Ctrl+K → ✅ (« Ctrl+K → ui/switcher.rs ✅ », aucune source)
- #64 inventory.md : Paramètres : Accessibilité → ✅ (page Accessibilité présente, pas de source)
- #93 navigation.md : bouton Inviter de l'en-tête du serveur : ouvre la … → ✅ (n'avait aucun hand… (« ouvre la modale » + handler, rien d'autre)
- #37 emoji.md : Lottie stickers : Lottie → ✅ (« Lottie / media::lottie (unchanged) »)
- #153 settings.md : Bannissements → Rechercher : filtre déjà en direct → ✅ ; ⚠ la modale de dé… (« revient à la page 1 », comportement client affirmé sans source)

## Limites

- Classement par une seule personne, sur le texte : une ligne B peut avoir été vérifiée sans que le doc le dise (et inversement un « live » peut ne porter que sur une partie de la ligne).
- A1 est généreux pour les lignes « Gaps closed » de real-compare.md (rangées A1 parce que la section suit une mesure « After »).
- Docs à 3 lignes : classement des plus faibles très bruité (intervalle large) ; inventory.md (7/7) et settings-wiring.md (6/6) sont les signaux les plus solides.
- Les lignes dont l'état mêle ✅ et ⚠ (« ✅ JS ; ⚠ … ») sont comptées comme ✅.
