import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Construit dans `dist/`, servi par le serveur du magasin sur /admin (fichiers sous
// /admin/assets). En développement (`pnpm dev`), l'API est relayée vers le serveur local.
export default defineConfig({
  base: "/admin/",
  plugins: [vue()],
  build: { outDir: "dist", assetsDir: "assets", emptyOutDir: true },
  server: {
    port: 5180,
    proxy: {
      "/v1": "http://127.0.0.1:8080",
      "/admin/login": "http://127.0.0.1:8080",
      "/admin/callback": "http://127.0.0.1:8080",
      "/admin/logout": "http://127.0.0.1:8080",
    },
  },
});
