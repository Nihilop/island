// Météo : lieu (automatique par IP ou choisi), prévisions Open-Meteo (via Rust, cache
// 10 min), unités °C/°F. Un lieu choisi à la main reste fixe ; en automatique, la
// détection IP est refaite à chaque démarrage sans écraser un choix fait entre-temps.

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  PhCloud,
  PhCloudFog,
  PhCloudLightning,
  PhCloudMoon,
  PhCloudRain,
  PhCloudSnow,
  PhCloudSun,
  PhDrop,
  PhMoon,
  PhSun,
} from "@phosphor-icons/vue";
import type { Component } from "vue";
import { persisted } from "../../notch/persist";
import { settings } from "../../notch/settings";
import { t } from "../../notch/i18n";

export interface Place {
  name: string;
  detail: string;
  lat: number;
  lon: number;
}

export const { state: place, ready: placeReady } = persisted("weather", {
  location: null as Place | null,
  auto: true,
});

export const weather = reactive({
  loading: false,
  error: false,
  locating: false,
  current: null as null | { temp: number; feels: number; code: number; wind: number; day: boolean },
  days: [] as { date: string; code: number; max: number; min: number; rain: number }[],
});

/** Recherche de lieu affichée (aussi quand la détection IP échoue sans lieu). */
export const searching = ref(false);

/** Code WMO → icône + libellé. */
export function wmo(code: number, day = true): { icon: Component; label: string } {
  const k =
    code === 0 ? "clear" : code === 1 ? "mostlyClear" : code === 2 ? "partly" : code === 3 ? "overcast"
      : code <= 48 ? "fog" : code <= 57 ? "drizzle" : code <= 67 || (code >= 80 && code <= 82) ? "rain"
        : code <= 77 || code === 85 || code === 86 ? "snow" : "storm";
  const icon = {
    clear: day ? PhSun : PhMoon,
    mostlyClear: day ? PhCloudSun : PhCloudMoon,
    partly: day ? PhCloudSun : PhCloudMoon,
    overcast: PhCloud,
    fog: PhCloudFog,
    drizzle: PhDrop,
    rain: PhCloudRain,
    snow: PhCloudSnow,
    storm: PhCloudLightning,
  }[k];
  return { icon, label: t(`weather.wmo.${k}`) };
}

export const fahrenheit = computed(() => settings.tempUnit === "f");

export async function refresh() {
  const loc = place.location;
  if (!loc) return;
  weather.loading = !weather.current;
  weather.error = false;
  try {
    const v = await invoke<any>("weather_forecast", { lat: loc.lat, lon: loc.lon, fahrenheit: fahrenheit.value });
    const c = v.current;
    weather.current = {
      temp: c.temperature_2m,
      feels: c.apparent_temperature,
      code: c.weather_code,
      wind: c.wind_speed_10m,
      day: c.is_day === 1,
    };
    const d = v.daily;
    weather.days = d.time.map((date: string, i: number) => ({
      date,
      code: d.weather_code[i],
      max: d.temperature_2m_max[i],
      min: d.temperature_2m_min[i],
      rain: d.precipitation_probability_max?.[i] ?? 0,
    }));
  } catch {
    weather.error = true;
  } finally {
    weather.loading = false;
  }
}

/** Détection automatique par IP. */
export async function locate() {
  weather.locating = true;
  const before = place.location;
  try {
    const p = await invoke<Place>("weather_locate");
    // Ne pas écraser un choix manuel fait pendant la requête.
    if (place.location === before) {
      place.location = p;
      place.auto = true;
      searching.value = false;
    }
  } catch {
    if (!place.location) searching.value = true;
  } finally {
    weather.locating = false;
  }
}

export function choose(p: Place) {
  place.location = p;
  place.auto = false;
  searching.value = false;
}

let timer: number | undefined;
export async function initWeather() {
  await placeReady;
  if (place.auto || !place.location) await locate();
  watch([() => place.location, fahrenheit], refresh, { immediate: true });
  timer = window.setInterval(refresh, 10 * 60 * 1000);
  void timer;
}
