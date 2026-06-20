import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

// Build d'extension Island : ESM unique, vue + @island/sdk EXTERNALISÉS.
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
