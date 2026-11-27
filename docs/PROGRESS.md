# Avancement réel

Audit du 27/11/2026 sur origin/main `8c7b358b`. Ce document remplace le pourcentage de lignes ✅ comme mesure de l'avancement. Il se refait à la main : chaque état ci-dessous doit citer une preuve, et un état ne monte d'un cran que sur une nouvelle preuve réelle.

<!-- real-progress {"score":57.8,"low":45.6,"high":73.1,"features":122,"verified":27,"unverified":76,"broken":9,"missing":10,"critical":26,"critical_verified":14,"critical_broken":6,"proof_measured":44,"proof_compared":8,"at":"2026-11-27"} -->

## Résultat

| mesure | valeur |
|---|---|
| **Score réel pondéré** | **58 %** (fourchette 46 à 73 %) |
| Couverture fonctionnelle | 112 fonctionnalités sur 122 existent dans le code (94 % du poids), mais **27 seulement sont vérifiées en vrai** (33 % du poids) |
| Niveau de preuve des ✅ | **44 %** s'appuient sur une mesure du vrai client, **8 %** ont été comparés à un rendu Phoenix ; 40 % viennent du CSS ou du JS seuls, 8 % d'un test unitaire, 7 % de rien |
| Parcours critiques | **14 sur 26** prouvés en usage ou en test réel, 6 cassés connus, 6 non prouvés |

L'ancien chiffre (94,2 % : 2287 ✅, 114 ⚠, 26 ❌, 64 ➖ dans `docs/parity/*.md`) mesure autre chose : la part des détails inventoriés qui ont été codés. Il est biaisé pour trois raisons.

1. Il ne compte que ce qui a été inventorié. Les fonctionnalités qu'aucun doc ne liste (Stage, activités, correcteur, liens `discord://`) n'y pèsent rien.
2. Un ✅ veut surtout dire « codé d'après le CSS ou le JS ». Sur l'échantillon, 56 % des ✅ n'ont aucune mesure dans le client, et pour les interactions (survol, défilement, clic) ce taux monte à 67 %. Le défilement du chat et la lecture vidéo étaient ✅ et ne marchaient pas.
3. Toutes les lignes pèsent pareil : une marge de 1 px compte autant qu'« entendre les autres en vocal ».

## Méthode

### 1. Inventaire du client officiel

Source : le bundle 2026 en cache (`~/discord-phoenix-ref/current-client/js`, 2840 chunks), lu en texte, sans exécuter de code dans le client.

| élément | nombre | d'où |
|---|---|---|
| Routes | 151 clés | objet des routes (`LOGIN`, `CHANNEL`, `GUILD_DISCOVERY`, … `ICYMI`) du chunk `fast-connect` |
| Sections des paramètres utilisateur | 40 | énumération `ACCOUNT` … `DEVELOPER_OPTIONS` (`account`, `voice`, `clips`, `overlay`, `streamer-mode`, `family-center`, …) |
| Sections des paramètres de serveur | ≈ 75 (≈ 45 pages visibles) | énumération `LANDING` … `BOOST_PERKS_VANITY_URL` du chunk `web` |
| Sections des paramètres de salon | 20 | `OVERVIEW`, `PERMISSIONS`, `INSTANT_INVITES`, `INTEGRATIONS`, `WEBHOOKS`, `CHANGE_RTC_REGION`, `EDIT_FORUM_TAG`, … |
| Menus (`navId`) | 99 | toutes les valeurs `navId:"…"` des chunks |
| Modales | ≥ 47 | [inventory.md](parity/inventory.md) ; 28 seulement ont un `modalKey` lisible, les autres sont minifiées |

Ces éléments sont regroupés en 122 fonctionnalités de taille comparable : une fonctionnalité est ce qu'un utilisateur reconnaît comme « une chose qu'il fait » (envoyer, répondre, entendre, regarder un stream). Une page de paramètres sans action réelle compte pour une ligne de détail, un parcours quotidien pour une ligne critique.

### 2. État dans Phoenix

Présence : lecture de `crates/phoenix-app/src`, `crates/phoenix-voice/src` et `crates/phoenix-api/src`. État de vérification : journaux d'usage (`phoenix.log`, `phoenix-voice.log`), QA d'entrées sur le vrai compte (`docs/qa.md`, runs du 27/11 à 12:14 et 13:45), changelog du dashboard, docs de parité.

| état | sens | points (bas / central / haut) |
|---|---|---|
| vérifiée en vrai | vue marcher sur le vrai compte : usage de l'utilisateur, test réel sur un salon de test, QA d'entrées, ou affichage réel comparé au client | 1 / 1 / 1 |
| présente, non vérifiée | le code existe, la preuve est un test, une démo, le CSS ou le JS | 0,25 / 0,5 / 0,75 |
| cassée connue | un test réel ou l'utilisateur l'a vue échouer, sans preuve de correctif depuis | 0 / 0 / 0,25 |
| absente | rien dans le code, ou un bouton inerte | 0 / 0 / 0 |

Poids : critique ×5, important ×2, détail ×1 (26 critiques, 61 importantes, 35 détails, poids total 287). Le score est la somme des points × poids, divisée par le poids total. La fourchette basse suppose que la moitié des « présentes non vérifiées » ne marche pas en vrai, ce que l'échantillon des ✅ rend plausible ; la haute suppose que les correctifs non testés marchent.

### 3. Niveau de preuve des ✅

183 lignes ✅ tirées au hasard sur 2465 (tirage stratifié par doc, graine 20261127), classées à la main. Détail ligne par ligne : [progress-proof-sample.md](progress-proof-sample.md). Le décompte de 2465 diffère des 2287 du dashboard parce que le tirage lit la colonne d'état de chaque tableau, y compris les tableaux de synthèse.

| catégorie | part extrapolée | ≈ lignes |
|---|---|---|
| mesuré dans le client et comparé à un rendu Phoenix | 8 % | 205 |
| mesuré dans le client seulement (dump, CDP) | 36 % | 885 |
| tiré du CSS ou du JS, sans comparaison | 40 % | 1000 |
| test unitaire ou démo seuls | 8 % | 210 |
| sans source | 7 % | 170 |

| type de ligne | part des ✅ | mesuré dans le client | CSS/JS seul | test seul | sans source |
|---|---|---|---|---|---|
| visuel statique | 56 % | 56 % | 35 % | 4 % | 4 % |
| interaction | 25 % | 33 % | 56 % | 0 % | 11 % |
| fonction | 19 % | 26 % | 35 % | 31 % | 8 % |

Docs les plus faibles : `inventory.md` (« page présente ✅ » sans mesure), `settings-wiring.md` et `notifications.md` (tests seuls), `composer.md`, `typing.md`, `search.md`, `hover-animations.md`. Les plus solides : `real-compare.md`, `context-menus.md`, `icons.md`, `home.md`.

## Parcours critiques au quotidien

| parcours | poids | état réel | preuve la plus forte |
|---|---|---|---|
| Connexion par session en cache | critique | prouvé (usage) | 22 démarrages réels, aucune erreur |
| Connexion e-mail, QR, 2FA | critique | non prouvé | aucun login interactif tracé |
| Reprise après coupure réseau | critique | prouvé (usage) | 26/11 : deux coupures reprises en < 0,1 s |
| Statut propre visible des autres | critique | cassé, correctif non prouvé | hors ligne pour tous jusqu'au 26/11 |
| Lire et défiler un salon | critique | prouvé (QA) | QA run2 OK ; run1 KO ; le build de l'utilisateur n'a pas encore le correctif |
| Charger l'historique ancien | critique | non prouvé | jamais testé jusqu'à la pagination |
| Revenir au présent | important | cassé | QA KO deux fois le 27/11 |
| Envoyer, éditer, supprimer, réagir | critique | prouvé (test réel) | salon de test, 26/11 |
| Menu contextuel d'un message | critique | cassé | QA KO deux fois le 27/11 |
| Répondre | critique | non prouvé | aucun envoi de réponse |
| Pièces jointes (envoi) | critique | prouvé (test réel) | PNG de 3,3 Mo envoyé |
| Images | critique | prouvé (usage) | ≈ 8 échecs en deux jours, liens expirés |
| Vidéos | critique | cassé | QA : rien ne se lit après 20 s |
| Embeds | important | prouvé (rendus réels) | mosaïque X jamais vue en vrai |
| MP : liste et ouverture | critique | prouvé | ouverture réelle |
| MP : envoi | critique | non prouvé | aucun envoi en MP |
| Notifications de bureau | critique | prouvé (test réel) | popup, son, clic (27/11) |
| Non-lus et pastilles | critique | non prouvé | pastilles seulement simulées |
| Recherche | important | prouvé (rendus réels) | résultats réels affichés |
| Vocal : rejoindre | critique | non prouvé | DAVE rejoint, `rtp_audio=0` |
| Vocal : entendre | critique | cassé, correctif non prouvé | -100 dB sur toute la session du 26/11 |
| Vocal : parler | critique | non prouvé | aucun auditeur |
| Vocal : changer de salon | critique | cassé, correctif non prouvé | plus de connexion après le changement |
| Mute, sourdine | important | non prouvé | seulement `deafened=false` dans les traces |
| Caméra, stream, appel MP | important | non prouvés | aucune image reçue en direct, aucun appel |
| Paramètres principaux | important | ouverture prouvée, effets non prouvés | panique de Mon compte corrigée sans réouverture |
| Stabilité en usage long | critique | cassé (mémoire) | 10 h sans plantage, mais RSS de 423 à 875 Mio, correctif non remesuré |

Le build de l'utilisateur au moment de l'audit (`766fa7ba`) ne contient ni les correctifs de défilement ni le lecteur vidéo. Les plaintes sur ces deux points portent donc sur un build plus ancien, mais la QA du 27/11 montre que la vidéo et le saut au présent échouent aussi sur main.

## Les manques les plus lourds

Triés par impact (poids × distance à « vérifiée en vrai ») :

1. Vocal : entendre les autres (cassé)
2. Vocal : changer de salon (cassé)
3. Lecture vidéo (cassée)
4. Menu contextuel d'un message (cassé)
5. Statut propre visible des autres (correctif jamais vérifié)
6. Stabilité : fuite mémoire (correctif non remesuré)
7. Liste des membres : défilement et clic (cassés)
8. Revenir au présent (cassé)
9. Correcteur orthographique (absent)
10. Suppression de bruit et annulation d'écho (absentes)
11. Salons Stage, activités, liens `discord://`, lecteur d'écran (absents)

Puis les critiques jamais prouvées : rejoindre et parler en vocal, répondre, envoyer en MP, historique ancien, non-lus et pastilles, login interactif. Le backlog de travail tiré de cette liste est tenu hors du dépôt.

## Les 122 fonctionnalités

### Connexion et compte

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 1 | Session en cache, jeton dans le trousseau | critique | vérifiée en vrai | 22 démarrages réels du 25/11 au 27/11 dans phoenix.log, sans erreur 4004 |
| 2 | Connexion e-mail, mot de passe et 2FA (TOTP, SMS, WebAuthn) | critique | présente, non vérifiée | `ui/login.rs`, `auth/` ; aucun login interactif réussi tracé |
| 3 | Connexion par QR code (remote auth) | important | présente, non vérifiée | `auth::QrState` ; jamais tracé en réel |
| 4 | Captcha à la connexion ou à l'envoi | important | présente, non vérifiée | code présent, jamais déclenché en réel |
| 5 | Reprise de la passerelle après coupure réseau | critique | vérifiée en vrai | phoenix.log 26/11 : deux coupures reprises (`resume=true`) en moins de 0,1 s |
| 6 | Plusieurs comptes (changer de compte) | détail | présente, non vérifiée | `modals/accounts.rs`, rendu seul |
| 7 | Inscription (route REGISTER) | détail | absente | simple lien sur l'écran de connexion |

### Présence

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 8 | Statut propre diffusé aux autres (en ligne, inactif, DND, invisible) | critique | cassée connue | compte vu hors ligne de tous jusqu'au 26/11 (4f4995e4), correctif jamais vérifié depuis un autre compte |
| 9 | Statut personnalisé | important | présente, non vérifiée | modale présente, écriture jamais faite sur le compte |
| 10 | Statuts et activités des autres (jeu, musique, stream) | important | vérifiée en vrai | rendus real_shots de la liste des membres et des MP (25-26/11), affichage seul |

### Navigation

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 11 | Rail des serveurs, dossiers, changer de serveur | critique | vérifiée en vrai | QA d'entrées 27/11 : « Changer de serveur au clic » OK |
| 12 | Liste des salons (catégories, repli, défilement, clic) | critique | vérifiée en vrai | QA 27/11 : « Changer de salon au clic » et « Liste des salons : molette » OK |
| 13 | Non-lus, pastilles de mention, marquer comme lu | critique | présente, non vérifiée | pastilles 1/12/120 seulement simulées en local ; rail réel vérifié seulement sans pastille |
| 14 | Sélecteur rapide Ctrl+K | important | vérifiée en vrai | QA 27/11 OK |
| 15 | Réorganiser serveurs et salons (glisser-déposer) | détail | présente, non vérifiée | `rail_dnd.rs`, `channel_dnd.rs`, écritures jamais exécutées |
| 16 | Favoris | détail | présente, non vérifiée | `favorites.rs`, rendus démo et réels, écritures non exécutées |
| 17 | Liste d'amis (en ligne, tous, en attente) | important | vérifiée en vrai | rendus réels de l'accueil et dumps `home.md` |
| 18 | Ajouter, accepter, retirer un ami | important | présente, non vérifiée | code présent, aucune action réelle |
| 19 | Liste des MP et ouverture d'un MP | critique | vérifiée en vrai | ouverture réelle pendant un balayage (25/11) |
| 20 | Groupes privés (créer, modifier, quitter, nettoyage) | détail | présente, non vérifiée | `modals/gdm.rs`, `gdm_cleanup.rs` |
| 21 | Boîte de réception (mentions, non-lus, pour toi) | important | présente, non vérifiée | `inbox.rs`, rendus seuls |
| 22 | Fenêtre détachée, incrustation (PiP) | détail | présente, non vérifiée | `pip.rs`, `call_popout.rs` |

### Lecture du chat

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 23 | Afficher les messages d'un salon (groupage, auteurs, dates) | critique | vérifiée en vrai | real-compare.md et real-glitches.md : environ 30 écrans réels comparés |
| 24 | Défilement (molette, pavé tactile, PageUp/PageDown) | critique | vérifiée en vrai | QA run2 27/11 13:45 OK ; run1 12:14 KO ; le build de l'utilisateur (766fa7ba) n'a pas encore ces correctifs |
| 25 | Charger l'historique plus ancien | critique | présente, non vérifiée | aucune étape QA ne remonte jusqu'à la pagination |
| 26 | Revenir au présent (barre de saut) | important | cassée connue | QA run1 et run2 du 27/11 : `at_bottom` KO deux fois ; 8c7b358b non retesté |
| 27 | Sauter à un message (réponse, lien, épingle, résultat) | important | présente, non vérifiée | code présent, non testé en réel |
| 28 | Indicateur de frappe | important | présente, non vérifiée | typing.md : valeurs CSS, jamais vu avec un vrai tiers qui écrit |
| 29 | Markdown (titres, listes, citations, spoilers, code coloré) | important | vérifiée en vrai | real-glitches.md : glitchs relevés sur des écrans réels puis corrigés |
| 30 | Mentions, émojis personnalisés et animés, horodatages | important | vérifiée en vrai | rendus réels comparés (real-compare.md) |
| 31 | Images et mosaïques | critique | vérifiée en vrai | usage réel : environ 8 échecs en 2 jours (liens expirés), rendus réels |
| 32 | Lecture vidéo (lecture, seek, plein écran) | critique | cassée connue | QA run1 et run2 27/11 : lecture KO (`video_t=-1` après 20 s), seek KO |
| 33 | Embeds (liens, YouTube, X, riches) | important | vérifiée en vrai | rendus réels ; mosaïque X jamais vue sur un vrai post |
| 34 | Fichiers et audio joints (lecture audio) | important | présente, non vérifiée | `media/audio_file.rs`, lecture jamais prouvée |
| 35 | Messages vocaux (lecture et enregistrement) | important | présente, non vérifiée | voice-messages.md : CSS et JS, aucune lecture réelle |
| 36 | Messages système, bannières de fil, annonces | détail | présente, non vérifiée | rendus, pas de comparaison dédiée |
| 37 | Composants d'application (boutons, menus, conteneurs v2) | important | présente, non vérifiée | affichage présent, interactions jamais envoyées |
| 38 | Stickers (affichage) | détail | présente, non vérifiée | 404 de stickers dans phoenix.log |
| 39 | Super réactions (affichage, animation) | détail | cassée connue | plantage le 26/11 14:20, corrigé par 80f8c2ca, jamais revu en réel |
| 40 | Sondages (affichage, vote, création) | important | présente, non vérifiée | rendus et tests ; vote réel non tracé dans le changelog |
| 41 | Utilisateurs bloqués ou ignorés (messages masqués) | détail | présente, non vérifiée | `chat/blocked.rs` |

### Écriture

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 42 | Envoyer un message | critique | vérifiée en vrai | envoi réel dans le salon de test (26/11 19:55) |
| 43 | Auto-complétion (mentions, émojis, salons, commandes) | important | vérifiée en vrai | QA 27/11 : « Composer : frappe et auto-complétion » OK |
| 44 | Éditer un message | critique | vérifiée en vrai | édition réelle sur le salon de test (26/11 19:55) |
| 45 | Supprimer un message | critique | vérifiée en vrai | suppression réelle avec confirmation (26/11 19:55) |
| 46 | Menu contextuel d'un message (clic droit) | critique | cassée connue | QA run1 et run2 27/11 : `msgmenu == 1` KO |
| 47 | Réagir (ajouter, retirer) | critique | vérifiée en vrai | 👍 ajouté puis retiré en réel (26/11 19:55) |
| 48 | Répondre à un message | critique | présente, non vérifiée | aucun envoi réel de réponse tracé |
| 49 | Envoyer des pièces jointes (bouton +, glisser, préupload) | critique | vérifiée en vrai | PNG de 3,3 Mo envoyé en réel (25/11, 26/11) |
| 50 | Coller une image du presse-papiers | important | présente, non vérifiée | `chat/state.rs` gère `ClipboardEntry::Image`, jamais testé en réel |
| 51 | Sélecteur d'émojis | important | présente, non vérifiée | pickers.md : surtout CSS |
| 52 | GIF (Tenor) et favoris | important | présente, non vérifiée | `pickers/gif.rs`, aucun envoi réel |
| 53 | Envoyer un sticker | détail | présente, non vérifiée | `pickers/sticker.rs` |
| 54 | Commandes slash d'applications | important | présente, non vérifiée | `commands/`, interactions jamais envoyées |
| 55 | Transférer un message | détail | présente, non vérifiée | `modals/forward.rs` |
| 56 | Créer un fil, un sondage, un message programmé, un rappel | important | présente, non vérifiée | code présent, rien d'exécuté en réel hors salon de test |
| 57 | Épingles (voir, épingler) | détail | présente, non vérifiée | `actions/pins.rs` |
| 58 | Correcteur orthographique | important | absente | inventory.md : pas de correcteur (pas de suggestions ni de dictionnaire) |
| 59 | Salons d'annonces : publier, suivre | détail | présente, non vérifiée | `chat/announce.rs`, `modals/follow.rs` |
| 60 | Message vocal : enregistrer et envoyer | détail | présente, non vérifiée | `chat/voice_message.rs`, jamais envoyé |

### Fils et forums

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 61 | Lire forums, salons média et fils | important | vérifiée en vrai | lecture réelle de forums et salons média (27/11 12:32) |
| 62 | Créer un post, répondre dans un fil, suivre | important | présente, non vérifiée | aucune création ni réponse prouvée |

### Recherche

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 63 | Recherche (filtres, résultats) | important | vérifiée en vrai | résultats réels en rendu (25/11, 26/11), pas d'usage de l'utilisateur |
| 64 | Aller au résultat de recherche | important | présente, non vérifiée | non testé |

### Notifications

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 65 | Notifications de bureau (popup, son, clic) | critique | vérifiée en vrai | sonde réelle du 27/11 09:10 (popup, son, clic vers le salon) |
| 66 | Sons de l'interface (message, appel, vocal) | important | présente, non vérifiée | sounds.md : tests unitaires seuls |
| 67 | Réglages de notification (rendre muet, niveaux) | important | vérifiée en vrai | sourdine et flags réels sur un fil de test (25/11), remis à zéro |
| 68 | Badge de la barre des tâches, zone de notification | détail | présente, non vérifiée | `tray.rs`, badge non comparé |

### Vocal

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 69 | Rejoindre un salon vocal (connexion, chiffrement DAVE) | critique | présente, non vérifiée | 26/11 18:51 : DAVE rejoint mais `rtp_audio=0` |
| 70 | Entendre les autres | critique | cassée connue | phoenix-voice.log 26/11 : -100 dB sur les 16 relevés, `no key for sender` ; correctif dcadb31b jamais testé |
| 71 | Parler (être entendu) | critique | présente, non vérifiée | `Speaking true` local, aucune confirmation côté auditeur |
| 72 | Changer de salon vocal | critique | cassée connue | 26/11 18:51 : plus aucune connexion après changement ; correctif 766fa7ba sans session depuis |
| 73 | Couper le micro, mettre en sourdine | important | présente, non vérifiée | seulement `deafened=false` dans les traces |
| 74 | Push-to-talk, détection de voix, sensibilité | important | présente, non vérifiée | `voice/ptt.rs`, `audio/device.rs` |
| 75 | Suppression de bruit (Krisp) et annulation d'écho | important | absente | filtre maison simple ; annulation d'écho : drapeau sans traitement ; pas de Krisp |
| 76 | Volume par utilisateur, sourdine locale | important | présente, non vérifiée | code présent |
| 77 | Soundboard (jouer un son) | détail | présente, non vérifiée | popout mesuré dans le client, envoi jamais fait |
| 78 | Vue d'appel (grille, focus, tuiles) | important | présente, non vérifiée | call-view.md : surtout démo |
| 79 | Chat textuel du salon vocal | détail | présente, non vérifiée | « Ouvrir le chat » branché |
| 80 | Statut de salon vocal | détail | présente, non vérifiée | `modals/voice_status.rs` |

### Vidéo et stream

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 81 | Caméra : envoyer | important | présente, non vérifiée | `phoenix-voice/media/camera.rs` ; seule la caméra du client officiel a été mesurée |
| 82 | Caméra : recevoir | important | présente, non vérifiée | décodage H.264, VP8 et AV1 testé sur données synthétiques |
| 83 | Partage d'écran : diffuser | important | présente, non vérifiée | paquets acceptés par le serveur (25/11), aucun spectateur |
| 84 | Regarder un stream | important | présente, non vérifiée | call-view.md : aucune image décodée en direct |
| 85 | Appel en MP (passer, recevoir, sonnerie) | important | présente, non vérifiée | boutons refusés par la garde, aucun appel |
| 86 | Salons Stage (orateurs, public, lever la main) | important | absente | seulement l'icône et la ligne « public » dans la barre latérale |
| 87 | Activités (applications intégrées) | important | absente | le bouton existe, aucun lancement possible (pas de vue web) |
| 88 | Arrière-plans et effets de caméra | détail | absente | rien dans le code |

### Profils et membres

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 89 | Liste des membres (défilement, clic) | important | cassée connue | QA run2 27/11 : molette KO, élément #2 non placé, popout et menu KO depuis la liste |
| 90 | Popout et profil complet, notes | important | vérifiée en vrai | QA : « Menu utilisateur puis modale de profil » OK (run1) ; rendus réels |
| 91 | Éditer son profil (avatar, bannière, bio, décorations) | important | présente, non vérifiée | écritures bloquées, jamais exécutées |
| 92 | Bloquer, ignorer, signaler | détail | présente, non vérifiée | confirmation présente, jamais exécuté |

### Paramètres utilisateur

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 93 | Mon compte (e-mail, mot de passe, 2FA) | important | présente, non vérifiée | panique le 26/11 (corrigée par 4bc118f4), réouverture non tracée |
| 94 | Données, confidentialité, contenu et social | important | présente, non vérifiée | rendus réels, écritures bloquées |
| 95 | Apparence (thème, zoom, densité, police) | important | présente, non vérifiée | rendus réels ; effet des réglages comparé seulement en partie (themes.md) |
| 96 | Voix et vidéo (périphériques, test du micro) | important | présente, non vérifiée | rendus réels ; test micro jamais vérifié |
| 97 | Ouvrir et parcourir les paramètres | important | vérifiée en vrai | QA 27/11 : « Paramètres : ouvrir et défiler » OK |
| 98 | Notifications, raccourcis, langue, accessibilité, avancés | important | présente, non vérifiée | rendus réels ; effet des réglages non vérifié |
| 99 | Appareils, connexions, applications autorisées | détail | présente, non vérifiée | pages présentes, actions non exécutées |
| 100 | Nitro, boosts, abonnements, facturation, cadeaux (affichage) | détail | présente, non vérifiée | pages présentes ; aucun achat possible |
| 101 | Clips, overlay, centre familial, détection des jeux | détail | absente | sections du client absentes de Phoenix (pas de capture de jeu ni d'overlay) |

### Serveur

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 102 | Paramètres du serveur : consultation (≈45 sections) | important | présente, non vérifiée | lus en GET et rendus réels ; aucune écriture |
| 103 | Rôles, permissions, membres : modifier | important | présente, non vérifiée | écritures codées et testées, jamais exécutées |
| 104 | Paramètres de salon (aperçu, permissions, invitations, webhooks) | important | vérifiée en vrai | écritures réelles sur le salon de test, valeurs remises (channel-settings.md) |
| 105 | Modération (exclure, expulser, bannir, AutoMod, journal d'audit) | détail | présente, non vérifiée | code présent, jamais exécuté |
| 106 | Invitations : créer et rejoindre un serveur | important | présente, non vérifiée | `invite.rs`, `join_flow.rs`, rien d'exécuté hors salon de test |
| 107 | Créer un serveur, modèles | détail | présente, non vérifiée | `add_server.rs` |
| 108 | Événements (voir, créer, s'inscrire) | détail | présente, non vérifiée | `events.rs`, `modals/create_event.rs` |
| 109 | Onboarding, règles, vérification d'adhésion | détail | présente, non vérifiée | `server_pages/onboarding.rs`, `join_flow.rs` |

### Découverte et boutique

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 110 | Découvrir (serveurs, applications) | détail | présente, non vérifiée | rendus réels ; comparaison partielle (inventory.md ⚠) |
| 111 | Boutique et Nitro (navigation) | détail | présente, non vérifiée | `shop_*.rs`, rendus |
| 112 | Quêtes (liste, accepter, progresser) | détail | présente, non vérifiée | page présente ; aucune quête accomplie |
| 113 | Résumés de salon, ICYMI | détail | absente | aucune trace dans le code |
| 114 | Annuaire et installation d'applications | détail | présente, non vérifiée | `app_profile.rs`, `modals/app_install.rs` |

### Plateforme

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 115 | Mise à jour automatique | important | présente, non vérifiée | `updater.rs`, jamais vue passer en réel |
| 116 | Zone de notification, démarrage automatique | détail | présente, non vérifiée | `tray.rs`, `autostart.rs` |
| 117 | Liens discord:// et invitations ouvertes depuis le navigateur | important | absente | aucun `MimeType` dans le .desktop, pas de gestionnaire de schéma |
| 118 | Stabilité en usage long (mémoire, plantages) | critique | cassée connue | fuite mesurée : RSS de 423 à 875 Mio (perf.md) ; correctif 9bc58282 non remesuré ; 2 paniques le 26/11 |
| 119 | Performance et démarrage | important | présente, non vérifiée | CPU au repos de 0,45 % en release (25/11) ; pas de mesure de démarrage comparée |
| 120 | Lecteur d'écran | important | absente | aucune intégration AT-SPI ou accesskit |
| 121 | Raccourcis clavier | important | présente, non vérifiée | shortcuts.md, tests ; seul Ctrl+K est prouvé |
| 122 | Synthèse vocale (/tts) | détail | présente, non vérifiée | `tts.rs` |
