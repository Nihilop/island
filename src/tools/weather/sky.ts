// Ciel de l'outil Météo : résume les prochaines heures en UNE ambiance (de quoi savoir d'un
// coup d'œil s'il faut sortir couvert), le moment de la journée (aube, jour, crépuscule,
// nuit) et un conseil (« Parapluie · 70 % vers 17 h »). Rendu : WeatherSky.vue.
//
// Règle : l'ambiance est la pire condition « humide » (pluie, neige, orage) attendue avec au
// moins 40 % de probabilité dans les 12 prochaines heures ; sinon la condition sèche la plus
// présente (l'heure en cours compte triple). Forte chaleur : ciel dégagé et max ≥ 30 °C.
//
// Debug (`skyDebug`) : force une ambiance, un moment, une intensité et un vent pour tester
// tous les scénarios sans attendre la vraie météo.

import { computed, reactive, ref } from "vue";
import { fahrenheit, hourAt, weather } from "./store";

export type Mood = "clear" | "partly" | "cloudy" | "fog" | "drizzle" | "rain" | "storm" | "snow" | "heat";
export type Phase = "night" | "dawn" | "day" | "dusk";

export const MOODS: Mood[] = ["clear", "partly", "cloudy", "fog", "drizzle", "rain", "storm", "snow", "heat"];
export const PHASES: Phase[] = ["dawn", "day", "dusk", "night"];

export interface Sky {
  mood: Mood;
  phase: Phase;
  /** Force de l'ambiance (densité des nuages, de la pluie…), 0 → 1. */
  intensity: number;
  /** Vent, 0 → 1 (≈ 60 km/h et plus). */
  wind: number;
  /** Course du soleil (ou de la lune la nuit), 0 = lever → 1 = coucher. */
  arc: number;
}

export type OutlookKind = "umbrella" | "snow" | "storm" | "heat" | "wind" | "fog" | "dry";
export interface Outlook {
  kind: OutlookKind;
  /** Probabilité de précipitations (%). */
  prob?: number;
  /** Heure locale (0-23) du début ; `null` = dès maintenant. */
  at?: number | null;
  /** Valeur associée : température max, vent max. */
  value?: number;
}

/** Code WMO → ambiance. */
export function moodOf(code: number): Mood {
  if (code <= 1) return "clear";
  if (code === 2) return "partly";
  if (code === 3) return "cloudy";
  if (code <= 48) return "fog";
  if (code <= 57) return "drizzle";
  if (code <= 67 || (code >= 80 && code <= 82)) return "rain";
  if (code <= 77 || code === 85 || code === 86) return "snow";
  return "storm";
}

/** Force d'un code humide (bruine légère → averse violente). */
const HEAVY: Record<number, number> = {
  51: 0.3, 53: 0.45, 55: 0.6, 56: 0.4, 57: 0.6,
  61: 0.4, 63: 0.65, 65: 0.95, 66: 0.5, 67: 0.8,
  71: 0.4, 73: 0.65, 75: 0.95, 77: 0.3, 85: 0.55, 86: 0.9,
  80: 0.5, 81: 0.75, 82: 1, 95: 0.75, 96: 0.9, 99: 1,
};
const RANK: Record<Mood, number> = { clear: 0, heat: 0, partly: 1, cloudy: 2, fog: 3, drizzle: 4, rain: 5, snow: 6, storm: 7 };
const WET = new Set<Mood>(["drizzle", "rain", "snow", "storm"]);
/** Probabilité à partir de laquelle une heure humide compte. */
const LIKELY = 40;
/** Fenêtre du coup d'œil (heures). */
const AHEAD = 12;

const clamp = (n: number, a = 0, b = 1) => Math.min(b, Math.max(a, n));
const hot = () => (fahrenheit.value ? 86 : 30);
const strongWind = () => (fahrenheit.value ? 31 : 50);
const windScale = () => (fahrenheit.value ? 37 : 60);

// --- Debug ----------------------------------------------------------------------------------

export const skyDebug = reactive({
  on: false,
  mood: "rain" as Mood,
  phase: "day" as Phase,
  intensity: 0.7,
  wind: 0.3,
  arc: 0.5,
});
/** Incrémenté pour déclencher un éclair tout de suite (debug). */
export const flashRequest = ref(0);

// --- Moment de la journée -------------------------------------------------------------------

/** Demi-durée de l'aube et du crépuscule autour du lever / coucher (s). */
const TWILIGHT = 50 * 60;

function phaseNow(): { phase: Phase; arc: number } {
  const today = weather.days[0];
  const now = Date.now() / 1000;
  if (!today?.sunrise || !today.sunset) return { phase: weather.current?.day === false ? "night" : "day", arc: 0.5 };
  const { sunrise: rise, sunset: set } = today;
  if (Math.abs(now - rise) < TWILIGHT) return { phase: "dawn", arc: clamp((now - rise) / (set - rise)) };
  if (Math.abs(now - set) < TWILIGHT) return { phase: "dusk", arc: clamp((now - rise) / (set - rise)) };
  if (now > rise && now < set) return { phase: "day", arc: (now - rise) / (set - rise) };
  // Nuit : course de la lune du coucher au lever suivant (≈ 24 h − durée du jour).
  const night = 86400 - (set - rise);
  const since = now >= set ? now - set : now + 86400 - set;
  return { phase: "night", arc: clamp(since / night) };
}

// --- Résumé ---------------------------------------------------------------------------------

function summarize(): { sky: Sky; outlook: Outlook } | null {
  const cur = weather.current;
  if (!cur) return null;
  const hours = weather.hours.slice(0, AHEAD);
  const today = weather.days[0];
  const { phase, arc } = phaseNow();
  const maxWind = Math.max(cur.wind, ...hours.map((h) => h.wind), today?.wind ?? 0);
  const wind = clamp(maxWind / windScale());

  // Pire condition humide probable, et quand elle commence.
  let worst: { mood: Mood; code: number; prob: number } | null = null;
  let first = -1;
  let wetProb = 0;
  hours.forEach((h, i) => {
    const m = moodOf(h.code);
    if (!WET.has(m) || h.rain < LIKELY) return;
    if (first < 0) first = i;
    wetProb = Math.max(wetProb, h.rain);
    if (!worst || RANK[m] > RANK[worst.mood] || (RANK[m] === RANK[worst.mood] && (HEAVY[h.code] ?? 0) > (HEAVY[worst.code] ?? 0)))
      worst = { mood: m, code: h.code, prob: h.rain };
  });
  // Pas d'heures (API ancienne / hors ligne) : l'heure en cours seule.
  if (!hours.length && WET.has(moodOf(cur.code))) {
    worst = { mood: moodOf(cur.code), code: cur.code, prob: today?.rain ?? 60 };
    first = 0;
    wetProb = worst.prob;
  }

  if (worst) {
    const w = worst as { mood: Mood; code: number; prob: number };
    const intensity = clamp((HEAVY[w.code] ?? 0.5) * 0.65 + (wetProb / 100) * 0.35);
    const kind: OutlookKind = w.mood === "snow" ? "snow" : w.mood === "storm" ? "storm" : "umbrella";
    return {
      sky: { mood: w.mood, phase, intensity, wind, arc },
      outlook: { kind, prob: wetProb, at: first <= 0 ? null : hourAt(hours[first].at) },
    };
  }

  // Sec : condition la plus présente (heure en cours ×3) ; une heure humide improbable compte
  // comme « couvert ».
  const votes: Partial<Record<Mood, number>> = {};
  const vote = (code: number, w: number) => {
    let m = moodOf(code);
    if (WET.has(m)) m = "cloudy";
    votes[m] = (votes[m] ?? 0) + w;
  };
  vote(cur.code, 3);
  hours.slice(1).forEach((h) => vote(h.code, 1));
  let mood = (Object.entries(votes) as [Mood, number][]).sort((a, b) => b[1] - a[1] || RANK[b[0]] - RANK[a[0]])[0][0];
  const max = today?.max ?? cur.temp;
  if ((mood === "clear" || mood === "partly") && max >= hot()) mood = "heat";

  const cover = cur.clouds / 100;
  const intensity =
    mood === "clear" ? 0.15 + cover * 0.3
      : mood === "partly" ? clamp(0.3 + cover * 0.6)
        : mood === "cloudy" ? clamp(0.6 + cover * 0.4)
          : mood === "fog" ? 0.75
            : clamp(0.45 + (max - hot()) / 10);

  const maxProb = Math.max(0, ...hours.map((h) => h.rain));
  const outlook: Outlook =
    mood === "heat" ? { kind: "heat", value: max }
      : maxWind >= strongWind() ? { kind: "wind", value: maxWind }
        : mood === "fog" ? { kind: "fog" }
          : { kind: "dry", prob: maxProb };
  return { sky: { mood, phase, intensity, wind, arc }, outlook };
}

/** Jour choisi dans la bande des 7 jours (0 = aujourd'hui : coup d'œil des prochaines heures). */
export const selectedDay = ref(0);

/** Ambiance d'un jour à venir : code dominant du jour, ciel de midi. */
function summarizeDay(i: number): { sky: Sky; outlook: Outlook } | null {
  const d = weather.days[i];
  if (!d) return null;
  let mood = moodOf(d.code);
  const wind = clamp(d.wind / windScale());
  if (WET.has(mood) && d.rain >= 30) {
    const intensity = clamp((HEAVY[d.code] ?? 0.5) * 0.65 + (d.rain / 100) * 0.35);
    const kind: OutlookKind = mood === "snow" ? "snow" : mood === "storm" ? "storm" : "umbrella";
    return { sky: { mood, phase: "day", intensity, wind, arc: 0.5 }, outlook: { kind, prob: d.rain } };
  }
  // Humide mais peu probable : journée couverte.
  if (WET.has(mood)) mood = "cloudy";
  if ((mood === "clear" || mood === "partly") && d.max >= hot()) mood = "heat";
  const intensity = mood === "clear" ? 0.2 : mood === "partly" ? 0.55 : mood === "cloudy" ? 0.8 : mood === "fog" ? 0.75 : clamp(0.45 + (d.max - hot()) / 10);
  const outlook: Outlook =
    mood === "heat" ? { kind: "heat", value: d.max }
      : d.wind >= strongWind() ? { kind: "wind", value: d.wind }
        : mood === "fog" ? { kind: "fog" }
          : { kind: "dry", prob: d.rain };
  return { sky: { mood, phase: "day", intensity, wind, arc: 0.5 }, outlook };
}

/** Ambiance réelle (null tant que la météo n'est pas chargée). Se recalcule à chaque
 *  rafraîchissement (10 min) ; `tick` force la mise à jour du moment de la journée. */
const tick = ref(0);
setInterval(() => tick.value++, 60_000);
const real = computed(() => (void tick.value, selectedDay.value > 0 ? summarizeDay(selectedDay.value) : summarize()));

const DEBUG_OUTLOOK: Record<Mood, Outlook> = {
  clear: { kind: "dry", prob: 0 },
  partly: { kind: "dry", prob: 10 },
  cloudy: { kind: "dry", prob: 20 },
  fog: { kind: "fog" },
  drizzle: { kind: "umbrella", prob: 55, at: 17 },
  rain: { kind: "umbrella", prob: 80, at: null },
  storm: { kind: "storm", prob: 70, at: 18 },
  snow: { kind: "snow", prob: 65, at: 15 },
  heat: { kind: "heat", value: 34 },
};

export const sky = computed<Sky | null>(() =>
  skyDebug.on ? { mood: skyDebug.mood, phase: skyDebug.phase, intensity: skyDebug.intensity, wind: skyDebug.wind, arc: skyDebug.arc } : (real.value?.sky ?? null),
);
export const outlook = computed<Outlook | null>(() => (skyDebug.on ? DEBUG_OUTLOOK[skyDebug.mood] : (real.value?.outlook ?? null)));
