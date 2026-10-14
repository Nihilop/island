<script setup lang="ts">
// Outil « Convertisseur » (520 × 290) : Unités (longueur, masse, température, volume,
// vitesse) et Fichiers (images, audio, vidéo).
import { useDraft } from "../../stores/drafts";
import { computed, watch } from "vue";
import { PhArrowsLeftRight } from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import Select from "../../ui/Select.vue";
import FilesPane from "./FilesPane.vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { UNITS, convert, format, parse, type Category } from "./units";
import { takeKeyboard } from "../../notch/state";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const tab = useDraft<"files" | "units">("convert.tab", () => "files");
const cat = useDraft<Category>("convert.cat", () => "length");
const from = useDraft("convert.from", () => UNITS.length.pair[0]);
const to = useDraft("convert.to", () => UNITS.length.pair[1]);
const input = useDraft("convert.input", () => "100");

watch(cat, (c) => {
  [from.value, to.value] = UNITS[c].pair;
});

const options = computed(() => UNITS[cat.value].units.map((u) => ({ value: u.id, label: t(`convert.unit.${u.id}`) })));
const result = computed(() => {
  const v = parse(input.value);
  return format(v === null ? null : convert(cat.value, v, from.value, to.value), locale.value);
});

function swap() {
  [from.value, to.value] = [to.value, from.value];
}
function filter() {
  input.value = input.value.replace(/[^\d.,-]/g, "");
}
</script>

<template>
  <div class="flex h-full flex-col gap-3 px-4">
    <Segmented
      v-model="tab"
      size="sm"
      :options="[
        { value: 'files', label: t('convert.files') },
        { value: 'units', label: t('convert.units') },
      ]"
    />

    <Transition
      mode="out-in"
      enter-active-class="transition-[opacity,filter] duration-160 ease-(--ease-out)"
      enter-from-class="opacity-0 blur-[4px]"
      leave-active-class="transition-[opacity,filter] duration-160 ease-(--ease-out)"
      leave-to-class="opacity-0 blur-[4px]"
    >
      <FilesPane v-if="tab === 'files'" key="files" />
      <div v-else key="units" class="flex flex-col gap-3.5">
        <Segmented
          v-model="cat"
          size="sm"
          :options="(['length', 'mass', 'temperature', 'volume', 'speed'] as const).map((c) => ({ value: c, label: t(`convert.cat.${c}`) }))"
        />
        <div class="flex items-center gap-2">
          <div class="flex min-w-0 flex-1 flex-col gap-2 rounded-md bg-fill-1 p-3">
            <Input
              v-model="input"
              class="h-7 rounded-none bg-transparent p-0 text-[22px] font-semibold leading-7 tabular-nums text-label focus-visible:ring-0"
              inputmode="decimal"
              data-no-drag
              @input="filter"
              @focus="takeKeyboard"
            />
            <Select v-model="from" :options="options" />
          </div>
          <Button
            size="icon"
            class="size-8 text-label-2 transition-transform duration-140 ease-(--ease-out) hover:bg-fill-2"
            :title="t('convert.swap')"
            @click="swap"
          >
            <PhArrowsLeftRight :size="16" weight="bold" class="size-4" />
          </Button>
          <div class="flex min-w-0 flex-1 flex-col gap-2 rounded-md bg-fill-1 p-3">
            <div class="h-7 min-w-0 select-text truncate text-[22px] font-semibold leading-7 tabular-nums text-label">{{ result }}</div>
            <Select v-model="to" :options="options" />
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>
