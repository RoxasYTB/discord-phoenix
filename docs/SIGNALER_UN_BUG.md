# Signaler un bug

*English version: [REPORTING_BUGS.md](REPORTING_BUGS.md).*

Discord Phoenix est en bêta : tes signalements sont ce qui aide le plus. Un bon signalement permet à un mainteneur de voir le bug sur sa propre machine en quelques minutes. Cette page explique quoi récupérer, comment, et ce qu'il ne faut **jamais** publier.

Ouvre un signalement depuis [New issue](https://github.com/RoxasYTB/project-phoenix/issues/new/choose) et choisis le bon formulaire :

| Formulaire | Pour |
|---|---|
| 🐞 Signaler un bug | Quelque chose ne marche pas |
| 🎨 Bug visuel ou différence avec le client officiel | Quelque chose n'a pas le même rendu que Discord : taille, couleur, espacement, police, icône, animation |
| 💥 Plantage ou blocage | Phoenix se ferme, se fige, ne démarre pas ou affiche une fenêtre noire |
| 🔊 Vocal ou vidéo | Son, micro, caméra, partage d'écran, soundboard |
| ✨ Demande de fonction | Une fonction du client officiel qui manque dans Phoenix, ou une idée |

Les formulaires sont bilingues : tu peux les remplir en français.

Pour les questions et l'aide, passe par le [serveur Discord du projet](https://discord.gg/uF3wtckHkN) (en anglais, le français reste bienvenu). Les problèmes de sécurité, surtout tout ce qui pourrait faire fuiter un token, passent par le [signalement privé](https://github.com/RoxasYTB/project-phoenix/security/advisories/new), jamais par une issue publique.

## 1. Trouver ta version

Ouvre les **Paramètres** (la roue dentée à côté de ton nom). Le bas de la barre de gauche affiche deux lignes :

```
stable 471091 • 1.0.9215 x64
Discord Phoenix 0.1.0-beta.1 (abcd1234)
```

La première ligne est le build Discord sous lequel Phoenix se présente. La seconde est la version de Phoenix et le commit dont elle vient. Copie la seconde dans le champ **Version de Phoenix**.

Avant de signaler, vérifie que tu as la dernière [version publiée](https://github.com/RoxasYTB/project-phoenix/releases) : ton bug est peut-être déjà corrigé.

## 2. Copier les infos de débogage

Sous la version, clique sur **Copier les infos de débogage**. Le lien devient **Copié !** et ton presse-papiers contient par exemple :

```
Discord Phoenix 0.1.0-beta.1 (abcd1234)
OS: Debian GNU/Linux 13 (trixie), kernel 6.12.48 (x86_64)
Session: Wayland (KDE)
GPU: NVIDIA GeForce GTX 1080 | driver: NVIDIA 580.95.05
Renderer: wgpu | hardware acceleration: on
Language: fr
Zoom: 100% | font: 16px | density: default
Theme: dark
Log level: info
```

Colle-le tel quel dans le champ **Infos de débogage**. Il ne contient aucune donnée de compte : ni pseudo, ni identifiant, ni serveur, ni token.

Si Phoenix ne va pas jusqu'aux Paramètres, remplis à la main les champs OS, GPU et pilote.

## 3. Récupérer les logs

Phoenix écrit son journal de débogage dans un fichier. Clique sur **Ouvrir le dossier des logs** (à côté de **Copier les infos de débogage**) ou vas-y toi-même :

| OS | Dossier |
|---|---|
| Linux | `~/.cache/discord-phoenix/logs/` (ou `$XDG_CACHE_HOME/discord-phoenix/logs/`) |
| Windows | `%LOCALAPPDATA%\discord-phoenix\logs\` |
| macOS | `~/.cache/discord-phoenix/logs/` |

Le fichier courant est `phoenix.log`. À 5 Mo, il devient `phoenix.log.1`, puis `phoenix.log.2` ; les plus anciens sont supprimés. Colle les 200 dernières lignes environ, à partir d'un peu avant le bug. Après un plantage suivi d'un redémarrage, les lignes utiles sont peut-être à la fin de `phoenix.log.1`.

Phoenix masque les tokens, cookies, en-têtes d'autorisation, e-mails et textes de messages avant d'écrire quoi que ce soit. **Relis quand même ce que tu colles** et retire ce que tu ne veux pas rendre public : noms de serveurs ou de salons, noms de fichiers, identifiants.

### Plus de détails : lancer depuis un terminal

Pour un plantage, une fenêtre noire, ou quand on te demande plus de détails, lance Phoenix depuis un terminal. Tu obtiens alors le journal, le message de panique et la pile d'appels :

**Linux**

```sh
RUST_LOG=debug RUST_BACKTRACE=1 discord-phoenix 2>&1 | tee ~/phoenix-run.txt
```

Avec l'AppImage : `RUST_LOG=debug RUST_BACKTRACE=1 ./discord-phoenix-*.AppImage 2>&1 | tee ~/phoenix-run.txt`.

**Windows** (PowerShell, dans le dossier qui contient `discord-phoenix.exe`)

```powershell
$env:RUST_LOG="debug"; $env:RUST_BACKTRACE="1"; .\discord-phoenix.exe 2>&1 | Tee-Object $HOME\phoenix-run.txt
```

**macOS**

```sh
RUST_LOG=debug RUST_BACKTRACE=1 "/Applications/Discord Phoenix.app/Contents/MacOS/discord-phoenix" 2>&1 | tee ~/phoenix-run.txt
```

`RUST_LOG` règle le niveau : `info` (par défaut), `debug`, ou `trace` pour un seul module (`RUST_LOG=info,phoenix_voice=trace`). `trace` partout produit énormément de lignes : ne l'utilise que si on te le demande.

## 4. Captures et vidéos

Une capture est **obligatoire** pour tout bug visuel, et une courte vidéo est le meilleur moyen de montrer une animation, un clignotement ou un bug en plusieurs étapes.

| OS | Capture | Vidéo |
|---|---|---|
| Linux, KDE | **Spectacle** (`Impr. écran`, ou `Meta+Maj+S` pour une zone) | Spectacle, mode *Enregistrer*, ou OBS Studio |
| Linux, GNOME | `Impr. écran` ouvre l'outil de capture | Le même outil, bouton vidéo (`Ctrl+Alt+Maj+R` sur les anciennes versions) |
| Windows | `Win+Maj+S` (Outil Capture) | Outil Capture, *Enregistrer*, ou `Win+Alt+R` (Xbox Game Bar) |
| macOS | `Cmd+Maj+4` (zone), `Cmd+Maj+4` puis `Espace` (fenêtre) | `Cmd+Maj+5`, *Enregistrer la partie sélectionnée* |

Conseils :

- Capture toute la fenêtre de Phoenix, pas seulement un morceau, sauf si le détail est minuscule ; ajoute alors aussi un zoom recadré.
- Garde l'échelle de l'OS et le zoom de Phoenix que tu avais pendant le bug, et indique-les.
- Vidéos : 10 à 30 secondes, en MP4 ou WebM, moins de 10 Mo pour que GitHub les accepte. Survole lentement ce qui compte.

### Flouter

Avant d'envoyer, **floute ou masque** chaque pseudo, avatar, message, e-mail, numéro de téléphone, ainsi que le nom et l'icône de tout serveur privé. Les noms de serveurs dans ta barre latérale comptent aussi.

- **Spectacle** : *Annoter*, puis l'outil de flou ou de pixellisation. **Outil Capture de Windows** : *Modifier*, puis une forme opaque (le surligneur ne suffit pas). **Aperçu sur macOS** : *Outils › Annoter › Rectangle*, rempli en noir.
- Utilise des rectangles opaques ou une forte pixellisation : un léger flou sur du texte peut parfois se relire.
- Les vidéos sont difficiles à flouter : enregistre plutôt sur un serveur de test ou un salon sans rien de privé.

## 5. Comparer avec le client officiel

Phoenix vise un rendu et un comportement identiques au client officiel 2026 : le signalement le plus utile est souvent « Discord fait X, Phoenix fait Y ».

1. Ouvre le client Discord officiel et Phoenix avec **la même taille de fenêtre**. Sous KDE, *Plus d'actions › Configurer les paramètres spéciaux de la fenêtre* permet de fixer une taille exacte ; sous Windows et macOS, redimensionne les deux à la main et vérifie la taille sur les captures.
2. Mets **le même zoom** : `Ctrl+0` remet le client officiel à 100 % ; vérifie celui de Phoenix dans *Paramètres › Apparence*.
3. Utilise le même thème, le même espacement (*Apparence › Espacement des messages*) et la même langue.
4. Ouvre la même vue dans les deux, dans le même état (survol, menu ouvert, salon sélectionné).
5. Prends une capture de chacun et mets-les **côte à côte : Phoenix à gauche, Discord à droite**. Si tu ne sais pas les assembler, joins les deux images.
6. Dans le formulaire, indique si le client officiel fait pareil.

Si le client officiel a le même bug, c'est un bug de Discord, pas de Phoenix, et Phoenix le reproduira.

## 6. Ce qu'il ne faut JAMAIS publier

Rien de tout ceci n'a sa place dans une issue, un commentaire, une capture ou un message Discord, même « juste pour cette fois » :

- **Ton token Discord**, ou quoi que ce soit venant du `localStorage`, du trousseau ou du Gestionnaire d'identification. Avec lui, n'importe qui contrôle ton compte. Personne du projet ne te le demandera jamais.
- **`journal.log`** (dans `~/.cache/discord-phoenix/session/`). C'est une copie locale de tes **messages**, pas un journal de débogage. Les seuls logs de débogage sont `phoenix.log` et ses copies dans `logs/`.
- Les **fichiers HAR** ou exports réseau des DevTools : ils contiennent ton token et tes cookies.
- Les **cookies** et en-têtes de requête (`Authorization`, `Cookie`, `X-Super-Properties`).
- Ton adresse e-mail, ton numéro de téléphone, tes moyens de paiement, et les messages ou profils d'autres personnes.

Si tu as publié l'un de ces éléments par erreur, supprime le commentaire ou modifie l'issue tout de suite, puis **change ton mot de passe Discord** : cela invalide tous les tokens existants.

## 7. Exemple de bon signalement

> **[Visual] La barre d'outils au survol d'un message est 2 px plus bas que dans le client officiel**
>
> **Résumé** : la barre qui apparaît au survol d'un message est 2 px plus bas et déborde sur le message suivant.
>
> **Zone** : Messages
>
> **Étapes pour reproduire**
> 1. Ouvrir n'importe quel salon textuel de serveur.
> 2. Survoler le deuxième message d'un groupe (pas le premier).
> 3. Regarder la barre d'outils à droite.
>
> **Attendu** : le haut de la barre est 16 px au-dessus du message, comme dans le client officiel.
> **Obtenu** : il est 14 px au-dessus et recouvre le haut du message suivant.
>
> **Fréquence** : toujours · **Régression** : ça marchait dans 0.1.0-beta.1 (1a2b3c4d) · **Client officiel** : fonctionne
>
> **Captures** : `phoenix-vs-discord.png` (Phoenix à gauche, Discord à droite, les deux en 1280×800 à 100 %, pseudos floutés)
>
> **Infos de débogage**
> ```
> Discord Phoenix 0.1.0-beta.2 (9f8e7d6c)
> OS: Fedora Linux 43 (KDE Plasma), kernel 6.17.4 (x86_64)
> Session: Wayland (KDE)
> GPU: AMD Radeon RX 6700 XT (RADV NAVI22) | driver: radv Mesa 25.2.4
> Renderer: wgpu | hardware acceleration: on
> Language: fr
> Zoom: 100% | font: 16px | density: default
> Theme: dark
> Log level: info
> ```
>
> **Écran** : échelle 150 %, zoom 100 %, 2560×1440 · **Thème** : sombre

Un bug par issue. Si tu en trouves plusieurs, ouvre plusieurs issues : c'est plus simple à suivre et à fermer.

## Pour les mainteneurs : le tri

Les labels, la commande pour les créer, le cycle de tri et les priorités sont décrits dans la [version anglaise](REPORTING_BUGS.md#for-maintainers-triage). En résumé :

1. Lire sous quelques jours et retirer tout de suite toute donnée privée (token, cookie, HAR, `journal.log`, e-mail, capture non floutée).
2. Fermer les doublons avec `duplicate` et un lien vers l'original.
3. Demander ce qui manque et garder `needs-repro` ; fermer après 14 jours sans réponse.
4. Reproduire, puis remplacer `needs-repro` par `confirmed` ; ajouter `regression` si une version précédente marchait.
5. Pour `visual` et `parity` : mesurer le client officiel et ajouter la ligne dans le tableau de [docs/parity/](parity/), avec le lien de l'issue.
6. Priorité : `priority: high` (plantage, connexion, envoi de messages, vocal cassé, fuite de données), `priority: medium` (bug visible avec contournement), `priority: low` (cosmétique ou rare).
7. Référencer l'issue dans le commit (`Fixes #123`) et demander confirmation au rapporteur à la version suivante.
