// Build d'une extension Island avec le Vite DU REPO (pratique pour le dev cœur,
// sans installer les deps de l'extension). Externalise vue + @island/sdk et sort
// un ESM unique `dist/index.mjs` — exactement le même contrat que le
// `vite.config.ts` embarqué dans chaque extension.
//
// Usage : node scripts/build-extension.mjs extensions/spotify

import { build } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";
import { existsSync } from "node:fs";

const arg = process.argv[2];
if (!arg) {
  console.error("Usage: node scripts/build-extension.mjs <dossier-extension>");
  process.exit(1);
}
const dir = resolve(arg);
const entry = ["index.ts", "index.js", "index.mjs"].map((f) => resolve(dir, f)).find(existsSync);
if (!entry) {
  console.error(`✗ Pas d'entrée (index.ts) dans ${dir}`);
  process.exit(1);
}

await build({
  configFile: false,
  root: dir,
  plugins: [vue()],
  logLevel: "warn",
  build: {
    lib: { entry, formats: ["es"], fileName: () => "index.mjs", cssFileName: "style" },
    rollupOptions: { external: ["vue", "@island/sdk"] },
    cssCodeSplit: false,
    outDir: resolve(dir, "dist"),
    emptyOutDir: true,
  },
});
console.log(`✓ build → ${resolve(dir, "dist")}`);
