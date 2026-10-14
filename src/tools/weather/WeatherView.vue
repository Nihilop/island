<script setup lang="ts">
// Outil « Météo » (520 × 250) : conditions actuelles + grille 7 jours, recherche de lieu.
import { computed } from "vue";
import { PhDrop, PhMapPin, PhNavigationArrow, PhWind } from "@phosphor-icons/vue";
import Empty from "../../ui/Empty.vue";
import PlaceSearch from "./PlaceSearch.vue";
import { Skeleton } from "@/sdk/ui/skeleton";
import { fahrenheit, place, searching, weather, wmo } from "./store";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const now = computed(() => (weather.current ? wmo(weather.current.code, weather.current.day) : null));
const deg = (n: number) => `${Math.round(n)}°`;
const day = (iso: string, i: number) =>
  i === 0 ? t("weather.today") : new Intl.DateTimeFormat(locale.value, { weekday: "short" }).format(new Date(iso + "T12:00"));
</script>

<template>
  <div class="flex h-full flex-col gap-3 px-4">
    <PlaceSearch v-if="searching || (!place.location && !weather.locating)" />
    <div v-else-if="weather.loading || weather.locating" class="flex flex-col gap-2">
      <Skeleton class="h-12 rounded-md bg-fill-1 animate-[pulse_1.4s_ease-in-out_infinite]" />
      <Skeleton class="h-20 rounded-md bg-fill-1 animate-[pulse_1.4s_ease-in-out_infinite]" />
    </div>
    <Empty v-else-if="weather.error || !weather.current" :title="t('weather.unavailable')" />
    <template v-else>
      <div class="flex items-center gap-3">
        <component :is="now!.icon" :size="38" weight="duotone" class="shrink-0" />
        <div class="text-[34px] leading-[38px] font-semibold tracking-[-0.03em] tabular-nums">{{ deg(weather.current.temp) }}</div>
        <div class="min-w-0 flex-1">
          <button type="button" class="type-body flex max-w-full items-center gap-1 text-label" @click="searching = true">
            <PhNavigationArrow v-if="place.auto" :size="11" weight="fill" />
            <PhMapPin v-else :size="11" weight="fill" />
            <span class="truncate">{{ place.location?.name }}</span>
          </button>
          <div class="type-caption text-label-3">{{ now!.label }}</div>
        </div>
        <div class="type-caption flex flex-col items-end gap-0.5 text-label-2">
          <span>{{ t("weather.feels", { n: deg(weather.current.feels) }) }}</span>
          <span class="flex items-center gap-1"><PhWind :size="12" weight="bold" />{{ Math.round(weather.current.wind) }} {{ fahrenheit ? "mph" : "km/h" }}</span>
          <span v-if="weather.days[0]?.rain" class="flex items-center gap-1"><PhDrop :size="12" weight="fill" />{{ weather.days[0].rain }} %</span>
        </div>
      </div>
      <div class="grid grid-cols-7 gap-1">
        <div v-for="(d, i) in weather.days" :key="d.date" class="flex flex-col items-center gap-1 rounded-md py-2" :class="{ 'bg-fill-1': i === 0 }">
          <span class="type-caption text-label-3">{{ day(d.date, i) }}</span>
          <component :is="wmo(d.code).icon" :size="18" weight="duotone" />
          <span class="type-footnote font-semibold tabular-nums">{{ deg(d.max) }}</span>
          <span class="type-caption tabular-nums text-label-3">{{ deg(d.min) }}</span>
        </div>
      </div>
    </template>
  </div>
</template>
