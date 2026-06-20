import { defineConfig, searchForWorkspaceRoot } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'
import fs from 'node:fs'

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Le dossier de config Tauri = %APPDATA%/<identifier> (PAS "island"). On lit
// l'identifier depuis tauri.conf.json pour éviter toute dérive.
const tauriConf = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, "src-tauri/tauri.conf.json"), "utf-8")
);
const identifier: string = tauriConf.identifier ?? "com.nihil.island";

// Extensions installées (%APPDATA%/<identifier>/extensions). En mode dev, le loader
// importe leur SOURCE via /@fs/ → il faut autoriser Vite à servir ce dossier
// (hors racine projet) pour avoir la transpilation + le HMR.
// @ts-expect-error process is a nodejs global
const appData: string | undefined = process.env.APPDATA;
const extensionsDir = appData ? path.join(appData, identifier, "extensions") : undefined;
// @ts-expect-error process is a nodejs global
const workspaceRoot = searchForWorkspaceRoot(process.cwd());

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],

  // Le SDK est livré avec Island : les extensions l'importent via "@island/sdk".
  resolve: {
    alias: {
      "@island/sdk": fileURLToPath(new URL("./src/sdk/index.ts", import.meta.url)),
      '@': path.resolve(__dirname, './src'),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    // Autorise le service de fichiers hors racine : workspace + extensions installées
    // (sinon /@fs/<%APPDATA%>/… est refusé par Vite).
    fs: {
      allow: [workspaceRoot, ...(extensionsDir ? [extensionsDir] : [])],
    },
  },
}));
