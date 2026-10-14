<script setup lang="ts" generic="T">
// Contrôle segmenté sur ToggleGroup (shadcn) : piste fill-1 en pilule, pastille fill-3 qui
// glisse vers l'option active (spring snappy). Les valeurs peuvent être de tout type
// (booléen, nombre, null…) : le ToggleGroup travaille sur l'index.
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { motion } from "motion-v";
import { ToggleGroup, ToggleGroupItem } from "@/sdk/ui/toggle-group";
import { spring } from "../design/motion";

const props = withDefaults(defineProps<{ options: { value: T; label: string }[]; size?: "sm" | "md" }>(), {
  size: "md",
});
const model = defineModel<T>({ required: true });

const index = computed({
  get: () => String(props.options.findIndex((o) => o.value === model.value)),
  set: (i: string) => {
    // Un ToggleGroup « single » peut se désélectionner : on ignore.
    if (i !== "" && i !== undefined && props.options[+i]) model.value = props.options[+i].value;
  },
});

const root = ref<{ $el: HTMLElement }>();
const pill = ref({ x: 0, w: 0 });
const ready = ref(false);

function measure() {
  const el = root.value?.$el?.querySelectorAll<HTMLElement>("[data-slot=toggle-group-item]")[+index.value];
  if (el) pill.value = { x: el.offsetLeft, w: el.offsetWidth };
}

watch([model, () => props.options], () => nextTick(measure));
onMounted(() => {
  measure();
  requestAnimationFrame(() => (ready.value = true));
});
</script>

<template>
  <ToggleGroup ref="root" v-model="index" type="single" :size="size === 'sm' ? 'sm' : 'default'" data-no-drag>
    <motion.div
      class="absolute inset-y-0.5 left-0 rounded-full bg-fill-3"
      :initial="false"
      :animate="{ x: pill.x, width: pill.w }"
      :transition="ready ? spring('snappy') : { duration: 0 }"
    />
    <ToggleGroupItem v-for="(o, i) in options" :key="i" :value="String(i)">{{ o.label }}</ToggleGroupItem>
  </ToggleGroup>
</template>
