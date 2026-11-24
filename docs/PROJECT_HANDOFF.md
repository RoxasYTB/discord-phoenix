# Project Phoenix — état et handoff

> Dernière vérification de cette note : 2026-11-24. Référence locale : `main` à `e6f3b7a`.

Ce document est le point de reprise pour les sessions qui poursuivent la parité visuelle et fonctionnelle de Discord Phoenix. Il complète les tableaux par fonctionnalité dans [`docs/parity/`](parity/) et l’inventaire des routes dans [`docs/PARITY.md`](PARITY.md). Il ne remplace pas les preuves d’exécution ni les tests.

## Objectif

Construire un client natif Rust + GPUI qui reproduit les comportements utiles et les états visibles du client Discord officiel, en s’appuyant sur les endpoints et une comparaison visuelle du client officiel déjà ouvert en CDP. Les claims doivent être vérifiés sur le flux réel ou sur des données de démonstration clairement identifiées, avec des écarts documentés plutôt que masqués.

## État du dépôt au 2026-11-24

- `main` est synchronisé avec `origin/main` à `e6f3b7a` après avance rapide de 301 commits.
- Le dernier état de production n’a pas été compilé dans cette reprise.
- Modification locale non commitée : fallbacks SVG `icons/viewer/zoom.svg` et `icons/viewer/open.svg` dans `crates/phoenix-app/src/assets.rs`. Elle répond au test d’intégration isolé du lot lightbox, qui échouait sur `no fallback for viewer/zoom`. La correction est **non vérifiée par compilation/test**.
- `docs/screenshots/` contenait quatre PNG non suivis au début de la reprise. Ils ont été préservés. `voice-settings.png` est une ancienne capture de développement sans données de compte; les autres captures ont un logo Discord visible. Ne rien publier avant contrôle de conformité aux règles ci-dessous.
- Une sauvegarde du commit, du diff et des PNG existants se trouve dans `/mnt/windata/project-phoenix/backups/20261124-112201Z/` (`main.bundle`, `worktree.patch`, `screenshots/`, `status.txt`). Elle ne contient aucun jeton.
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
2. Faire une vérification ciblée, sérialisée, du fallback ajouté (`cargo test -p phoenix-app --lib icons::tests::every_name_has_fallback`) si l’espace disque et le cache le permettent. Sinon documenter le blocage et ne pas déclarer la correction validée.
3. Mesurer le problème GIF sur une fixture expurgée, ou choisir un lot indépendant à partir des six priorités ci-dessus. Un lot = un périmètre/fichier ou worktree clairement réservé.
4. Obtenir les captures de référence via observation visuelle read-only. Comparer mêmes dimensions, densité, thème, locale, état et données; garder toute capture réelle dans `/tmp` et la supprimer après mesure si elle n’est plus nécessaire.
5. Faire un seul build/clippy/test d’intégration, après les modifications d’un lot. Mettre à jour les tableaux avec le résultat exact; commit local régulier avec un message descriptif, sans préfixes `feat:`/`fix:`.
6. Actualiser ce handoff, la sauvegarde et le statut du goal après chaque étape importante. Ne pas pousser ni publier sans instruction correspondante.

## Sauvegarde et reprise

La sauvegarde créée pendant cette reprise : `/mnt/windata/project-phoenix/backups/20261124-112201Z/`. Le bundle restaure `main`; le patch restaure les modifications suivies de l’arbre; le sous-dossier `screenshots/` préserve les quatre PNG non suivis. Après restauration du bundle, appliquer le patch depuis la racine du dépôt et recopier les PNG uniquement si nécessaire.
