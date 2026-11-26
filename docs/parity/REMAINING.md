# Lignes ⚠ et ❌ restantes : classement (balayage du 26/11)

Source : `grep -n '❌\|⚠' docs/parity/*.md` sur origin/main 64f81bb5 (125 lignes). Les légendes, les avertissements CDP (écritures automatiques du client, acks) et les décomptes ne sont pas des écarts : ils ne sont pas classés ici.

Catégories :
- **Faisable** : CSS ou JS en cache, dumps, chunks du CDN en GET, sans CDP ni écriture.
- **Direct** : demande une mesure ou un essai dans le client officiel (CDP), donc l'utilisateur présent.
- **2e compte** : demande un état que le compte seul ne peut pas produire (quelqu'un d'autre en vocal, en stream, non-admin, autre plan).
- **Décision** : hors périmètre ou demande une écriture sur le compte ; c'est à l'utilisateur de trancher.

## Fermées par ce balayage (wt/parity-sweep)

| ligne | correction | source |
|---|---|---|
| message-content : fond de la barre des contrôles audio | `--opacity-black-40` (et non 28) | CSS `375724.c3be17a6e5ee3d3b.css` ; `.background-opacity-*` n'est posée que par le widget de l'overlay de jeu (module 672396 `Kv`, chunk `6e57441e56987c3e.js`) |
| real-compare : nom des groupes MP avec les surnoms d'amis | `relationship.nickname` puis le nom d'affichage, joints par « , » ; nom explicite distingué (`group_named`) | `computeChannelName` (module 47167) ; test `unnamed_group_names_use_friend_nicknames` |
| real-compare « Still open » : surnom en MP, pastille de statut des groupes, point « nouveaux sons », glissement du hover roll (×2) | déjà corrigés plus bas dans le même fichier, lignes mises à jour | tableau wt/live-diff-a2 |
| settings : titre de la carte Nitro | « NITRO SQUAD » quand `isPremiumWithPremiumGroup()` | chunk `3ba45b1b6984e0cd.js` (`premiumGroupHeader_20JRSQ`, `aFBQ3d`), dump `vanilla-settings-05b` #3013 |
| call-view : tuile de stream sans image | illustration `emptyPreviewImage_hELJz2` (80 % × 60 %, sombre / clair) à la place de l'icône écran | CSS `390430.0c9088eae4beff43.css` |
| fonts : `--font-code` sans gg mono | pile complète du client résolue par fontconfig | `current-client/css` ; test `code_stack_follows_the_client_order` |
| settings-wiring : « Synchroniser le thème » | grisé tant que « Thème de l'appareil » est actif | `shouldSync("appearance")` |
| chat : sous-titre « Tu quittes Discord » (lien d'application) | partie `$b` en semibold | `aCYv1z`, `strong{font-weight:var(--font-weight-semibold)}` |

## Fermées par wt/remaining1 (26/11)

| ligne | correction | commit |
|---|---|---|
| call-view : aperçus d'appel, pièce jointe | `V` du module 448368 (en cache, `73d5fba3085633c7.js`) : textes bloqué / ignoré / épinglé / autocollant / commande / vocal / composants v2 / « Clique pour voir les pièces jointes », icônes de fin et de transfert | dcc9aa26 |
| call-view : poignée de redimensionnement de l'appel MP | tracée : module 587837 → `resizeHandle_poo5Jt` (module 50031), géométrie déjà juste | dcc9aa26 |
| settings : logo Nitro hors Nitro Squad | wordmark du module 224016 extrait au runtime (`settings/nitro-wordmark`, `fill-rule` du `<g>` conservé), 160×40 mb 4 | 270492c4 |
| settings : popout de couleur du tag | `NW`/`VN` (saturation, teinte, hex, colonne Secondaire si `C9` ≥ 2) ; ⚠ PATCH à la fermeture, hex en lecture seule, pipette inerte | 5083ea27, 0873b032 |
| settings : A2F, clés de sécurité | liste en GET, « Ajouter » = POST refusé par la garde + toast `xSCvBf` ; ⚠ menu « … » d'une clé non construit | b3b2ad43 |
| settings : recherche dans les pages personnalisées | haut de Voix & Vidéo filtré ; ⚠ termes cachés des `useSearchTerms` non mappés | 61804d8f, 3781b244 |
| popouts : puce de tag de la Mod View | déjà faite (a8bb91f1), doc corrigé | 9a2d37de |
| report : `share_with_parents` / `settings_upsells` | tracés (conditions, textes) mais non construits : ado + Centre familial, et sous-type du signalement absent du nœud | a07df836 (doc) |

## Faisable (pas encore fait)

| fichier | ligne | ce qu'il faut |
|---|---|---|
| settings | Autocollants / Soundboard : modales d'ajout | modales du client (nom, émoji, volume), sans exécuter les POST |
| settings | guide du serveur (étape 4) : vignettes, sélecteurs d'émoji, glisser-déposer | code en cache |
| navigation | Favoris : « Paramètres du fil » (stub), lignes de la modale d'ajout | modules 167846 et 677993 |
| composer | `margin-inline` 1 px de `before_s/u/em` ; `> ` caché qui garde deux arrêts du curseur ; séparation `[`/`](url)` | CSS en cache ; comportement de l'éditeur |
| composer | coller texte + image : GPUI lit le texte d'abord | patch GPUI (presse-papiers Wayland) |
| popouts | Shop This Look : fiche produit | modale `722258.B` |
| report | lignes `share_with_parents` et `settings_upsells` | tracées (a07df836) ; il manque le sous-type du signalement et l'état Centre familial |
| notifications | autres types de messages système, invitations d'activité, cartes parrainage / checkpoint | `stringify` en cache |
| window-chrome | icône de la barre : sourdine / suppression de serveur, états événement / stage / activité | `hasAnyUnread` et priorités en cache |
| text | modales non mesurées (réactions, transfert, recherche, soundboard, popout de groupe, création de fil) | dumps à faire ou CSS en cache |

## Direct (client officiel, utilisateur présent)

| fichier | ligne | pourquoi |
|---|---|---|
| settings | select « Quelqu'un réagit » : `max-content` + la moitié de l'espace libre (316 px mesurés) | la largeur intrinsèque du select n'est connue que mesurée ; Phoenix met 280 |
| real-compare | barre « Nouveaux non lus » en haut (#bienvenue) | rejouer avec `RUST_LOG=phoenix::unread_bar=debug` sur un serveur avec des non-lus |
| real-compare | coupe du placeholder du compositeur, position verticale du compositeur | zone de wt/placeholder-cut |
| text | soulignement du nom sans émoji | survol en direct |
| message-content | carte longue : cas limite Lofi Girl (1,6 px) | mesure en direct |
| composer | fond du lanceur d'apps dans les autres thèmes ; bannière des apps promues | ⚠ ne plus ouvrir le lanceur d'apps (il écrit settings-proto) |
| chat | modale de sondage : sélecteur d'émoji | le chunk est lazy et l'ouvrir demande d'aller sur Aperture (POST migrate-command-scope) |
| call-view | effet d'émoji sur la tuile, Go Live (qualité) | dans le vocal de test |
| voice-messages | cadence des niveaux, débit du client | enregistrement dans le client |
| window-chrome | clic gauche sur l'icône de la barre | une seconde instance ajouterait une fenêtre et une icône à l'écran |
| settings | Membres (page member-safety) | la liste vient de `POST members-search`, interdit aux agents |
| settings | Connexions : lignes de métadonnées | le compte n'a aucune connexion |
| forum (toutes les lignes ⚠) | | zone de wt/load-bugs |

## Second compte

| fichier | ligne | pourquoi |
|---|---|---|
| mentions-voice-row | carte « Dans un salon vocal » du popout | il faut un membre en vocal hors de l'appel de l'utilisateur |
| settings | variantes non-admin de la barre latérale des paramètres de serveur | le compte est admin |
| settings | Nitro sans abonnement, A2F désactivée, applications autorisées (variante jeu) | le compte est Nitro Squad, A2F active |
| popouts | profil : amis de jeu (`getGameFriendsForUser`) | aucun ami de jeu sur le compte |
| notifications | demandes en attente : amis de jeu | idem |

## Décision de l'utilisateur

| fichier | ligne | question |
|---|---|---|
| navigation | Favoris : état replié | synchroniser veut dire `PATCH settings-proto` (écriture) ; aujourd'hui local |
| popouts | Boîte de réception : repli des cartes | même chose (`collapsedInInbox` dans le proto) |
| window-chrome, settings-wiring | fermer = réduire au lieu de cacher | GPUI sous Wayland ne sait pas cacher une fenêtre sans la détruire |
| settings-wiring | aperçu Markdown dans la zone d'édition des messages | l'édition est un `InputState` d'une ligne : la remplacer par l'éditeur du compositeur |
| chat | menu des messages en échec réduit à 3 entrées | volontaire : les autres entrées appelleraient l'API avec un id local |
| composer | image d'en-tête envoyée en pièce jointe et non en `MEDIA_GALLERY` | composants v2 à l'envoi (zone forums) |
| notifications | heures restreintes du Centre familial, classements, aperçu complet du serveur, overlay de jeu, activités | fonctions absentes de Phoenix |
| sounds, shortcuts | sons d'activité / de stage ; modale Lo-fi Wumpus (animation Rive) | pas d'activités ; GPUI ne dessine pas Rive |
| inventory | lecteur vidéo intégré (menus « … » et vitesse), Nouveautés, Centre familial complet | fonctions absentes de Phoenix |
| settings | Avantages de boost : art de la barre de progression ; dégradés radiaux du profil de serveur | GPUI n'a pas de dégradé radial ; image du client |
