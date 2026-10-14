// Build d'une extension Island v2 : la logique (src/main.ts → dist/main.js) et la vue
// (inbox.html → dist/inbox.html), chemins RELATIFS (servis par island-ext://<id>/dist/).
// La CSP des extensions interdit les scripts en ligne : pas de polyfill de préchargement.

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
    emptyOutDir: true,
    target: "es2022",
    modulePreload: false,
    rollupOptions: {
      input: { main: "src/main.ts", inbox: "inbox.html" },
      output: {
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
});
