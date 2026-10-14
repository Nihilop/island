<script setup lang="ts">
// Anneau de compte à rebours (18 px) : la progression décroît chaque seconde en 1 s
// linéaire, rouge dans les 5 dernières secondes, saut sans animation au renouvellement.
import { computed, ref, watch } from "vue";
import { PhLock } from "@phosphor-icons/vue";

const props = defineProps<{ remaining: number; period: number; locked?: boolean }>();

const R = 7;
const C = 2 * Math.PI * R;
const jump = ref(false);
watch(
  () => props.remaining,
  (now, before) => {
    jump.value = before !== undefined && now > before;
  },
);
const offset = computed(() => C * (1 - Math.max(0, props.remaining - 1) / props.period));
</script>

<template>
  <span class="relative inline-grid size-[18px] place-items-center">
    <svg width="18" height="18" viewBox="0 0 18 18" class="-rotate-90 fill-none [stroke-width:2]">
      <circle cx="9" cy="9" :r="R" class="stroke-fill-2" />
      <circle
        v-if="!locked"
        cx="9"
        cy="9"
        :r="R"
        :class="[
          '[stroke-linecap:round]',
          remaining <= 5 ? 'stroke-sys-red' : 'stroke-label-2',
          jump ? 'transition-none' : 'transition-[stroke-dashoffset,stroke] duration-1000 ease-linear',
        ]"
        :stroke-dasharray="C"
        :stroke-dashoffset="offset"
      />
    </svg>
    <PhLock v-if="locked" :size="9" weight="fill" class="absolute text-label-3" />
  </span>
</template>
