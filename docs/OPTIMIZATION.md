# Phase d'optimisation : instructions

Ce document guide la phase qui suit la parité 1:1. L'ordre des phases, fixé par le mainteneur le 24/11, est le suivant :
1. Parité visuelle et fonctionnelle 1:1 avec le client 2026.
2. **Performance et instantanéité** (ce document).
3. Passe UX finale, cas d'usage par cas d'usage, côte à côte avec le client officiel.
4. Modularisation du code.

On ne commence la phase 2 que lorsque la parité est close, c'est-à-dire quand les lignes ❌ et ⚠ de `docs/parity/REMAINING.md` sont fermées ou tranchées. D'ici là, chaque problème de performance ou de tracé repéré est **ajouté ici**, et pas corrigé en passant.

Les mesures et les pistes déjà chiffrées sont dans `docs/perf.md`, sections « What's next » et « Next targets ». Les chiffres comparés au client officiel sont dans `docs/BENCHMARKS.md`. Ce document ne les recopie pas : il fixe les règles de la phase et liste ce qui reste à faire.

## Objectif

L'utilisateur ne doit jamais voir un état intermédiaire que le client officiel ne montre pas, et Phoenix doit être plus rapide que lui à chaque étape. Concrètement :
- **Au lancement**, le premier frame utile vient du cache disque : serveurs, salons, MP et dernière page du salon ouvert. Ce n'est jamais un écran vide ni un placeholder.
- **Au changement de salon**, de serveur ou de MP, les messages sont déjà en mémoire. Une page affichée en moins d'un frame est la cible, un spinner est un échec.
- **Pas de flash** : aucune couleur, image ou mise en page provisoire qui serait ensuite remplacée sous les yeux de l'utilisateur. Si une donnée manque, on la précharge ou on la cache sur disque. Sinon, on reproduit exactement l'état de chargement du client, avec sa transition.
- **Au repos**, le CPU de Phoenix doit tendre vers 0 % hors pilote, et rien ne doit rebâtir l'arbre si rien n'a changé.
- **En mémoire**, Phoenix doit rester très en dessous du client Electron sur le même compte, mesuré de la même façon.

## Règles de la phase

- **Mesurer avant et après** chaque changement, avec les outils de `docs/perf.md` : `PHX_TIMINGS=1`, `int-bin/bench`, `examples/frames`, `/proc` pour la mémoire et les réveils. Une optimisation sans chiffre avant et après ne passe pas. Les chiffres vont dans `docs/perf.md`, un tableau par série de commits.
- Mesurer dans un profil optimisé, voir la cible 9 de `docs/perf.md` : le profil dev d'int-bin multiplie les temps de frame par 5 à 10.
- **Ne jamais casser la parité.** Chaque optimisation visible se compare de nouveau au client avec real_shots et CDP, aux mêmes dimensions. Un rendu plus rapide mais différent est une régression.
- Une seule chose par commit, pour pouvoir revenir en arrière proprement.
- Les règles du brief des agents restent valables :
  - builds via `phoenix-submit` uniquement ;
  - aucune écriture sur le compte ;
  - rendus hors écran ;
  - pas de commentaires dans le code.
- La modularisation (phase 4) n'est pas une optimisation. Découper `PhoenixApp` en vues mises en cache (`AnyView::cached`, voir `docs/modularization-notes.md`) fait partie de la phase 2 seulement si une mesure montre que c'est ce qui coûte.

## À traiter

### Tuiles vocales et appels (signalé par le mainteneur le 26/11)

Le mainteneur a fourni deux captures sur le vrai compte. Elles sont privées et restent hors du dépôt.

1. **Couleur provisoire avant le dégradé du profil.** En entrant dans un vocal, la tuile montre d'abord une couleur marron, puis le dégradé du thème de profil. La chaîne actuelle est dans `ui/call_view.rs` (`tile_background`, `guild_theme`) :
   1. `PRIMARY_800` pendant le chargement ;
   2. puis la couleur dominante de l'avatar (`avatar_color`) ;
   3. puis le thème du profil global (`ensure_profile`) ;
   4. enfin le thème du profil de serveur (GET `/users/{id}/profile?guild_id=`).

   Chaque étape repeint la tuile. Pistes :
   - persister sur disque les thèmes de profil (global et par serveur) et les couleurs dominantes d'avatar, avec le cache de session, pour les avoir au premier frame ;
   - précharger le profil de serveur des membres dès qu'ils apparaissent dans un salon vocal de la liste, avant que l'utilisateur entre dans l'appel ;
   - ne jamais peindre une couleur de repli quand un thème est attendu : garder l'état de chargement du client jusqu'à la réponse ;
   - vérifier dans le client (composant 697682, expérience `2026-09-video-background-profile-fetch`) ce qu'il montre pendant le chargement, et reproduire sa transition s'il en a une.
2. **Bugs de tracé dans la vue d'appel et la liste des salons.** Sur les captures :
   - le minuteur du salon vocal passe à la ligne (« 0:3 » puis « 5 » en dessous) au lieu de rester sur une ligne ;
   - la disposition des tuiles diffère d'une capture à l'autre : empilées en hauteur avec l'étiquette du nom, ou côte à côte sans étiquette. Il faut relever la règle de la grille du client selon la largeur et la hauteur, et savoir quand l'étiquette du nom s'affiche ;
   - le panneau vocal affiche « Aucun chemin de connexion » d'un côté et « Voix connectée » de l'autre. Il faut vérifier si c'est un vrai état RTC ou un état mal calculé.

   Avant de corriger, mesurer chaque point côte à côte avec le client, aux mêmes dimensions.

### Démarrage, données et rendu

Les pistes déjà chiffrées sont dans `docs/perf.md` : repli des polices hors du démarrage, allocations par frame de défilement, frames au repos, minuteries et réveils, un seul runtime et un seul client HTTP, snapshot binaire pour le RESUME, budget des médias décodés, dépendances lourdes, profil optimisé. On les traite dans cet ordre, sauf si une nouvelle mesure en change le classement.

S'y ajoutent :
- **Préchargement** :
  - les salons avec mentions non lues après READY ;
  - le premier écran de chaque serveur survolé dans le rail ;
  - les profils et avatars des auteurs visibles.
- **Images et avatars** : ne plus montrer de cercle vide ni de place réservée qui se remplit ensuite. Décoder à la taille affichée, depuis le cache disque, avant le premier frame quand c'est possible.
- **Changements d'état visibles** (popouts, menus, modales) : la première ouverture ne doit pas être plus lente que les suivantes. Mesurer la première ouverture et la suivante, puis préparer à l'avance ce qui est froid.

## Comment ajouter un point

Une ligne par problème, dans la bonne section :
- ce qui se voit ou se mesure ;
- où, avec le fichier et la fonction ;
- la capture ou la mesure (une capture de données réelles reste hors du dépôt) ;
- les pistes.

Ne corrige pas pendant la phase de parité, sauf si le problème casse la parité elle-même.
