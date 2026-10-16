# Manifeste & build

## `manifest.json`

```json
{
  "id": "dev.island.github",
  "name": "GitHub",
  "version": "0.1.0",
  "author": "Island",
  "description": "Revues demandées, PR ouvertes et état de la CI, dans l'île.",
  "icon": "icons/github.svg",
  "main": "dist/main.js",
  "permissions": ["network:api.github.com", "secrets", "notifications"],
  "contributes": {
    "tools":    [{ "id": "inbox", "title": "GitHub", "icon": "icons/github.svg", "view": "dist/inbox.html", "size": [560, 340] }],
    "live":     [{ "id": "status", "tool": "inbox" }],
    "launcher": [{ "id": "repos", "prefix": "gh", "title": "GitHub" }],
    "commands": [{ "id": "refresh", "title": "GitHub : rafraîchir", "shortcut": "Control+Alt+G" }],
    "settings": [{ "key": "token", "type": "secret", "label": "Jeton d'accès" }]
  },
  "locales": { "fr": "locales/fr.json", "en": "locales/en.json" }
}
```

| Champ | Obligatoire | Rôle |
|---|---|---|
| `id` | oui | Identifiant unique, nom de domaine inversé : segments `a-z0-9-`, au moins deux, pas de segment vide |
| `name`, `version` | oui | Affichés dans Réglages › Extensions et à l'installation |
| `author`, `description` | non | Idem |
| `icon` | non | SVG monochrome (dessiné en noir) : Island le teinte selon le thème |
| `main` | oui | Module JS de la logique (tourne en fond dans une iframe cachée) |
| `permissions` | non | Voir [Permissions & sécurité](./permissions) |
| `contributes` | non | Les emplacements occupés, voir [Emplacements](./emplacements) |
| `locales` | non | Fichiers JSON de traduction par langue (`island.t(clé)`) |

Tous les chemins sont **relatifs** au dossier de l'extension, sans `..` ni chemin absolu.
Island valide le manifeste à l'installation et refuse un paquet invalide.

## Build avec Vite

Contraintes du runtime à respecter :

- **Chemins relatifs** : les fichiers sont servis sous `island-ext://<id>/…`, donc
  `base: "./"`.
- **Pas de script en ligne** : la CSP des extensions n'autorise que les fichiers de
  l'extension → `build.modulePreload: false`.
- **Pas de réseau direct** : `fetch` est bloqué, tout passe par `island.http`.

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

`input` liste la logique (`src/main.ts` → `dist/main.js`) et chaque vue d'outil (une page
HTML → `dist/<nom>.html`). Les chemins du manifeste pointent vers `dist/`.

## Traductions

```json
// locales/fr.json
{ "refresh": "Rafraîchir", "updated": "mis à jour {age}" }
```

```ts
island.t("updated", { age: "à l'instant" }); // « mis à jour à l'instant »
```

La langue suit celle d'Island ; le SDK recharge les textes quand elle change.
