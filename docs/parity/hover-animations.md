# Animations d'icônes au survol

Relevé du 25/11/2026 sur le client 2026 (chunks JS et CSS du cache local, survols CDP sur la route ouverte, sans navigation). Phoenix lit les JSON Lottie du client dans le cache Discord local au démarrage (`icons/lottie_icons.rs`, copie dans `~/.cache/…/icons/lottie/`, jamais committée) et les dessine avec le moteur Lottie déjà utilisé pour les effets vocaux (`media/lottie.rs`).

## Comportement commun (composant `lottieIcon`, module 744682)

- Lecture : `playSegments([marqueur.start, marqueur.start + marqueur.duration])` à la cadence du JSON (60 i/s pour toutes les icônes ci-dessous), sans boucle (expérience `lottie_hover_multiple_loop` inactive). Fin sur l'image `start + duration - 1` (comportement de lottie-web), puis l'icône reste figée tant que la souris est dessus.
- Sortie du survol : `stopIfPlaying(marqueur de survol)`. Si le dernier segment joué est celui du survol, l'icône revient **immédiatement** sur l'image de départ du segment. Il n'y a pas de lecture inverse : mesuré en direct sur la roue des paramètres (65 images en ~1,08 s, puis retour à l'image de repos dans l'image qui suit la sortie). Si un clic ou un changement d'état a joué un autre segment entre-temps, la sortie ne change rien.
- Changement d'état (micro, casque, caméra, partage, Krisp) : `useEffect(() => () => play(ancienne action))`, donc l'animation de transition de l'ancien état est jouée (ex. « mute » 0→69 quand on coupe le micro).
- Réduire les animations (ou fenêtre principale masquée) : `play` saute à l'image `start + duration` et s'arrête, `stopIfPlaying` ne fait rien. Phoenix suit `settings::effects::reduced_motion()`.
- Couleur : `.lottieIconColors_69Onuk :not(defs *)[fill][fill-opacity]{fill:var(--__lottieIconColor)}` (idem `stroke`). Toutes les formes prennent la couleur de l'icône, les mattes et masques gardent la leur. Phoenix teinte la frame entière et rend les mattes sans teinte.
- Moteur : ajout du trim paths (`tm`, modes simultané et individuel sur un seul tracé), des masques soustractifs et inversés, des mattes inversées (`tt` 2 et 4) et de la teinte.
- Tests : `ui::hover_icon::tests` (minutage image par image, retour au départ, transitions, clic, réduction des animations), `media::lottie::tests` (trim, masques, mattes), `icons::lottie_icons::tests` (extraction et identification), `no_lottie_json_in_repo`.
- Rendus : `real_shots` avec `PHX_MOUSE=x,y PHX_HOVER_ANIM=0,100,200,…` écrit une capture par instant (`-t0100.png`…), et `PHX_MOUSE_LEAVE=x,y` une capture après la sortie.

## Icônes

| Zone | Icône | JSON (`nm`) | Marqueurs (image de début, durée) | Survol joue | Clic / état | Phoenix |
|---|---|---|---|---|---|---|
| Panneau utilisateur | Micro | `Lottie_HoverOnOff_Microphone` | mute 0/70, unmute 100/70, hover_unmuted 180/40, hover_muted 240/40 | hover_unmuted (micro ouvert) ou hover_muted | changement d'état : mute ou unmute | ✅ `voice_panel::audio_button` |
| Panneau utilisateur | Casque | `Lottie_HoverOnOff_Deafen` | deafen 0/70, undeafen 110/70, hover_undeafened 200/70, hover_deafened 300/70 | hover_undeafened ou hover_deafened | changement d'état | ✅ `voice_panel::audio_button` |
| Panneau utilisateur | Paramètres (roue) | `L` (24×24, 66 images) | all 0/66 | all | aucun | ✅ `sidebar` (panel_button « settings ») |
| Panneau vocal | Krisp | `Lottie_HoverOnOff_NoiseSuppression` | disable 0/70, enable 100/70, hover_enabled 200/70, hover_disabled 300/70 | hover_disabled (Krisp coupé) ou hover_enabled | changement d'état | ✅ `voice-noise` |
| Panneau vocal | Raccrocher | `Lottie_Hover_EndCall` | all 0/90 | all | aucun | ✅ `hangup` |
| Panneau vocal | Caméra | `Lottie_HoverOnOff_Camera` | disable 0/70, enable 100/70, hover_enabled 180/70, hover_disabled 280/70 | hover_disabled (caméra coupée) ou hover_enabled | changement d'état | ✅ `voice_panel::camera_button` |
| Panneau vocal | Partage d'écran | `Lottie_Hover_Screenshare` | enable 0/70, disable 90/70, hover_enabled 180/40, hover_disabled 240/40 | hover_disabled ou hover_enabled | changement d'état | ✅ `voice_panel::screen_button` |
| Panneau vocal | Activités | `Shapes-ButtonDark 2` (96×96) | hover 0/59, click 68/26 | hover | clic : click | ✅ `voice-activity` |
| Panneau vocal | Soundboard | `Lottie_HoverOn_Soundboard` | all 0/66 | all | clic : all | ✅ `pickers::soundboard::panel_button` |
| Vue d'appel | Micro, caméra, partage, activités, soundboard, raccrocher | mêmes JSON et hooks que le panneau vocal (`905216` raccrocher, `528057` partage, caméra `K`) | idem | idem | idem | ✅ `call_view::bottom_controls`, `soundboard::call_button` |
| Compositeur | Cadeau | `Lottie_Web_Gift` | all 0/66 | all | aucun | ✅ `composer_button_ext("gift")` |
| Compositeur | GIF | `Lottie_HoverOn_GIF` | click 0/66, hover 90/40 | hover | clic : click | ✅ `composer_button("gif")` |
| Compositeur | Autocollants | `Lottie_HoverOn_Sticker` | click 0/66, hover 90/40 | hover | clic : click | ✅ `composer_button("sticker")` |
| Compositeur | Applications | `Shapes-ButtonDark 2` (même JSON que Activités, hook 697744) | hover 0/59, click 68/26 | hover | clic : click | ✅ bouton `apps` |
| Compositeur | Emoji (visage) | pas de Lottie : sprite `1b05b930a77fbff9.png` (couleur) et masque `decf85f79e02d276.png`, grille 20×4, cases de 48 px | au survol, visage tiré au hasard parmi 77 (`Math.floor(77*Math.random())`), affiché en couleur ; au repos, le dernier visage en gris (`--interactive-text-default`), visage 50 au départ | échelle 1 → 1,14 avec un ressort react-spring tension 800, friction 24 (dépassement ~1,17, stable en ~0,35 s) | sélecteur ouvert : reste en couleur | ✅ `hover_icon::emoji_button` (sprite chargé depuis le cache, jamais committé) |

## SVG animés en CSS

Aucun `@keyframes` ni changement de `d`/`stroke-dashoffset` au survol sur les icônes des barres. Les seuls effets CSS d'icône au survol relevés :

| Zone | Règle | Phoenix |
|---|---|---|
| Barre d'actions des messages (popover `_DpHsW6`) | `.hoverBarButton:hover .icon{transform:scale(1.1)}`, `.full-motion .icon{transition:transform .4s cubic-bezier(.16,1,.3,1)}`, `:active` → `scale(1)` | ✅ déjà fait (`chat::message_ext::tool_button`, 20 → 22 px en 400 ms, même courbe) |
| Barre d'outils du profil (`_yDAPhH`) | `:hover .icon{transform:scale(1.1)}`, même transition 0,4 s | ⚠ la barre elle-même (popover au survol des sections du profil) n'existe pas encore dans Phoenix |

## Non couvert

- Barre d'en-tête (fils, notifications, épingles, membres), boîte de réception, aide, « + » d'ajout de serveur, Découvrir, Nitro, Quêtes : icônes statiques dans le client 2026 (pas de Lottie, pas de keyframes), seule la couleur change.
- ⚠ Survoler « Commencer une activité » dans la vue d'appel du client officiel envoie `PATCH /users/@me/settings-proto/1`. Ne plus le survoler en CDP.
