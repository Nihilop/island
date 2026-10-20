import { createApp, type Component } from "vue";
import { createPinia } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./style.css";
import { registerMessages, setLocale } from "./sdk/i18n";
import { useAppStore } from "./stores/appStore";

// Une seule build sert toutes les fenêtres : l'overlay du notch, et les fenêtres détachées
// (`note-<id>`, `ext-<id>__<fenêtre>`, cf. src-tauri/src/detached.rs). Le composant racine se choisit d'après le
// libellé ; chaque racine est chargée à la demande (une fenêtre de note ne charge pas le notch).
const label = (() => {
  try {
    return getCurrentWindow().label;
  } catch {
    return "overlay";
  }
})();

async function root(): Promise<Component> {
  if (label.startsWith("note-")) return (await import("./notes/NoteWindow.vue")).default;
  if (label.startsWith("ext-")) return (await import("./extensions/ExtWindow.vue")).default;
  if (label === "wallpaper-editor") return (await import("./wallpaper/EditorWindow.vue")).default;
  if (label === "install-extension") return (await import("./extensions/InstallWindow.vue")).default;
  if (label === "capture-select") return (await import("./capture/CaptureSelect.vue")).default;
  return (await import("./windows/Overlay.vue")).default;
}

const pinia = createPinia();
void root().then((c) => {
  const app = createApp(c);
  app.use(pinia);
  app.mount("#app");
});

// Préférences de l'app (langue…), puis locale de l'interface.
const store = useAppStore(pinia);
store.init().then(() => setLocale(store.lang));

// i18n de l'hôte : catalogue "host" en lazy-load (un chunk par langue via import()),
// et la locale suit la langue d'Island (event lang://changed, émis par appStore.onLang).
registerMessages("host", (l) => import(`./i18n/${l}.json`).then((m) => m.default).catch(() => ({})));
listen<string>("lang://changed", (e) => setLocale(e.payload || "fr"));
