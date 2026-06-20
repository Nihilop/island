import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { enable as enableAutostart, disable as disableAutostart, isEnabled as isAutostartEnabled } from "@tauri-apps/plugin-autostart";

export const useAppStore = defineStore('appStore', () => {
    const APP = "__app__";
    const theme = ref('dark')
    const lang = ref('fr')
    const autostart = ref(false);

    // STORE KV
    async function appGet<T>(key: string, fallback: T): Promise<T> {
        const v = await invoke<T | null>("storage_get", { ext: APP, key }).catch(() => null);
        return (v ?? fallback) as T;
    }
    function appSet(key: string, value: unknown) {
        invoke("storage_set", { ext: APP, key, value }).catch(() => { });
    }

    // Le thème s'applique au DOM de LA fenêtre courante (chaque webview a le sien).
    function applyTheme(t: string) {
        document.documentElement.style.colorScheme = t;
        document.documentElement.classList.toggle("dark", t === "dark"); // tokens shadcn
    }

    // APPLY SETTINGS
    function onTheme() {
        applyTheme(theme.value);
        appSet("theme", theme.value);
        // Broadcast → toutes les autres fenêtres (overlay/île, modal…) réappliquent.
        emit("theme://changed", theme.value).catch(() => { });
    }
    function onLang() {
        appSet("lang", lang.value);
        emit("lang://changed", lang.value).catch(() => { });
    }
    async function onAutostart() {
        try {
            if (autostart.value) await enableAutostart();
            else await disableAutostart();
        } catch { /* noop */ }
    }
    // À appeler une fois par fenêtre (depuis main.ts). Charge les prefs, applique
    // le thème localement, et écoute les changements émis par les autres fenêtres.
    let started = false;
    async function init() {
        if (started) return;
        started = true;
        theme.value = await appGet("theme", "dark");
        lang.value = await appGet("lang", "fr");
        try { autostart.value = await isAutostartEnabled(); } catch { /* noop */ }
        applyTheme(theme.value);

        await listen<string>("theme://changed", (e) => {
            theme.value = e.payload;
            applyTheme(e.payload);
        });
        await listen<string>("lang://changed", (e) => { lang.value = e.payload; });
    }

    return {
        theme,
        lang,
        autostart,
        appGet,
        appSet,
        onTheme,
        onLang,
        onAutostart,
        init,
    }
})
