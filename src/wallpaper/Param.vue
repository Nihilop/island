<script setup lang="ts">
// Réglage numérique de l'éditeur : libellé, curseur, valeur.
import { computed } from "vue";
import { Slider } from "@/sdk/ui/slider";

const props = withDefaults(defineProps<{ label: string; min: number; max: number; step?: number; unit?: string; digits?: number }>(), { step: 0.01, unit: "", digits: 2 });
const model = defineModel<number>({ required: true });
const shown = computed(() => `${model.value.toFixed(props.digits)}${props.unit}`);
</script>

<template>
  <label class="flex items-center gap-3 py-1">
    <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ label }}</span>
    <Slider class="flex-1" :model-value="[model]" :min="min" :max="max" :step="step" @update:model-value="(v) => v && (model = v[0])" />
    <span class="type-caption w-11 shrink-0 text-right tabular-nums text-label-3">{{ shown }}</span>
  </label>
</template>
