<script setup lang="ts">
// Outil « Météo » (540 × 300) sur le ciel du jour (WeatherSky.vue, fond du notch) : lieu,
// grande température, condition et écart du jour à gauche ; conseil (« Parapluie · 70 %
// vers 17 h ») et détails à droite ; bande des 7 jours en verre dépoli. Le texte porte une
// ombre douce de la couleur du notch : lisible sur tous les ciels, dans les deux thèmes.
import { computed, onBeforeUnmount, ref } from "vue";
import {
  PhCloudFog,
  PhDrop,
  PhDropSlash,
  PhFlask,
  PhLightning,
  PhMapPin,
  PhNavigationArrow,
  PhSnowflake,
  PhThermometerHot,
  PhUmbrella,
  PhWind,
} from "@phosphor-icons/vue";
import type { Component } from "vue";
import Empty from "../../ui/Empty.vue";
import IconButton from "../../ui/IconButton.vue";
import PlaceSearch from "./PlaceSearch.vue";
import SkyDebug from "./SkyDebug.vue";
import { Skeleton } from "@/sdk/ui/skeleton";
import { fahrenheit, place, searching, weather, wmo } from "./store";
import { outlook, selectedDay, skyDebug, type OutlookKind } from "./sky";
import { settings } from "../../notch/settings";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const DEV = import.meta.env.DEV;
const debugOpen = ref(false);

const now = computed(() => (weather.current ? wmo(weather.current.code, weather.current.day) : null));
const today = computed(() => weather.days[0]);

// Jour choisi dans la bande : le haut et le ciel suivent ce jour (0 = maintenant).
const picked = computed(() => (selectedDay.value > 0 ? weather.days[selectedDay.value] ?? null : null));
const pickedWmo = computed(() => (picked.value ? wmo(picked.value.code) : null));
const longDay = (iso: string) => new Intl.DateTimeFormat(locale.value, { weekday: "long", day: "numeric", month: "long" }).format(new Date(iso + "T12:00"));
const pick = (i: number) => (selectedDay.value = selectedDay.value === i ? 0 : i);
// Rouvrir l'outil revient à aujourd'hui.
onBeforeUnmount(() => (selectedDay.value = 0));
const deg = (n: number) => `${Math.round(n)}°`;
const day = (iso: string, i: number) =>
  i === 0 ? t("weather.today") : new Intl.DateTimeFormat(locale.value, { weekday: "short" }).format(new Date(iso + "T12:00"));
const speed = computed(() => (fahrenheit.value ? "mph" : "km/h"));

/** Heure locale du lieu (0-23) → « 17 h » / « 5 PM », selon le format d'horloge. */
const hour = (h: number) =>
  new Intl.DateTimeFormat(locale.value, { hour: "numeric", hour12: !settings.clock24, timeZone: "UTC" }).format(Date.UTC(2000, 0, 1, h));

const OUTLOOK: Record<OutlookKind, { icon: Component; tone: string }> = {
  umbrella: { icon: PhUmbrella, tone: "text-sys-link" },
  snow: { icon: PhSnowflake, tone: "text-[#9cc9ff]" },
  storm: { icon: PhLightning, tone: "text-sys-yellow" },
  heat: { icon: PhThermometerHot, tone: "text-sys-orange" },
  wind: { icon: PhWind, tone: "text-label" },
  fog: { icon: PhCloudFog, tone: "text-label-2" },
  dry: { icon: PhDropSlash, tone: "text-sys-green" },
};

const advice = computed(() => {
  const o = outlook.value;
  if (!o) return null;
  const title =
    o.kind === "heat" ? t("weather.outlook.heat", { n: deg(o.value ?? 0) })
      : o.kind === "wind" ? t("weather.outlook.wind", { n: `${Math.round(o.value ?? 0)} ${speed.value}` })
        : t(`weather.outlook.${o.kind}`);
  const when = o.at === undefined ? "" : o.at === null ? t("weather.outlook.now") : t("weather.outlook.at", { h: hour(o.at) });
  const sub =
    o.kind === "umbrella" || o.kind === "snow" || o.kind === "storm" ? [`${o.prob} %`, when].filter(Boolean).join(" · ")
      : o.kind === "dry" ? (o.prob && o.prob >= 15 ? t("weather.outlook.dryProb", { n: o.prob }) : t(picked.value ? "weather.outlook.dryDay" : "weather.outlook.drySub"))
        : o.kind === "heat" ? t("weather.outlook.heatSub")
          : o.kind === "wind" ? t("weather.outlook.windSub")
            : t("weather.outlook.fogSub");
  return { ...OUTLOOK[o.kind], title, sub };
});

/** Ombre de lecture (couleur du notch) : texte sur ciel. */
const ink = "[text-shadow:0_1px_12px_color-mix(in_srgb,var(--notch)_75%,transparent),0_0_3px_color-mix(in_srgb,var(--notch)_50%,transparent)]";
/** Verre dépoli sur le ciel. */
const glass =
  "bg-[color-mix(in_srgb,var(--notch)_38%,transparent)] backdrop-blur-xl backdrop-saturate-150 ring-1 ring-inset ring-label/10 shadow-[0_8px_24px_-12px_rgb(0_0_0/0.45)]";
</script>

<template>
  <div class="relative flex h-full flex-col px-5 pt-0.5 pb-2.5">
    <Teleport v-if="DEV" defer to="#notch-header-actions">
      <IconButton :icon="PhFlask" :active="debugOpen || skyDebug.on" :label="t('weather.debug.title')" @click="debugOpen = !debugOpen" />
    </Teleport>

    <PlaceSearch v-if="searching || (!place.location && !weather.locating)" />
    <div v-else-if="(weather.loading || weather.locating) && !skyDebug.on" class="flex flex-col gap-2">
      <Skeleton class="h-16 rounded-[14px] bg-fill-1 animate-[pulse_1.4s_ease-in-out_infinite]" />
      <Skeleton class="h-24 rounded-[16px] bg-fill-1 animate-[pulse_1.4s_ease-in-out_infinite]" />
    </div>
    <Empty v-else-if="weather.error || !weather.current" :title="t('weather.unavailable')" />
    <template v-else>
      <!-- Haut : maintenant ou jour choisi (gauche), conseil et détails (droite). -->
      <div class="flex min-h-0 items-start justify-between gap-4" :class="ink">
        <div class="min-w-0">
          <button
            type="button"
            class="type-footnote -ml-1 flex max-w-full items-center gap-1 rounded-[6px] px-1 py-0.5 font-medium text-label transition-colors duration-100 hover:bg-label/10"
            :title="t('weather.change')"
            @click="searching = true"
          >
            <PhNavigationArrow v-if="place.auto" :size="11" weight="fill" class="shrink-0" />
            <PhMapPin v-else :size="11" weight="fill" class="shrink-0" />
            <span class="truncate">{{ place.location?.name }}</span>
            <span v-if="picked" class="truncate font-normal text-label-2">· {{ longDay(picked.date) }}</span>
          </button>
          <div class="flex items-start gap-1.5">
            <span class="text-[60px] leading-[64px] font-extralight tracking-[-0.045em] tabular-nums">{{ Math.round(picked ? picked.max : weather.current.temp) }}</span>
            <span class="mt-2 text-[26px] leading-none font-extralight text-label">°</span>
            <span v-if="picked" class="mt-[34px] text-[20px] leading-none font-light tabular-nums text-label-2">{{ deg(picked.min) }}</span>
          </div>
          <div class="type-footnote -mt-0.5 flex items-center gap-1.5 font-medium text-label">
            <component :is="(pickedWmo ?? now)!.icon" :size="14" weight="fill" class="shrink-0" />
            <span class="truncate">{{ (pickedWmo ?? now)!.label }}</span>
            <span v-if="!picked && today" class="shrink-0 font-normal tabular-nums text-label-2">· {{ deg(today.max) }} / {{ deg(today.min) }}</span>
          </div>
        </div>

        <div class="flex shrink-0 flex-col items-end gap-2 pt-1">
          <div v-if="advice" class="flex max-w-[220px] items-center gap-2.5 rounded-[14px] py-2 pr-3.5 pl-2.5 [text-shadow:none]" :class="glass">
            <span class="grid size-8 shrink-0 place-items-center rounded-full bg-label/10" :class="advice.tone">
              <component :is="advice.icon" :size="17" weight="fill" />
            </span>
            <span class="min-w-0 leading-tight">
              <span class="type-footnote block truncate font-semibold text-label">{{ advice.title }}</span>
              <span class="type-caption block truncate text-label-2">{{ advice.sub }}</span>
            </span>
          </div>
          <div class="type-caption flex items-center gap-2.5 font-medium text-label tabular-nums">
            <template v-if="picked">
              <span class="flex items-center gap-1"><PhWind :size="12" weight="bold" />{{ Math.round(picked.wind) }} {{ speed }}</span>
              <span class="flex items-center gap-1"><PhDrop :size="12" weight="fill" />{{ picked.rain }} %</span>
            </template>
            <template v-else>
              <span>{{ t("weather.feels", { n: deg(weather.current.feels) }) }}</span>
              <span class="flex items-center gap-1"><PhWind :size="12" weight="bold" />{{ Math.round(weather.current.wind) }} {{ speed }}</span>
              <span v-if="today?.rain" class="flex items-center gap-1"><PhDrop :size="12" weight="fill" />{{ today.rain }} %</span>
            </template>
          </div>
        </div>
      </div>

      <!-- Bande des 7 jours : un clic montre ce jour (ciel compris), un second revient à maintenant. -->
      <div class="mt-auto grid grid-cols-7 gap-0.5 rounded-[16px] p-1" :class="glass">
        <button
          v-for="(d, i) in weather.days"
          :key="d.date"
          type="button"
          class="flex flex-col items-center gap-[3px] rounded-[12px] py-1.5 transition-colors duration-150"
          :class="selectedDay === i ? 'bg-label/14 ring-1 ring-inset ring-label/15' : 'hover:bg-label/6'"
          :aria-pressed="selectedDay === i"
          @click="pick(i)"
        >
          <span class="type-caption" :class="selectedDay === i ? 'font-semibold text-label' : i === 0 ? 'font-semibold text-label-2' : 'text-label-2'">{{ day(d.date, i) }}</span>
          <component :is="wmo(d.code).icon" :size="19" weight="duotone" class="text-label" />
          <span class="h-3 text-[10px] leading-3 font-semibold tabular-nums text-sys-link" :class="{ invisible: d.rain < 30 }">{{ d.rain }} %</span>
          <span class="type-footnote leading-4 font-semibold tabular-nums">{{ deg(d.max) }}</span>
          <span class="type-caption leading-4 tabular-nums text-label-2">{{ deg(d.min) }}</span>
        </button>
      </div>
    </template>

    <SkyDebug v-if="DEV && debugOpen" class="absolute top-0 right-3 bottom-2.5 z-10" @close="debugOpen = false" />
  </div>
</template>
