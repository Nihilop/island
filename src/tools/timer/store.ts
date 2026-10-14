// Minuteur focus / pause. État persisté : un minuteur en cours survit à un redémarrage
// (la sonnerie, elle, ne l'est pas). Un seul timeout planifié à l'instant de fin (pas de
// polling quand rien ne tourne) ; réveillé trop tôt (changement d'horloge) → replanifié.

import { computed, ref, watch } from "vue";
import { persisted } from "../../notch/persist";
import { settings } from "../../notch/settings";
import { playSound, stopSound } from "../../design/sound";

export type TimerMode = "focus" | "pause";

export const PRESETS: Record<TimerMode, number[]> = { focus: [15, 25, 45], pause: [5, 10, 15] };
const DEFAULT_MIN: Record<TimerMode, number> = { focus: 25, pause: 5 };
export const MIN_MINUTES = 1;
export const MAX_MINUTES = 180;

const { state: timer, ready } = persisted("timer", {
  mode: "focus" as TimerMode,
  /** Durée totale (s). */
  total: 25 * 60,
  /** Instant de fin (ms epoch) si en cours. */
  endAt: null as number | null,
  /** Restant (s) si en pause. */
  remaining: null as number | null,
  /** Terminé : reste affiché jusqu'à acquittement. */
  finished: false,
  /** Dernière durée personnalisée par mode (min). */
  custom: { focus: null as number | null, pause: null as number | null },
});
export { timer };

/** Sonnerie en cours (non persistée). */
export const ringing = ref(false);
/** Horloge locale (rafraîchie toutes les 200 ms en course). */
const now = ref(Date.now());

export const running = computed(() => timer.endAt !== null && !timer.finished);
export const paused = computed(() => timer.endAt === null && timer.remaining !== null && !timer.finished);
/** Intact = jamais démarré depuis le dernier réglage. */
export const pristine = computed(() => timer.endAt === null && timer.remaining === null && !timer.finished);

/** Secondes restantes. */
export const left = computed(() => {
  if (timer.finished) return 0;
  if (timer.endAt !== null) return Math.max(0, (timer.endAt - now.value) / 1000);
  return timer.remaining ?? timer.total;
});
export const progress = computed(() => (timer.total ? 1 - left.value / timer.total : 0));

/** m:ss arrondi au-dessus. */
export function fmt(s: number): string {
  const t = Math.ceil(s);
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
}

let endTimer: number | undefined;
let tick: number | undefined;
let ringTimer: number | undefined;

function schedule() {
  window.clearTimeout(endTimer);
  window.clearInterval(tick);
  if (timer.endAt === null || timer.finished) return;
  tick = window.setInterval(() => (now.value = Date.now()), 200);
  const wait = timer.endAt - Date.now();
  endTimer = window.setTimeout(() => {
    if (timer.endAt !== null && Date.now() < timer.endAt - 50) return schedule();
    finish();
  }, Math.max(0, wait));
}

function finish() {
  window.clearInterval(tick);
  timer.finished = true;
  timer.endAt = null;
  timer.remaining = null;
  now.value = Date.now();
  const a = settings.alarm;
  playSound(a.sound, a.volume, a.duration);
  ringing.value = true;
  // Sonnerie : durée choisie (« jusqu'à l'arrêt » = 120 s), 3 s minimum.
  const secs = a.duration === -1 ? 120 : Math.max(3, a.duration || 3);
  window.clearTimeout(ringTimer);
  ringTimer = window.setTimeout(() => (ringing.value = false), secs * 1000);
}

export function start() {
  if (timer.finished) return;
  timer.endAt = Date.now() + (timer.remaining ?? timer.total) * 1000;
  timer.remaining = null;
  schedule();
}

export function pause() {
  if (timer.endAt === null) return;
  timer.remaining = left.value;
  timer.endAt = null;
  schedule();
}

export const toggle = () => (running.value ? pause() : start());

export function reset() {
  timer.endAt = null;
  timer.remaining = null;
  timer.finished = false;
  stopAlarm();
  schedule();
}

export function stopAlarm() {
  ringing.value = false;
  window.clearTimeout(ringTimer);
  stopSound();
}

/** Durée (min) d'un mode : dernière perso, sinon préréglage par défaut. */
function minutesFor(mode: TimerMode): number {
  return timer.custom[mode] ?? DEFAULT_MIN[mode];
}

export function setMode(mode: TimerMode) {
  timer.mode = mode;
  timer.total = minutesFor(mode) * 60;
  reset();
}

export function setMinutes(min: number) {
  const m = Math.round(Math.min(MAX_MINUTES, Math.max(MIN_MINUTES, min)));
  if (!PRESETS[timer.mode].includes(m)) timer.custom[timer.mode] = m;
  timer.total = m * 60;
  reset();
}

/** Fermer un minuteur terminé : après un focus, prépare une pause (et inversement). */
export function dismiss() {
  stopAlarm();
  setMode(timer.mode === "focus" ? "pause" : "focus");
}

export async function initTimer() {
  await ready;
  // Fini pendant qu'Island était fermé → terminé (sans sonnerie).
  if (timer.endAt !== null && timer.endAt <= Date.now()) {
    timer.finished = true;
    timer.endAt = null;
  }
  schedule();
}

watch(
  () => timer.mode,
  () => (now.value = Date.now()),
);
