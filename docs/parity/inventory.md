# Inventaire 2026 : menus, sous-menus, modales et pages

Référence : le bundle du client 2026 (chunks en cache dans `~/discord-phoenix-ref/current-client/js`, chunks manquants récupérés en GET dans `js-extra/`), les `navId` des menus (95 au total), les traductions FR/EN du bundle, les dumps `vanilla-menu-*` et des relevés CDP en direct du 25/11 (`~/discord-phoenix-ref/inventory/menu.mjs` : ouvrir, lire les entrées, Échap). Pas de paramètres de serveur, de salon ni de catégorie ici (wt/srvset1, wt/srvset2, wt/chanset).

Légende : ✅ présent et conforme, ⚠ partiel (entrée inerte, rendu non mesuré, donnée absente), ❌ absent.

## Synthèse

| zone | éléments | ✅ | ⚠ | ❌ | fermé par wt/inventory |
|---|---|---|---|---|---|
| Menus contextuels (clic droit, sous-menus compris) | 22 | 15 | 6 | 1 | image (Copier / Enregistrer), groupe privé (Invitations, Modifier le groupe), dossier (Paramètres du dossier) ; zone de texte passée de ❌ à ⚠ |
| Menus « … », popouts-menus | 18 | 6 | 4 | 8 | ligne d'ami, « … » du profil, « … » de la visionneuse ; « + » complété |
| Modales | 46 | 29 | 10 | 7 | Modifier le groupe, Paramètres du dossier, Retirer l'ami, Bloquer ; Invitations du groupe en ⚠ |
| Pages | 32 | 26 | 1 | 5 | — |
| **Total** | **118** | **76** | **21** | **21** | 12 éléments fermés, 2 passés de ❌ à ⚠ |

Une ligne compte comme ⚠ dès qu'une partie reste inerte ou non mesurée, même si l'essentiel est conforme. Les compteurs « ✅ » incluent les éléments fermés par ce lot.

## 1. Menus contextuels (clic droit)

| menu (`navId`) | où | entrées du client (FR, ordre) | Phoenix | statut |
|---|---|---|---|---|
| `message` | message | 4 réactions rapides / Ajouter une réaction ›, Voir les réactions / Modifier, Répondre, Transférer, Créer un fil / Copier le texte, Épingler, Enregistrer, Me le rappeler ›, Applications ›, Marquer comme non lu, Copier le lien, Lire le message / Supprimer les réactions ›, Supprimer toutes, Supprimer, Signaler / Copier l'identifiant | `ui/chat/panels.rs` | ✅ (sous-menu Applications vide : pas de commandes de message) |
| `message` + image | image d'un message | menu du message + Copier l'image, Enregistrer l'image / Copier le lien de l'image, Ouvrir le lien | `panels.rs`, `extra_menus::copy_image` / `save_image` | ✅ (copier et enregistrer branchés dans ce lot) |
| `message` + lien | lien d'un message | menu du message + Copier le lien, Ouvrir le lien | `panels.rs` | ✅ |
| `message` + émoji | émoji perso | menu du message | `panels.rs` | ✅ |
| `user-context` | membre, auteur | Profil, Mentionner, Envoyer un message, Appeler, Ajouter une note, Pseudo d'ami / Changer le pseudo, Applications ›, Inviter sur le serveur ›, Retirer l'ami, Ignorer, Bloquer / Rôles › / Vue modérateur, Exclure temporairement, Expulser, Bannir / Copier l'identifiant | `ui/user_menu.rs` | ✅ (Retirer l'ami et Bloquer demandent maintenant confirmation) |
| `user-context` (vocal) | membre en vocal | … Regarder le stream, Déplacer vers ›, Mettre en sourdine serveur, Déconnecter | `actions/user.rs voice_items` | ✅ |
| `channel-context` | salon texte | Marquer comme lu / Favoris › / Inviter, Épingler en haut, Copier le lien / Rendre muet ›, Notifications › / Déplacer vers › / Modifier, Cloner, Créer, Supprimer / Copier l'identifiant | `ui/nav_menus.rs` | ✅ |
| `channel-context` (vocal) | salon vocal | … Ouvrir le chat, Masquer les noms / … | `nav_menus.rs` | ✅ |
| `channel-context` (catégorie) | catégorie | Marquer comme lu / Réduire, Tout réduire / Rendre muet ›, Notifications › / Déplacer vers › / Modifier, Supprimer / Copier l'identifiant | `nav_menus.rs` | ⚠ « Déplacer vers › » : sous-menu inerte (stub) |
| `guild-context` | serveur du rail | Marquer comme lu / Inviter / Rendre muet ›, Notifications ›, Masquer les muets, Tout afficher / Paramètres ›, Confidentialité, Profil par serveur / Créer salon, catégorie, événement / Déplacer vers… › (Dossier › Aucun dossier + dossiers ; Premier / Dernier dans la liste ou le dossier) / Sécurité, Quitter / Copier l'identifiant | `nav_menus.rs`, `gdm::move_tree` | ✅ lot 2 (« Déplacer vers… » était inerte) |
| `guild-context` (dossier) | dossier du rail | Marquer le dossier comme lu (grisé sans non-lu) / Déplacer vers… › (Premier / Dernier dans la liste) / Paramètres du dossier, Fermer tous les dossiers (si un dossier est ouvert) | `nav_menus.rs` | ✅ lots 1 et 2 |
| `thread-context` | fil (barre latérale, carte de forum) | Marquer comme lu / Favoris › / Suivre, Fermer, Verrouiller, Modifier, Tags ›, Copier le lien / Rendre muet, Notifications / Épingler, Supprimer / Copier l'identifiant | `ui/thread_menu.rs` | ✅ |
| DM (`channel-context` privé) | MP | Marquer comme lu / Épingler, Favoris › / Profil, Appeler, Note, Pseudo d'ami, Fermer le MP / Applications ›, Inviter ›, Retirer l'ami, Ignorer, Bloquer / Rendre muet @x › / Copier les identifiants | `nav_menus.rs` | ✅ (Retirer l'ami seulement pour un ami, avec confirmation) |
| `gdm-context` | groupe privé | Marquer comme lu / Épingler, Favoris › / Invitations (propriétaire), Modifier le groupe / Rendre muet › / Quitter le groupe / Copier l'identifiant | `nav_menus.rs` | ✅ (Invitations et Modifier le groupe étaient des stubs, branchés dans ce lot) |
| `textarea-context` | compositeur | Suggestions › (Commandes slash ☐), Bouton Envoyer un message ☐ / Correcteur ☐, Langues › / Couper, Copier (si sélection), Coller `Ctrl+V` | `ui/extra_menus.rs` (`Target::Textarea`) | ⚠ ce lot : menu au lieu de celui de gpui-component ; « Langues › » absent et « Correcteur » / « Commandes slash » sans effet (pas de correcteur orthographique dans Phoenix) |
| `image-context` | image hors message | Copier l'image, Enregistrer l'image, Copier le lien | — | ❌ (avatars, bannières : pas de clic droit) |
| `audio-device-context` | bouton micro / casque | périphériques, mode d'entrée, volume, Paramètres vocaux | `ui/voice_panel.rs` | ✅ |
| `video-device-context` | bouton caméra | périphériques, Paramètres vidéo | `voice_panel.rs` | ✅ |
| `user-profile-widget-context-menu` | widget du profil | Supprimer le widget, Signaler | `ui/profile_board.rs` | ✅ (Signaler seulement pour un autre utilisateur) |
| `favorites-channel-list-context` | serveur Favoris | Retirer des favoris, … | `ui/favorites.rs` (retrait par la modale Favoris) | ⚠ pas de clic droit dédié |
| `manage-streams` / `stream-options` | stream en cours | Son de l'appareil, Résolution, Fréquence, Couper le son, Aperçu, Notifier | `ui/call_view.rs` | ⚠ non mesuré (pas de stream sans vocal) |
| `component-button` | bouton d'application | Copier | — | ✅ (sans objet : pas de clic droit sur les boutons dans le client desktop par défaut) |

## 2. Menus « … », popouts-menus et barres

| menu (`navId`) | où | entrées du client | Phoenix | statut |
|---|---|---|---|---|
| `friend-row` | « … » d'une ligne d'ami | Démarrer un appel vidéo, Démarrer un appel vocal, Retirer l'ami (danger, confirmation) | `extra_menus::friend_row_items` | ✅ ce lot (ouvrait le menu utilisateur complet) |
| `user-profile-overflow-menu` | « … » du popout / profil | Voir le profil complet, Inviter sur le serveur › / Ignorer, Bloquer, Signaler le profil d'utilisateur / Copier les infos utilisateur › (identifiant, lien du profil, lien de l'avatar, lien de la bannière) | `extra_menus::profile_items` | ✅ ce lot ; ⚠ « Partager le profil », « Achète ce look » et « Partage Nitro » dépendent d'expériences et ne sont pas reproduits |
| `image-menu` | « … » de la visionneuse | Copier l'image, Copier le lien du média / Voir les détails › (Nom de fichier, Taille, Description) / Copier l'ID de la pièce jointe | `extra_menus::viewer_items` | ✅ ce lot (le bouton était inerte) |
| `channel-attach` | bouton « + » | Uploader un fichier, Uploader ton message en tant que fichier (brouillon non vide), Créer un fil, Créer le sondage, Utiliser des applications, Programmer un message › (Aujourd'hui/Demain à 9 h 00, à 13 h 00, Lundi à 9 h 00 / Personnalisé...), Enregistrer un message vocal | `ui/chat/composer.rs attach_menu`, `ui/chat/scheduled.rs` | ✅ lots 1 et 2 (relevé CDP du 25/11 : même ordre) |
| `guild-header-popout` | en-tête du serveur | Boost / Inviter, Paramètres, Statistiques, Créer salon, catégorie, événement, Fils actifs, Annuaire d'applications / Tout afficher, Notifications, Confidentialité / Profil par serveur, Masquer les muets / Sécurité, Signaler un raid / Quitter / Copier l'identifiant | `nav_menus::guild_dropdown` | ⚠ Boost, Statistiques, Fils actifs, Annuaire d'applications et Signaler un raid sont des stubs |
| `set-status-submenu` | popout du compte | En ligne, Inactif, Ne pas déranger (durées), Invisible | `ui/status_picker.rs` | ✅ |
| `switch-accounts-submenu` | popout du compte | comptes connectés, Gérer les comptes | `status_picker.rs` | ❌ stub (Phoenix ne gère qu'un compte) |
| `edit-profile-popout` | « Modifier les profils » | Profil principal, Profil par serveur | `settings::profile_editor` | ✅ |
| `clean-up-inactive-gdms` | « … » de « Messages privés » | Nettoyage des Groupes privés… | `ui/sidebar.rs dm-more` | ❌ stub (la modale de nettoyage n'existe pas) |
| `sort-and-view` | forum | Trier par activité / date, Liste / Grille, Tags (tous / un), Réinitialiser | `ui/forum.rs` | ✅ |
| `mentions-filter`, `recents-notifications` | boîte de réception | filtres Mentions, notifications de fils | `ui/inbox.rs` | ✅ |
| `send-announcement-options` | salon d'annonces | Créer un fil, Envoyer et publier | — | ❌ |
| `video-player-overflow` | vidéo | Transcription, Sous-titres, Télécharger | — | ❌ |
| `media-player-playback-speed` | vidéo / audio | vitesses 0,25×–2× | — | ❌ |
| `plaintext-preview-overflow-menu` | pièce jointe texte | Télécharger, Retour à la ligne, Rendu Markdown | — | ❌ |
| `settings-footer-more-menu` | bas de la barre des paramètres | Nouveautés, Remerciements, Assistance | — | ❌ |
| `wishlist-overflow-menu` | liste de souhaits du profil | Parcourir la boutique, Ajouter Nitro, Confidentialité | `ui/profile_wishlist.rs` (bouton Parcourir seulement) | ❌ |
| `exit-options`, `more-settings-context` | appel / Go Live | Inviter des amis, Région, Vue grille, Inviter au stream | `call_view.rs` (menu de tuile seulement) | ⚠ |

Menus réservés au personnel, aux propriétaires d'applications ou à des fonctions absentes de Phoenix (`devtools-*`, `playground-*`, `staff-help-popout`, `checkpoint-qa-submenu`, `guild-product-context`, `guild-entry-context`, `member-safety-*`, `members-table-*`, `guild-settings-role-context`, `overlay-*`, `clips-*`, `quest-bar-bounty`, `quests-entry`, `app-channel-*`, `activity-shelf-item-context`, `registered-game-overflow-menu`, `subscription-context`, `unapplied-boost-actions`, `authorized-app-action-menu`) : hors de cette ligne, ou couverts par les pages correspondantes.

## 3. Modales accessibles à un utilisateur normal

| modale | déclencheur | Phoenix | statut |
|---|---|---|---|
| Supprimer le message | menu message | `request_delete` (Maj = sans confirmation) | ✅ |
| Transférer le message | menu message | `modals/forward.rs` | ✅ |
| Réactions | Voir les réactions | `modals/reactions.rs` | ✅ |
| Signaler (message, utilisateur, serveur) | menus | `modals/report.rs` | ✅ |
| Créer un fil | menu message, « + » | `thread_create.rs` | ✅ |
| Rappel personnalisé | Me le rappeler › Personnalisé | `open_custom_reminder` (formulaire générique) | ⚠ le client a un sélecteur de date `message-reminder-create` |
| Mettre à jour le message programmé | boîte de réception › Programmés | `scheduled::Picker { existing }` | ✅ lot 2 (`PATCH …/{id}` avec `scheduled_timestamp` seul, toast « Message programmé mis à jour ») ; Envoyer maintenant (`POST …/{id}/send`) et Annuler (`DELETE`) branchés, « Modifier le message » non fait |
| Programmer un message | « + » › Personnalisé..., bandeau du brouillon | `ui/chat/scheduled.rs` (`Modal::Schedule`) | ⚠ lot 2 : bandeau « Programmation d'un message pour le {date} à {heure} », modale Date / Heure (480, « Définir un planning », erreurs « trop proche » / « trop éloignée »), envoi `POST /users/@me/scheduled-messages` ; les sélecteurs sont des listes (le client a un calendrier et un champ d'heure), pas de pièces jointes programmées |
| Créer un sondage | « + » | `modals/poll_create.rs` | ✅ (bouton « Programmer » du sondage inerte, même fonction) |
| Uploader (description, spoiler) | pièce jointe | `upload.modify` | ✅ |
| Lien externe | lien | `modals/leave_link.rs` | ✅ |
| Sujet du salon | en-tête | `Overlay::Topic` | ✅ |
| Changer le pseudo | menu membre | `open_nickname` (formulaire générique) | ⚠ pas de lien « Réinitialiser le pseudo » ni de compteur |
| Pseudo d'ami | menus | `open_friend_nickname` | ⚠ même formulaire générique |
| Ajouter une note | menus | ouvre le profil (note éditable) | ✅ |
| Retirer l'ami | ligne d'ami, menus, intro MP | `confirm_remove_friend` | ✅ ce lot (« Retirer '{name}' », bouton danger) |
| Bloquer | menus, profil, intro MP | `confirm_block` | ✅ ce lot (« Bloquer {name} ? ») |
| Ignorer | menus | bascule directe | ⚠ le client ouvre « Ignorer {username} ? » avec l'explication |
| Profil complet | profil | `open_profile_modal` | ✅ |
| Vue modérateur, exclusion, expulsion, bannissement | menu membre | `modals/member.rs` | ✅ |
| Inviter sur le serveur | menus | `ui/invite.rs` | ✅ |
| Rejoindre un serveur | « + » du rail | `ui/invite.rs` | ⚠ l'étape « Créer mon serveur » (modèles) manque |
| Quitter le serveur | menu serveur | `Overlay::LeaveGuild` | ✅ |
| Notifications du serveur | menu serveur | `modals/guild_notify.rs` | ✅ |
| Confidentialité du serveur | menu serveur | `modals/privacy.rs` | ✅ |
| Actions de sécurité | menu serveur | `privacy::Security` | ✅ |
| Profil par serveur | menus | `open_server_profile` | ✅ |
| Créer un salon / une catégorie | menus | `modals/create_channel.rs` | ✅ |
| Créer un événement | menus | formulaire générique | ⚠ le client a un assistant en 3 étapes |
| Événements | en-tête | `ui/events.rs` | ✅ |
| Créer un groupe privé | « + » des MP | `open_group_dm_picker` | ✅ |
| Modifier le groupe | menu groupe, intro du groupe | `modals/gdm.rs render_edit_group` | ✅ ce lot (icône 120 px + crayon, « Supprimer l'icône », nom ≤ 100, Annuler / Enregistrer pleine largeur) |
| Invitations du groupe | menu groupe (propriétaire) | `gdm.rs render_invites` | ⚠ ce lot : structure et données du client ; le CSS du tableau (`qF1Ceg`) n'est pas dans le cache |
| Paramètres du dossier | menu dossier | `gdm.rs render_folder` | ✅ ce lot (nom ≤ 32, « Dossier du serveur », 2 grandes pastilles + 20 couleurs de rôle) ; ⚠ la pastille personnalisée n'ouvre pas encore la pipette |
| Nettoyage des Groupes privés | « … » des MP | — | ❌ |
| Statut personnalisé | popout du compte | bulle en ligne | ⚠ pas de modale (émoji, durée « Effacer après ») |
| Changer de compte | popout du compte | — | ❌ |
| Boost du serveur | menu serveur | — | ❌ |
| Fils actifs | menu serveur | — | ❌ |
| Raccourcis clavier | Ctrl+/ | `shortcuts::render_help` | ✅ |
| Sélecteur rapide | Ctrl+K | `ui/switcher.rs` | ✅ |
| Go Live | panneau vocal | `modals/go_live.rs` | ✅ |
| Déconnexion | paramètres | formulaire | ✅ |
| Accélération matérielle | paramètres | formulaire | ✅ |
| Modifier e-mail, téléphone, mot de passe, nom d'utilisateur, 2FA | Compte | — | ❌ boutons inertes (écritures sur le compte, à coder sans jamais les exécuter) |
| Nouveautés (changelog) | menu « … » des paramètres | — | ❌ |
| Infos d'interaction | menu message (app) | toast | ⚠ le client ouvre une modale |

## 4. Pages

| page | route / entrée | Phoenix | statut |
|---|---|---|---|
| Amis (En ligne, Tous, En attente, Ajouter) | `/channels/@me` | `ui/home/friends.rs` | ✅ |
| En ligne maintenant | colonne des amis | `friends.rs active_now` | ✅ |
| Demandes de message | MP | `friends.rs message_requests` | ✅ |
| Nitro | `/store` | `ui/home/nitro.rs` | ✅ |
| Boutique | `/shop` | `ui/shop_page.rs` | ✅ (achats inertes) |
| Quêtes | `/quest-home` (rangée « Quêtes ») | — | ❌ stub |
| Découvrir | `/discovery` (Serveurs, Applications, Quêtes) | — | ❌ stub (bouton boussole du rail) |
| Annuaire d'applications | menu serveur, « Utiliser des applications » | `ui/chat/app_launcher.rs` (lanceur) | ❌ page complète |
| Centre familial | Compte › Centre familial | bouton « Configurer » | ❌ |
| Boîte de réception (Pour toi, Non lus, Mentions) | en-tête | `ui/inbox.rs` | ✅ |
| Messages enregistrés / rappels | boîte de réception | `inbox.rs` | ✅ |
| Fils du salon, Épingles, Recherche | en-tête | `chat/panels.rs`, `ui/search.rs` | ✅ |
| Salons et rôles, Parcourir les salons, Guide du serveur | serveur communautaire | `ui/community.rs`, `ui/guide.rs` | ✅ |
| Paramètres : Compte | | `settings/account.rs` | ✅ (sous-section Centre familial ❌) |
| Paramètres : Profils (éditeur plein écran) | | `settings/profile_editor.rs` | ✅ |
| Paramètres : Données et confidentialité | | `pages.rs` | ✅ |
| Paramètres : Permissions de messagerie | | `pages.rs` | ✅ |
| Paramètres : Notifications | | `pages.rs` | ✅ |
| Paramètres : Nitro | | `pages.rs` | ✅ |
| Paramètres : Boost de serveur | | `settings/boost.rs` | ✅ |
| Paramètres : Abonnements | | `settings/subscriptions.rs` | ✅ |
| Paramètres : Inventaire des cadeaux | | `settings/gift.rs` | ✅ |
| Paramètres : Facturation | | `settings/billing.rs` | ⚠ moyens de paiement et transactions non lus |
| Paramètres : Voix et vidéo | | `settings/voice.rs` | ✅ |
| Paramètres : Apparence | | `pages.rs` | ✅ |
| Paramètres : Accessibilité | | `pages.rs` | ✅ |
| Paramètres : Système | | `pages.rs` | ✅ |
| Paramètres : Langue et heure | | `pages.rs` | ✅ |
| Paramètres : Jeux enregistrés | | `pages.rs` | ✅ |
| Paramètres : Confidentialité des activités | | `pages.rs` | ✅ |
| Paramètres : Applications connectées | | `settings/authorized_apps.rs` | ✅ |
| Paramètres : Développeur | | `pages.rs` | ✅ |

## 5. Ce lot (wt/inventory) et ce qui reste

Fait (code, tests unitaires, aucune écriture réelle) :

- Menu de la zone de texte (`textarea-context`) : remplace le menu par défaut de gpui-component. Le raccourci affiché à droite (`shortcutContainer_M3qk4j` : marge 8, hauteur max 18, couleur `--interactive-text-default`, relevé CDP : « Ctrl+V » en `oklab(0.678…)`, libellé en `--text-strong`) est un nouveau `MenuItem::with_shortcut`.
- Ligne d'ami, « … » du profil et de la visionneuse : menus du bundle (`friend-row`, `user-profile-overflow-menu`, `image-menu`). Liens copiés à `size=1024` comme le client.
- Confirmations « Retirer '{name}' » et « Bloquer {name} ? » (chaînes `fPLvZd`, `l5FFq6`, `x5pOn9`, `pegItC`), aussi sur les boutons de l'intro des MP qui étaient des stubs.
- Copier l'image (téléchargement en GET, conversion PNG, presse-papiers, toast « Image copiée ! ») et Enregistrer l'image (boîte de dialogue, écriture locale).
- Modifier le groupe (`group-dm-edit-modal`, module 66442) : `PATCH /channels/{id}` avec seulement les champs modifiés (`name`, `icon` en data URI ou `null`). CSS `zwiT-r` : icône 120 px, crayon 32 px en haut à droite (bordure 4 px de la couleur du fond), « Supprimer l'icône » en critique.
- Invitations du groupe (module 454963) : `GET /channels/{id}/invites`, tri par hôte, révocation par `DELETE /invites/{code}`, « Aucune invitation pour l'instant ! », « Terminé ». Réservé au propriétaire du groupe, comme le client.
- Paramètres du dossier (module 672551) : nom (≤ 32, « Dossier du serveur »), couleur (défaut `BRAND_500` stocké comme absent, 20 couleurs `_tK`, CSS `BorJ2C`). L'écriture remplace seulement la liste `folders` du champ 14 de `settings-proto/1` et garde `guild_positions` (`settings::with_guild_folders`), comme `saveGuildFolders` du client.
- « Uploader ton message en tant que fichier » dans le menu « + » quand le brouillon n'est pas vide (`message.txt`).

Lot 2 (wt/inventory) :

- Messages programmés, reproduits d'après les modules 970244 (sous-menu et modale), 566908 (règles) et 151282 (appels) : créneaux `FF` (9 h et 13 h aujourd'hui si c'est dans plus de 10 minutes, sinon demain ; lundi suivant à 9 h), heure par défaut = heure pleine suivante (+1 h si à moins de 10 min), bornes 10 minutes – 8 jours (`691200` s), corps `POST` identique au client (`channel_id`, `content`, `scheduled_timestamp` ISO en millisecondes, `flags`, `attachments: []`, plus `message_reference` / `allowed_mentions` pour une réponse). Le modèle `ScheduledMessage` suit maintenant la réponse lue par le client (`scheduled_message_id`, `send_at_timestamp`, `create_args`, `state`) : l'ancien modèle (`id`, `channel_id`, `content`, `scheduled_timestamp`) n'aurait pas lu une liste non vide. Sonde gardée `examples/schedule_probe.rs` (Aperture Sciences / #phoenix uniquement) pour créer, lister, reprogrammer puis annuler ou envoyer : ⚠ pas encore exécutée, l'intégrateur ne compile pas cet exemple (liste `EX` de `phoenix-integrate`).
- « Déplacer vers… » des serveurs et des dossiers (module 816581 / 865815) : sous-menu « Dossier » (Aucun dossier, puis les dossiers ; l'actuel est grisé), « Premier / Dernier dans la liste » ou « dans le dossier », grisés en tête et en fin, absents s'il n'y a qu'un élément. Le déplacement réécrit l'arbre (`gdm::move_tree`, testé) puis l'enregistre comme le glisser-déposer.
- Le glisser-déposer du rail et les dossiers enregistrent désormais `guild_folders` dans `settings-proto/1` comme le client (`saveGuildFolders`), au lieu du `PATCH /users/@me/settings` JSON hérité.

Reste, par ordre de visibilité :

1. Pages Quêtes et Découvrir (stubs visibles sur le rail et dans les MP).
2. « Déplacer vers › » des catégories, Fils actifs, Boost, Annuaire d'applications.
3. Calendrier et champ d'heure de la modale « Programmer un message », « Modifier le message » programmé.
4. Nettoyage des Groupes privés, Changer de compte, modale de statut personnalisé, confirmation « Ignorer ».
5. Menus vidéo (vitesse, transcription), aperçu des fichiers texte, menu « … » des paramètres.

## Vérifications sur le vrai compte (25/11, `real_shots` 1536×840 @1,5, #phoenix ; client CDP sur #général)

| vue | client (CDP) | Phoenix | écart restant |
|---|---|---|---|
| Paramètres du dossier | `folder-modal.png` : 480 × 385, libellés 16/20 medium, pastilles 70 × 50 et 20 × 20, grille sous les grandes pastilles, pastille personnalisée = couleur courante + pipette | `phx-foldermodal.png` | grille passée à la ligne après la première capture (lot 3) ; retrait du texte du champ (gpui-component) |
| Programmer un message | `sched-modal.png` : Date « 25 nov. 2026 » + calendrier, Heure + chevron, astérisques rouges, 16 px sous les champs | `phx-schedmodal.png` | espace sous les champs ajouté (lot 3) ; calendrier non reproduit |
| « + » › Programmer un message › | `sched-submenu.png` : 3 créneaux / Personnalisé..., sous-menu remonté pour tenir dans la fenêtre | `phx-schedmenu.png` | sous-menus recalés dans la fenêtre (marge 12) au lot 3 |
| Menu de la zone de texte | relevé CDP | `phx-textmenu.png` | « Langues › » absent |
| Menu du dossier | relevé CDP (4 entrées) | `phx-foldermenu.png` | aucun |
| Bandeau programmé | non capturé (cliquer un créneau modifie le brouillon du client) | `phx-schedbar.png` | — |

Captures privées dans `/mnt/windata/project-phoenix/compare/inventory/`.
