// Matériel : collecte dès le démarrage si l'outil Matériel est actif (historique déjà plein
// à l'ouverture). Affiché : tout chaque seconde. En fond : CPU + RAM toutes les 3 s,
// interpolés pour garder 1 valeur/s ; GPU et ping deviennent des trous (leurs compteurs
// sont coûteux) mais les derniers chiffres restent affichés.

import { computed, reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { settings } from "../../notch/settings";
import { activeTool, state } from "../../notch/state";
import { gamingActive } from "./gaming";

interface Sample {
  cpu: number;
  cpuName: string;
  memUsed: number;
  memTotal: number;
  gpu: number | null;
  gpuName: string | null;
  vramUsed: number | null;
  vramTotal: number | null;
  ping: number | null;
}

export const POINTS = 60;
type Series = (number | null)[];
const empty = (): Series => Array(POINTS).fill(null);

export const hw = reactive({
  last: null as Sample | null,
  /** Ping hors ligne (dernier essai sans réponse). */
  offline: false,
  cpu: empty(),
  ram: empty(),
  gpu: empty(),
  ping: empty(),
  /** Incrémenté à chaque nouveau point (anime le défilement des courbes). */
  tick: 0,
});

/** Outil Matériel affiché → collecte complète chaque seconde. */
export const visible = computed(() => (state.value === "open" && activeTool.value === "system") || gamingActive.value);
const enabled = computed(() => gamingActive.value || settings.tools.some((t) => t.id === "system" && t.enabled));

function push(series: Series, v: number | null) {
  series.push(v);
  if (series.length > POINTS) series.splice(0, series.length - POINTS);
}

const pct = (s: Sample) => (s.memTotal ? (s.memUsed / s.memTotal) * 100 : 0);

async function sample(full: boolean): Promise<Sample | null> {
  return invoke<Sample>("hw_sample", { full }).catch(() => null);
}

let timer: number | undefined;
let lastLight: { cpu: number; ram: number } | null = null;

async function tickFull() {
  const s = await sample(true);
  if (!s) return;
  hw.last = { ...s, gpu: s.gpu ?? hw.last?.gpu ?? null, gpuName: s.gpuName ?? hw.last?.gpuName ?? null };
  const p = await invoke<number>("hw_ping_state").catch(() => -2);
  hw.offline = p === -1;
  push(hw.cpu, s.cpu);
  push(hw.ram, pct(s));
  push(hw.gpu, s.gpu);
  push(hw.ping, s.ping);
  lastLight = { cpu: s.cpu, ram: pct(s) };
  hw.tick++;
}

/** En fond : 1 lecture CPU + RAM, interpolée en 3 points (1 valeur/s). */
async function tickLight() {
  const s = await sample(false);
  if (!s) return;
  const ram = pct(s);
  const from = lastLight ?? { cpu: s.cpu, ram };
  for (let i = 1; i <= 3; i++) {
    push(hw.cpu, from.cpu + ((s.cpu - from.cpu) * i) / 3);
    push(hw.ram, from.ram + ((ram - from.ram) * i) / 3);
    push(hw.gpu, null);
    push(hw.ping, null);
  }
  lastLight = { cpu: s.cpu, ram };
  if (hw.last) Object.assign(hw.last, { cpu: s.cpu, memUsed: s.memUsed, memTotal: s.memTotal });
  else hw.last = s;
}

function schedule() {
  window.clearInterval(timer);
  if (!enabled.value) return;
  if (visible.value) {
    void tickFull();
    timer = window.setInterval(tickFull, 1000);
  } else timer = window.setInterval(tickLight, 3000);
}

export function initHardware() {
  watch([enabled, visible], schedule, { immediate: true });
}

/** Couleur selon la tension : ≥ 70 orange, ≥ 90 rouge, sinon neutre. */
export function tone(v: number | null | undefined, kind: "pct" | "ping" = "pct"): string {
  if (v === null || v === undefined) return "var(--label)";
  if (kind === "ping") return v >= 150 ? "var(--sys-red)" : v >= 60 ? "var(--sys-orange)" : "var(--label)";
  return v >= 90 ? "var(--sys-red)" : v >= 70 ? "var(--sys-orange)" : "var(--label)";
}

export const pingScale = computed(() => Math.max(60, ...hw.ping.filter((v): v is number => v !== null)));
