<script setup lang="ts">
// Égaliseur 4 barres (graves à gauche). Une seule boucle d'animation pour tous les états,
// valeurs écrites directement en scaleY (aucun rendu Vue par frame) :
//   - lecture avec niveaux réels → suit le son (plancher 0,2) ;
//   - silence en lecture (> 150 ms) ou pas de niveaux (navigateur) → « respiration »
//     synthétique, mélangée progressivement sur 350 ms ;
//   - pause (débouncée 320 ms) → hauteur calme uniforme 0,24.
// Lissage exponentiel : attaque 45 ms, relâchement 140 ms, retour au repos 180 ms.
// Plafond ~60 fps ; la boucle s'endort en pause une fois stabilisée.
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { np } from "../tools/media/store";
import { lastLevels, lastSound, levels, useEqSubscription } from "./eq";

const props = withDefaults(defineProps<{ height?: number; color?: string }>(), { height: 12 });

const bars = ref<HTMLElement[]>([]);
const FLOOR = 0.2;
const REST = 0.24;
const MIN_FRAME = 13;

let raf = 0;
let lastFrame = 0;
let mix = 0; // 0 = niveaux réels, 1 = synthétique
let paused = !np.playing;
let pauseTimer: number | undefined;
const cur = [REST, REST, REST, REST];

watch(
  () => np.playing,
  (playing) => {
    window.clearTimeout(pauseTimer);
    if (playing) {
      paused = false;
      wake();
    } else pauseTimer = window.setTimeout(() => (paused = true), 320);
  },
);

function synth(i: number, t: number): number {
  const s = Math.sin(t * (2.1 + i * 0.7) + i * 1.7) * 0.5 + Math.sin(t * (3.3 + i * 0.45) + i) * 0.3;
  return 0.45 + s * 0.35;
}

function frame(now: number) {
  raf = 0;
  if (now - lastFrame < MIN_FRAME) return wake();
  const dt = lastFrame ? Math.min(100, now - lastFrame) : 16;
  lastFrame = now;
  const t = now / 1000;

  const live = now - lastLevels < 300;
  const silent = now - lastSound > 150;
  const wantSynth = !live || silent ? 1 : 0;
  mix += (wantSynth - mix) * Math.min(1, dt / 350);

  let settled = true;
  for (let i = 0; i < 4; i++) {
    let target: number;
    if (paused) target = REST;
    else {
      const real = Math.max(FLOOR, levels[i]);
      target = real * (1 - mix) + synth(i, t) * mix;
    }
    const tau = paused ? 180 : target > cur[i] ? 45 : 140;
    cur[i] += (target - cur[i]) * (1 - Math.exp(-dt / tau));
    if (Math.abs(target - cur[i]) > 0.002) settled = false;
    const el = bars.value[i];
    if (el) el.style.transform = `scaleY(${cur[i].toFixed(3)})`;
  }
  if (!(paused && settled)) wake();
}

function wake() {
  if (!raf) raf = requestAnimationFrame(frame);
}

let release: () => void = () => {};
onMounted(() => {
  release = useEqSubscription();
  wake();
});
onBeforeUnmount(() => {
  release();
  cancelAnimationFrame(raf);
  window.clearTimeout(pauseTimer);
});
</script>

<template>
  <div
    class="inline-flex items-center gap-0.5 transition-colors duration-400 ease-linear"
    :style="{ height: `${height}px`, color: color ?? 'var(--tint)' }"
    aria-hidden="true"
  >
    <span v-for="i in 4" :key="i" ref="bars" class="h-full w-[2.5px] origin-center rounded-full bg-current" style="transform: scaleY(0.24)" />
  </div>
</template>
