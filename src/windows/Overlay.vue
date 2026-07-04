<script setup lang="ts">
// Fenêtre overlay unique (transparente, plein écran) : héberge l'île et la modal
// comme composants d'une même app Vue. Le click-through est géré nativement
// (poll GetCursorPos côté Rust + régions publiées par chaque surface).
import { onMounted, computed } from "vue";
import { listen } from "@tauri-apps/api/event";
import Island from "./Island.vue";
import Modal from "./Modal.vue";
import FloatWindows from "./FloatWindows.vue";
import MinimizedDock from "./MinimizedDock.vue";
import RegionSelect from "./RegionSelect.vue";
import { loadExtensions } from "../composables/extensions";
import { selecting, regionOutline, activeView, modalSpec } from "../composables/overlay";
import { initNotifications } from "../composables/notifications";
import { initIslandTheme } from "../composables/islandTheme";
import { checkForUpdate } from "../composables/updater";

onMounted(() => {
  loadExtensions();
  initNotifications();
  initIslandTheme(); // charge le thème de l'île persisté + suit les changements (Réglages)
  // Vérif de mise à jour peu après le démarrage (silencieuse s'il n'y a rien).
  window.setTimeout(() => void checkForUpdate(), 4000);

  // Garde-fou mémoire : si le watchdog Rust signale une conso trop haute, on recharge
  // l'overlay AU REPOS (rien d'ouvert) → repart propre, réactive les extensions, storage
  // conservé. Jamais pendant une interaction (view/modal/sélection ouverte).
  listen<number>("island://reclaim", (e) => {
    if (activeView.value || modalSpec.value || selecting.value) return; // occupé → on attend le prochain tick
    console.warn(`[island] récupération mémoire (${e.payload} Mo) — rechargement de l'overlay`);
    location.reload();
  });
});

// Le contour est stocké en px PHYSIQUES → on reconvertit en CSS px (÷ dpr).
const outlineStyle = computed(() => {
  const r = regionOutline.value;
  if (!r) return null;
  const dpr = window.devicePixelRatio || 1;
  return {
    left: r.x / dpr + "px",
    top: r.y / dpr + "px",
    width: r.w / dpr + "px",
    height: r.h / dpr + "px",
  };
});
</script>

<template>
  <Island />
  <FloatWindows />
  <MinimizedDock />
  <div class="fixed inset-0 w-screen h-screen overflow-hidden bg-transparent">
    <Modal />
  </div>
  <RegionSelect v-if="selecting" />
  <div
    v-if="outlineStyle"
    class="pointer-events-none fixed z-[9998] rounded-[2px] border-2 border-[#ff453a] shadow-[0_0_0_1px_rgba(0,0,0,0.3),inset_0_0_0_1px_rgba(0,0,0,0.3)] animate-[outline-pulse_1.6s_ease-in-out_infinite]"
    :style="outlineStyle"
  ></div>
</template>

<style scoped>
/* Keyframes only (non exprimable en classe) — l'animation est appliquée en inline. */
@keyframes outline-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
</style>