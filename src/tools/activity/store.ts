// Activité (cf. docs/internal/activity-spec.md) : jeu en cours, app au premier plan, temps
// passé. Le service Rust détecte (catalogues des launchers, base Discord, signaux GPU,
// corrections) et pousse `activity://changed` ; ici l'état réactif et les appels.

import { computed, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Kind = "game" | "app";
export type Source = "steam" | "epic" | "gog" | "ea" | "ubisoft" | "battlenet" | "xbox" | "discord" | "heuristic" | "user" | "app";

export interface ActApp {
  pid: number;
  exe: string;
  path: string;
  title: string;
  name: string;
  kind: Kind;
  source: Source;
  appId?: string;
  art?: string;
  icon?: string;
}

export interface GameSession {
  key: string;
  name: string;
  source: Source;
  appId?: string;
  art?: string;
  pid: number;
  startedAt: number;
}

export interface Usage {
  key: string;
  name: string;
  seconds: number;
}
export interface DayStats {
  date: string;
  games: Usage[];
  apps: Usage[];
}

export const activity = reactive({
  foreground: null as ActApp | null,
  game: null as GameSession | null,
  ready: false,
});

/** Horloge partagée (durée de la session de jeu en direct), seulement pendant un jeu. */
export const now = ref(Date.now());
let tick: number | undefined;

/** Durée de la session de jeu en cours, « 1:12 » / « 12 min ». */
export const playing = computed(() => {
  const g = activity.game;
  if (!g) return null;
  return { ...g, elapsed: Math.max(0, now.value - g.startedAt) };
});

export function duration(ms: number): string {
  const m = Math.floor(ms / 60000);
  if (m < 60) return `${m} min`;
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")}`;
}

function apply(s: { foreground: ActApp | null; game: GameSession | null }) {
  activity.foreground = s.foreground;
  activity.game = s.game;
  window.clearInterval(tick);
  tick = undefined;
  if (s.game) tick = window.setInterval(() => (now.value = Date.now()), 15_000);
  now.value = Date.now();
}

let started = false;
export async function initActivity() {
  if (started) return;
  started = true;
  void listen<{ foreground: ActApp | null; game: GameSession | null }>("activity://changed", (e) => apply(e.payload));
  const s = await invoke<{ foreground: ActApp | null; game: GameSession | null }>("activity_state").catch(() => null);
  if (s) apply(s);
  activity.ready = true;
}

export const running = () => invoke<ActApp[]>("activity_running");
export const setOverride = (path: string, kind: Kind | "ignore" | null, name?: string) =>
  invoke("activity_set_override", { path, kind, name: name ?? null });
export const stats = (from: string, to: string) => invoke<{ days: DayStats[] }>("activity_stats", { from, to }).then((r) => r.days);
export const refreshCatalog = () => invoke<number>("activity_refresh_catalog");
/** Corrections de l'utilisateur (chemin d'exe → jeu / app / ignorer). */
export const overrides = () => invoke<{ path: string; kind: Kind | "ignore"; name?: string }[]>("activity_overrides");

export interface ActivitySettings {
  trackApps: boolean;
  discordDb: boolean;
}
export const setActivitySettings = (s: ActivitySettings) => invoke("activity_settings_set", { settings: s });
