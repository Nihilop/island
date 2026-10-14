// Mode jeu (réglage off / en plein écran / toujours) : live activity élargie avec FPS,
// CPU, GPU et RAM. Les FPS viennent d'ETW (Rust), relevés chaque seconde tant que le mode
// est actif.

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../../notch/settings";

/** Une app est en plein écran (détection Rust). */
export const fullscreen = ref(false);

export const gamingActive = computed(
  () => settings.gaming === "always" || (settings.gaming === "fullscreen" && fullscreen.value),
);

export const fps = reactive({
  /** ok | noAdmin | relogin | error | starting */
  status: "starting" as string,
  value: null as number | null,
});

let timer: number | undefined;

async function poll() {
  const s = await invoke<{ status: string; fps: number | null }>("fps_sample").catch(() => null);
  if (!s) return;
  fps.status = s.status;
  fps.value = s.fps;
}

/** Débloque les FPS (UAC) : ok | relogin | cancelled | error. */
export async function unlockFps(): Promise<string> {
  const r = await invoke<string>("fps_unlock").catch(() => "error");
  if (r === "relogin") fps.status = "relogin";
  void poll();
  return r;
}

export function initGaming() {
  listen<{ active: boolean }>("fullscreen://changed", (e) => (fullscreen.value = e.payload.active));
  watch(
    gamingActive,
    (on) => {
      window.clearInterval(timer);
      if (!on) return;
      void poll();
      timer = window.setInterval(poll, 1000);
    },
    { immediate: true },
  );
  // Une fois au démarrage pour connaître l'état du déblocage (bouton des réglages).
  if (settings.gaming !== "off") void poll();
}
