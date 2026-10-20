// Conditions « Visible quand… » côté éditeur : même règle que `conditions.rs`, appliquée à
// l'état réel (lu dans le Rust) ou à une simulation (Scène › Aperçu des conditions), pour
// voir une scène de nuit ou sous la pluie sans attendre.

import { invoke } from "@tauri-apps/api/core";
import { reactive } from "vue";
import type { When } from "./scene";

export const MOMENTS = ["dawn", "day", "dusk", "night"] as const;
export const SKIES = ["clear", "cloudy", "fog", "rain", "snow", "storm"] as const;
export type Moment = (typeof MOMENTS)[number];
export type Sky = (typeof SKIES)[number];

export interface CondNow {
  moment: Moment;
  sky: Sky | null;
  /** Minutes depuis minuit (heure locale). */
  minutes: number;
}

/** Simulation de l'aperçu (null = valeur réelle). */
export const simulation = reactive({ moment: null as Moment | null, sky: null as Sky | null, minutes: null as number | null });

/** État réel, rafraîchi par l'aperçu. */
export const real = reactive({ now: null as CondNow | null });

export async function refreshReal() {
  real.now = await invoke<CondNow>("wallpaper_conditions").catch(() => real.now);
}

/** État appliqué à l'aperçu : réel, sauf ce qui est simulé. */
export function current(): CondNow {
  const r = real.now ?? { moment: "day", sky: null, minutes: 12 * 60 };
  return { moment: simulation.moment ?? r.moment, sky: simulation.sky ?? r.sky, minutes: simulation.minutes ?? r.minutes };
}

const hhmm = (v: string | null | undefined) => {
  const m = v?.match(/^(\d{1,2}):(\d{2})$/);
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
};

export function visibleWhen(w: When, n: CondNow): boolean {
  const [a, b] = [hhmm(w.from), hhmm(w.to)];
  const timeOk = a !== null && b !== null ? (a <= b ? n.minutes >= a && n.minutes < b : n.minutes >= a || n.minutes < b) : !w.moments.length || w.moments.includes(n.moment);
  const skyOk = !w.sky.length || n.sky === null || w.sky.includes(n.sky);
  return timeOk && skyOk;
}
