// Génère public/island-runtime/vue.mjs : un module qui ré-exporte CHAQUE export
// de Vue depuis le global `window.__ISLAND_VUE__` (posé par l'hôte). Garantit que
// les extensions (loader prod) utilisent la MÊME instance de Vue que l'hôte.
import * as Vue from "vue";
import { writeFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const keys = Object.keys(Vue).filter((k) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(k) && k !== "default");
const lines = [
  "// AUTO-GÉNÉRÉ par scripts/gen-runtime-shims.mjs — ne pas éditer à la main.",
  "// Ré-exporte le Vue de l'hôte (même instance) pour les extensions installées.",
  "const v = window.__ISLAND_VUE__;",
  ...keys.map((k) => `export const ${k} = v[${JSON.stringify(k)}];`),
  "export default v;",
];

const dir = resolve("public/island-runtime");
mkdirSync(dir, { recursive: true });
writeFileSync(resolve(dir, "vue.mjs"), lines.join("\n") + "\n");
console.log(`✓ public/island-runtime/vue.mjs (${keys.length} exports)`);
