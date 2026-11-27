# Correcteur orthographique

Le client officiel délègue la vérification au correcteur de Chromium (Electron, Hunspell) : `spellCheck` natif côté bureau, store `SpellcheckStore` côté web. Phoenix reproduit le comportement avec la crate `spellbook` (Rust pur, compatible Hunspell), module `crates/phoenix-app/src/spell/`.

| Élément | État | Source |
|---|---|---|
| Soulignement des mots inconnus (composer et édition d'un message) | ✅ | Capture CDP d'un `contenteditable` hors salon (aucune frappe dans le vrai composer), 27/11 : vaguelette `#FF0000` pur, ~1 px d'épaisseur, période ~7,2 px, crête à crête ~4,3 px, de baseline+0,4 à baseline+5,6 px (px CSS de la page). `rich_input::spell_style`. |
| Forme exacte de la vaguelette | ❌ partiel | Le shader GPUI (`gpui-pre-wgpu/src/shaders.wgsl`, `WAVE_FREQUENCY = 2`, `WAVE_HEIGHT_RATIO = 0.8`, hauteur = 3 × épaisseur) lie période et amplitude à l'épaisseur : avec 1 px on obtient une période de 9 px et une amplitude plus faible que Chromium. Pas de patch du vendor. |
| Mot en cours de frappe non vérifié | ✅ | Chromium ne vérifie un mot qu'après un séparateur : une insertion `Input.insertText` sans espace final ne souligne rien, même après 4 s. Phoenix saute le mot qui finit au curseur. |
| Menu clic droit : suggestions (5 max), « Ajouter au dictionnaire », « Enlever du dictionnaire », « Correcteur », « Langues » | ✅ | `c8f2abdd6320aee4.js` module `413756` (suggestions `correction-N`, `add-to-dictionary` `HJmG1G`, `remove-from-dictionary` `xXqIX0` affiché seulement sans mot faux et si le texte contient un mot appris, `spellcheck` `TKkotf`, `languages` `OlOHDH` = article d'aide `360000877191`) ; groupes de `194400` (`14ae1b9e3d94518f.js`) : [suggestions] / [ajout, correcteur, langues] / [couper, copier, coller]. Limite 5 : `getCachedMisspelling().corrections.slice(0, 5)` (`web.b63d846b329d6470.js`). |
| Remplacement | ✅ | Comme le client : on remplace l'occurrence du mot la plus proche du curseur (regex `(?<![\p{L}\p{N}_])mot(?![\p{L}\p{N}_])`), puis le focus revient au champ. |
| Dictionnaire perso | ✅ | `SpellcheckStore` (module `638128`) garde `{enabled, learnedWords}` en local, mots en minuscules (`toLocaleLowerCase`). Phoenix : `<config>/spellcheck.json` `{"learnedWords": [...]}` ; l'interrupteur reste la pref locale `spellcheck` (défaut vrai). |
| Langues | ✅ | `applyLanguages` : `[locale de l'app, ...navigator.languages]`, résolues sur les dictionnaires disponibles par code exact puis par langue. Phoenix : locale de l'app puis `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG` ; un mot est juste s'il l'est dans une des langues (multilingue). |
| Réglage dans Paramètres > Langue | — | N'existe pas dans le client 2026 : aucune chaîne « orthographe » hors menu (`fr.js`, `en2.js`) ; l'interrupteur et « Langues » vivent dans le menu du champ. Non ajouté (politique 1:1). |
| Exclusions | ✅ | Blocs et segments de code, liens et URL, mentions `<@id>`, `<@&id>`, `<#id>`, `@nom`, `#salon`, emoji `<:nom:id>` et `:nom:` (y compris `:nom` en cours de saisie), commande slash en tête, mots avec chiffres ou `_`. `spell/words.rs`. |

## Performance
- La vérification et les suggestions tournent sur le `background_executor` de GPUI ; le thread UI ne fait que découper le texte et lire un cache mot → résultat.
- Vérification incrémentale : seuls les mots absents du cache partent en tâche de fond ; à la fin, un compteur de génération invalide les décorations et `refresh_windows` redessine.
- Les dictionnaires sont chargés une fois, en tâche de fond, à la première vérification (et rechargés si la langue de l'app change). Un même fichier pointé par plusieurs liens (`fr_FR.dic -> fr.dic`) n'est chargé qu'une fois.

## Dictionnaires (aucun n'est committé)
Recherche dans l'ordre, premier `<code>.aff` + `<code>.dic` trouvé par code :
1. `<config Phoenix>/dictionaries` (tous les OS : déposer ici des fichiers Hunspell) ;
2. `$DICPATH` (liste de dossiers) ;
3. Linux : `~/.local/share/hunspell`, `/usr/share/hunspell`, `/usr/share/myspell`, `/usr/share/myspell/dicts`, `/usr/local/share/hunspell`, `/app/share/hunspell` (Flatpak) ;
   macOS : `~/Library/Spelling`, `/Library/Spelling`, `/opt/homebrew/share/hunspell`, `/usr/local/share/hunspell` ;
   Windows : `%LOCALAPPDATA%\hunspell`, `%ProgramData%\hunspell`.
Sans dictionnaire, rien n'est souligné (le menu garde « Correcteur » et « Langues »). Encodages acceptés : UTF-8, ISO8859-1, ISO8859-15 ; les autres sont ignorés avec un log. Les `.bdic` téléchargés par Chromium ne sont pas au format Hunspell et ne sont pas utilisés.

## Tests
`spell::tests` (dictionnaire minuscule écrit dans un dossier temporaire), `extra_menus::tests::textarea_menu_follows_the_client_order`, `spell_target_reads_the_word_under_the_pointer`. En test, seuls les dictionnaires du dossier de config sandboxé sont lus.
