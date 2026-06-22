# Extension — Toolchain (spec)

Cockpit de dev dans l'île : ouvrir les IDE, gérer Git, et lancer/superviser les process
(`pnpm dev`, `artisan serve`…) du **projet courant** — avec de vrais terminaux.

Tout vit dans **une seule view** avec un **système d'onglets** (au niveau du composant) :
**IDE · Git · Process**.

## Pré-requis hôte (nouveaux)
- **Service `pty`** (en cours) : terminaux interactifs (ConPTY/portable-pty) + `exec`
  one-shot (capture stdout). Permission **`terminal`** (confiance max). → process & git.
- **Extension VSCode compagnon** : pousse le **contexte projet** (path, scripts,
  fichiers/terminaux ouverts) via `island.serve`, et reçoit des commandes (ouvrir,
  lancer une task). C'est la seule façon fiable de connaître « quel projet/IDE est
  focus » parmi plusieurs fenêtres. Fallback léger : parser le **titre de la fenêtre
  au premier plan** (`island.windows`) pour le nom du projet focus.

## Onglet 1 — Launcher IDE
- Liste de projets ; clic → ouvre l'IDE : `code <path>` (Windows) **ou** `code` côté
  **WSL** (`code` depuis `\\wsl$\…` / `wsl code .`). **Gérer les deux cas** (chemin
  Windows vs distro WSL) — détecter via le path.
- **Pastille verte quand l'IDE est ouvert** sur ce projet → via **`island.windows`**
  (windows presence : titre contient le nom du projet).
- Lancement via `pty.exec` (ou `apps`/ShellExecute pour `code`).

## Onglet 2 — Git manager
- Projets en **collapse** ; dans chaque : **10 dernières branches**, la **branche
  active** mise en évidence, **select** d'une branche (checkout).
- Actions : **add + commit + push**, **créer une PR** (gh CLI ou API GitHub).
- **Diff par ligne** affiché (style `+582 −54`) → `git diff --numstat` / `--shortstat`.
- Données structurées via `pty.exec` : `git branch`, `git status`, `git diff --numstat`,
  `git log`. (Pas de PTY ici — one-shot capture parsée.)

## Onglet 3 — Process launcher (par projet)
- **Analyse automatique des `package.json`** du projet → propose les **scripts** (dev,
  build, test…) en boutons. (Composer/artisan possible aussi.)
- Un process lancé est **physiquement visible** : en **idle** (icône/compteur via
  `idle.action`) ET dans la view → accès rapide. Icônes de terminal qui **s'incrémentent**
  selon le nombre de process/terminaux ouverts.
- Clic → ouvre le process dans une **`island.window()`** : **xterm.js** (output parfait,
  PTY) + commandes de contrôle **restart / stop / kill**.

### Détail UI fenêtre process (idée clé)
- Les **commandes de contrôle sont détachées** de la fenêtre : un petit **« container »
  juste sous la fenêtre**, décalé de quelques px, qui **suit la fenêtre** dans ses
  déplacements (drag) → look flottant.
- La **fenêtre est redimensionnable** (resize) → xterm `fit` au resize → `pty.resize`.
- (Implique d'étendre `island.window` : support `resizable` réel + un slot/`footer`
  détaché qui suit la fenêtre. À spécifier côté hôte.)

## Permissions
`terminal` (pty/exec), `windows` (présence IDE), `apps` (ouvrir `code`), `network`
(créer PR via API), `storage` (projets enregistrés). `serve` si compagnon VSCode.

## Ordre de build
1. **Service hôte `pty`** (+ `exec`, permission `terminal`) ← on commence ici.
2. Onglet **Process** (xterm + window + restart/stop/kill) = le « waouh ».
3. Onglet **Git** (exec + diff + PR).
4. Onglet **IDE** + **compagnon VSCode** (contexte projet + présence).
5. Polish window : resize + footer de contrôle détaché qui suit.

## À trancher
- `island.window` : ajouter `resizable` + footer détaché qui suit (extension hôte).
- Contexte projet : compagnon VSCode (riche) vs parse de titre (léger) — sans doute les
  deux (titre en fallback).
- PR : `gh` CLI (via `exec`) vs API GitHub (via `http` + token `secrets`).
