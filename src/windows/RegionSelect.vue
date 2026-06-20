<script setup lang="ts">
// Couche plein écran de sélection de zone. Affichée par l'overlay quand une
// extension appelle island.capture.selectRegion(). Renvoie un rect en pixels
// PHYSIQUES (× devicePixelRatio) relatif au moniteur de l'overlay.
import { ref, onMounted, onUnmounted } from "vue";
import { finishRegion, setHitRegion } from "../composables/overlay";

const active = ref(false); // un drag est en cours
const rect = ref({ x: 0, y: 0, w: 0, h: 0 }); // CSS px
let startX = 0;
let startY = 0;

function onDown(e: PointerEvent) {
  if (e.button !== 0) return;
  active.value = true;
  startX = e.clientX;
  startY = e.clientY;
  rect.value = { x: startX, y: startY, w: 0, h: 0 };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}
function onMove(e: PointerEvent) {
  const x = Math.min(startX, e.clientX);
  const y = Math.min(startY, e.clientY);
  rect.value = { x, y, w: Math.abs(e.clientX - startX), h: Math.abs(e.clientY - startY) };
}
function onUp() {
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("pointerup", onUp);
  const r = rect.value;
  const dpr = window.devicePixelRatio || 1;
  if (r.w < 8 || r.h < 8) {
    finishRegion(null); // trop petit = annulation
    return;
  }
  finishRegion({
    x: Math.round(r.x * dpr),
    y: Math.round(r.y * dpr),
    w: Math.round(r.w * dpr),
    h: Math.round(r.h * dpr),
  });
}
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") finishRegion(null);
}

onMounted(() => {
  // Capture la souris sur tout l'écran (désactive le click-through le temps de la sélection).
  // Taille ÉCRAN (pas window.innerWidth) : l'overlay vient de passer plein écran de façon
  // async → innerWidth peut encore valoir la taille de la petite boîte.
  setHitRegion("region-select", { x: 0, y: 0, w: window.screen.width, h: window.screen.height });
  window.addEventListener("keydown", onKey, true);
});
onUnmounted(() => {
  setHitRegion("region-select", null);
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("pointerup", onUp);
});
</script>

<template>
  <div class="fixed inset-0 z-[9999] cursor-crosshair select-none" @pointerdown="onDown">
    <!-- Voile avant le drag -->
    <div v-if="!active" class="absolute inset-0 bg-black/30"></div>

    <!-- Rectangle de sélection : le box-shadow assombrit tout autour -->
    <div
      v-else
      class="absolute border border-white/90"
      :style="{
        left: rect.x + 'px',
        top: rect.y + 'px',
        width: rect.w + 'px',
        height: rect.h + 'px',
        boxShadow: '0 0 0 100vmax rgba(0,0,0,.45)',
      }"
    >
      <span class="absolute -top-5 left-0 rounded bg-black/70 px-1.5 py-0.5 text-[11px] tabular-nums text-white">
        {{ rect.w }}×{{ rect.h }}
      </span>
    </div>

    <!-- Aide -->
    <div class="absolute left-1/2 top-5 -translate-x-1/2 rounded-full bg-black/65 px-3 py-1 text-[12px] text-white/90">
      Glisse pour sélectionner une zone · Échap pour annuler
    </div>
  </div>
</template>
