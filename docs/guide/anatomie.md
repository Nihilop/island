# 3. Anatomie & contrat de build

## Structure d'un projet

```
com.island.<nom>/
├─ manifest.json      ← lu par l'hôte (identité, surfaces, permissions)
├─ package.json       ← toolchain de l'extension (build avec TON Vite)
├─ vite.config.ts     ← contrat de sortie (dist/index.mjs + style.css)
├─ tailwind.css       ← contrat de style (tokens de l'hôte, pas de reset)
├─ index.ts           ← point d'entrée (defineExtension)
├─ *.vue              ← tes surfaces / composants
└─ dist/              ← le build (CE que l'hôte charge)
```

## `manifest.json`

```json
{
  "id": "com.island.meme",
  "name": "Meme",
  "version": "0.1.0",
  "author": "toi",
  "description": "…",
  "main": "dist/index.mjs",
  "styles": "dist/style.css",
  "permissions": [],
  "surfaces": { "view": "island" }
}
```

- **`id`** : reverse-DNS, **doit matcher le nom du dossier**.
- **`surfaces`** : déclare quelles surfaces l'extension monte (`view`, `config`…). La
  valeur (`"island"`, `"modal"`) indique **où** la surface s'affiche ; le composant Vue
  réel est branché dans `index.ts`.
- **`permissions`** : les services backend que l'extension utilise (`capture`, `system`,
  `media`, `network`, `apps`, `native-encoder`, `storage`, `shortcuts`). **Vérifié à
  chaque appel** par l'hôte : un service appelé sans sa permission est refusé. Détail →
  [Services & permissions](05-services-et-permissions.md).

## `package.json`

```json
{
  "name": "island-ext-<nom>",
  "private": true,
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

> `vue` est en **devDependency** : il sert à compiler les `.vue` mais il est
> **externalisé** au build (jamais bundlé) — au runtime, l'extension partage l'instance
> Vue de l'hôte. `@island/sdk` n'est pas un paquet npm : c'est un module fourni par
> l'hôte, simplement externalisé.

## `vite.config.ts` (identique pour toute extension)

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

Sortie garantie : `dist/index.mjs` + `dist/style.css`.

> 🔴 **Règle d'or** : ne **jamais** bundler `vue` ni `@island/sdk`. Une double instance
> de Vue casse la réactivité ; un SDK dupliqué casse le pont avec l'hôte. C'est à ça que
> sert `rollupOptions.external`.

## `tailwind.css` (contrat de style)

```css
@import "tailwindcss/theme" theme(reference);   /* tokens en référence, sans les réémettre */
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

Trois points **obligatoires** :

1. **`theme(reference)`** : sans lui, les utilitaires d'espacement (`h-1`, `gap-2`,
   `inset-y-0`…) ne sont pas générés (la variable `--spacing` manque). Symptôme typique :
   jauges à hauteur 0, espacements écrasés.
2. **`@source not "./dist"`** : sinon Tailwind rescanne le build → boucle de rebuild
   infinie en mode `--watch`. Ajoute aussi un `.gitignore` (`node_modules`, `dist`).
3. **Pas de `@import "tailwindcss"` complet** : on évite le *preflight* (reset global)
   qui contaminerait l'hôte. Le `@theme inline` mappe les tokens de l'hôte → tes classes
   `bg-background`, `text-muted-foreground`… suivent le thème light/dark d'Island
   gratuitement. Un branding figé se met en valeur arbitraire (`bg-[#1db954]`).

## `index.ts` (point d'entrée)

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

→ Place : [Le SDK](04-le-sdk.md).
