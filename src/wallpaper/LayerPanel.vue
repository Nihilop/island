<script setup lang="ts">
// Propriétés du calque sélectionné : apparence (opacité, fusion, profondeur), contenu selon
// le type (image, couleur / dégradé, particules), placement, effets du calque.
import { PhImage, PhPaintBrush, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import Segmented from "../ui/Segmented.vue";
import Select from "../ui/Select.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import Param from "./Param.vue";
import { DEFAULTS, PARTICLE_PRESETS, type Anchor, type Layer } from "./scene";
import { t } from "../notch/i18n";

const props = defineProps<{ layer: Layer; importing: boolean; painting: number | null }>();
const emit = defineEmits<{ pickImage: []; paint: [slot: 0 | 1 | 2]; clearMask: [slot: 0 | 1 | 2] }>();

const ANCHORS: Anchor[] = ["tl", "t", "tr", "l", "c", "r", "bl", "b", "br"];

function particlePreset(k: keyof typeof PARTICLE_PRESETS) {
  const p = props.layer.particles;
  if (!p) return;
  Object.assign(p, PARTICLE_PRESETS[k]);
  // Les halos s'ajoutent à l'image : fusion « ajout » par défaut.
  props.layer.blend = p.shape === "glow" ? "add" : "normal";
}

function toggleWater(on: boolean) {
  props.layer.effects.water = on ? DEFAULTS.water() : null;
}
function toggleHeat(on: boolean) {
  props.layer.effects.heat = on ? DEFAULTS.heat() : null;
}
function toggleColor(on: boolean) {
  props.layer.effects.color = on ? DEFAULTS.color() : null;
}
</script>

<template>
  <div class="space-y-4">
    <!-- Apparence -->
    <section class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.appearance") }}</h3>
      <Param v-model="layer.opacity" :label="t('wall.p.opacity')" :min="0" :max="1" />
      <Param v-model="layer.depth" :label="t('wall.p.depth')" :min="0" :max="1" />
      <label class="flex items-center gap-3 py-1">
        <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.blend") }}</span>
        <div class="min-w-0 flex-1">
          <Select
            v-model="layer.blend"
            :options="[
              { value: 'normal', label: t('wall.blend.normal') },
              { value: 'add', label: t('wall.blend.add') },
              { value: 'screen', label: t('wall.blend.screen') },
              { value: 'multiply', label: t('wall.blend.multiply') },
            ]"
          />
        </div>
      </label>
    </section>

    <!-- Contenu : image -->
    <section v-if="layer.kind === 'image'" class="space-y-2">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.kind.image") }}</h3>
      <Button class="w-full" :loading="importing" @click="emit('pickImage')"><PhImage />{{ layer.image ? t("wall.changeImage") : t("wall.pickImage") }}</Button>
      <Segmented
        v-model="layer.transform.fit"
        size="sm"
        :options="[
          { value: 'cover', label: t('wall.fit.cover') },
          { value: 'contain', label: t('wall.fit.contain') },
          { value: 'stretch', label: t('wall.fit.stretch') },
          { value: 'free', label: t('wall.fit.free') },
        ]"
      />
      <template v-if="layer.transform.fit === 'free'">
        <div class="flex items-start gap-3 py-1">
          <span class="type-footnote w-[92px] shrink-0 pt-1 text-label-2">{{ t("wall.anchor") }}</span>
          <div class="grid grid-cols-3 gap-1">
            <button
              v-for="a in ANCHORS"
              :key="a"
              type="button"
              class="size-5 rounded-[4px] transition-colors"
              :class="layer.transform.anchor === a ? 'bg-sys-link' : 'bg-fill-2 hover:bg-fill-3'"
              :title="t(`wall.anchor.${a}`)"
              @click="layer.transform.anchor = a; layer.transform.x = 0; layer.transform.y = 0"
            />
          </div>
        </div>
        <Param v-model="layer.transform.width" :label="t('wall.p.width')" :min="0.02" :max="1.5" />
        <Param v-model="layer.transform.x" :label="t('wall.p.offsetX')" :min="-1" :max="1" />
        <Param v-model="layer.transform.y" :label="t('wall.p.offsetY')" :min="-1" :max="1" />
        <Param v-model="layer.transform.rotation" :label="t('wall.p.rotation')" :min="-180" :max="180" :step="1" unit="°" :digits="0" />
        <p class="type-caption text-label-3">{{ t("wall.freeHint") }}</p>
      </template>
    </section>

    <!-- Contenu : couleur / dégradé -->
    <section v-else-if="layer.kind === 'fill'" class="space-y-2">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.kind.fill") }}</h3>
      <Segmented
        v-model="layer.fill.mode"
        size="sm"
        :options="[
          { value: 'gradient', label: t('wall.bg.gradient') },
          { value: 'solid', label: t('wall.bg.solid') },
        ]"
      />
      <div class="flex items-center gap-3 py-1">
        <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.colors") }}</span>
        <input v-model="layer.fill.colors[0]" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
        <input v-if="layer.fill.mode === 'gradient'" v-model="layer.fill.colors[1]" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
      </div>
      <Param v-if="layer.fill.mode === 'gradient'" v-model="layer.fill.angle" :label="t('wall.angle')" :min="0" :max="360" :step="1" unit="°" :digits="0" />
    </section>

    <!-- Contenu : particules -->
    <section v-else-if="layer.particles" class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.kind.particles") }}</h3>
      <div class="flex flex-wrap gap-1 pb-1">
        <Button v-for="k in (['snow', 'rain', 'fireflies', 'embers'] as const)" :key="k" size="sm" variant="outline" @click="particlePreset(k)">{{ t(`wall.pk.${k}`) }}</Button>
      </div>
      <Segmented
        v-model="layer.particles.shape"
        size="sm"
        :options="[
          { value: 'dot', label: t('wall.shape.dot') },
          { value: 'streak', label: t('wall.shape.streak') },
          { value: 'glow', label: t('wall.shape.glow') },
        ]"
      />
      <Param v-model="layer.particles.density" :label="t('wall.p.density')" :min="0" :max="1" />
      <Param v-model="layer.particles.speed" :label="t('wall.p.speed')" :min="0" :max="1" />
      <Param v-model="layer.particles.direction" :label="t('wall.p.direction')" :min="0" :max="360" :step="1" unit="°" :digits="0" />
      <Param v-model="layer.particles.spread" :label="t('wall.p.spread')" :min="0" :max="1" />
      <Param v-model="layer.particles.turbulence" :label="t('wall.p.turbulence')" :min="0" :max="1" />
      <Param v-model="layer.particles.flicker" :label="t('wall.p.flicker')" :min="0" :max="1" />
      <Param v-model="layer.particles.size" :label="t('wall.p.size')" :min="0.2" :max="4" />
      <Param v-model="layer.particles.opacity" :label="t('wall.p.intensity')" :min="0" :max="1" />
      <div class="flex items-center gap-3 py-1">
        <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.p.color") }}</span>
        <input v-model="layer.particles.color" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
      </div>
    </section>

    <!-- Masque du calque (zone visible ; zone d'apparition des particules) -->
    <section class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ layer.kind === "particles" ? t("wall.mask.zone") : t("wall.mask.layer") }}</h3>
      <div class="flex items-center gap-1.5">
        <Button class="flex-1" :disabled="painting !== null" @click="emit('paint', 0)"><PhPaintBrush />{{ layer.mask ? t("wall.mask.edit") : t("wall.mask.paint") }}</Button>
        <IconButton v-if="layer.mask" :icon="PhX" :label="t('wall.mask.remove')" @click="emit('clearMask', 0)" />
      </div>
      <p class="type-caption text-label-3">{{ layer.kind === "particles" ? t("wall.mask.zoneHelp") : t("wall.mask.layerHelp") }}</p>
    </section>

    <!-- Effets du calque -->
    <section class="space-y-1">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.layerEffects") }}</h3>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.water") }}</span>
          <Switch :model-value="!!layer.effects.water" @update:model-value="toggleWater" />
        </div>
        <div v-if="layer.effects.water" class="pt-1">
          <Param v-model="layer.effects.water.strength" :label="t('wall.p.strength')" :min="0" :max="1" />
          <Param v-model="layer.effects.water.speed" :label="t('wall.p.speed')" :min="0" :max="1" />
          <Param v-model="layer.effects.water.scale" :label="t('wall.p.scale')" :min="0" :max="1" />
          <div class="flex items-center gap-1.5 py-1">
            <Button size="sm" class="flex-1" :disabled="painting !== null" @click="emit('paint', 1)"><PhPaintBrush />{{ layer.effects.water.mask ? t("wall.mask.editZone") : t("wall.mask.paintZone") }}</Button>
            <IconButton v-if="layer.effects.water.mask" :icon="PhX" :label="t('wall.mask.removeZone')" @click="emit('clearMask', 1)" />
          </div>
        </div>
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.heat") }}</span>
          <Switch :model-value="!!layer.effects.heat" @update:model-value="toggleHeat" />
        </div>
        <div v-if="layer.effects.heat" class="pt-1">
          <Param v-model="layer.effects.heat.strength" :label="t('wall.p.strength')" :min="0" :max="1" />
          <Param v-model="layer.effects.heat.speed" :label="t('wall.p.speed')" :min="0" :max="1" />
          <Param v-model="layer.effects.heat.scale" :label="t('wall.p.fineness')" :min="0" :max="1" />
          <Param v-model="layer.effects.heat.direction" :label="t('wall.p.direction')" :min="0" :max="360" :step="1" unit="°" :digits="0" />
          <div class="flex items-center gap-1.5 py-1">
            <Button size="sm" class="flex-1" :disabled="painting !== null" @click="emit('paint', 2)"><PhPaintBrush />{{ layer.effects.heat.mask ? t("wall.mask.editZone") : t("wall.mask.paintZone") }}</Button>
            <IconButton v-if="layer.effects.heat.mask" :icon="PhX" :label="t('wall.mask.removeZone')" @click="emit('clearMask', 2)" />
          </div>
          <p class="type-caption pb-1 text-label-3">{{ t("wall.heatHint") }}</p>
        </div>
      </div>
      <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
        <div class="flex items-center justify-between">
          <span class="type-body">{{ t("wall.fx.color") }}</span>
          <Switch :model-value="!!layer.effects.color" @update:model-value="toggleColor" />
        </div>
        <div v-if="layer.effects.color" class="pt-1">
          <Param v-model="layer.effects.color.hue" :label="t('wall.p.hue')" :min="-180" :max="180" :step="1" unit="°" :digits="0" />
          <Param v-model="layer.effects.color.saturation" :label="t('wall.p.saturation')" :min="0" :max="2" />
          <Param v-model="layer.effects.color.brightness" :label="t('wall.p.brightness')" :min="0" :max="2" />
          <Param v-model="layer.effects.color.contrast" :label="t('wall.p.contrast')" :min="0" :max="2" />
        </div>
      </div>
    </section>
  </div>
</template>
