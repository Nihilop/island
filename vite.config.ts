import { defineConfig, searchForWorkspaceRoot } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// @ts-expect-error process is a nodejs global
const workspaceRoot = searchForWorkspaceRoot(process.cwd());

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  // Pré-compilation des dépendances : la page d'Island seulement. Sans ça, Vite parcourt
  // TOUS les .html du dépôt, dont les pages des extensions d'exemple (examples/extensions),
  // qui ont leur propre build et leurs propres alias (`@island/sdk`…).
  optimizeDeps: {
    entries: ["index.html"],
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
      // 3. tell Vite to ignore watching `src-tauri` (et les extensions d'exemple, qui se
      //    construisent à part : leurs `dist/` changent à chaque build) — SAUF les shaders
      //    des fonds animés (`src-tauri/src/wallpaper/*.wgsl`), importés par l'aperçu de
      //    l'éditeur : sans ça, Vite sert la première version lue, à jamais. Les seuls
      //    dossiers parcourus sont le chemin jusqu'à eux (jamais `target/`).
      ignored: (file: string) => {
        const p = file.replace(/\\/g, "/");
        if (p.includes("/examples/")) return true;
        const i = p.indexOf("/src-tauri");
        if (i < 0) return false;
        const rest = p.slice(i);
        return !(rest === "/src-tauri" || rest === "/src-tauri/src" || rest === "/src-tauri/src/wallpaper" || /^\/src-tauri\/src\/wallpaper\/[^/]+\.wgsl$/.test(rest));
      },
    },
    fs: {
      allow: [workspaceRoot],
    },
  },
}));
