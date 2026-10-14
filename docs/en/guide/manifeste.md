# Manifest & build

## `manifest.json`

```json
{
  "id": "dev.island.github",
  "name": "GitHub",
  "version": "0.1.0",
  "author": "Island",
  "description": "Requested reviews, open PRs and CI status, in the notch.",
  "icon": "icons/github.svg",
  "main": "dist/main.js",
  "permissions": ["network:api.github.com", "secrets", "notifications"],
  "contributes": {
    "tools":    [{ "id": "inbox", "title": "GitHub", "icon": "icons/github.svg", "view": "dist/inbox.html", "size": [560, 340] }],
    "live":     [{ "id": "status", "tool": "inbox" }],
    "launcher": [{ "id": "repos", "prefix": "gh", "title": "GitHub" }],
    "commands": [{ "id": "refresh", "title": "GitHub: refresh", "shortcut": "Control+Alt+G" }],
    "settings": [{ "key": "token", "type": "secret", "label": "Access token" }]
  },
  "locales": { "fr": "locales/fr.json", "en": "locales/en.json" }
}
```

| Field | Required | Purpose |
|---|---|---|
| `id` | yes | Unique identifier, reverse domain name: `a-z0-9-` segments, at least two, no empty segment |
| `name`, `version` | yes | Shown in Settings › Extensions and at install time |
| `author`, `description` | no | Same |
| `icon` | no | Monochrome SVG (drawn in black): Island tints it according to the theme |
| `main` | yes | JS module for the logic (runs in the background in a hidden iframe) |
| `permissions` | no | See [Permissions & security](./permissions) |
| `contributes` | no | The slots used, see [Slots](./emplacements) |
| `locales` | no | Per-language JSON translation files (`island.t(key)`) |

All paths are **relative** to the extension folder, with no `..` and no absolute path.
Island validates the manifest at install time and rejects an invalid package.

## Build with Vite

Runtime constraints to respect:

- **Relative paths**: files are served under `island-ext://<id>/…`, hence `base: "./"`.
- **No inline script**: the extensions' CSP only allows the extension's own files →
  `build.modulePreload: false`.
- **No direct network access**: `fetch` is blocked, everything goes through `island.http`.

```ts
// vite.config.ts
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const sdk = fileURLToPath(new URL("../../../packages/sdk/src/", import.meta.url));

export default defineConfig({
  base: "./",
  resolve: {
    alias: [
      { find: /^@island\/sdk\/ui\.css$/, replacement: `${sdk}ui.css` },
      { find: /^@island\/sdk$/, replacement: `${sdk}index.ts` },
    ],
  },
  build: {
    outDir: "dist",
    target: "es2022",
    modulePreload: false,
    rollupOptions: {
      input: { main: "src/main.ts", inbox: "inbox.html" },
      output: { entryFileNames: "[name].js", chunkFileNames: "chunks/[name]-[hash].js", assetFileNames: "assets/[name]-[hash][extname]" },
    },
  },
});
```

`input` lists the logic (`src/main.ts` → `dist/main.js`) and each tool view (an HTML page →
`dist/<name>.html`). The manifest paths point to `dist/`.

## Translations

```json
// locales/en.json
{ "refresh": "Refresh", "updated": "updated {age}" }
```

```ts
island.t("updated", { age: "just now" }); // "updated just now"
```

The language follows Island's; the SDK reloads the strings when it changes.
