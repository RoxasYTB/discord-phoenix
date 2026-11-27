# Parité des icônes et des libellés (vues sans ack)

Icônes de Phoenix comparées au SVG du client : chemins `d`, viewBox et taille de rendu.

Sources :
- dumps DOM `vanilla-friends-addfriend` (`/channels/@me`), `vanilla-home-friends`, `vanilla-shop` (`/shop`), `vanilla-nitro-home` (`/store`) et `vanilla-settings-08b-Apparence` (superposition), capturés à 1280×700 et 1536×788 ;
- chunks JS du cache local : `web.*.js` pour la liste des MP (`ph`, `po`, `pu`, `Iv`, `fU`, `I9`) et la barre de titre (`a1`) ;
- CSS `834050.*.css` (`linkButton_zln_tS`, `interactive_zln_tS`) et `884848.*.css` (`interactive_nl_k3a`).

Le nom d'icône est retrouvé en faisant correspondre les chemins `d` des dumps aux composants `XxxIcon:function(` du JS (557 chemins indexés). Phoenix extrait ces icônes à l'exécution par nom (`icons/names.rs`), donc les chemins sont identiques dès que le nom est le bon. Les écarts relevés ici portaient tous sur le choix du nom ou la taille.

## Haut de la liste des MP

| Ligne | Client | Phoenix | Statut |
|---|---|---|---|
| Amis | `FriendsIcon` 20 px, « Amis » (TdEu5X), compteur des demandes reçues | `friends` | ✅ |
| Demandes de message | `EnvelopeIcon` 20 px, « Demandes de message », seulement s'il y en a | `Inbox` → `server/envelope` | ✅ corrigé |
| Nitro | `NitroWheelIcon` 20 px ; « Ton Nitro » (4gwVVn) si l'abonnement est Nitro (tier 2), sinon « Nitro » (Ipxkog) | toujours « Ton Nitro » → selon `premium_tier2()` | ✅ corrigé |
| Boutique | `ShopIcon` 20 px, « Boutique » (pWG4ze) | `GiftIcon` → `ShopIcon` | ✅ corrigé |
| Quêtes | `QuestsIcon` 20 px, « Quêtes » (JALI2K) ; badge « nouveau » (y2b7CA) et image d'accroche si une quête est mise en avant | `quests` + `quests_page::entry_point` | ✅ (détail plus bas) |
| Ordre | Amis, ICYMI, Bibliothèque, Demandes, Nitro, Boutique, Centre famille, Quêtes, séparateur | même ordre (sans ICYMI, Bibliothèque ni Centre famille) | ✅ |
| Ligne | `li` 40 px, padding 1px 0, marge gauche 8 ; lien 38 px, padding 8 16 8 10, rayon 8, gap 12 ; texte 16 px graisse 500, interligne 20 | identique | ✅ |
| Repos | texte `--text-muted`, icône `--icon-muted` | icône `TEXT_MUTED` → `ICON_MUTED` | ✅ corrigé |
| Survol | fond `--interactive-background-hover`, texte `--interactive-text-hover` (nl_k3a), icône inchangée | identique | ✅ |
| Sélectionné | fond `--interactive-background-selected`, texte `--text-default`, icône `--icon-strong` | fond `BACKGROUND_MOD_STRONG` → `INTERACTIVE_BACKGROUND_SELECTED`, icône `TEXT_STRONG` → `ICON_STRONG` | ✅ corrigé |
| En-tête « Messages privés » + | `PlusMediumIcon` 16 px | `PlusLargeIcon` → `plus-medium` | ✅ corrigé |
| En-tête « Plus » | `MoreHorizontalIcon` 16 px | `more-horizontal` | ✅ |

## Barre de titre

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| Retour / Avancer | `ArrowSmallLeftIcon` / `ArrowSmallRightIcon` 18 px | `ArrowLarge*` → `title/back`, `title/forward` | ✅ corrigé |
| Titre Amis | `FriendsIcon` 18 px + « Amis » 14 px 500 `--text-default` | `GroupIcon` 16 px `TEXT_STRONG` → `FriendsIcon` 18 px `TEXT_DEFAULT` | ✅ corrigé |
| Titre MP ouvert | `ClydeIcon` 18 px + « Messages privés » (YUU0RF) | `GroupIcon` → `ClydeIcon` | ✅ corrigé |
| Titre Nitro | `NitroWheelIcon` + « Nitro » (Ipxkog) | identique, 18 px | ✅ |
| Titre Boutique | `ShopIcon` + « Boutique » | `GiftIcon` → `ShopIcon` | ✅ corrigé |
| Titre Quêtes | `QuestsIcon` + « Quêtes » | identique | ✅ |
| Titre Découvrir | `CompassIcon` + « Découvrir » (4nEZLk) | identique | ✅ |
| Demandes de message | pas de titre (aucune branche dans `a1`) | « Amis » → aucun titre | ✅ corrigé |
| Boîte de réception, Aide | `InboxIcon`, `CircleQuestionIcon` 18 px, `--icon-muted` | identique | ✅ |

## Rail

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| Ajouter un serveur | `CirclePlusIcon`, `.circleIcon_g8YUxD` 20 px | `PlusLargeIcon` → `plus-circle` | ✅ corrigé |
| Découvrir | `CompassIcon` 20 px | `compass` | ✅ |
| Dossiers | `FolderIcon` | `folder` | ✅ |
| Accueil | image du logo animé (pas de SVG) | logo | ✅ (voir window-chrome.md) |

## Page Amis

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| En-tête | `FriendsIcon` 20 px | identique, suit maintenant le zoom | ✅ corrigé |
| Nouveau groupe privé | `ChatPlusIcon` 20 px | `new-group-dm` | ✅ |
| Carte « Explorer les serveurs » | `CompassIcon` 24 px ; flèche propre à la page (`EF`, viewBox 0 0 20 20, `m5.41667 4.2625…`), `flex: 0 1 auto`, base 20, marge droite 16 ; le texte est aussi en `0 1 auto` : les deux rétrécissent au prorata de leur largeur max-content. Mesuré dans `vanilla-friends-addfriend` (1280×700) : carte 216, texte 125,8, boîte de la flèche 8,7×20, donc glyphe de 8,7 px | `ChevronSmallRightIcon` → `friends/discovery-arrow`, 20×20 fixe → boîte `flex_initial` base 20 `min_w_0`, texte `flex_initial` ; le glyphe prend le côté le plus petit de la boîte mesurée (canvas) | ✅ corrigé |
| Recherche | `MagnifyingGlassIcon` 16 px | identique, suit maintenant le zoom | ✅ corrigé |
| Activités | `GameControllerIcon`, `MusicIcon` 12 px | `activity_icon` | ✅ |

## Panneau utilisateur

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| Micro, casque, paramètres | Lottie (masques `M0,0 L600,0…`) 20 px | `lottie_icons` | ✅ |
| Options d'entrée et de sortie | `ChevronSmallDownIcon` 16 px | `Icon::Chevron` | ✅ |

## Boutique

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| En-tête | `ShopIcon` 24 px, `ChevronSmallDownIcon` 18 px, `TagIcon` 18 px | identique | ✅ |
| Recherche | `MagnifyingGlassIcon` 16 px | identique | ✅ |
| Liste de souhaits (en-tête) | `HeartIcon` 16 px | 20 px → 16 px | ✅ corrigé |
| Flèches du carrousel | `ArrowLargeLeftIcon` / `ArrowLargeRightIcon` 20 px | chevrons → `nav/back`, `nav/forward` | ✅ corrigé |
| Carte : cadeau | `GiftIcon` 20 px | identique | ✅ |
| Carte : souhait | `wishlistButton_M2FQyv md variantDefault` (chunk `4d6e4cce03a0a508.js`, CSS `662387.600c33990e4c134f.css`) : 40×40 en haut à droite (marges 12), rayon 8, bord 1 px `--border-muted`, fond `--control-overlay-secondary-background-default` (survol `-hover`, appui `-active`) ; `HeartOutlineIcon` 20 px `--icon-strong`, ou `HeartIcon` `#ec1060` si l'article est dans la liste ; caché (`visuallyHidden`, opacité 0) tant que la carte n'est pas survolée, sauf si l'article est déjà dans la liste ; infobulle « Ajouter à ta liste de souhaits » (8DkMEQ) / « Retirer de ta liste de souhaits » (yr9TTf) | `shop_page::card` + `wishlist_write` : `POST /users/@me/wishlist/items {sku_id}` ou `DELETE /users/@me/wishlists/{id}/items/{sku}` (`USER_WISHLIST_ITEM`), toujours via `blocked_write` ; l'état vient de `GET /users/{moi}/profile` + `/wishlists/{id}` | ✅ ; remaining4 : pulsation au survol (`hoverPulse_M2FQyv` 1,25 s en boucle, 1 → 1,1 → 0,9 → 1 aux clés 0/20/60/80 %, `cubic-bezier(.23,0,.58,1)` par segment) tant que l'article n'est pas dans la liste, et animation de clic à l'ajout (`clickScale` 0 → 1,35 → 1 et `clickColor` blanc → `#ec1060`, 0,4 s, `cubic-bezier(.67,0,.26,1)`), coupées en mouvement réduit (`.full-motion`) ; test `wishlist_heart_follows_the_client_keyframes`. ⚠ pas de `holdScale` pendant l'appui, pas de « Ça te plaît ? » (47Rhc3) au premier ajout |
| Prix Nitro | `NitroWheelIcon` 18 px | identique | ✅ |

## Paramètres (superposition)

| Entrée | Client | Phoenix | Statut |
|---|---|---|---|
| Compte | `UserIcon` | `user` | ✅ |
| Données et confidentialité | `ShieldLockIcon` | `shield-lock` | ✅ |
| Permissions de messagerie | `ChatShieldIcon` | `chat-shield` | ✅ |
| Notifications | `BellIcon` | `bell` | ✅ |
| Nitro | `NitroWheelIcon` | `nitro` | ✅ |
| Boost de serveur | `BoostGemIcon` | `BoostTier2Icon` → `chat/boost` | ✅ corrigé |
| Abonnements | `SubscriptionIcon` | `subscription` | ✅ |
| Inventaire des cadeaux | `GiftIcon` | `gift` | ✅ |
| Facturation | `CreditCardIcon` | `credit-card` | ✅ |
| Voix & Vidéo | `MicrophoneIcon` | `mic` | ✅ |
| Apparence | `PaintPaletteIcon` | `palette` | ✅ |
| Accessibilité | `AccessibilityIcon` | `accessibility` | ✅ |
| Système | `KeyboardIcon` | `keyboard` | ✅ |
| Langue et heure | `LanguageIcon` | `language` | ✅ |
| Jeux enregistrés | `GameControllerIcon` | `game-controller` | ✅ |
| Confidentialité des activités | `UserPlatformIcon` | `user-platform` | ✅ |
| Applications connectées | `LinkIcon` | `link` | ✅ |
| Développeur | `AngleBracketsIcon` | `angle-brackets` | ✅ |
| Déconnexion | `DoorExitIcon` en `--icon-feedback-critical` | `door-exit` | ✅ |
| Fermer | `XLargeIcon` 16 px | `close` 16 px | ✅ |

Libellés de la barre (fr) : identiques au dump, y compris « Voix & Vidéo », « Expérience », « Jeux et applications » et « Autre ».

## Ligne Quêtes : badge et image d'accroche

Source : composant `I9` de `web.*.js` (`icon:aV.QuestsIcon,listItemRef`), hook `I1.I` (module 113597), CSS `556128.71e05633e1e4192d.css` (`_hjotnx`), badge module 805251 + CSS `384209.a1808559fefd7c5d.css` (`_4guu7-`).

| Cas | Client | Phoenix | Statut |
|---|---|---|---|
| Onboarding | si des quêtes existent (`QuestsStore.quests.size > 0`) et que le contenu masquable 632 `QUEST_HOME_ENTRYPOINT_ONBOARDING` n'est pas fermé (bit 632 de `user_content.dismissed_contents`, champ 4 → 1 du settings-proto), et qu'il n'y a pas de contenu « hero » : fond `linear-gradient(90deg,#394aff,#e901d9)` opacité 0,16 (0,2 au survol), inset haut/bas 1 px ; texte `--text-strong`, icône `--icon-strong` ; badge `type:"new"` variante `primary` | `entry_point().onboarding`, `nav::quest_onboarding_dismissed` (READY et USER_SETTINGS_PROTO_UPDATE) | ✅ ; ⚠ reflet (`shine`) et lueur (`glow`) du survol non reproduits ; la bulle d'onboarding n'est pas affichée |
| Hero | `GET /quests/get-decisions?placement=3&num_decisions_requested=1` (`QUEST_HOME_BANNER_DESKTOP`), créatif `creative_type` 2 ; contenu si `quest_home_entrypoint` a une image, un dégradé linéaire ou radial (`start` et `end`), ou un titre ET un sous-titre d'infobulle | `quests_page::ensure_hero`, `parse_home_hero` | ✅ |
| Badge hero | « nouveau » (y2b7CA) variante `expressive`, tant que l'id du créatif n'est pas vu (`AdContentSeenStore`, local) ; vu au clic | `entry_point().hero_badge`, `mark_hero_seen` | ✅ ; état « vu » persisté dans `ad-content-seen.json` au format `seenContentIds` du client (4e52b039) |
| Image hero | `questHomeHeroBackgroundImage_hjotnx` : à droite, hauteur 100 %, `object-fit: contain`, opacité 0 → 1 au survol (0,125 s) ; texte et icône forts au survol | image absolue à droite, fondu `states::opacity_fade` 125 ms (`transition:.125s`, courbe `ease`) ; infobulle riche `tooltipTitle` / `tooltipSubtitle` (`fm.A`) | ✅ wt/inv-home ; ⚠ pas de dégradé de survol par preset (`gradientPreset`, couleurs `QUEST_HOME_TAB_GRADIENT_*`) ni d'image `tooltipImage` dans l'infobulle |
| Badge | `inline-flex`, padding 0 8, rayon rond, majuscules (pas de `mana-badge-refresh` sur ce compte), `body-xs/semibold` 12/16 ; `primary` : fond et texte `--badge-primary-*` ; `expressive` : `--badge-expressive-*` | `sidebar::text_badge` | ✅ |

## En-tête de serveur (liste des salons)

Source : module 554783 (`k`), types du module 426983, traits du module 176128, étoile du module 152578.

| Type | Client | Phoenix | Statut |
|---|---|---|---|
| Ordre | staff (`INTERNAL_EMPLOYEE_ONLY`) > vérifié + partenaire > vérifié > partenaire > découverte (`COMMUNITY` + `DISCOVERABLE`) > communauté | `guild_badge::kind` | ✅ |
| Découverte | `ExperimentalWebBadgeIcon` 18 px, `--badge-inverted-background` / `--badge-inverted-text` | `discovery-badge` (inchangé) | ✅ |
| Communauté | étoile fleurie 18 px fond `WHITE` (thème sombre, `PRIMARY_500` en clair) + `HomeIcon` 11 px (`floor(0,75 × 18) − 2`) `PRIMARY_500` ; serveur boosté : dégradé `PREMIUM_TIER_2_PURPLE` → `PREMIUM_TIER_2_PINK` (rotate 45) et maison blanche | `guild_badge::element` (SVG composé, `guild/home`) | ✅ ajouté |
| Vérifié | étoile `GREEN_360` + `CheckmarkSmallIcon` 13 px blanc | idem | ✅ ajouté |
| Partenaire, staff | partenaire : étoile `BRAND_500` + poignée de main du module 816166 (viewBox 16, mise à `floor(0,75 × taille)`) ; staff : étoile `GREEN_360` + `StaffBadgeIcon` (table `w` du module 554783) | `guild/partner` (`path:M10.5906 6.39993`), `guild/staff`, échelle selon la viewBox de l'icône (`view_box_width`) | ✅ remaining4, test `community_badge_colors_and_glyph_size` (aucun serveur partenaire ou staff sur le compte pour une capture) |

## Barre d'outils des messages, en-tête des MP, compositeur

Comparés en direct (lecture du DOM, survol d'un message) sur la route où se trouvait le client (un MP) :

| Zone | Client | Phoenix | Statut |
|---|---|---|---|
| En-tête MP | `PhoneCallIcon`, `VideoIcon`, `PinIcon`, `GroupPlusIcon` (Ajouter au MP), `UserCircleStatusIcon`, `MagnifyingGlassIcon` 16, `XSmallIcon` | `phone-call`, `video-call`, `pin`, `add-to-dm`, `user-profile`, `search`, `close-small` | ✅ chemins identiques |
| Barre d'outils | réactions rapides (images 28), `ReactionIcon`, `PencilIcon`, `ArrowAngleRightUpIcon` (Transférer), `MoreHorizontalIcon`, tous 20 px | `chat/add-reaction`, `chat/edit`, `chat/forwarded`, `chat/more` 20 px | ✅ |
| Compositeur | `PlusLargeIcon` 20 ; cadeau, GIF, autocollant, Applications en Lottie | `plus` + `lottie_icons` | ✅ |

## Menus contextuels

Les 22 dumps `vanilla-menu-*` ont été comparés automatiquement : libellé fr → clé de traduction → icône utilisée dans le code. Toutes les icônes correspondent (y compris les cases à cocher `CheckmarkSmallBoldIcon`). « Détails du message » (`CircleInformationIcon`, icône `info`) est maintenant dans le menu des messages (4e52b039, `panels.rs`) ✅.

## Popouts de profil et vue d'appel

Dumps `vanilla-profile-popout`, `vanilla-dm-profile-panel`, `vanilla-account-popout`, `vanilla-status-popout`, `vanilla-call-view` : chaque SVG du client a son équivalent extrait. Écarts relevés, hors de cette branche :
- indicateur de connexion RTC (`ConnectionFineIcon` 24 dans une boîte 32, panneau vocal) : Phoenix dessine les barres à la main (`voice_panel::signal_bars`), zone vocale ;
- badge de profil « RetryIcon » 12 px dans `badgeContainer_CTWncy` : à vérifier avec la zone profils.

## Découvrir

Source : composant `ll` / `lo` de `web.*.js`, onglets `ln.iy = ["apps","servers","games","quests"]`, capture CDP `compare/discover/c-disc.json` (1536×788).

| Élément | Client | Phoenix | Statut |
|---|---|---|---|
| Titre | « Découvrir » (1KqYnl) `body-lg/semibold` | `discovery.title` | ✅ |
| Onglets | Applications `AppsIcon`, Serveurs `ServerIcon`, 24 px ; Jeux (`GameControllerIcon`) seulement avec l'expérience `2026-10-game-discovery`, Quêtes (`QuestsIcon`) sauf avec `2026-10-remove-quests-from-discovery` | `apps`, `server/server`, deux onglets | ✅ (deux onglets dans la capture) ; ⚠ les expériences ne sont pas lues |
| Libellé Applications | `bGwCoR` « Applications » | `PHjkRE` « Applications » | ✅ même texte |
| En-tête de page | icône 24 px | 24 px | ✅ |
| Recherche | `MagnifyingGlassIcon` 16 px | `search` 16 | ✅ |
| Carte : agrandir | 16 px | `maximize` 16 | ✅ |
| Carte : badge vérifié / partenaire | module 714991 : étoile fleurie 16 px (`--green-360` vérifié, `--brand-500` partenaire) + `CheckmarkSmallIcon` 10 px blanc (`redesignIconChildContainer` 10×10) | rond `#3d9e60` + `check` → `guild/flower-star` + `check-small` | ✅ corrigé ; remaining4 : partenaire seul = icône partenaire (type 0 du module 714991), vérifié + partenaire garde la coche (type 3) |

## Libellés

| Clé | Client (fr) | Hash | Statut |
|---|---|---|---|
| `dm.nitro` | « Ton Nitro » | 4gwVVn (avant : 0ModIv, qui vaut « Nitro ») | ✅ corrigé |
| `dm.nitro_tab` | « Nitro » | Ipxkog | ✅ ajouté |
| `dm.shop` | « Boutique » | 2QW9nR (pWG4ze dans la liste, même texte) | ✅ |
| `dm.quests` | « Quêtes » | EcaD4e (JALI2K, même texte) | ✅ |
| `friends.title` | « Amis » | 2C4YGg | ✅ |
| `friends.message_requests` | « Demandes de message » | 5IN9wR | ✅ |
| `dm.title` | « Messages privés » | /U8Iwa | ✅ |
