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

## Fermées par wt/remaining2 (26/11)

| ligne | correction | commit |
|---|---|---|
| settings : modale d'ajout d'autocollant | module 445002 (chunk 429143 du CDN) : aperçu sombre / clair, fichier, émoji associé, nom, description, contrôles de type et de taille, redimensionnement 320×320 ; envoi multipart identique au FormData de Chromium (`phoenix_api::rest::form`, en-tête `X-Discord-Original-MD5`), refusé par la garde | f4b36dcd |
| settings : modale d'ajout de son | module 122998 (chunk 68027) : découpe avec la forme d'onde du worker 658777, poignées, tête de lecture, limite de 5 s, volume plafonné par le multiplicateur de sonie ; décodage symphonia, clip encodé en Ogg Opus, POST refusé par la garde | f4b36dcd |
| navigation : Favoris, « Paramètres du fil » | déjà construite (modale 167846, b81fdd3d), doc corrigé | — |
| navigation : Favoris, lignes de la modale d'ajout | ligne de partage 61916 → 359378 (48 px, libellé semibold, sous-libellé, case indicateur), icône serveur + type de salon 255266 ; partagée avec Transférer comme dans le client | ac47dac9 |
| notifications : autres types de messages système | tous les cas de `stringify` (appel, renommage, icône, appli ajoutée, paliers de boost, rappel d'invitation, fil, achat, AutoMod, cybersécurité, contenu par défaut), liste d'arrivées corrigée | d2efb24e |
| window-chrome : icône du rail, événement et scène | ordre de la fabrique `N` (événement, scène, partage, vidéo, voix), données du gateway ; sourdine déjà conforme | 191b3cfb |
| composer : marges de 1 px, `> ` caché, liens | marges du CSS par boîtes transparentes ; le curseur saute le `> ` caché ; découpage `[` / `](url)` vérifié dans le décorateur du client (déjà conforme) | 9f1d903c |

## Fermées par wt/small-warn (26/11)

| ligne | correction | source |
|---|---|---|
| real_shots : défilement des paramètres du serveur | `PHX_SERVER_SETTINGS_SCROLL=<px>` (poignée `server-settings` 1) ; sous-route Accès à plusieurs extras (`discoverable+primary=<id>+cat`, `+subcat`) pour ouvrir les menus de catégories | — |
| settings : aperçu de la vérification, réponses | champs éditables (texte 150, paragraphe 1000 avec le reste, RadioGroup), premier champ focalisé, remis à zéro à la fermeture | `8192e61e17dc8592.js` `em`, `02017b99037ec61c.js` (453016, 601047, 414626) |
| settings : aperçu, dégradé de gauche | palette de l'icône 40 px (MMCQ 5, 1 pixel sur 10) + teintes `Q7`, `-45deg` de `palette[0]` à `palette[2]` | modules 654107, 515718, 998304 |
| settings : textes de la checklist | mots colorés des hooks (`checkBackHook` avertissement gras, `channelHook` / `nameHook` / `descriptionHook` critique), liens Conditions / Charte / Sécurité, liste des salons à mots interdits | `916d0bef82cfb632.js` `u_`, CSS `5cbb82e2bdb90a4d.css`, ASTs intl fr/en |
| settings : mentions dans les règles | pastilles de salon et de rôle dans l'éditeur (jetons) et dans l'aperçu (Markdown des messages) | `241499` (`RULES_INPUT`), `GUILD_VERIFICATION_FORM_RULES` |
| settings / discovery : masques squircle | vrai contour `svg-mask-squircle` (forme SVG pour les fonds, image cuite avec le masque pour les icônes) sur la carte statique (48/40) et les cartes Découvrir (56/48) ; le rail garde le rayon 12 (écart ≤ 0,17 px à 40 px) | mask defs du bundle web |
| settings : autocollant Lottie | aperçu animé à 160 via `media::lottie` | module 445002 `ef` |
| settings : son, tête de lecture | avance pendant la lecture et revient au début à la fin | module 122998 `el` (intervalle de 16 ms) |
| settings : son, encodage | canaux conservés (stéréo), Ogg comme le client (pre-skip 0, tags vides, un paquet par page) | module 903259 |

Reste : l'autocomplétion de mentions dans l'éditeur de règles (seulement avec `PREVIEW_ENABLED`).

## Fermées par wt/discover-fix (26/11)

| ligne | correction | source |
|---|---|---|
| discovery : « Pôles étudiants » sous le champ de recherche | barre d'onglets à débordement du client (module 876622 + `3666.Wv`) : largeurs mesurées en body-md/medium, écart 20, « Plus » (`UKOtz+`, body-sm/semibold + chevron xs) qui ouvre le menu `global-discovery-tabs-overflow-menu` (124589, coche sur l'onglet choisi) ; recherche à trois états (770472) : réduite en loupe quand la place libre passe sous 20 px (champ vide) ou flottante (champ rempli), retour à 236 / 284 px ; onglets coupés au conteneur | `7a4fc2e4363f5e29.js`, `3eba9a97d220046a.js`, `46a690ee2e821785.js`, CSS `102247` (`uP9HND`, `B1ffsk`, `Y0eJLL`) |
| settings : fenêtre de détails de l'activité | module 247920 (chunk 963401), CSS `gm2kSr` ; lien affiché seulement avec des mesures | `6cd06666e3f4353c.js`, `b80318ca3118bf73.css` |
| settings : fondu de l'aperçu | rampes de 180 px en haut et en bas, alpha de bord sur 48 px | `8192e61e17dc8592.js` `W` |
| settings : son, tête de lecture | transition `left` 500 ms `ease` hors lecture et hors glisser | CSS `410a6c65e50f79ef.css` `playhead_LGRor2` |

| home : Boutique figée | la grille construisait toutes les cartes à chaque rendu et chaque image chargée relançait un rendu complet (real_shots bloqué 240 s) ; seules les rangées visibles sont construites, espaceurs ailleurs | `shop_page::visible_rows` |
| home : Boutique, détails | espace après « N objets » des lots, titre de fenêtre « Boutique » | — |
| discovery : thème clair | dégradé clair `radial-gradient(122.23% 351.84% at -4.04% 0, #eff8ff, #d7dcff 30.28%, #fdfdff)` à .8, description `text-strong` (`462887.M`) | CSS `-6JdM0` |

Reste sur Découvrir : le masque `floatingSearchTabsMask_Y0eJLL` sous la recherche flottante (pas de `mask-image` dans GPUI).

## Fermées par wt/remaining3 (26/11)

| ligne | correction | source |
|---|---|---|
| settings : guide du serveur, étape 4 | sélecteurs d'émoji, upsell boost et glisser-déposer étaient déjà faits (b5833d8a) ; ajout des vignettes des lignes (icône 48 / 32, émoji dans son disque, icône du salon par défaut), description des ressources, erreurs `CbTEKP` / `kTdL8X` avec la vraie règle de permission | module 754596 (`916d0bef82cfb632.js`), CSS `fc6c9def8b40b57f.css`, `374084.Rc` / `Yt` |
| popouts : Shop This Look, fiche produit | modale 960×650 (`size-xl`), rail gauche 330 (aperçu, titre, résumé, prix membre barré pour Nitro, Acheter / Offrir), fond de la collection à droite, fermeture ; achat = même avis « indisponible » que la Boutique | 722258 → 31541 → 844821 (`12bdcdfc1503e9e6.js`), CSS `fEFP8r` `xC7tSN` `w2vPtx` `XcVISC` `80OD1-` |
| text : barres des modales non mesurées | réactions (thin + fade / auto + fade), transfert (auto), recherche (thin, déjà), soundboard (thin), groupe privé (auto + fade), création de fil (thin + fade) ; nouveau `Bar::AutoFade` | fabriques 516521 / 475825 dans les chunks en cache |
| text : modale des réactions | refaite d'après le client : taille md (480), colonne d'émojis 98 px (bordure, pr 16, lignes 32 r 8, mb 8, tooltip `:nom:`), lignes de 52 px (avatar 32, nom body-md/medium, pseudo 12/14 muted), retrait par réacteur au survol (DELETE refusé par la garde), plus de bouton « Supprimer les réactions » en pied (absent du client) | `853bbaf8e88952d2.js`, CSS `3c89da594347a9b2.css` |
| home : bouton « Nouveau groupe privé » des Amis | ouvrait rien ; ouvre le sélecteur de groupe privé | — |

Reste :
- fiche produit : aperçu de droite (chat pour les décorations, carte de profil sinon), badge, « Aller à la collection », liste de souhaits, partage, variantes, lots ;
- réactions : la hauteur suit le plafond de 720 de `modals::card` (client : `min(800px, 100%)`), réactions Super (section burst) non faites ;
- coller texte + image : la lecture est dans `gpui-pre-linux` 0.3.12 (`src/linux/wayland/clipboard.rs`, `Clipboard::read` : `read_text().or_else(read_image)`), qui vient de crates.io et n'est pas vendored. Le correctif tient en quelques lignes (renvoyer les deux entrées, le compositeur prend l'image en premier), mais il faut copier tout le crate dans `vendor/` et l'ajouter à `[patch.crates-io]`, ce qui recompile toute la pile GPUI pour tous les agents : pas fait, risque trop élevé pour aujourd'hui.

## Faisable (pas encore fait)

| fichier | ligne | ce qu'il faut |
|---|---|---|
| composer | coller texte + image : GPUI lit le texte d'abord | patch du presse-papiers Wayland : `gpui-pre-linux` 0.3.12 n'est pas vendored (crates.io), voir plus bas |
| report | lignes `share_with_parents` et `settings_upsells` | tracées (a07df836) ; il manque le sous-type du signalement et l'état Centre familial |

## Direct (client officiel, utilisateur présent)

| fichier | ligne | pourquoi |
|---|---|---|
| window-chrome | icône du rail : état activité (AppsIcon) | le store des activités intégrées ne se remplit qu'avec une activité lancée dans un vocal ; rien en GET |
| notifications | invitations d'activité, cartes parrainage / checkpoint | il faut recevoir ces messages (activité lancée, parrainage) pour voir leur forme |
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
