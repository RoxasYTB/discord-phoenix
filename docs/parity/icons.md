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
| Quêtes | `QuestsIcon` 20 px, « Quêtes » (JALI2K) ; badge « nouveau » (y2b7CA) et image d'accroche si une quête est mise en avant | `quests` | ✅ icône ; ❌ badge et image d'accroche pas encore reproduits |
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
| Carte « Explorer les serveurs » | `CompassIcon` 24 px ; flèche propre à la page (`EF`, viewBox 0 0 20 20, `m5.41667 4.2625…`) | `ChevronSmallRightIcon` → `friends/discovery-arrow` | ✅ corrigé ; ⚠ le client la rétrécit en 9×20 par le flex, Phoenix garde 20×20 |
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
| Carte : souhait | `HeartOutlineIcon` 20 px | absent des cartes | ❌ à ajouter |
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

## Découvrir

Non vérifié : aucun dump DOM de `/discovery`, et le client était sur un salon de serveur pendant ce passage. Le JS ne permet pas de retrouver directement les icônes de la barre latérale.

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
