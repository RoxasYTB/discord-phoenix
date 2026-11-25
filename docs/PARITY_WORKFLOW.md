# Protocole de parité visuelle et fonctionnelle

Ce protocole garde les comparaisons reproductibles pour atteindre la parité 1:1 décrite dans [`CONTRIBUTING.md`](../CONTRIBUTING.md). Pour les détails sur l’état actuel du projet, voir [`PROJECT_HANDOFF.md`](PROJECT_HANDOFF.md).

## 1. Définir une observation

Avant de toucher au code, consigner :

- la route/vue et l’état (repos, hover, focus, menu ouvert, sélection, chargement, vide, erreur);
- le résultat que l’utilisateur doit voir ou obtenir;
- la source de la référence (documentation publique, résultat d’un endpoint de lecture, capture visuelle du client ouvert, ou fixture synthétique);
- la dimension de fenêtre, échelle/zoom, thème, langue et densité;
- la date, le commit Phoenix et les limites de la capture (animations, données manquantes, état de compte différent);
- si le comportement appelle un endpoint, sa méthode HTTP et les effets secondaires possibles.

Une capture seule ne prouve pas que le comportement fonctionne. Un mock seul ne prouve pas que l’API réelle fournit les mêmes données. Garder ces deux niveaux séparés.

## 2. Référence officielle via CDP

Le client officiel est déjà ouvert et le port de débogage est lié à `127.0.0.1:9222`. Le script disponible est `/home/yohan/discord-phoenix-ref/cdp.mjs`.

Pour une comparaison visuelle ponctuelle, produire une capture temporaire :

```sh
node /home/yohan/discord-phoenix-ref/cdp.mjs shot /tmp/phoenix-reference.png
```

Inspecter localement puis noter seulement les mesures nécessaires (dimensions visibles, alignements, états, texte générique si non privé). Ne pas capturer le corps complet de la page, ne pas exporter les dumps DOM/ARIA, ne pas enregistrer les URL privées ou signées. Une capture de l’interface réelle peut exposer conversations, noms, serveurs, notifications ou recherche : elle reste temporaire et ne va ni dans Git, ni dans `docs/`, ni dans le changelog.

Le CSS, les styles calculés, les SVG, les polices, les traductions et le comportement du JS du client sont la référence : les valeurs mesurées vont dans le code avec leur source (règle CSS, dump, mesure en direct). Les assets binaires se chargent au runtime depuis l’installation Discord locale et ne sont jamais committés. Seules restrictions qui restent : la confidentialité (aucune donnée de compte réel dans git) et aucune écriture sur le compte en dehors du salon de test autorisé.

Pour vérifier un état sans toucher au compte réel, l’exemple `screenshot` utilise le `MockBackend` :

```sh
PHX_COMPACT=1 PHX_COMPACT_AVATARS=1 cargo run -p phoenix-app --example screenshot -- /tmp/phoenix-compact-avatars.png
PHX_COMPACT=1 PHX_COMPACT_AVATARS=0 cargo run -p phoenix-app --example screenshot -- /tmp/phoenix-compact-no-avatars.png
```

Ces variables ne changent que les préférences en mémoire de la capture synthétique. La sortie n’est pas une référence officielle et ne doit pas être utilisée pour fermer une mesure de parité sans comparaison séparée avec la capture CDP correspondante.

## 3. Référence API

- Privilégier les endpoints documentés et les requêtes GET de lecture de l’utilisateur autorisé.
- Vérifier la méthode avant d’exécuter une requête. `POST`, `PATCH`, `PUT` et `DELETE` sont des mutations, même si le nom semble anodin.
- Vérifier si un `GET` a un effet annexe (ack de lecture, présence, compteur) avant de le considérer sans effet. Utiliser les exemples hors écran qui annoncent explicitement l’absence d’ack lorsque c’est applicable.
- Expurger les corps et journaux avant partage. Ne jamais committer de tokens, cookies, IDs privés, messages réels ou URLs signées.
- L’inventaire `docs/PARITY.md` n’est pas une liste de fonctionnalités toutes promises. Relier chaque endpoint à une vue/action utile, une preuve et son effet secondaire avant de l’implémenter.

## 4. Dimensions, hovers et états

Créer une matrice par composant. Mesurer des tailles de fenêtre qui couvrent le nominal et les seuils observés; ne pas extrapoler un breakpoint à partir d’un seul rendu.

| Axe | Cas à relever |
|---|---|
| Largeur | nominale, proche d’un seuil observé, plus étroite, plus large |
| Hauteur | nominale, faible hauteur, contenu défilé |
| Zoom/densité | valeur par défaut, une étape basse et une étape haute prise en charge |
| Interaction | repos, survol, pression, focus clavier, menu ouvert, fermeture |
| Données | vide, une entrée, plusieurs entrées, contenu long, erreur/chargement |
| Thème/langue | valeurs identiques sur les deux clients; noter séparément les cas non mesurés |

Les réglages du compte officiel ne sont jamais modifiés pendant les comparaisons. Vérifier les contrôles de réglage dans Phoenix sur une configuration de démo/mocks ou par test unitaire; une différence de rendu ne justifie pas de sauvegarder un réglage réel.

## 5. Comparaison des images

Pour chaque paire Phoenix/référence :

1. même état et même contenu synthétique, ou documenter précisément la différence de données;
2. mêmes dimensions physiques et device scale factor; même thème, locale, zoom et densité;
3. attendre les médias et polices, mais éviter de capturer au milieu d’une animation;
4. vérifier visuellement l’alignement, puis produire une différence de pixels avec le script adapté, par exemple `scripts/compare/compare.py`;
5. interpréter le score : une petite différence globale peut masquer une grosse erreur dans une petite zone; compléter par des crops et des mesures locales;
6. conserver le PNG public uniquement s’il est synthétique/assaini et utile à comprendre l’avancement.

Les noms, messages, URLs et autres contenus réels n’apparaissent jamais dans une image publique. Chaque capture de progrès affichée dans le README doit porter :

**<u>THESE SCREENSHOTS ARE NOT FINAL VERSIONS.</u>**

## 6. Triage et état des lignes

- `✅` : implémenté et vérifié avec une preuve citée (endpoint public, observation visuelle documentée, test ou comparaison reproductible).
- `⚠` : implémentation présente mais comparaison partielle, état absent de la capture, donnée indisponible, ou mesure pas encore confirmée.
- `❌` : fonction ou rendu clairement absent/cassé dans un scénario reproductible.
- « Donnée absente », « pas vu dans le dump » et « pas de compte avec cette fonctionnalité » ne suffisent pas à conclure `❌`.
- Un bug réel requiert un scénario reproduisible et une preuve (erreur réseau expurgée, état attendu/observé ou rendu comparé).
- Après une correction, noter les cas vérifiés, ceux encore ouverts et le commit exact. Ne pas élargir artificiellement le score en reclassant une ligne sans preuve.

## 7. Lots parallèles et ressources

- Distribuer des lots dont les fichiers, routes et écrans ne se recouvrent pas. Avant de déléguer, réserver un périmètre par branche/worktree et donner les contraintes de sécurité et de test.
- Les agents peuvent inspecter ou implémenter en parallèle, mais un seul intégrateur fait les builds. Aucun agent n’exécute Cargo en parallèle.
- Un lot finit par une revue du diff, un test ciblé et une mise à jour du tableau. Ensuite l’intégrateur regroupe les lots et lance une seule compilation complète/clippy/tests.
- `CARGO_BUILD_JOBS=4` maximum, priorité basse, cache partagé sous `target/`, `LIBRARY_PATH=/home/yohan/.local/lib/devlinks`, jamais de `RUSTFLAGS`.
- Vérifier `df -h /home` avant un build et surveiller l’espace utilisé par `target/`. Garder au moins assez de marge pour les objets temporaires et le linker. Ne pas supprimer des branches/worktrees ou caches non identifiés pour gagner de la place.
- Ne pas utiliser l’ancien intégrateur qui pousse automatiquement à `origin/main`; pas de GitHub Actions dans cette campagne. Commits locaux fréquents autorisés, push séparé seulement après demande.

## 8. Liste de contrôle d’un handoff

À la fin de chaque session, mettre à jour :

- date/commit/branche et sorties Git (`status`, commits propres, worktrees actifs);
- fichiers modifiés, statut des commits, sauvegarde créée et lieu;
- ce qui a été testé réellement, commandes et résultats; compilations non lancées et raison;
- preuves visuelles/API, chemin local et statut privé/synthétique;
- lignes de parité fermées/ouvertes avec références;
- prochain lot précis, fichiers concernés, dépendances et effets secondaires à éviter;
- espace disque, processus/build actif et consigne de ne pas en lancer un second;
- état du goal et conditions pour le terminer.

## 9. `real_shots` : survol et réglages en mémoire

- `PHX_MOUSE=x,y` envoie un déplacement de souris (px logiques) avant la capture. Le pilote de balayage place la souris du client en `W-2,H-2` (dans la liste des membres à 1280×720) : passer la même position pour comparer les états de survol, par exemple la scrollbar fine `thin fade`.
- `PHX_PROTO=id=valeur,...` force des réglages synchronisés (ids de `ui/settings/synced_map.rs`) en mémoire seulement, par exemple `PHX_PROTO=alt-text=1` pour le badge ALT. Rien n'est envoyé au compte.
- `PHX_LOCAL=id=valeur,...` fait de même pour les préférences locales (`LocalPrefs::ui`), par exemple `PHX_LOCAL=streamer=1` pour le bandeau du mode streamer ou `PHX_LOCAL=indicators=1` pour les indicateurs d'interrupteur. Rien n'est écrit dans `settings.json`.
