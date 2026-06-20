<script setup lang="ts">
// Fenêtre overlay unique (transparente, plein écran) : héberge l'île et la modal
// comme composants d'une même app Vue. Le click-through est géré nativement
// (poll GetCursorPos côté Rust + régions publiées par chaque surface).
import { onMounted, computed } from "vue";
import Island from "./Island.vue";
import Modal from "./Modal.vue";
import FloatWindows from "./FloatWindows.vue";
import RegionSelect from "./RegionSelect.vue";
import { loadExtensions } from "../composables/extensions";
import { selecting, regionOutline } from "../composables/overlay";
import { initNotifications } from "../composables/notifications";

onMounted(() => {
  loadExtensions();
  initNotifications();
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
  <div class="fixed inset-0 w-screen h-screen overflow-hidden bg-transparent">
    <Modal />
  </div>
  <RegionSelect v-if="selecting" />
  <div
    v-if="outlineStyle"
    class="region-outline pointer-events-none fixed z-[9998] border-2 border-[#ff453a]"
    :style="outlineStyle"
  ></div>
</template>

<style scoped>
.region-outline {
  border-radius: 2px;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.3), inset 0 0 0 1px rgba(0, 0, 0, 0.3);
  animation: outline-pulse 1.6s ease-in-out infinite;
}
@keyframes outline-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
</style>