<script setup lang="ts">
// Propriétés de la scène (élément « Scène » de l'outliner) : effets globaux — parallaxe
// (la profondeur se règle par calque), couleur, grain, vignette — et interactions souris.
import { Switch } from "@/sdk/ui/switch";
import Param from "./Param.vue";
import { DEFAULTS, type Scene } from "./scene";
import Select from "../ui/Select.vue";
import { MOMENTS, SKIES, real, simulation, type Moment, type Sky } from "./conditions";
import { t } from "../notch/i18n";

const props = defineProps<{ scene: Scene }>();

/** Aperçu des conditions : la scène telle qu'elle serait à un autre moment / sous un autre ciel. */
const momentOptions = [{ value: "", label: t("wall.sim.real") }, ...MOMENTS.map((m) => ({ value: m, label: t(`wall.when.m.${m}`) }))];
const skyOptions = [{ value: "", label: t("wall.sim.real") }, ...SKIES.map((k) => ({ value: k, label: t(`wall.when.s.${k}`) }))];
const usesConditions = () => props.scene.layers.some((l) => l.when);

type Key = "parallax" | "color" | "grain" | "vignette" | "ripple" | "trail";
function isOn(k: Key): boolean {
  const s = props.scene;
  if (k === "ripple" || k === "trail") return !!s.interactions[k];
  return s[k] !== null;
}
function setOn(k: Key, on: boolean) {
  const s = props.scene;
  if (k === "ripple") s.interactions.ripple = on ? DEFAULTS.ripple() : null;
  else if (k === "trail") s.interactions.trail = on ? DEFAULTS.trail() : null;
  else if (k === "grain") s.grain = on ? 0.2 : null;
  else if (k === "vignette") s.vignette = on ? 0.4 : null;
  else if (k === "parallax") s.parallax = on ? DEFAULTS.parallax() : null;
  else s.color = on ? DEFAULTS.color() : null;
}
</script>

<template>
  <div class="space-y-4">
    <section class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.sceneEffects") }}</h3>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.parallax") }}</span>
          <Switch :model-value="isOn('parallax')" @update:model-value="setOn('parallax', $event)" />
        </div>
        <div v-if="scene.parallax" class="pt-1">
          <Param v-model="scene.parallax.mouse" :label="t('wall.p.mouse')" :min="0" :max="1" />
          <Param v-model="scene.parallax.drift" :label="t('wall.p.drift')" :min="0" :max="1" />
          <Param v-model="scene.parallax.zoom" :label="t('wall.p.zoom')" :min="1" :max="1.3" />
          <p class="type-caption pb-1 text-label-3">{{ t("wall.parallaxHint") }}</p>
        </div>
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.color") }}</span>
          <Switch :model-value="isOn('color')" @update:model-value="setOn('color', $event)" />
        </div>
        <div v-if="scene.color" class="pt-1">
          <Param v-model="scene.color.hue" :label="t('wall.p.hue')" :min="-180" :max="180" :step="1" unit="°" :digits="0" />
          <Param v-model="scene.color.saturation" :label="t('wall.p.saturation')" :min="0" :max="2" />
          <Param v-model="scene.color.brightness" :label="t('wall.p.brightness')" :min="0" :max="2" />
          <Param v-model="scene.color.contrast" :label="t('wall.p.contrast')" :min="0" :max="2" />
        </div>
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.grain") }}</span>
          <Switch :model-value="isOn('grain')" @update:model-value="setOn('grain', $event)" />
        </div>
        <Param v-if="scene.grain !== null" v-model="scene.grain" :label="t('wall.p.amount')" :min="0" :max="1" />
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.vignette") }}</span>
          <Switch :model-value="isOn('vignette')" @update:model-value="setOn('vignette', $event)" />
        </div>
        <Param v-if="scene.vignette !== null" v-model="scene.vignette" :label="t('wall.p.amount')" :min="0" :max="1" />
      </div>
    </section>

    <section class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.interactions") }}</h3>
      <p class="type-caption px-1 text-label-3">{{ t("wall.interactionsHelp") }}</p>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.ripple") }}</span>
          <Switch :model-value="isOn('ripple')" @update:model-value="setOn('ripple', $event)" />
        </div>
        <div v-if="scene.interactions.ripple" class="pt-1">
          <Param v-model="scene.interactions.ripple.radius" :label="t('wall.p.radius')" :min="20" :max="1200" :step="10" unit=" px" :digits="0" />
          <Param v-model="scene.interactions.ripple.strength" :label="t('wall.p.strength')" :min="0" :max="2" />
          <Param v-model="scene.interactions.ripple.duration" :label="t('wall.p.duration')" :min="0.2" :max="5" unit=" s" :digits="1" />
        </div>
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.trail") }}</span>
          <Switch :model-value="isOn('trail')" @update:model-value="setOn('trail', $event)" />
        </div>
        <div v-if="scene.interactions.trail" class="pt-1">
          <Param v-model="scene.interactions.trail.width" :label="t('wall.p.width')" :min="1" :max="80" :step="1" unit=" px" :digits="0" />
          <Param v-model="scene.interactions.trail.fade" :label="t('wall.p.duration')" :min="0.05" :max="3" unit=" s" :digits="2" />
          <Param v-model="scene.interactions.trail.intensity" :label="t('wall.p.intensity')" :min="0" :max="2" />
          <div class="flex items-center gap-3 py-1">
            <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.p.color") }}</span>
            <input v-model="scene.interactions.trail.color" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
          </div>
        </div>
      </div>
    </section>

    <section v-if="usesConditions()" class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.sim.title") }}</h3>
      <div class="rounded-[10px] bg-fill-1 px-3 py-2">
        <div class="flex items-center gap-3 py-1">
          <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.sim.moment") }}</span>
          <Select :model-value="simulation.moment ?? ''" :options="momentOptions" @update:model-value="simulation.moment = ($event || null) as Moment | null" />
        </div>
        <div class="flex items-center gap-3 py-1">
          <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.sim.sky") }}</span>
          <Select :model-value="simulation.sky ?? ''" :options="skyOptions" @update:model-value="simulation.sky = ($event || null) as Sky | null" />
        </div>
        <p class="type-caption pt-1 text-label-3">
          {{ t("wall.sim.realNow", { moment: real.now ? t(`wall.when.m.${real.now.moment}`) : "—", sky: real.now?.sky ? t(`wall.when.s.${real.now.sky}`) : t("wall.sim.unknown") }) }}
        </p>
      </div>
    </section>
  </div>
</template>
