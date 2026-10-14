<script setup lang="ts">
// Courbe d'une minute (160 × 22) : trait 1,5 px à 80 %, ligne de base, trous pour les
// valeurs manquantes. Défilement fluide : la courbe est dessinée décalée d'un pas et
// glisse d'un pas vers la gauche en 1 s linéaire à chaque nouvelle valeur.
import { computed, nextTick, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{ values: (number | null)[]; max?: number; tick: number; color?: string; width?: number; height?: number }>(),
  { max: 100, color: "var(--label)", width: 160, height: 22 },
);

const step = computed(() => props.width / (props.values.length - 1));

const path = computed(() => {
  const h = props.height - 2;
  let d = "";
  let pen = false;
  props.values.forEach((v, i) => {
    if (v === null) {
      pen = false;
      return;
    }
    const x = (i + 1) * step.value; // décalé d'un pas (glisse vers la gauche)
    const y = 1 + h - (Math.min(props.max, Math.max(0, v)) / props.max) * h;
    d += `${pen ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)} `;
    pen = true;
  });
  return d;
});

const shift = ref(0);
const animate = ref(false);
watch(
  () => props.tick,
  async () => {
    animate.value = false;
    shift.value = 0;
    await nextTick();
    requestAnimationFrame(() => {
      animate.value = true;
      shift.value = -step.value;
    });
  },
);
</script>

<template>
  <svg :width="width" :height="height" class="overflow-hidden" aria-hidden="true">
    <line :x1="0" :x2="width" :y1="height - 0.5" :y2="height - 0.5" class="stroke-separator [stroke-width:1]" />
    <g :style="{ transform: `translateX(${shift}px)`, transition: animate ? 'transform 1s linear' : 'none' }">
      <path
        :d="path"
        :stroke="color"
        class="fill-none opacity-80 transition-[stroke] duration-300 [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:1.5]"
      />
    </g>
  </svg>
</template>
