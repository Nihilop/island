<script setup lang="ts">
// Rend les fenêtres flottantes draggables de l'overlay. Barre minimale (drag + ✕).
// Chaque fenêtre publie sa zone comme hit-region → l'overlay capte la souris dessus.
import { watch } from "vue";
import { floatWindows, focusWindow, closeWindow, setHitRegion } from "../composables/overlay";

// Synchronise les hit-regions avec les fenêtres ouvertes (création / drag / fermeture).
const winKeys = new Set<string>();
watch(
  floatWindows,
  (wins) => {
    const live = new Set(wins.map((w) => `fwin:${w.id}`));
    for (const k of [...winKeys]) {
      if (!live.has(k)) {
        setHitRegion(k, null);
        winKeys.delete(k);
      }
    }
    for (const w of wins) {
      const k = `fwin:${w.id}`;
      setHitRegion(k, { x: w.x, y: w.y, w: w.width, h: w.height });
      winKeys.add(k);
    }
  },
  { deep: true, immediate: true },
);

// Drag par la barre de titre.
let drag: { id: string; dx: number; dy: number } | null = null;
function startDrag(id: string, e: PointerEvent) {
  const w = floatWindows.value.find((x) => x.id === id);
  if (!w) return;
  focusWindow(id);
  drag = { id, dx: e.clientX - w.x, dy: e.clientY - w.y };
  window.addEventListener("pointermove", onDrag);
  window.addEventListener("pointerup", endDrag);
}
function onDrag(e: PointerEvent) {
  if (!drag) return;
  const w = floatWindows.value.find((x) => x.id === drag!.id);
  if (!w) return;
  w.x = Math.min(Math.max(0, e.clientX - drag.dx), window.innerWidth - 80);
  w.y = Math.min(Math.max(0, e.clientY - drag.dy), window.innerHeight - 28);
}
function endDrag() {
  drag = null;
  window.removeEventListener("pointermove", onDrag);
  window.removeEventListener("pointerup", endDrag);
}
</script>

<template>
  <div
    v-for="w in floatWindows"
    :key="w.id"
    class="fixed flex flex-col overflow-hidden rounded-xl border bg-background shadow-2xl pointer-events-auto select-none"
    :style="{ left: w.x + 'px', top: w.y + 'px', width: w.width + 'px', height: w.height + 'px', zIndex: w.z }"
    @pointerdown="focusWindow(w.id)"
  >
    <div class="flex h-7 flex-none items-center justify-between gap-2 px-2 cursor-move bg-foreground/[0.06]" @pointerdown.stop="startDrag(w.id, $event)">
      <span class="truncate text-[11px] text-muted-foreground">{{ w.title }}</span>
      <button class="grid h-5 w-5 flex-none place-items-center rounded text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground" @pointerdown.stop @click="closeWindow(w.id)" aria-label="Fermer">
        <svg viewBox="0 0 24 24" class="h-3 w-3"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M6 6l12 12M18 6 6 18" /></svg>
      </button>
    </div>
    <div class="min-h-0 flex-1">
      <component :is="w.component" />
    </div>
  </div>
</template>
