<script setup lang="ts">
// Debug du ciel (dev seulement, bouton fiole de l'en-tête) : force une ambiance, un moment,
// l'intensité, le vent et la course du soleil ; déclenche un éclair. « Simulation » coupée =
// retour à la vraie météo.
import type { Component } from "vue";
import {
  PhCloud,
  PhCloudFog,
  PhCloudLightning,
  PhCloudRain,
  PhCloudSnow,
  PhCloudSun,
  PhDrop,
  PhLightning,
  PhSun,
  PhThermometerHot,
  PhX,
} from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Switch } from "@/sdk/ui/switch";
import { Slider } from "@/sdk/ui/slider";
import Segmented from "../../ui/Segmented.vue";
import { MOODS, PHASES, flashRequest, skyDebug, type Mood, type Phase } from "./sky";
import { t } from "../../notch/i18n";

defineEmits<{ close: [] }>();

const ICON: Record<Mood, Component> = {
  clear: PhSun,
  partly: PhCloudSun,
  cloudy: PhCloud,
  fog: PhCloudFog,
  drizzle: PhDrop,
  rain: PhCloudRain,
  storm: PhCloudLightning,
  snow: PhCloudSnow,
  heat: PhThermometerHot,
};

const pick = (m: Mood) => {
  skyDebug.mood = m;
  skyDebug.on = true;
};
const phases = PHASES.map((p) => ({ value: p as Phase, label: t(`weather.debug.phase.${p}`) }));

/** Slider shadcn (tableau) ↔ valeur 0-1 de l'état de debug. */
const bind = (key: "intensity" | "wind" | "arc") => ({
  modelValue: [Math.round(skyDebug[key] * 100)],
  "onUpdate:modelValue": (v: number[] | undefined) => {
    if (!v) return;
    skyDebug[key] = v[0] / 100;
    skyDebug.on = true;
  },
});
</script>

<template>
  <div
    class="flex w-[264px] flex-col gap-2.5 overflow-y-auto rounded-[16px] bg-[color-mix(in_srgb,var(--elevated)_88%,transparent)] p-3 ring-1 ring-label/10 backdrop-blur-xl [scrollbar-width:none] shadow-[0_12px_32px_-10px_rgb(0_0_0/0.6)]"
    data-no-drag
  >
    <div class="flex items-center gap-2">
      <span class="type-footnote flex-1 font-semibold">{{ t("weather.debug.title") }}</span>
      <span class="type-caption text-label-3">{{ t("weather.debug.simulate") }}</span>
      <Switch v-model="skyDebug.on" />
      <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('common.close')" @click="$emit('close')"><PhX weight="bold" /></Button>
    </div>

    <div class="grid grid-cols-3 gap-1">
      <button
        v-for="m in MOODS"
        :key="m"
        type="button"
        class="type-caption flex h-12 flex-col items-center justify-center gap-1 rounded-[10px] transition-colors duration-100"
        :class="skyDebug.on && skyDebug.mood === m ? 'bg-label text-notch' : 'bg-fill-1 text-label-2 hover:bg-fill-2'"
        @click="pick(m)"
      >
        <component :is="ICON[m]" :size="16" weight="fill" />
        {{ t(`weather.debug.mood.${m}`) }}
      </button>
    </div>

    <Segmented v-model="skyDebug.phase" :options="phases" size="sm" @update:model-value="skyDebug.on = true" />

    <label class="flex flex-col gap-1.5">
      <span class="type-caption flex justify-between text-label-2">{{ t("weather.debug.intensity") }}<span class="tabular-nums text-label-3">{{ Math.round(skyDebug.intensity * 100) }} %</span></span>
      <Slider v-bind="bind('intensity')" :min="0" :max="100" :step="1" />
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="type-caption flex justify-between text-label-2">{{ t("weather.debug.wind") }}<span class="tabular-nums text-label-3">{{ Math.round(skyDebug.wind * 100) }} %</span></span>
      <Slider v-bind="bind('wind')" :min="0" :max="100" :step="1" />
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="type-caption flex justify-between text-label-2">{{ t("weather.debug.arc") }}<span class="tabular-nums text-label-3">{{ Math.round(skyDebug.arc * 100) }} %</span></span>
      <Slider v-bind="bind('arc')" :min="0" :max="100" :step="1" />
    </label>

    <Button size="sm" variant="secondary" :disabled="!skyDebug.on || skyDebug.mood !== 'storm'" @click="flashRequest++">
      <PhLightning weight="fill" />{{ t("weather.debug.flash") }}
    </Button>
  </div>
</template>
