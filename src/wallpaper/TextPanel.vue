<script setup lang="ts">
// Réglages d'un calque texte : contenu (variables insérées au curseur par « Insérer »),
// police, style, contour, ombre et placement. Le rendu est fait par le Rust, identique dans
// l'aperçu et sur le bureau.
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhBracketsCurly, PhGlobe } from "@phosphor-icons/vue";
import Segmented from "../ui/Segmented.vue";
import Select from "../ui/Select.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/sdk/ui/popover";
import Param from "./Param.vue";
import { DEFAULTS, type Anchor, type Layer, type TextStyle } from "./scene";
import { t } from "../notch/i18n";

const props = defineProps<{ layer: Layer }>();
const text = computed(() => props.layer.text as TextStyle);

const ANCHORS: Anchor[] = ["tl", "t", "tr", "l", "c", "r", "bl", "b", "br"];
/** Pastille de couleur : taille fixe (jamais écrasée par ses voisins), avec un liseré. */
const SWATCH = "h-6 w-10 shrink-0 cursor-pointer rounded-[6px] border border-fill-3 bg-transparent p-0.5";
const WEIGHTS = [100, 200, 300, 400, 500, 600, 700, 800, 900];

// --- Polices installées -------------------------------------------------------------------

const fonts = ref<string[]>([]);
onMounted(async () => {
  fonts.value = await invoke<string[]>("wallpaper_fonts").catch(() => []);
});

// --- Variables ------------------------------------------------------------------------------

const VARIABLES = [
  { group: "time", items: ["{time}", "{time:HH:mm:ss}", "{time:h:mm a}", "{date}", "{date:EEEE}", "{date:d MMMM yyyy}", "{date:dd/MM/yyyy}"] },
  { group: "weather", items: ["{weather.temp}", "{weather.sky}", "{weather.city}", "{weather.max}", "{weather.min}", "{weather.feels}"] },
  { group: "media", items: ["{media.title}", "{media.artist}"] },
];

const area = ref<HTMLTextAreaElement | null>(null);
const open = ref(false);

/** Insère `token` à la position du curseur (ou remplace la sélection). */
function insert(token: string) {
  const el = area.value;
  const s = text.value.content;
  const [a, b] = el ? [el.selectionStart, el.selectionEnd] : [s.length, s.length];
  text.value.content = s.slice(0, a) + token + s.slice(b);
  open.value = false;
  requestAnimationFrame(() => {
    el?.focus();
    el?.setSelectionRange(a + token.length, a + token.length);
  });
}

// Heure d'un autre fuseau : recherche parmi les fuseaux connus (IANA).
const zoneQuery = ref("");
const zones = computed(() => {
  const all: string[] = (Intl as unknown as { supportedValuesOf?: (k: string) => string[] }).supportedValuesOf?.("timeZone") ?? [];
  const q = zoneQuery.value.trim().toLowerCase().replace(/\s+/g, "_");
  return q ? all.filter((z) => z.toLowerCase().includes(q)).slice(0, 8) : [];
});
const city = (z: string) => z.split("/").pop()?.replace(/_/g, " ") ?? z;

function toggleShadow(on: boolean) {
  text.value.shadow = on ? DEFAULTS.shadow() : null;
}
</script>

<template>
  <section class="space-y-2">
    <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.kind.text") }}</h3>

    <textarea
      ref="area"
      v-model="text.content"
      rows="3"
      maxlength="500"
      class="type-footnote w-full resize-y rounded-[6px] bg-fill-2 px-2 py-1.5 font-mono outline-none focus-visible:ring-1 focus-visible:ring-fill-3"
      :placeholder="t('wall.text.placeholder')"
    />
    <Popover v-model:open="open">
      <PopoverTrigger as-child>
        <Button size="sm" class="w-full"><PhBracketsCurly />{{ t("wall.text.insert") }}</Button>
      </PopoverTrigger>
      <PopoverContent align="start" :side-offset="4" class="flex max-h-[360px] w-64 flex-col gap-2 overflow-y-auto">
        <div v-for="g in VARIABLES" :key="g.group">
          <div class="type-caption mb-0.5 font-semibold text-label-3">{{ t(`wall.text.group.${g.group}`) }}</div>
          <button
            v-for="v in g.items"
            :key="v"
            type="button"
            class="type-footnote flex w-full items-center justify-between gap-2 rounded-[6px] px-2 py-1 text-left hover:bg-fill-2"
            @click="insert(v)"
          >
            <span>{{ t(`wall.text.var.${v.replace(/[{}]/g, "").replace(/[:/ ]/g, "_")}`) }}</span>
            <code class="type-caption text-label-3">{{ v }}</code>
          </button>
        </div>
        <div>
          <div class="type-caption mb-1 flex items-center gap-1 font-semibold text-label-3"><PhGlobe :size="11" />{{ t("wall.text.group.zone") }}</div>
          <Input v-model="zoneQuery" :placeholder="t('wall.text.zoneSearch')" />
          <button
            v-for="z in zones"
            :key="z"
            type="button"
            class="type-footnote flex w-full items-center justify-between gap-2 rounded-[6px] px-2 py-1 text-left hover:bg-fill-2"
            @click="insert(`{time@${z}}`)"
          >
            <span>{{ city(z) }}</span>
            <code class="type-caption text-label-3">{{ z }}</code>
          </button>
        </div>
      </PopoverContent>
    </Popover>
    <p class="type-caption text-label-3">{{ t("wall.text.hint") }}</p>

    <div class="flex items-center gap-3 py-1">
      <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.text.font") }}</span>
      <Input v-model="text.font" list="wallpaper-fonts" class="flex-1" :placeholder="t('wall.text.fontDefault')" />
      <datalist id="wallpaper-fonts"><option v-for="f in fonts" :key="f" :value="f" /></datalist>
    </div>
    <div class="flex items-center gap-3 py-1">
      <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.text.weight") }}</span>
      <Select
        :model-value="String(text.weight)"
        :options="WEIGHTS.map((w) => ({ value: String(w), label: t(`wall.text.w${w}`) }))"
        @update:model-value="text.weight = Number($event)"
      />
    </div>
    <Param v-model="text.size" :label="t('wall.p.size')" :min="0.02" :max="0.5" />
    <div class="flex items-center gap-3 py-1">
      <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.text.color") }}</span>
      <input v-model="text.color" type="color" :class="SWATCH" />
    </div>
    <div class="flex items-center gap-4 py-1">
      <label class="type-footnote flex items-center gap-2 text-label-2"><Switch v-model="text.italic" />{{ t("wall.text.italic") }}</label>
      <label class="type-footnote flex items-center gap-2 text-label-2"><Switch v-model="text.uppercase" />{{ t("wall.text.upper") }}</label>
    </div>
    <Segmented
      v-model="text.align"
      size="sm"
      :options="[
        { value: 'left', label: t('wall.text.left') },
        { value: 'center', label: t('wall.text.center') },
        { value: 'right', label: t('wall.text.right') },
      ]"
    />
    <Param v-model="text.lineHeight" :label="t('wall.text.lineHeight')" :min="0.7" :max="2.5" />
    <Param v-model="text.letterSpacing" :label="t('wall.text.letterSpacing')" :min="-0.1" :max="0.5" unit=" em" />

    <h3 class="type-caption pt-2 font-medium uppercase text-label-3">{{ t("wall.text.outline") }}</h3>
    <div class="flex items-center gap-2">
      <Param v-model="text.outline" class="flex-1" :label="t('wall.text.thickness')" :min="0" :max="0.2" />
      <input v-model="text.outlineColor" type="color" :class="SWATCH" />
    </div>

    <div class="flex items-center justify-between pt-2">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.text.shadow") }}</h3>
      <Switch :model-value="!!text.shadow" @update:model-value="toggleShadow" />
    </div>
    <template v-if="text.shadow">
      <div class="flex items-center gap-3 py-1">
        <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.text.color") }}</span>
        <input v-model="text.shadow.color" type="color" :class="SWATCH" />
      </div>
      <Param v-model="text.shadow.opacity" :label="t('wall.p.opacity')" :min="0" :max="1" />
      <Param v-model="text.shadow.blur" :label="t('wall.text.blur')" :min="0" :max="0.5" />
      <Param v-model="text.shadow.x" :label="t('wall.p.offsetX')" :min="-0.3" :max="0.3" />
      <Param v-model="text.shadow.y" :label="t('wall.p.offsetY')" :min="-0.3" :max="0.3" />
      <p class="type-caption text-label-3">{{ t("wall.text.glowHint") }}</p>
    </template>

    <h3 class="type-caption pt-2 font-medium uppercase text-label-3">{{ t("wall.text.place") }}</h3>
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
    <Param v-model="layer.transform.x" :label="t('wall.p.offsetX')" :min="-1" :max="1" />
    <Param v-model="layer.transform.y" :label="t('wall.p.offsetY')" :min="-1" :max="1" />
    <Param v-model="layer.transform.rotation" :label="t('wall.p.rotation')" :min="-180" :max="180" :step="1" unit="°" :digits="0" />
  </section>
</template>
