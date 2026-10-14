// Mixer (volume par app) : état, polling 1,5 s tant qu'il est ouvert (suspendu pendant un
// drag), ordre des lignes (app en lecture en tête, puis Système, puis les autres dans
// l'ordre Windows) et taille demandée au notch (grandit de lignes × 34 + 14 px).

import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { np } from "./store";
import { requestSize } from "../registry";
import { activeTool } from "../../notch/state";

export interface MixerApp {
  exe: string;
  name: string;
  icon: string | null;
  level: number;
  muted: boolean;
}

export const mixerOpen = ref(false);
const system = ref<MixerApp | null>(null);
const apps = ref<MixerApp[]>([]);
export const mixerDragging = ref(false);

const BASE = { w: 520, h: 232 };
const ROW = 34;
const MAX_ROWS = 5;
const POLL = 1500;

/** Forme « écrasée » : alphanumérique minuscule, sans « .exe » (cf. Rust). */
function crush(s: string): string {
  const head = s.toLowerCase().split("!")[0];
  const base = head.split(/[\\/]/).pop() ?? head;
  return base.replace(/\.exe$/, "").replace(/[^a-z0-9]/g, "");
}

export function isPlayingApp(exe: string): boolean {
  if (!np.active) return false;
  const e = crush(exe);
  const a = crush(np.appId);
  if (!e || !a) return false;
  if (a === "308046b0af4a39cb") return e === "firefox";
  return e === a || (e.length >= 4 && a.includes(e)) || (a.length >= 4 && e.includes(a));
}

export type Row = { kind: "system"; app: MixerApp } | { kind: "app"; app: MixerApp; playing: boolean };

export const rows = computed<Row[]>(() => {
  const list = apps.value.map((app) => ({ kind: "app" as const, app, playing: isPlayingApp(app.exe) }));
  const playing = list.filter((r) => r.playing);
  const others = list.filter((r) => !r.playing);
  const sys: Row[] = system.value ? [{ kind: "system", app: system.value }] : [];
  return [...playing, ...sys, ...others];
});

async function refresh() {
  if (mixerDragging.value) return;
  const s = await invoke<{ system: MixerApp | null; apps: MixerApp[] }>("mixer_list").catch(() => null);
  if (!s || mixerDragging.value) return;
  system.value = s.system;
  apps.value = s.apps;
}

export async function setLevel(exe: string | null, level: number) {
  const target = exe === null ? system.value : apps.value.find((a) => a.exe === exe);
  if (target) {
    target.level = level;
    if (level > 0) target.muted = false;
  }
  await invoke("mixer_set", { exe, level }).catch(() => {});
}

export async function toggleMute(exe: string | null) {
  const target = exe === null ? system.value : apps.value.find((a) => a.exe === exe);
  if (!target) return;
  target.muted = !target.muted;
  await invoke("mixer_set", { exe, muted: target.muted }).catch(() => {});
}

let timer: number | undefined;

export function toggleMixer() {
  mixerOpen.value = !mixerOpen.value;
}

watch(mixerOpen, (open) => {
  window.clearInterval(timer);
  if (open) {
    void refresh();
    timer = window.setInterval(refresh, POLL);
  }
});

// Le notch grandit vers le bas selon le nombre de lignes (au-delà de 5 : défilement).
watch(
  [mixerOpen, () => rows.value.length],
  ([open, n]) => {
    if (!open) return requestSize("media", null);
    const lines = Math.min(MAX_ROWS, Math.max(2, n));
    requestSize("media", { w: BASE.w, h: BASE.h + lines * ROW + 14 });
  },
  { immediate: true },
);

// Quitter l'outil referme le mixer.
watch(activeTool, (id) => {
  if (id !== "media") mixerOpen.value = false;
});
