# Écrire une extension Island

> Référence du **contrat d'extension**. Une extension est un mini-projet Vue/TS
> autonome qui ne consomme QUE `@island/sdk`. L'hôte Island est 100 % agnostique :
> il ne lit que `manifest.json` + `dist/`, jamais ton code source.

## 0. Le plus rapide : générer depuis un template

Dans Island : tray → **Réglages → Extensions → « Créer une extension »**. Choisis
un template (**Complet** ou **Minimal**), donne un nom → un projet prêt à coder est
généré dans le dossier des extensions. Suis ensuite les commandes affichées
(`pnpm install` puis `pnpm dev`), puis active l'extension.

- **Minimal** : une seule `view`, le strict nécessaire pour démarrer.
- **Complet** : `view` + `config` (modal), compteur persistant (`storage`),
  contribution à l'île en idle, et notification — une vitrine du SDK.

Les templates de référence vivent dans `src-tauri/templates/{minimal,complete}/` du
repo Island (placeholders `__EXT_ID__` / `__EXT_NAME__` / `__EXT_SLUG__` substitués à
la génération). Le reste de ce document décrit ce que ces templates contiennent.

## 1. Où vit une extension

Le dossier des extensions installées **EST** l'espace de dev (on code en place) :

```
%APPDATA%\com.nihil.island\extensions\<id>\
```

soit `C:\Users\<user>\AppData\Roaming\com.nihil.island\extensions\<id>\`.

Island scanne ce dossier au démarrage. Une extension qui contient un
`package.json` est marquée **dev** (badge dans les Réglages + bouton « Packager »).
Une extension packagée (`.island`) ne contient que `manifest.json` + `dist/`.

## 2. Structure d'un projet d'extension

```
com.island.<nom>/
├─ manifest.json      ← lu par l'hôte (identité, surfaces, permissions)
├─ package.json       ← toolchain de l'extension (présence = mode dev)
├─ vite.config.ts     ← build en lib ESM, externalise vue + @island/sdk
├─ tailwind.css       ← réutilise les tokens de l'hôte (contrat ci-dessous)
├─ index.ts           ← point d'entrée : defineExtension({...})
├─ *.vue              ← tes surfaces (view, config…)
├─ .gitignore         ← node_modules + dist
└─ dist/              ← SORTIE du build (index.mjs + style.css) — le SEUL livrable
   ├─ index.mjs
   └─ style.css
```

## 3. Les fichiers, un par un

### `manifest.json`

```json
{
  "id": "com.island.meme",
  "name": "Meme",
  "version": "0.1.0",
  "author": "you",
  "description": "…",
  "main": "dist/index.mjs",
  "styles": "dist/style.css",
  "permissions": [],
  "surfaces": { "view": "island" }
}
```

- `id` : reverse-DNS, doit matcher le nom du dossier.
- `surfaces` : déclare quelles surfaces l'extension monte (`view`, `config`…).
  La valeur (`"island"`, `"modal"`) indique OÙ la surface s'affiche. Le composant
  Vue réel est branché dans `index.ts`.
- `permissions` : services backend que l'ext utilise (`capture`, `system`, `media`,
  `network`, `apps`, `native-encoder`, `storage`, `shortcuts`). **Vérifié à chaque
  appel** par l'hôte : un service appelé sans sa permission est refusé. Catalogue
  complet (méthodes, confiance, multi-OS) → [sdk-services.md](sdk-services.md).

### `package.json` (toolchain propre à l'extension)

```json
{
  "name": "island-ext-<nom>",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": { "build": "vite build", "dev": "vite build --watch" },
  "devDependencies": {
    "@tailwindcss/vite": "^4.3.1",
    "@vitejs/plugin-vue": "^6.0.0",
    "tailwindcss": "^4.3.1",
    "vite": "^8.0.0",
    "vue": "^3.5.0"
  }
}
```

> `vue` est en **devDependency** : il sert à compiler les `.vue`, mais il est
> EXTERNALISÉ au build (jamais bundlé). Au runtime, l'extension partage l'instance
> Vue de l'hôte. `@island/sdk` n'est pas une dépendance npm : c'est un module
> fourni par l'hôte, juste externalisé.

### `vite.config.ts` (identique pour toute extension)

```ts
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  build: {
    lib: { entry: "index.ts", formats: ["es"], fileName: () => "index.mjs", cssFileName: "style" },
    rollupOptions: { external: ["vue", "@island/sdk"] },
    cssCodeSplit: false,
    outDir: "dist",
    emptyOutDir: true,
  },
});
```

Sortie garantie : `dist/index.mjs` + `dist/style.css`. **Ne pas** bundler vue ni
le SDK (sinon : double instance Vue → réactivité cassée, ou SDK dupliqué).

### `tailwind.css` (contrat de style)

```css
@import "tailwindcss/theme" theme(reference);   /* tokens en référence, sans réémettre */
@import "tailwindcss/utilities";                /* PAS de preflight → pas de reset qui fuit */

@source not "./dist";                            /* n'auto-scanne pas son propre build (boucle) */

@theme inline {                                  /* réutilise les variables CSS de l'hôte */
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-muted-foreground: var(--muted-foreground);
  --color-border: var(--border);
}
```

- `theme(reference)` est **obligatoire** : sans lui, les utilitaires d'espacement
  (`h-1`, `gap-2`, `inset-y-0`…) ne sont pas générés (la variable `--spacing` manque).
- `@source not "./dist"` évite que Tailwind rescanne le build → boucle de rebuild infinie.
- On n'importe PAS `tailwindcss/preflight` : pas de reset global qui contaminerait l'hôte.

### `index.ts` (point d'entrée)

```ts
import { defineExtension } from "@island/sdk";
import View from "./View.vue";
import "./tailwind.css";

export default defineExtension({
  surfaces: { view: View },        // branche les composants déclarés dans le manifest

  activate(ctx) {                  // ctx = IslandApi + { id, storage }
    ctx.launcher.register({
      label: "Mon extension",
      icon: "<svg …>…</svg>",      // SVG en string (stroke=currentColor recommandé)
      onActivate: () => ctx.view.open(View, { width: 380, height: 400, radius: 28 }),
    });
  },

  deactivate() { /* nettoyage optionnel */ },
});
```

## 4. L'API SDK (`@island/sdk`)

Deux entrées principales :

- **`defineExtension(def)`** — déclare l'extension (`surfaces`, `activate`, `deactivate`).
- **`useIsland(extId?)`** — dans un composant, renvoie l'`IslandApi` (état média réactif
  + actions). **Pour les services gardés par une permission** (`capture`, `system`,
  `media`, `network`), passe ton `EXT_ID` : `useIsland(EXT_ID)` → l'hôte vérifie ta
  permission. Dans `activate(ctx)`, `ctx` est **déjà lié** à ton id. Détails →
  [sdk-services.md](sdk-services.md).

`activate(ctx)` reçoit l'API complète + `ctx.id` + `ctx.storage`. Surfaces clés :

| Domaine        | Usage |
| -------------- | ----- |
| `ctx.launcher` | `register({label, icon, onActivate})` / `remove()` — entrée dans le launcher |
| `ctx.view`     | `open(component, {width,height,radius,persistent})` / `close()` / `resize({width,height})` — monte une view dans l'île. `persistent: true` = reste ouverte malgré un clic ailleurs / perte de focus (ex. garder des stats visibles) ; sinon un clic hors de l'île la replie. |
| `ctx.window`   | `open(component, {id,title,icon,width,height,resizable})→id` / `close(id?)` / `focus(id)` — fenêtre flottante draggable. **`icon`** (SVG/lucide) = affiché dans la sphère quand l'utilisateur **minimise** la fenêtre (les minimisées apparaissent en sphères à droite de l'île, clic = restaure). |
| `ctx.idle`     | `state()` (couleur du cercle : `idle\|recording`), `center(component)` (monte une viz custom au centre — prime sur le cercle), `action("left"\|"right", …)`, `tap()` — contribue à l'île en idle |
| `ctx.notify`   | `notify({title, body, icon, color, source, timeout, actions})` → bannière + historique |
| `ctx.capture`  | `screenshot()`, `selectRegion()`, `showRegionOutline()`, `listDisplays()`… **L'enregistrement vidéo est agnostique** : `startRecording({ region, display, fps, encoder })` où `encoder = { extId, bin, args }` — l'extension fournit son propre binaire d'encodage (dans son dossier) + les args. L'hôte n'ajoute que l'entrée (frames brutes BGRA top-down, géométrie) et la sortie. `fetchBinary({ extId, url, dest, zipEntry? })` télécharge ce binaire dans le dossier de l'extension (progress via event `encoder://download`). Voir « Embarquer un binaire natif » ci-dessous. |
| `ctx.shortcuts`| `register(accel, handler)` / `unregister(accel)` — raccourcis GLOBAUX |
| `ctx.terminal` | ⚠⚠ **perm `terminal` (confiance maximale)** : `spawn({cwd,cmd,args,cols,rows})→id`, `write(id,data)`, `resize(id,cols,rows)`, `kill(id)`, `exec({cmd,args,cwd})→{code,stdout,stderr}`, `onData`/`onExit` — terminaux PTY (xterm) + commandes one-shot. Voir `docs/sdk-services.md`. |
| `ctx.system`   | `stats()` → `{cpu, cores[], memUsed, memTotal}` |
| `ctx.storage`  | `get/set/delete/keys` — store clé→valeur persistant, isolé par extension |
| `ctx.invoke`   | `invoke(cmd, args)` — accès brut aux commandes hôte (échappatoire) |
| `ctx.on`       | `on(event, cb)` — écoute un event hôte |

Source de vérité du typage : `src/sdk/island.ts` dans le repo Island.

### Saisie clavier dans une view

L'overlay ne prend **pas** le focus clavier par défaut (volontaire : il ne vole pas
le focus aux jeux). Si ta view a un `<input>`, appelle `ctx.invoke("overlay_focus")`
juste après `ctx.view.open(...)`, puis `inputEl.focus()` au montage → la frappe arrive
dans le champ. Quand l'overlay perd le focus (clic ailleurs), l'île se referme toute
seule. C'est ce que fait l'extension **Flow** (launcher).

### Lancer des applications & chercher des fichiers (`apps`)

Permission `"apps"`. L'hôte expose :

- `invoke("list_apps", { extId })` → `[{ name, path }]` : apps **Win32 (menu Démarrer)
  + UWP/Store + jeux Steam** (le `path` sert au lancement ET à l'icône).
- `invoke("launch_path", { extId, path })` (ShellExecute) et
  `invoke("launch_admin", { extId, path })` (**élévation UAC**).
- `invoke("app_icons", { extId, paths })` → icônes PNG (data-URL).
- `invoke("search_files", { extId, query, roots, limit })` → `[{ name, path, isDir }]` :
  **Everything** (voidtools) si présent → tout-disque ; sinon **index maison** des `roots`
  (défaut : Bureau / Documents / Téléchargements).
- `invoke("files_engine", { extId })` → `boolean` : Everything détecté (à afficher dans tes réglages).

Même palier de confiance que `native-encoder` (peut lancer des exécutables, avec
élévation) → affiché à l'install. Exemple complet : extension **Flow**.

### Composants UI prêts à l'emploi

Le SDK ré-exporte des composants du design system (mêmes tokens que l'hôte, dark/light
auto) : `Button`, `Switch`, `Progress`, `Kbd`/`KbdGroup`, la famille `Select…`, et le
**menu contextuel** `ContextMenu` / `ContextMenuTrigger` / `ContextMenuContent` /
`ContextMenuItem` (clic droit, basé sur reka-ui).

```vue
<script setup>
import { ContextMenu, ContextMenuTrigger, ContextMenuContent, ContextMenuItem } from "@island/sdk";
const rootEl = ref();
</script>
<template>
  <div ref="rootEl">
    <ContextMenu>
      <ContextMenuTrigger as-child><button>…</button></ContextMenuTrigger>
      <ContextMenuContent :collision-boundary="rootEl">
        <ContextMenuItem @select="copier()">Copier</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
```

⚠️ **Dans une view de l'île**, passe `:collision-boundary` = l'élément racine de ta view.
La zone interactive de l'overlay se limite à la **boîte de l'île** (cf.
[perf-overlay-freeze](perf-overlay-freeze.md)) : un menu portalisé qui déborde la boîte
ne serait pas cliquable. Le `collisionBoundary` l'y maintient.

### Safe-zone des views

L'île touche le bord haut de l'écran et porte une poignée de collapse. L'hôte
réserve donc automatiquement une **zone haute** (`--safe-top`, ~14px) sur les
surfaces `view` et les listes de notifs : ton contenu ne commence jamais tout en
haut. Inutile d'ajouter un gros `padding-top` toi-même — garde un padding normal
(`p-3`/`p-3.5`) et laisse l'hôte gérer le haut.

### Embarquer un binaire natif (encodeur, outil CLI…)

L'hôte reste **agnostique** : il ne décide pas du codec. Une extension peut fournir
son propre binaire natif (ex. ffmpeg pour l'enregistrement) — c'est le pattern des
extensions VS Code. C'est un **cran de confiance au-dessus du JS sandboxé** :

- **Permission obligatoire** : déclare `"native-encoder"` dans `permissions` du
  manifeste → affichée en évidence à l'install (« exécute un programme natif »).
  Sans elle, l'hôte refuse de lancer/télécharger le binaire.
- **Cantonné au dossier de l'extension** : le binaire (et la destination de
  `fetchBinary`) sont résolus DANS `extensions/<id>/` (anti `..`/chemins absolus).
  Impossible de lancer un programme système.
- **Pas embarqué dans le `.island`** : garde le paquet léger ; télécharge le binaire
  au 1er besoin via `island.capture.fetchBinary(...)` dans le dossier de l'extension
  (gitignore `binaries/`). L'hôte contrôle le chemin de SORTIE de l'encodage.

Exemple (extension Capture) : `fetchBinary({ extId, url: <zip ffmpeg>, dest: "binaries/ffmpeg.exe", zipEntry: "bin/ffmpeg.exe" })`, puis `startRecording({ …, encoder: { extId, bin: "binaries/ffmpeg.exe", args: ["-c:v","libx265","-crf","25", …] } })`.

**Son système** : `startRecording({ …, audio: true, encoder: { …, audioArgs: ["-c:a","aac","-b:a","160k"] } })`. L'hôte capture le son du PC (WASAPI loopback), encode la vidéo, puis muxe vidéo + audio (`audioArgs` = codec audio). L'extension ne fait que demander `audio: true` et fournir le codec audio.

### Réseau

L'hôte tourne avec `csp: null` → un `fetch()` standard vers une API externe
fonctionne (si l'API renvoie du CORS). Pas besoin de plugin HTTP. Exemple : le
meme fetché depuis `https://meme-api.com/gimme` (CORS `*`).

### Traductions (i18n)

L'i18n est **partagé** (une instance dans `@island/sdk`) : pas de vue-i18n par extension,
et la langue suit celle d'Island (Réglages → Langue). Ton extension est **namespacée par
son id** → pas de collision de clés.

```ts
// locales/en.json, locales/fr.json  → { "nowPlaying": "Now playing: {title}" }
import en from "./locales/en.json";
import fr from "./locales/fr.json";

activate(ctx) {
  // Register une MAP de locales (import statique). `t()` est réactif : la langue change
  // → tes textes se retraduisent sans rien faire.
  ctx.i18n.register((locale) => ({ en, fr }[locale] ?? en));

  ctx.notify({ title: ctx.i18n.t("nowPlaying", { title: track }) });
}
```

> ⚠️ **Embarque tes locales par IMPORT STATIQUE** (comme ci-dessus), pas via
> `import('./locales/'+l+'.json')`. En **prod**, le `dist/` est chargé en **Blob URL** →
> un import dynamique *relatif* ne résout pas. L'import statique bundle les JSON dans
> `dist/index.mjs` → **embarqué dans le `.island` automatiquement** (le packager prend tout
> `dist/`) et **prod-safe**. Pour des extensions à peu de texte, le coût est négligeable
> même avec beaucoup de langues. (L'hôte, lui, lazy-load un chunk par langue — il n'est pas
> chargé en Blob.)

## 5. Workflow de dev

1. Créer le dossier dans `%APPDATA%\com.nihil.island\extensions\<id>\` avec les fichiers ci-dessus.
2. `pnpm install` puis `pnpm build` (ou `pnpm dev` = `vite build --watch`).
3. Dans Island : tray → **Réglages → Extensions** → activer l'extension.
   - En **dev (Island lancé via `pnpm tauri dev`)** : le `dist/` est chargé via
     Vite (`/@fs/…`) et **live-reload** quand tu rebuild (watcher hôte sur `dist/`).
   - En **prod (Island installé)** : le `dist/` est lu par la commande `read_ext_file`
     et importé via `Blob`/`createObjectURL`. Pas de Vite requis.
4. Ouvrir le launcher sur l'île → cliquer l'entrée → ta `view` s'affiche.

> Une nouvelle extension ajoutée pendant qu'Island tourne : rouvrir les Réglages
> (re-scan) pour la voir apparaître, puis l'activer (reconciliation runtime, pas
> de redémarrage).

## 6. Distribution : packager un `.island`

Un `.island` = zip de `manifest.json` + `dist/` (sans le source). Deux voies :

- **Dans l'app** : Réglages → Extensions → bouton **« Packager »** (visible sur les
  extensions dev) → choisir la destination → un `.island` est créé.
- Double-clic sur un `.island` (si l'association de fichiers est activée) → modal
  d'installation → copié dans le dossier des extensions.

## 7. Checklist express

- [ ] `id` du manifest = nom du dossier.
- [ ] `vite.config.ts` externalise `["vue", "@island/sdk"]`.
- [ ] `tailwind.css` : `theme(reference)` + `@source not "./dist"` + `@theme inline`.
- [ ] `index.ts` exporte `defineExtension` par défaut + importe `./tailwind.css`.
- [ ] `pnpm build` produit `dist/index.mjs` + `dist/style.css`.
- [ ] Couleurs via les tokens hôte (`text-foreground`, `bg-primary`…), pas en dur.
- [ ] i18n (si textes) : locales en **import statique** + `ctx.i18n.register((l) => map[l])` (pas d'import dynamique relatif → casse en prod/Blob).
