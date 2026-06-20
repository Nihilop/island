import * as Vue from "vue";
import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./style.css";
import Overlay from "./windows/Overlay.vue";
import Settings from "./windows/Settings.vue";
import Install from "./windows/Install.vue";
import Create from "./windows/Create.vue";
import { installBridge } from "./island-bridge";
import * as Sdk from "./sdk";
import { useAppStore } from "./stores/appStore";

// Runtime PARTAGÉ avec les extensions (loader prod) : l'import map mappe
// `vue`/`@island/sdk` vers des shims qui lisent ces globals → MÊME instance.
(window as any).__ISLAND_VUE__ = Vue;
(window as any).__ISLAND_SDK__ = Sdk;

// Expose le pont hôte (window.__ISLAND__) consommé par le SDK des extensions.
installBridge();

// Une seule build sert les fenêtres : on choisit le composant racine
// en fonction du label de la fenêtre Tauri courante.
function resolveLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "overlay";
  }
}

const label = resolveLabel();

const roots: Record<string, typeof Overlay> = {
  settings: Settings,
  install: Install,
  create: Create,
};
const pinia = createPinia();
const app = createApp(roots[label] ?? Overlay);
app.use(pinia);
app.mount("#app");

// Chaque fenêtre charge les prefs, applique le thème, et écoute les changements
// émis par les autres fenêtres → le thème est appliqué PARTOUT.
useAppStore(pinia).init();
