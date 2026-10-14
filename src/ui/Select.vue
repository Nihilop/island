<script setup lang="ts" generic="T">
// Sélecteur sur le Select shadcn (pas de <select> natif, étranger sous Windows). Le menu est
// téléporté : il reste cliquable hors de la forme grâce à `notch/portals.ts`. Les valeurs
// peuvent être de tout type (null compris) : le Select travaille sur l'index.
import { computed } from "vue";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/sdk/ui/select";

const props = defineProps<{ options: { value: T; label: string }[]; placeholder?: string }>();
const model = defineModel<T>({ required: true });

const index = computed({
  get: () => {
    const i = props.options.findIndex((o) => o.value === model.value);
    return i < 0 ? undefined : String(i);
  },
  set: (i: string | undefined) => {
    if (i !== undefined && props.options[+i]) model.value = props.options[+i].value;
  },
});
</script>

<template>
  <Select v-model="index">
    <SelectTrigger data-no-drag>
      <SelectValue :placeholder="placeholder" />
    </SelectTrigger>
    <SelectContent position="popper" :side-offset="4">
      <SelectItem v-for="(o, i) in options" :key="i" :value="String(i)">{{ o.label }}</SelectItem>
    </SelectContent>
  </Select>
</template>
