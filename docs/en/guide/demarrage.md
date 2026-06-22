# 2. Quick start

## Fastest: generate from a template

In Island: **tray → Settings → Extensions → "Create an extension"**. Pick a template,
give it a name → a ready-to-code project is generated in the extensions folder.

- **Minimal**: a single `view`, the bare minimum.
- **Complete**: `view` + `config` (modal), a persistent counter (`storage`), an idle
  contribution and a notification — a showcase of the SDK.

The placeholders `__EXT_ID__` / `__EXT_NAME__` / `__EXT_SLUG__` are substituted at
generation time. Reference templates live in `src-tauri/templates/{minimal,complete}/`.

## Where an extension lives

The installed-extensions folder **IS** the dev workspace (you code in place):

```
%APPDATA%\com.nihil.island\extensions\<id>\
```

i.e. `C:\Users\<you>\AppData\Roaming\com.nihil.island\extensions\<id>\`.

> ⚠️ The config folder uses the Tauri **identifier** (`com.nihil.island`), not "island".
> Classic gotcha.

Island scans this folder at startup. An extension that contains a `package.json` is
flagged **dev** (badge + "Pack" button in Settings). A packaged `.island` only contains
`manifest.json` + `dist/`.

## The dev loop

1. Open a terminal in the extension folder.
2. `pnpm install` then **`pnpm dev`** (= `vite build --watch` → rebuilds `dist/` on every
   save).
3. In Island: **Settings → Extensions** → enable the extension.
4. Open the island launcher → click your entry → your `view` appears.

### Live-reload

- In **dev** (Island launched via `pnpm tauri dev`): a host watcher monitors the `dist/`.
  As soon as `vite build --watch` rewrites your build, the extension is **hot-reloaded** —
  no restart.
- In **prod** (Island installed): `dist/` is read and imported via a Blob URL. No Vite
  required.

> An extension added while Island is running: reopen Settings (re-scan) to see it, then
> enable it. (De)activation is handled at runtime (reconciliation), without a restart.

## Next

You have a running extension. To understand each file and the build contract (externals
`vue`/`@island/sdk`, Tailwind) → see the **Anatomy & build contract** page. For the API →
the **SDK** page. (These pages are being translated — the French guide is complete.)
