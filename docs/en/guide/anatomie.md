# 3. Anatomy & build contract

## Project layout

```
com.island.<name>/
├─ manifest.json      ← read by the host (identity, surfaces, permissions)
├─ package.json       ← the extension's toolchain (built with YOUR Vite)
├─ vite.config.ts     ← output contract (dist/index.mjs + style.css)
├─ tailwind.css       ← style contract (host tokens, no reset)
├─ index.ts           ← entry point (defineExtension)
├─ *.vue              ← your surfaces / components
└─ dist/              ← the build (what the host actually loads)
```

## `manifest.json`

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

- **`id`**: reverse-DNS, **must match the folder name**.
- **`surfaces`**: declares which surfaces the extension mounts (`view`, `config`…). The
  value (`"island"`, `"modal"`) says **where** the surface shows up; the real Vue component
  is wired in `index.ts`.
- **`permissions`**: the backend services the extension uses (`capture`, `system`, `media`,
  `network`, `apps`, `native-encoder`, `storage`, `shortcuts`, `terminal`…). **Checked on
  every call** by the host: a service called without its permission is denied. Details →
  [Services & permissions](/en/guide/services).

## `package.json`

```json
{
  "name": "island-ext-<name>",
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

> `vue` is a **devDependency**: it compiles the `.vue` files but is **externalized** at
> build time (never bundled) — at runtime the extension shares the host's Vue instance.
> `@island/sdk` is not an npm package: it's a module provided by the host, simply
> externalized.

## `vite.config.ts` (identical for every extension)

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

Guaranteed output: `dist/index.mjs` + `dist/style.css`.

> 🔴 **Golden rule**: **never** bundle `vue` or `@island/sdk`. A second Vue instance breaks
> reactivity; a duplicated SDK breaks the bridge to the host. That's exactly what
> `rollupOptions.external` is for.

## `tailwind.css` (style contract)

```css
@import "tailwindcss/theme" theme(reference);   /* tokens as reference, without re-emitting them */
@import "tailwindcss/utilities";                /* NO preflight → no global reset that leaks */

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

Three **mandatory** points:

1. **`theme(reference)`**: without it, spacing utilities (`h-1`, `gap-2`, `inset-y-0`…) are
   not generated (the `--spacing` variable is missing). Typical symptom: zero-height bars,
   collapsed spacing.
2. **`@source not "./dist"`**: otherwise Tailwind rescans the build → infinite rebuild loop
   in `--watch` mode. Also add a `.gitignore` (`node_modules`, `dist`).
3. **No full `@import "tailwindcss"`**: this avoids the *preflight* (global reset) that
   would contaminate the host. The `@theme inline` maps the host's tokens → your
   `bg-background`, `text-muted-foreground`… classes follow Island's light/dark theme for
   free. A fixed brand color uses an arbitrary value (`bg-[#1db954]`).

## `index.ts` (entry point)

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

→ Next: [The SDK](/en/guide/sdk).
