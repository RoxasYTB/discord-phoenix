# Project Phoenix — état et handoff

> Baseline de cette reprise : `main` à `1583cf0`, dix commits locaux devant `origin/main`. Deux commits locaux ajoutent l’intégration d’avatars compacts et son harness de capture; `main` est ensuite à douze commits devant `origin/main`, sans push.

Ce document est le point de reprise pour les sessions qui poursuivent la parité visuelle et fonctionnelle de Discord Phoenix. Il complète les tableaux par fonctionnalité dans [`docs/parity/`](parity/) et l’inventaire des routes dans [`docs/PARITY.md`](PARITY.md). Il ne remplace pas les preuves d’exécution ni les tests.

## Objectif

Construire un client natif Rust + GPUI qui reproduit les comportements utiles et les états visibles du client Discord officiel, en s’appuyant sur les endpoints et une comparaison visuelle du client officiel déjà ouvert en CDP. Les claims doivent être vérifiés sur le flux réel ou sur des données de démonstration clairement identifiées, avec des écarts documentés plutôt que masqués.

## État du dépôt au 2026-11-24

- `main` est synchronisé avec `origin/main` à `e6f3b7a` après avance rapide de 301 commits.
- Le dernier état de production n’a pas été compilé dans cette reprise.
- La correction des fallbacks SVG `icons/viewer/zoom.svg` et `icons/viewer/open.svg` est commitée en `2972f75`. Le test ciblé `cargo test -p phoenix-app --lib icons::tests::every_name_has_fallback -- --exact` passe (1 réussi). Clippy et la suite complète ne sont pas vérifiés.
- `int-bin/REV` indique `e6f3b7a`; le binaire local ne contient donc pas le commit de fallbacks. Le test a compilé le crate dans `target/`.
- `docs/screenshots/` contenait quatre PNG non suivis au début de la reprise. Ils ont été préservés; `voice-settings.png` est maintenant suivi et intégré au README comme capture historique de développement. Les trois autres PNG restent non suivis; ils montrent un logo Discord et ne sont pas publiés.
- Le client officiel était disponible via CDP. Une capture temporaire à 1920×1050 a confirmé une vue de salon avec le panneau des membres; le contenu était privé et la capture a été supprimée. Cette vue personnalisée n’est pas utilisée comme référence de score.
- La sauvegarde initiale et son journal de test sont dans `/mnt/windata/project-phoenix/backups/20261124-112201Z/`. Une sauvegarde plus récente de `main` à `b22e6b1` est dans `/mnt/windata/project-phoenix/backups/20261124-113638Z/` (`main.bundle`, `worktree.patch`, `screenshots/`, `status.txt`). Elles ne contiennent aucun jeton.
- Le commit `3cf6524` complète le handoff avec la vérification ciblée et les limites de la capture CDP. La suite `cargo test -p phoenix-app --lib` passe: 211 réussis, 2 ignorés.
- Le commit `c479f54` câble l’option locale `send-btn` à un bouton Phoenix qui réutilise `submit_composer`, avec une icône SVG originale. Le bouton apparaît actuellement pour un brouillon non vide et autorisé. Ce critère reste à comparer au client officiel; aucun message n’a été envoyé pendant la validation.
- La dernière sauvegarde vérifiée est le `main` à `68856ec` dans `/mnt/windata/project-phoenix/backups/20261124-120131Z/` (`main.bundle`, `main-since-origin.patch`, `HEAD.txt`, état Git, log de test et copie des trois captures PNG non suivies). Le bundle a été vérifié; le log contient le résultat des 211 tests. Les commits documentaires ultérieurs doivent être inclus dans la prochaine sauvegarde.
- Reprise suivante : le client officiel est encore accessible en CDP sur la capture d’écran. Une capture visuelle temporaire 1920×1050 du compositeur a été inspectée puis supprimée. La préférence Send Message n’a pas été modifiée; le contrôle n’était pas visible dans cet état, ce qui ne permet pas d’inférer le rendu activé. Cette limite et la méthode à suivre figurent dans `docs/parity/composer.md`.
- La documentation Soundboard est enrichie à partir du guide officiel : le flux de haut niveau est ouvrir les contrôles vocaux, choisir un serveur disponible puis un son. L’interface et les endpoints Phoenix ne sont toujours pas mesurés; aucune action vocale ou lecture audio n’a été faite. Voir `docs/parity/pickers.md`.
- Le checkpoint précédent consignait les limites de référence composer/Soundboard. À ce point de reprise, la branche était en avance de dix commits sur `origin/main`, sans push, et `/home` avait environ 4 Go libres.
- Le lot suivant utilise `display_compact_avatars` uniquement avec l’affichage compact: avatar 40px à x=16px et en-tête déplacé à x=64px. La documentation Discord confirme que l’option est destinée au mode Compact; la capture officielle activée n’a pas été mesurée et ces dimensions restent donc provisoires. La suite de bibliothèque a compilé et passé: 211 réussis, 2 ignorés. Aucun réglage du compte réel n’a été modifié.
- L’exemple `screenshot` accepte maintenant `PHX_COMPACT=1` et `PHX_COMPACT_AVATARS=0|1`, uniquement sur `MockBackend`; voir `docs/PARITY_WORKFLOW.md`. Les deux PNG synthétiques ont été rendus en 2560×1600, inspectés puis supprimés de `/tmp`. L’état activé place l’avatar avant l’en-tête sans chevauchement visible dans le scénario mock; ce n’est pas une comparaison avec le client officiel activé.
- Le contrat public officiel de `@game` est maintenant noté dans `docs/parity/composer.md` et `docs/parity/settings-wiring.md` : saisie `@game`, recherche de jeux ayant un profil Discord, insertion d’un embed de jeu. La changelog du 11 août 2026 ne précise ni route de recherche, ni forme des suggestions, ni payload exact. Phoenix ne possède que `GET /games/{id}` pour des IDs déjà connus dans les widgets de profil; `ac-games` reste donc explicitement non câblé, sans détourner la liste d’activités comme faux catalogue.
- La piste Soundboard progresse : l’inspection ciblée du bundle local a établi le chargement des sons globaux par GET et des sons de serveur par requête Gateway en lecture seule, ainsi que les champs des entrées retournées. Voir `docs/parity/pickers.md` et `docs/handoffs/2026-11-24-game-autocomplete.md`. Le rendu du picker et l’envoi/playback restent à mesurer; aucune lecture audio n’a été déclenchée.
- Nouvelle lacune isolée côté chat : le client interprète `<sound:guildId:soundId>` comme une mention de son et affiche le nom résolu depuis le store Soundboard; Phoenix ne parse pas cette syntaxe et son modèle de message ignore `soundboard_sounds`. Voir `docs/parity/message-content.md`. Le rendu statique peut être implémenté et validé sur une fixture sans envoyer ni jouer de son.
- La sauvegarde préexistante `/mnt/windata/project-phoenix/backups/20261124-121028Z/` contient l’implémentation et le test, mais précède le harness synthétique et ses docs. Après le commit du harness, sauvegarder le nouvel état dans `/mnt/windata/project-phoenix/backups/20261124-1218Z/` (bundle, patch, log(s), status et copie des trois PNG non suivis).
- Le test `cargo test -p phoenix-app --lib` passe: 211 réussis, 2 ignorés. Le `cargo run -p phoenix-app --example screenshot` avec `PHX_COMPACT=1 PHX_COMPACT_AVATARS=0|1` compile et rend les deux captures mock 2560×1600; elles ont été visualisées puis supprimées. Il reste ~3,3 Go libres sur `/home`; ne pas lancer d’autre build sans recontrôle.
- Aucun intégrateur, build Cargo ou agent de la session précédente ne tournait pendant la vérification.

## Règle de contribution et méthode de référence

[`CONTRIBUTING.md`](../CONTRIBUTING.md) impose une approche clean-room : ne pas copier, traduire ni dériver de code des bundles Discord; ne pas ajouter leurs CSS, polices, traductions, sons, logos, icônes ou illustrations. Les observations du client peuvent guider une description de comportement, mais l’implémentation doit rester originale et s’appuyer sur la documentation publique, les réponses d’API autorisées et des captures de pixels.

Le client officiel est déjà accessible via le débogueur lié à `127.0.0.1:9222`. Le helper de référence est `/home/yohan/discord-phoenix-ref/cdp.mjs`. Pour une capture temporaire et visuelle seulement :

```sh
node /home/yohan/discord-phoenix-ref/cdp.mjs shot /tmp/phoenix-reference.png
```

Ne pas lancer `dump` ou `autodump.sh` : ils peuvent enregistrer du texte, des libellés ARIA, des noms et des captures privés dans le dépôt. Ne pas archiver une capture de compte réel ni la joindre au README, au changelog ou à un commit sans inspection et assainissement explicite. Pour les comparaisons publiques, utiliser en priorité les fixtures/données synthétiques et masquer tout identifiant ou contenu réel. Les captures de progression doivent porter cet avertissement, en majuscules, gras et souligné :

**<u>CES CAPTURES NE SONT PAS LES VERSIONS DÉFINITIVES.</u>**

La capture écran permet de mesurer le rendu visible. Ne pas extraire les styles calculés, les feuilles CSS ou les SVG du client pour les transformer en code Phoenix. Les hovers, états de focus et largeurs doivent être décrits à partir de ce qui est visiblement observé et reproduits avec du code original.

## Garde-fous persistants

- Sur le compte Discord réel : lecture seule. Aucun message, réaction, suivi, achat, réglage, connexion vocale, test micro, appel ou accusé de lecture volontaire.
- Les interactions CDP se limitent à l’observation nécessaire. Une navigation pouvant acquitter des messages ou changer un état n’est pas présumée sans effet : la tester sur les fixtures ou avec un état de démonstration.
- Ne jamais afficher ni écrire dans les logs : jeton, cookie, e-mail, identifiant de compte/salon/message, texte privé ou URL signée. Le jeton reste dans le trousseau système.
- Pas de GitHub Actions. Ne pas lancer l’ancien intégrateur automatique : `integrate-running.sh` pousse directement sur `main`, ce qui peut déclencher des Actions. Les validations restent locales tant qu’un push n’est pas explicitement demandé.
- `/home` était à 94 % (5,4 Go libres) pour une cible Cargo de 11 Go. Vérifier `df -h /home` avant chaque build; garder un seul build à la fois, utiliser quatre jobs maximum et une priorité basse. Ne pas lancer quatre compilations en parallèle.
- Compilation : partager `CARGO_TARGET_DIR=/home/yohan/discord-phoenix/target` et `LIBRARY_PATH=/home/yohan/.local/lib/devlinks`; ne pas définir `RUSTFLAGS` (cela invalide une grande partie du cache).
- Préserver les worktrees et PNG existants; avant un lot significatif, faire une sauvegarde datée vers `/mnt/windata/project-phoenix/backups/`.

## Progrès déjà présents sur `main`

La reprise du handoff a confirmé que plusieurs éléments mentionnés comme en cours ont depuis été intégrés : historique détaché pour les sauts vers d’anciens messages, surbrillance pulsée, barre de titre, icônes de contrôles vidéo, barre de scène, thèmes, tooltips et carte de prévisualisation du stream. Les commits récents comprennent `90bb675`, `e45e2c5`, `cb8fa6f` et `e6f3b7a`.

Depuis la référence initiale, `b50d800` a ajouté le handoff et le protocole, `2972f75` a ajouté deux icônes de secours, `b22e6b1` a documenté le résultat ciblé, `3cf6524` a complété les limites CDP, et `c479f54` a câblé la préférence Send Message. Le test ciblé de fallback et la suite bibliothèque passent. `/home` a environ 4,0 Go libres; vérifier avant toute compilation suivante et sérialiser les builds.

Ces intégrations ne signifient pas que toute la parité est atteinte. Les lignes des tableaux doivent être réévaluées avec leur preuve et leur état courant. Le tableau de bord de développement et `BACKLOG.md` contiennent des affectations d’agents périmées; aucun travailleur n’était actif à la reprise. Ne pas supposer qu’un label `CLAIMED` est encore pris en charge.

## Écarts prioritaires recensés

Audit en lecture seule du dépôt `e6f3b7a`, avec références code/doc :

1. **Soundboard inerte** — [`docs/parity/shortcuts.md`](parity/shortcuts.md) décrit le placeholder; `crates/phoenix-app/src/ui/call_view.rs:765-773` fournit un callback vide. C’est un manque fonctionnel réel. L’interface peut être construite avec données synthétiques; aucun test sur un vrai appel ou envoi audio.
2. **Réglages persistés sans effet** — [`docs/parity/settings-wiring.md`](parity/settings-wiring.md) identifie des contrôles stockés mais non consommés (badges non lus, démarrage, accélération matérielle, avatars compacts, saisie enrichie, autocomplétion de jeux et stickers). Séparer préférences locales, réglages synchronisés et consentements REST; valider les mutations uniquement sur un faux backend.
3. **Pages Nitro et Boutique absentes** — [`docs/parity/home.md`](parity/home.md) lignes 63–64 les marque comme overlays factices, sans dump de référence. Priorité au rendu catalogue/navigation; aucune opération d’achat.
4. **Actions de posts forum incomplètes** — [`docs/parity/forum.md`](parity/forum.md) ligne 26; `crates/phoenix-app/src/ui/chat/panels.rs:383-425` ne propose que copier le lien et remonter. Le suivi et les réactions demandent un modèle/API. Tester les rendus dans les mocks; ne pas suivre réellement un forum.
5. **Message vocal Alt+R placeholder** — [`docs/parity/shortcuts.md`](parity/shortcuts.md) ligne 46. Le cycle capture/encodage/envoi est un sous-projet distinct et n’est pas à activer pendant cette campagne.
6. **Tuile GIF trending blanche sur données réelles** — [`docs/parity/pickers.md`](parity/pickers.md) ligne 19. L’échec visuel est observé, sa cause ne l’est pas. Reproduire le type d’URL/format avec fixtures et logs expurgés avant toute correction.

Les lignes « non mesuré faute de données » ou « aucun élément dans le dump » restent `⚠`/non vérifiées; ne pas les présenter comme bugs confirmés.

## Branches et intégration héritées

Les fichiers `.result` de `/home/yohan/.local/share/phoenix-queue/` indiquaient les derniers états suivants; ces branches ont pu évoluer depuis :

- Conflits lors du cherry-pick : `wt/composer-real`, `wt/forum2`, `wt/pickers`, `wt/setwire2`, `wt/typing`.
- Échecs d’intégration de `wt/members26` et `wt/msgcol`. Le log `bisect-111444.log` montre pour `wt/msgcol` que `icons::tests::every_name_has_fallback` échouait pour `viewer/zoom`; le fallback local ajouté ici n’a pas encore été testé.
- Les claims du backlog ne sont pas une file active. Rafraîchir chaque branche depuis `origin/main` et comparer son diff avant toute reprise. Les anciens worktrees sont nombreux; ne pas supprimer à l’aveugle.

## Ordre de reprise conseillé

1. Relire ce fichier, [`docs/PARITY_WORKFLOW.md`](PARITY_WORKFLOW.md), `CONTRIBUTING.md` et les consignes de branche; vérifier `git status`, `git worktree list`, le disque et les processus actifs.
2. La vérification ciblée du fallback et la suite bibliothèque ont passé (`211 passed; 2 ignored`). Ne pas relancer ces builds immédiatement : `/home` est presque plein.
3. Mesurer le problème GIF sur une fixture expurgée, ou choisir un lot indépendant à partir des six priorités ci-dessus. Un lot = un périmètre/fichier ou worktree clairement réservé.
4. Obtenir les captures de référence via observation visuelle read-only. Comparer mêmes dimensions, densité, thème, locale, état et données; garder toute capture réelle dans `/tmp` et la supprimer après mesure si elle n’est plus nécessaire.
5. Faire un seul build/clippy/test d’intégration, après les modifications d’un lot. Mettre à jour les tableaux avec le résultat exact; commit local régulier avec un message descriptif, sans préfixes `feat:`/`fix:`.
6. Actualiser ce handoff, la sauvegarde et le statut du goal après chaque étape importante. Ne pas pousser ni publier sans instruction correspondante.

## Sauvegarde et reprise

La sauvegarde la plus récente est `/mnt/windata/project-phoenix/backups/20261124-120000Z/`. Le bundle restaure la branche locale, le patch `main-since-origin.patch` contient les cinq commits locaux au-dessus de `origin/main`, et `screenshots/` préserve les trois captures non suivies. Le log `phoenix-app-lib-test-retry.log` correspond à `c479f54`.

## 2026-11-24 — Soundboard chat mention continuation

- Added an initial static `<sound:guildId:soundId>` parser/resolver path using message-local `soundboard_sounds` metadata, including optional emoji; unresolved tokens stay literal. The regular chat and off-thread history-page renderers are wired, and a synthetic showcase fixture was added.
- Official local bundle evidence confirms the known sound label/emoji presentation and a click path that depends on voice/channel context. Playback is intentionally not connected. Exact inline styling, hover/focus states, and unresolved visual treatment remain unmeasured; the current chip colors/weight are provisional.
- Updated `docs/parity/message-content.md`; detailed next steps and validation limits are in `docs/handoffs/2026-11-24-soundboard-mentions.md`.
- At checkpoint: branch `main` was 16 commits ahead of origin; seven source files are modified and the pre-existing three untracked screenshots remain. `/home` had 3.3 GB free and load average was 5.34/3.33/2.99. No compilation/test has been run for this slice; check disk/CPU before any single serialized build.
