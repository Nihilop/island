// Placement du notch (moniteur + décalage horizontal). Source de vérité côté Rust
// (store `__app__`, clé `display`), lu au démarrage pour placer la fenêtre avant le front.

import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface MonitorInfo {
  name: string;
  primary: boolean;
  width: number;
  height: number;
}

export const display = reactive<{ monitor: string | null; offset: number }>({ monitor: null, offset: 0 });

export async function initDisplay() {
  try {
    Object.assign(display, await invoke<{ monitor: string | null; offset: number }>("notch_get_display"));
  } catch {
    /* défauts : écran principal, centré */
  }
}

export function setDisplay(patch: Partial<typeof display>, persist = true) {
  Object.assign(display, patch);
  return invoke("notch_set_display", { monitor: display.monitor, offset: display.offset, persist }).catch(() => {});
}

export const recenter = () => setDisplay({ offset: 0 });

export const listMonitors = () => invoke<MonitorInfo[]>("notch_monitors").catch(() => [] as MonitorInfo[]);
