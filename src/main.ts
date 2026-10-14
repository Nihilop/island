import { createApp } from "vue";
import { createPinia } from "pinia";
import { listen } from "@tauri-apps/api/event";
import "./style.css";
import Overlay from "./windows/Overlay.vue";
import { registerMessages, setLocale } from "./sdk/i18n";
import { useAppStore } from "./stores/appStore";

// Island n'a qu'une fenêtre : l'overlay du notch.
const pinia = createPinia();
const app = createApp(Overlay);
app.use(pinia);
app.mount("#app");

// Préférences de l'app (langue…), puis locale de l'interface.
const store = useAppStore(pinia);
store.init().then(() => setLocale(store.lang));

// i18n de l'hôte : catalogue "host" en lazy-load (un chunk par langue via import()),
// et la locale suit la langue d'Island (event lang://changed, émis par appStore.onLang).
registerMessages("host", (l) => import(`./i18n/${l}.json`).then((m) => m.default).catch(() => ({})));
listen<string>("lang://changed", (e) => setLocale(e.payload || "fr"));
