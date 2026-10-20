<script setup lang="ts">
// « Visible quand… » d'un calque : moments de la journée réels (aube, jour, crépuscule,
// nuit, d'après le lever / coucher du soleil de la Météo) ou une plage horaire, ciel réel
// (pluie, neige…), durée du fondu. Tous les critères choisis doivent être vrais.
import { computed } from "vue";
import { Switch } from "@/sdk/ui/switch";
import Segmented from "../ui/Segmented.vue";
import Param from "./Param.vue";
import { DEFAULTS, type Layer, type When } from "./scene";
import { MOMENTS, SKIES, current, visibleWhen } from "./conditions";
import { t } from "../notch/i18n";

const props = defineProps<{ layer: Layer }>();
const w = computed(() => props.layer.when as When);

function toggle(on: boolean) {
  props.layer.when = on ? DEFAULTS.when() : null;
}
const mode = computed({
  get: () => (w.value.from !== null && w.value.to !== null ? "hours" : "moments"),
  set: (m: "moments" | "hours") => {
    if (m === "hours") Object.assign(w.value, { from: "22:00", to: "06:00" });
    else Object.assign(w.value, { from: null, to: null });
  },
});
function flip<T extends string>(list: T[], v: T) {
  const i = list.indexOf(v);
  if (i >= 0) list.splice(i, 1);
  else list.push(v);
}
/** Le calque serait-il visible maintenant (état réel ou simulé) ? */
const now = computed(() => (props.layer.when ? visibleWhen(props.layer.when, current()) : true));
const chip = (on: boolean) => ["type-caption rounded-full px-2.5 py-1 transition-colors", on ? "bg-sys-link text-white" : "bg-fill-2 text-label-2 hover:bg-fill-3"];
</script>

<template>
  <section class="space-y-2">
    <div class="flex items-center justify-between">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.when.title") }}</h3>
      <Switch :model-value="!!layer.when" @update:model-value="toggle" />
    </div>
    <p v-if="!layer.when" class="type-caption text-label-3">{{ t("wall.when.off") }}</p>
    <template v-else>
      <Segmented
        v-model="mode"
        size="sm"
        :options="[
          { value: 'moments', label: t('wall.when.moments') },
          { value: 'hours', label: t('wall.when.hours') },
        ]"
      />
      <div v-if="mode === 'moments'" class="flex flex-wrap gap-1">
        <button v-for="m in MOMENTS" :key="m" type="button" :class="chip(w.moments.includes(m))" @click="flip(w.moments, m)">{{ t(`wall.when.m.${m}`) }}</button>
      </div>
      <div v-else class="flex items-center gap-2">
        <input v-model="w.from" type="time" class="type-footnote rounded-[6px] bg-fill-2 px-2 py-1 outline-none" />
        <span class="type-caption text-label-3">→</span>
        <input v-model="w.to" type="time" class="type-footnote rounded-[6px] bg-fill-2 px-2 py-1 outline-none" />
      </div>
      <div class="type-caption pt-1 font-medium text-label-3">{{ t("wall.when.sky") }}</div>
      <div class="flex flex-wrap gap-1">
        <button v-for="k in SKIES" :key="k" type="button" :class="chip(w.sky.includes(k))" @click="flip(w.sky, k)">{{ t(`wall.when.s.${k}`) }}</button>
      </div>
      <Param v-model="w.fade" :label="t('wall.when.fade')" :min="0" :max="10" :step="0.5" unit=" s" :digits="1" />
      <p class="type-caption" :class="now ? 'text-sys-green' : 'text-label-3'">{{ now ? t("wall.when.shownNow") : t("wall.when.hiddenNow") }}</p>
    </template>
  </section>
</template>
