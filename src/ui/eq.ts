// Niveaux de l'égaliseur partagés par toutes ses instances. On s'abonne côté Rust tant
// qu'au moins un égaliseur est monté ET que la lecture joue (capture conservée 2 s après
// la pause, pour une reprise instantanée).

import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { np } from "../tools/media/store";

export const levels = new Float32Array(4);
/** Dernier instant (ms) où un niveau non nul est arrivé. */
export let lastSound = 0;
export let lastLevels = 0;

const mounted = ref(0);
let subscribed = false;
let unsubTimer: number | undefined;

listen<number[]>("eq://levels", (e) => {
  const now = performance.now();
  lastLevels = now;
  let any = false;
  e.payload.forEach((v, i) => {
    levels[i] = v;
    if (v > 0.02) any = true;
  });
  if (any) lastSound = now;
});

function setSubscribed(on: boolean) {
  if (on === subscribed) return;
  subscribed = on;
  invoke("eq_subscribe", { on }).catch(() => {});
}

watch(
  () => mounted.value > 0 && np.playing,
  (want) => {
    window.clearTimeout(unsubTimer);
    if (want) setSubscribed(true);
    else if (mounted.value === 0) setSubscribed(false);
    else unsubTimer = window.setTimeout(() => setSubscribed(false), 2000);
  },
);

export function useEqSubscription() {
  mounted.value++;
  return () => {
    mounted.value = Math.max(0, mounted.value - 1);
  };
}
