<script setup lang="ts">
// Fenêtre overlay = fenêtre fixe du notch (760 × 560, haut-centre). v2 : le loader
// d'extensions est gelé (cf. docs/internal/v2-migration.md) ; Island.vue, la modal et
// les fenêtres flottantes ne sont plus montés.
import { onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Notch from "../notch/Notch.vue";
import { state } from "../notch/state";
import { registerBuiltinTools } from "../tools/builtin";

import { loadSettings, settings } from "../notch/settings";
import { initTheme } from "../notch/theme";
import { startAutoUpdate } from "../composables/updater";
import { initChangelog } from "../composables/changelog";
import { initNowPlaying } from "../tools/media/store";
import { initVault } from "../tools/vault/store";
import { initTimer } from "../tools/timer/store";
import { initWeather } from "../tools/weather/store";
import { initClipboard } from "../tools/clipboard/store";
import { initShelf } from "../tools/shelf/store";
import { initDrop } from "../notch/drop";
import { initHardware } from "../tools/system/store";
import { initPresentation } from "../notch/presentation";
import { initDiscord } from "../tools/discord/store";
import { initGaming } from "../tools/system/gaming";
import { initActivity, setActivitySettings } from "../tools/activity/store";
import { initAutomations } from "../automations/store";
import { initAsk } from "../tools/ask/store";
import { initHello } from "../notch/hello";
import { initNotifs } from "../tools/notifications/store";
import { initBudget } from "../tools/budget/store";
import { initTodos } from "../tools/todos/store";
import { initLauncher } from "../launcher/store";
import { initExtensionsHost } from "../extensions/install";
import { initWallpaperShare } from "../wallpaper/share";

registerBuiltinTools();

// Le notch se monte une fois les réglages chargés (raccourci, outils, style…).
const ready = ref(false);
loadSettings().finally(() => {
  ready.value = true;
  initTheme();
  startAutoUpdate(() => settings.autoUpdate);
  initHello();
  initNotifs();
  initBudget();
  initTodos();
  initLauncher();
  void initActivity();
  void initAutomations();
  // Options du service Activité (Rust) : poussées au démarrage et à chaque changement.
  watch(
    () => ({ trackApps: settings.activity.trackApps, discordDb: settings.activity.discordDb }),
    (s) => setActivitySettings(s).catch(() => {}),
    { immediate: true },
  );
  void initExtensionsHost();
  initWallpaperShare();
  watch(
    () => settings.vaultTimeout,
    (seconds) => invoke("vault_set_timeout", { seconds }).catch(() => {}),
    { immediate: true },
  );
});
initChangelog();
initNowPlaying();
initVault();
initTimer();
initWeather();
initClipboard();
initShelf();
initDrop();
initHardware();
initPresentation();
initDiscord();
initGaming();
initAsk();

onMounted(() => {

  // Garde-fou mémoire : si le watchdog Rust signale une conso trop haute, on recharge
  // l'overlay AU REPOS (notch fermé) → repart propre.
  listen<number>("island://reclaim", (e) => {
    if (state.value !== "closed") return; // occupé → on attend le prochain tick
    console.warn(`[island] récupération mémoire (${e.payload} Mo) — rechargement de l'overlay`);
    location.reload();
  });
});
</script>

<template>
  <Notch v-if="ready" />
</template>
