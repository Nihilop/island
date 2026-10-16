// Build de l'extension Capture : logique (raccourcis, activité « REC », notifications) +
// vue de l'outil (Vue). Chemins RELATIFS (servis par island-ext://<id>/dist/).

import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));
const sdk = here("../../../packages/sdk/src/");

export default defineConfig({
  base: "./",
  plugins: [vue()],
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
      input: { main: "src/main.ts", capture: "capture.html" },
      output: {
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
});
