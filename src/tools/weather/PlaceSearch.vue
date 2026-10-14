<script setup lang="ts">
// Recherche de lieu : géocodage Open-Meteo (debounce 250 ms, 2 caractères min), bouton
// « automatique par IP » et « Annuler » si un lieu existe déjà.
import { useDraft } from "../../stores/drafts";
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhMagnifyingGlass, PhNavigationArrow } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { choose, locate, place, searching, weather, type Place } from "./store";
import { takeKeyboard } from "../../notch/state";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const query = useDraft("weather.query", () => "");
const results = useDraft<Place[]>("weather.results", () => []);
const loading = ref(false);
let timer: number | undefined;

watch(query, (q) => {
  window.clearTimeout(timer);
  if (q.trim().length < 2) {
    results.value = [];
    return;
  }
  loading.value = true;
  timer = window.setTimeout(async () => {
    results.value = await invoke<Place[]>("weather_geocode", { query: q, lang: locale.value }).catch(() => []);
    loading.value = false;
  }, 250);
});
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="type-caption text-label-3">{{ t("weather.where") }}</div>
    <div class="flex items-center gap-2">
      <label class="flex h-7 flex-1 items-center gap-1.5 rounded-full bg-fill-2 px-2.5" data-no-drag>
        <PhMagnifyingGlass :size="13" weight="bold" class="text-label-3" />
        <Input
          v-model="query"
          class="type-footnote h-auto flex-1 rounded-none bg-transparent px-0 text-label placeholder:text-current/50 focus-visible:ring-0"
          :placeholder="t('weather.searchPh')"
          autofocus
          @focus="takeKeyboard"
        />
      </label>
      <Button :loading="weather.locating" :title="t('weather.auto')" @click="locate">
        <PhNavigationArrow v-if="!weather.locating" :size="13" weight="fill" />
        {{ t("weather.auto") }}
      </Button>
      <Button v-if="place.location" @click="searching = false">{{ t("common.cancel") }}</Button>
    </div>
    <div class="flex flex-col">
      <button v-for="r in results" :key="`${r.lat},${r.lon}`" type="button" class="flex h-[30px] items-baseline gap-2 rounded-sm px-2 text-left hover:bg-fill-1" @click="choose(r)">
        <span class="type-body">{{ r.name }}</span>
        <span class="type-caption truncate text-label-3">{{ r.detail }}</span>
      </button>
    </div>
  </div>
</template>
