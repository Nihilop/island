# Authoring an Island extension

> Reference for the **extension contract**. An extension is a self-contained Vue/TS
> mini-project that consumes ONLY `@island/sdk`. The Island host is 100% agnostic: it reads
> only `manifest.json` + `dist/`, never your source code.

## 0. Fastest: generate from a template

In Island: tray → **Settings → Extensions → "Create an extension"**. Pick a template
(**Complete** or **Minimal**), give it a name → a ready-to-code project is generated in the
extensions folder. Then follow the printed commands (`pnpm install` then `pnpm dev`), and
enable the extension.

- **Minimal**: a single `view`, the bare minimum to start.
- **Complete**: `view` + `config` (modal), a persistent counter (`storage`), an idle
  contribution, and a notification — a showcase of the SDK.

Reference templates live in `src-tauri/templates/{minimal,complete}/` of the Island repo
(placeholders `__EXT_ID__` / `__EXT_NAME__` / `__EXT_SLUG__` substituted at generation). The
rest of this document describes what these templates contain.

## 1. Where an extension lives

The installed-extensions folder **IS** the dev workspace (you code in place):

```
%APPDATA%\com.nihil.island\extensions\<id>\
```

i.e. `C:\Users\<user>\AppData\Roaming\com.nihil.island\extensions\<id>\`.

Island scans this folder at startup. An extension that contains a `package.json` is flagged
**dev** (badge in Settings + "Pack" button). A packaged extension (`.island`) contains only
`manifest.json` + `dist/`.

## 2. Extension project layout

```
com.island.<name>/
├─ manifest.json      ← read by the host (identity, surfaces, permissions)
├─ package.json       ← the extension's toolchain (its presence = dev mode)
├─ vite.config.ts     ← ESM lib build, externalizes vue + @island/sdk
├─ tailwind.css       ← reuses the host's tokens (contract below)
├─ index.ts           ← entry point: defineExtension({...})
├─ *.vue              ← your surfaces (view, config…)
├─ .gitignore         ← node_modules + dist
└─ dist/              ← build OUTPUT (index.mjs + style.css) — the ONLY deliverable
   ├─ index.mjs
   └─ style.css
```

## 3. The files, one by one

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

- `id`: reverse-DNS, must match the folder name.
- `surfaces`: declares which surfaces the extension mounts (`view`, `config`…). The value
  (`"island"`, `"modal"`) says WHERE the surface shows up. The real Vue component is wired in
  `index.ts`.
- `permissions`: backend services the ext uses (`capture`, `system`, `media`, `network`,
  `apps`, `native-encoder`, `storage`, `shortcuts`, `terminal`). **Checked on every call** by
  the host: a service called without its permission is denied. Full catalog (methods, trust,
  multi-OS) → [Services catalog](/en/reference/services-catalog).

### `package.json` (the extension's own toolchain)

```json
{
  "name": "island-ext-<name>",
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

> `vue` is a **devDependency**: it compiles the `.vue` files, but is EXTERNALIZED at build
> (never bundled). At runtime the extension shares the host's Vue instance. `@island/sdk` is
> not an npm dependency: it's a module provided by the host, just externalized.

### `vite.config.ts` (identical for every extension)

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

Guaranteed output: `dist/index.mjs` + `dist/style.css`. **Do not** bundle vue or the SDK
(otherwise: a second Vue instance → broken reactivity, or a duplicated SDK).

### `tailwind.css` (style contract)

```css
@import "tailwindcss/theme" theme(reference);   /* tokens as reference, without re-emitting */
@import "tailwindcss/utilities";                /* NO preflight → no leaking reset */

@source not "./dist";                            /* don't auto-scan your own build (loop) */

@theme inline {                                  /* reuse the host's CSS variables */
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-muted-foreground: var(--muted-foreground);
  --color-border: var(--border);
}
```

- `theme(reference)` is **mandatory**: without it, spacing utilities (`h-1`, `gap-2`,
  `inset-y-0`…) are not generated (the `--spacing` variable is missing).
- `@source not "./dist"` prevents Tailwind from rescanning the build → infinite rebuild loop.
- DON'T import `tailwindcss/preflight`: no global reset that would contaminate the host.

### `index.ts` (entry point)

```ts
import { defineExtension } from "@island/sdk";
import View from "./View.vue";
import "./tailwind.css";

export default defineExtension({
  surfaces: { view: View },        // wire the components declared in the manifest

  activate(ctx) {                  // ctx = IslandApi + { id, storage }
    ctx.launcher.register({
      label: "My extension",
      icon: "<svg …>…</svg>",      // SVG as a string (stroke=currentColor recommended)
      onActivate: () => ctx.view.open(View, { width: 380, height: 400, radius: 28 }),
    });
  },

  deactivate() { /* optional cleanup */ },
});
```

## 4. The SDK API (`@island/sdk`)

Two main entry points:

- **`defineExtension(def)`** — declares the extension (`surfaces`, `activate`, `deactivate`).
- **`useIsland(extId?)`** — inside a component, returns the `IslandApi` (reactive media state
  + actions). **For services gated by a permission** (`capture`, `system`, `media`, `network`,
  `terminal`), pass your `EXT_ID`: `useIsland(EXT_ID)` → the host checks your permission.
  Inside `activate(ctx)`, `ctx` is **already bound** to your id. Details →
  [Services catalog](/en/reference/services-catalog).

`activate(ctx)` receives the full API + `ctx.id` + `ctx.storage`. Key areas:

| Area           | Usage |
| -------------- | ----- |
| `ctx.launcher` | `register({label, icon, onActivate})` / `remove()` — launcher entry |
| `ctx.view`     | `open(component, {width,height,radius,persistent})` / `close()` / `resize({width,height})` — mount a view in the island. `persistent: true` = stays open despite a click elsewhere / loss of focus (e.g. keeping stats visible); otherwise a click outside the island collapses it. |
| `ctx.window`   | `open(component, {id,title,icon,width,height,resizable})→id` / `close(id?)` / `focus(id)` — draggable floating window. **`icon`** (SVG/lucide) = shown in the sphere when the user **minimizes** the window (minimized ones appear as spheres to the right of the island, click = restore). |
| `ctx.idle`     | `state()` (circle color: `idle\|recording`), `center(component)` (mount a custom viz at the center — takes precedence over the circle), `action("left"\|"right", …)`, `tap()` — contributes to the idle island |
| `ctx.notify`   | `notify({title, body, icon, color, source, timeout, actions})` → banner + history |
| `ctx.capture`  | `screenshot()`, `selectRegion()`, `showRegionOutline()`, `listDisplays()`… **Video recording is agnostic**: `startRecording({ region, display, fps, encoder })` where `encoder = { extId, bin, args }` — the extension provides its own encoding binary (in its folder) + the args. The host only adds the input (raw top-down BGRA frames, geometry) and the output. `fetchBinary({ extId, url, dest, zipEntry? })` downloads that binary into the extension's folder (progress via the `encoder://download` event). See "Embedding a native binary" below. |
| `ctx.shortcuts`| `register(accel, handler)` / `unregister(accel)` — GLOBAL shortcuts |
| `ctx.terminal` | ⚠⚠ **perm `terminal` (maximum trust)**: `spawn({cwd,cmd,args,cols,rows})→id`, `write(id,data)`, `resize(id,cols,rows)`, `kill(id)`, `exec({cmd,args,cwd})→{code,stdout,stderr}`, `onData`/`onExit` — PTY terminals (xterm) + one-shot commands. See [Services catalog](/en/reference/services-catalog). |
| `ctx.system`   | `stats()` → `{cpu, cores[], memUsed, memTotal}` |
| `ctx.storage`  | `get/set/delete/keys` — persistent key→value store, isolated per extension |
| `ctx.invoke`   | `invoke(cmd, args)` — raw access to host commands (escape hatch) |
| `ctx.on`       | `on(event, cb)` — listen to a host event |

Source of truth for the types: `src/sdk/island.ts` in the Island repo.

### Keyboard input in a view

The overlay does **not** take keyboard focus by default (intentional: it doesn't steal focus
from games). If your view has an `<input>`, call `ctx.invoke("overlay_focus")` right after
`ctx.view.open(...)`, then `inputEl.focus()` on mount → keystrokes reach the field. When the
overlay loses focus (a click elsewhere), the island collapses by itself. That's what the
**Flow** extension (launcher) does.

### Launching applications & searching files (`apps`)

The `"apps"` permission. The host exposes:

- `invoke("list_apps", { extId })` → `[{ name, path }]`: **Win32 (Start Menu) + UWP/Store
  + Steam games** (`path` is used both to launch and to fetch the icon).
- `invoke("launch_path", { extId, path })` (ShellExecute) and
  `invoke("launch_admin", { extId, path })` (**UAC elevation**).
- `invoke("app_icons", { extId, paths })` → PNG icons (data-URL).
- `invoke("search_files", { extId, query, roots, limit })` → `[{ name, path, isDir }]`:
  **Everything** (voidtools) if present → whole-disk; otherwise a **home-grown index** of
  the given `roots` (default: Desktop / Documents / Downloads).
- `invoke("files_engine", { extId })` → `boolean`: Everything detected (to show in your settings).

Same trust level as `native-encoder` (can launch executables, with elevation) → shown at
install. Full example: the **Flow** extension.

### Ready-made UI components

The SDK re-exports design-system components (same tokens as the host, auto dark/light):
`Button`, `Switch`, `Progress`, `Kbd`/`KbdGroup`, the `Select…` family, and the
**context menu** `ContextMenu` / `ContextMenuTrigger` / `ContextMenuContent` /
`ContextMenuItem` (right-click, built on reka-ui).

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
        <ContextMenuItem @select="copy()">Copy</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
```

⚠️ **Inside an island view**, pass `:collision-boundary` = your view's root element. The
overlay's interactive area is limited to the **island box** (see
[perf-overlay-freeze](perf-overlay-freeze.md)): a portaled menu that overflows the box
wouldn't be clickable. `collisionBoundary` keeps it inside.

### View safe-zone

The island touches the top edge of the screen and carries a collapse handle. The host
therefore automatically reserves a **top area** (`--safe-top`, ~14px) on `view` surfaces and
notification lists: your content never starts at the very top. No need to add a big
`padding-top` yourself — keep normal padding (`p-3`/`p-3.5`) and let the host manage the top.

### Embedding a native binary (encoder, CLI tool…)

The host stays **agnostic**: it doesn't decide the codec. An extension can provide its own
native binary (e.g. ffmpeg for recording) — the VS Code extensions pattern. It's **one trust
notch above sandboxed JS**:

- **Mandatory permission**: declare `"native-encoder"` in the manifest's `permissions` → shown
  prominently at install ("runs a native program"). Without it, the host refuses to
  launch/download the binary.
- **Confined to the extension's folder**: the binary (and `fetchBinary`'s destination) are
  resolved INSIDE `extensions/<id>/` (against `..`/absolute paths). Impossible to launch a
  system program.
- **Not bundled in the `.island`**: keep the package light; download the binary on first need
  via `island.capture.fetchBinary(...)` into the extension's folder (gitignore `binaries/`).
  The host controls the encoding OUTPUT path.

Example (Capture extension): `fetchBinary({ extId, url: <ffmpeg zip>, dest: "binaries/ffmpeg.exe", zipEntry: "bin/ffmpeg.exe" })`, then `startRecording({ …, encoder: { extId, bin: "binaries/ffmpeg.exe", args: ["-c:v","libx265","-crf","25", …] } })`.

**System audio**: `startRecording({ …, audio: true, encoder: { …, audioArgs: ["-c:a","aac","-b:a","160k"] } })`. The host captures the PC's audio (WASAPI loopback), encodes the video, then muxes video + audio (`audioArgs` = audio codec). The extension only requests `audio: true` and provides the audio codec.

### Network

The host runs with `csp: null` → a standard `fetch()` to an external API works (if the API
returns CORS). No HTTP plugin needed. Example: the meme fetched from
`https://meme-api.com/gimme` (CORS `*`).

### Translations (i18n)

i18n is **shared** (one instance in `@island/sdk`): no per-extension vue-i18n, and the
language follows Island's (Settings → Language). Your extension is **namespaced by its id** →
no key collision.

```ts
// locales/en.json, locales/fr.json  → { "nowPlaying": "Now playing: {title}" }
import en from "./locales/en.json";
import fr from "./locales/fr.json";

activate(ctx) {
  // Register a MAP of locales (static import). `t()` is reactive: the language changes
  // → your texts re-translate with no extra work.
  ctx.i18n.register((locale) => ({ en, fr }[locale] ?? en));

  ctx.notify({ title: ctx.i18n.t("nowPlaying", { title: track }) });
}
```

> ⚠️ **Embed your locales via STATIC IMPORT** (as above), not via
> `import('./locales/'+l+'.json')`. In **prod**, `dist/` is loaded from a **Blob URL** → a
> *relative* dynamic import doesn't resolve. The static import bundles the JSON into
> `dist/index.mjs` → **automatically embedded in the `.island`** (the packager takes all of
> `dist/`) and **prod-safe**. For text-light extensions the cost is negligible even with many
> languages. (The host, on the other hand, lazy-loads one chunk per language — it isn't loaded
> from a Blob.)

## 5. Dev workflow

1. Create the folder in `%APPDATA%\com.nihil.island\extensions\<id>\` with the files above.
2. `pnpm install` then `pnpm build` (or `pnpm dev` = `vite build --watch`).
3. In Island: tray → **Settings → Extensions** → enable the extension.
   - In **dev (Island launched via `pnpm tauri dev`)**: `dist/` is loaded via Vite (`/@fs/…`)
     and **live-reloads** when you rebuild (host watcher on `dist/`).
   - In **prod (Island installed)**: `dist/` is read by the `read_ext_file` command and
     imported via `Blob`/`createObjectURL`. No Vite required.
4. Open the island launcher → click the entry → your `view` appears.

> A new extension added while Island is running: reopen Settings (re-scan) to see it, then
> enable it (runtime reconciliation, no restart).

## 6. Distribution: packaging an `.island`

An `.island` = a zip of `manifest.json` + `dist/` (without the source). Two ways:

- **In the app**: Settings → Extensions → **"Pack"** button (visible on dev extensions) →
  choose the destination → an `.island` is created.
- Double-click an `.island` (if the file association is enabled) → install modal → copied to
  the extensions folder.

## 7. Quick checklist

- [ ] manifest `id` = folder name.
- [ ] `vite.config.ts` externalizes `["vue", "@island/sdk"]`.
- [ ] `tailwind.css`: `theme(reference)` + `@source not "./dist"` + `@theme inline`.
- [ ] `index.ts` default-exports `defineExtension` + imports `./tailwind.css`.
- [ ] `pnpm build` produces `dist/index.mjs` + `dist/style.css`.
- [ ] Colors via host tokens (`text-foreground`, `bg-primary`…), not hardcoded.
- [ ] i18n (if there's text): locales via **static import** + `ctx.i18n.register((l) => map[l])` (no relative dynamic import → breaks in prod/Blob).
