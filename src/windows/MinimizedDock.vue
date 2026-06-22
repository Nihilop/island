<script setup lang="ts">
// Sphères des fenêtres MINIMISÉES, détachées À DROITE de l'île (ancrées sur islandRect).
// Clic = restaure la fenêtre. Chaque sphère porte l'icône passée à window.open({ icon }).
import { computed, watch } from "vue";
import { floatWindows, restoreWindow, islandRect, setHitRegion } from "../composables/overlay";

const mins = computed(() => floatWindows.value.filter((w) => w.minimized));

const dockStyle = computed(() => {
  const r = islandRect.value;
  if (!r) return { display: "none" } as Record<string, string>;
  return { left: r.x + r.w + 10 + "px", top: r.y + r.h / 2 + "px", transform: "translateY(-50%)" };
});

// Hit-region : sans ça l'overlay laisse passer le clic (click-through).
const SPHERE = 26, GAP = 6;
watch(
  [mins, islandRect],
  () => {
    const r = islandRect.value;
    if (!r || !mins.value.length) { setHitRegion("min-dock", null); return; }
    const w = mins.value.length * SPHERE + (mins.value.length - 1) * GAP;
    setHitRegion("min-dock", { x: Math.round(r.x + r.w + 6), y: Math.round(r.y + r.h / 2 - SPHERE / 2 - 2), w: w + 8, h: SPHERE + 4 });
  },
  { deep: true, immediate: true },
);
</script>

<template>
  <div v-if="mins.length" class="fixed z-[150] flex items-center gap-1.5 pointer-events-auto select-none" :style="dockStyle">
    <button
      v-for="w in mins" :key="w.id"
      class="grid size-[26px] place-items-center rounded-full border border-border bg-background text-muted-foreground shadow-lg transition hover:scale-110 hover:text-foreground [&_svg]:size-3.5"
      :title="w.title"
      @click="restoreWindow(w.id)"
    >
      <span v-if="w.icon" v-html="w.icon"></span>
      <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" /></svg>
    </button>
  </div>
</template>
