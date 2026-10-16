// Automatisations (cf. docs/internal/automations-launcher-spec.md §1) : règles « pendant /
// quand … alors … ». Le moteur tourne dans l'overlay, qui voit déjà tout l'état (activité,
// minuteur, réglages) ; le Rust ne fournit que les actions système (sortie audio, lancer /
// fermer une app).
//
// Règle « pendant » : à l'entrée, Island applique les actions et retient l'ancienne valeur
// de chaque action réversible ; à la sortie, il la restaure — seulement si la valeur est
// encore celle que la règle a posée (l'utilisateur a pu la changer entre-temps).

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { persisted } from "../notch/persist";
import { settings } from "../notch/settings";
import { open as openIsland, setFocusMode } from "../notch/state";
import { presentation, setPresentation } from "../notch/presentation";
import { activity } from "../tools/activity/store";
import { setMinutes, start as startTimer, timer } from "../tools/timer/store";
import { notify } from "../tools/notifications/store";
import { t } from "../notch/i18n";
import { inTimeRange } from "./time";

export { inTimeRange };

// --- Modèle -------------------------------------------------------------------------------

export type Trigger =
  | { type: "game"; key: string | null; name?: string }
  | { type: "app"; path: string; name: string }
  | { type: "time"; from: string; to: string; days: number[] }
  | { type: "start" };

export type Action =
  | { type: "volume"; level: number }
  | { type: "mute"; on: boolean }
  | { type: "output"; id: string; name: string }
  | { type: "notifications"; on: boolean }
  | { type: "focus"; on: boolean }
  | { type: "presentation"; on: boolean }
  | { type: "gaming"; on: boolean }
  | { type: "launch"; path: string; name: string }
  | { type: "close"; exe: string; name: string }
  | { type: "timer"; minutes: number }
  | { type: "tool"; id: string }
  | { type: "notify"; text: string };

export interface Rule {
  id: string;
  name: string;
  enabled: boolean;
  trigger: Trigger;
  actions: Action[];
}

/** Valeur posée par une action réversible, et celle d'avant (pour la restauration). */
interface Applied {
  index: number;
  prev: unknown;
  set: unknown;
}

interface LogEntry {
  at: number;
  rule: string;
  event: "enter" | "exit" | "run";
  error?: string;
}

export const { state: automations, ready: automationsReady } = persisted("automations", {
  rules: [] as Rule[],
  /** Règles actives et ce qu'elles ont changé (survit à un redémarrage d'Island). */
  active: {} as Record<string, { since: number; applied: Applied[] }>,
  log: [] as LogEntry[],
});

export const REVERSIBLE = new Set<Action["type"]>(["volume", "mute", "output", "notifications", "focus", "presentation", "gaming"]);

// --- Actions -----------------------------------------------------------------------------

interface Volume {
  level: number;
  muted: boolean;
}
const readVolume = () => invoke<Volume | null>("master_volume");

/** Valeur actuelle de ce que touche une action réversible. */
async function read(a: Action): Promise<unknown> {
  switch (a.type) {
    case "volume":
      return (await readVolume())?.level ?? null;
    case "mute":
      return (await readVolume())?.muted ?? null;
    case "output":
      return (await invoke<{ id: string; default: boolean }[]>("audio_outputs")).find((o) => o.default)?.id ?? null;
    case "notifications":
      return !settings.notifSilent;
    case "focus":
      return settings.focusMode;
    case "presentation":
      return presentation.value;
    case "gaming":
      return settings.gaming;
    default:
      return null;
  }
}

/** Écrit une valeur (application ou restauration). */
async function write(a: Action, value: unknown) {
  switch (a.type) {
    case "volume":
      return invoke("mixer_set", { exe: null, level: Number(value) });
    case "mute":
      return invoke("mixer_set", { exe: null, muted: !!value });
    case "output":
      return invoke("audio_set_output", { id: String(value) });
    case "notifications":
      settings.notifSilent = !value;
      return;
    case "focus":
      return setFocusMode(!!value);
    case "presentation":
      return setPresentation(!!value);
    case "gaming":
      settings.gaming = value as typeof settings.gaming;
      return;
  }
}

/** Valeur cible d'une action réversible. */
function target(a: Action): unknown {
  switch (a.type) {
    case "volume":
      return Math.min(1, Math.max(0, a.level));
    case "mute":
    case "notifications":
    case "focus":
    case "presentation":
      return a.on;
    case "output":
      return a.id;
    case "gaming":
      return a.on ? "always" : "off";
    default:
      return null;
  }
}

/** Actions ponctuelles (exécutées à l'entrée seulement). */
async function runOnce(a: Action) {
  switch (a.type) {
    case "launch":
      return invoke("app_launch", { path: a.path });
    case "close":
      return invoke("app_close", { exe: a.exe });
    case "timer":
      if (timer.mode !== "focus") timer.mode = "focus";
      setMinutes(a.minutes);
      return startTimer();
    case "tool":
      return openIsland({ tool: a.id });
    case "notify":
      return void notify({ title: a.text, app: t("auto.title") });
  }
}

const same = (a: unknown, b: unknown) => (typeof a === "number" && typeof b === "number" ? Math.abs(a - b) < 0.02 : a === b);

function log(rule: Rule, event: LogEntry["event"], error?: string) {
  automations.log.unshift({ at: Date.now(), rule: rule.name, event, error });
  automations.log.splice(50);
}

async function enter(rule: Rule) {
  const applied: Applied[] = [];
  const errors: string[] = [];
  for (const [index, a] of rule.actions.entries()) {
    try {
      if (REVERSIBLE.has(a.type)) {
        const prev = await read(a);
        const set = target(a);
        await write(a, set);
        applied.push({ index, prev, set });
      } else await runOnce(a);
    } catch (e) {
      errors.push(`${a.type}: ${String(e)}`);
    }
  }
  if (rule.trigger.type !== "start") automations.active[rule.id] = { since: Date.now(), applied };
  log(rule, rule.trigger.type === "start" ? "run" : "enter", errors.join(" · ") || undefined);
}

async function exit(rule: Rule) {
  const st = automations.active[rule.id];
  delete automations.active[rule.id];
  if (!st) return;
  const errors: string[] = [];
  for (const ap of [...st.applied].reverse()) {
    const a = rule.actions[ap.index];
    if (!a || !REVERSIBLE.has(a.type) || ap.prev === null) continue;
    try {
      // Restaurer seulement si personne n'a changé la valeur entre-temps.
      if (same(await read(a), ap.set)) await write(a, ap.prev);
    } catch (e) {
      errors.push(`${a.type}: ${String(e)}`);
    }
  }
  log(rule, "exit", errors.join(" · ") || undefined);
}

// --- Conditions -------------------------------------------------------------------------

const now = ref(new Date());

function holds(tr: Trigger): boolean {
  switch (tr.type) {
    case "game":
      return !!activity.game && (tr.key === null || activity.game.key === tr.key);
    case "app":
      return !!activity.foreground && activity.foreground.path.toLowerCase() === tr.path.toLowerCase();
    case "time":
      return inTimeRange(tr, now.value);
    case "start":
      return false;
  }
}

/** Anti-rebond : la condition doit tenir 3 s pour entrer, être fausse 5 s pour sortir. */
const ENTER_MS = 3000;
const EXIT_MS = 5000;
const since = new Map<string, { value: boolean; at: number }>();
let busy = false;

async function evaluate() {
  if (busy) return;
  busy = true;
  try {
    const t0 = Date.now();
    for (const rule of automations.rules) {
      if (rule.trigger.type === "start") continue;
      const on = rule.enabled && holds(rule.trigger);
      const s = since.get(rule.id);
      if (!s || s.value !== on) since.set(rule.id, { value: on, at: t0 });
      const stable = t0 - (since.get(rule.id)?.at ?? t0);
      const active = rule.id in automations.active;
      if (on && !active && stable >= ENTER_MS) await enter(rule);
      else if (!on && active && stable >= EXIT_MS) await exit(rule);
    }
    // Règles supprimées alors qu'actives : on restaure ce qu'elles avaient changé.
    for (const id of Object.keys(automations.active)) {
      if (!automations.rules.some((r) => r.id === id)) delete automations.active[id];
    }
  } finally {
    busy = false;
  }
}

/** Il faut évaluer en continu tant qu'une règle « pendant » est active ou en attente. */
const needsTick = computed(() => automations.rules.some((r) => r.enabled && r.trigger.type !== "start"));

let tick: number | undefined;
let started = false;

export async function initAutomations() {
  if (started) return;
  started = true;
  await automationsReady;
  // Règles « au démarrage ».
  for (const r of automations.rules) if (r.enabled && r.trigger.type === "start") void enter(r);
  watch(
    needsTick,
    (on) => {
      window.clearInterval(tick);
      tick = undefined;
      if (on) {
        tick = window.setInterval(() => {
          now.value = new Date();
          void evaluate();
        }, 1000);
      }
    },
    { immediate: true },
  );
}

// --- Édition ------------------------------------------------------------------------------

export function newRule(partial?: Partial<Rule>): Rule {
  return { id: crypto.randomUUID(), name: t("auto.newRule"), enabled: true, trigger: { type: "game", key: null }, actions: [], ...partial };
}

/** Désactiver / modifier une règle active : on la fait sortir proprement d'abord. */
export async function deactivate(id: string) {
  const rule = automations.rules.find((r) => r.id === id);
  if (rule && id in automations.active) await exit(rule);
  since.delete(id);
}

export async function removeRule(id: string) {
  await deactivate(id);
  automations.rules = automations.rules.filter((r) => r.id !== id);
}

/** Exécuter une règle maintenant (test depuis l'éditeur), sans l'activer. */
export async function testRule(rule: Rule) {
  for (const a of rule.actions) {
    if (REVERSIBLE.has(a.type)) await write(a, target(a));
    else await runOnce(a);
  }
  log(rule, "run");
}

export const editing = reactive({ id: null as string | null });
