// Voyants de confidentialité + mode présentation.
//   - voyants : vert = micro ou caméra utilisés, rouge = écran enregistré/partagé ;
//   - mode présentation : masque les aperçus du presse-papiers et verrouille le coffre.
//     Manuel, ou automatique pendant un enregistrement d'écran détecté. Le désactiver
//     pendant un enregistrement auto = « snooze » jusqu'à la fin de cet enregistrement.
// Relevé toutes les 1,5 s, seulement si les voyants ou la présentation auto sont actifs.

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { settings } from "./settings";

export const privacy = reactive({ mic: false, camera: false, screen: false });

/** Activation manuelle (non persistée). */
const manual = ref(false);
/** Désactivé pendant l'enregistrement en cours (se réactive au suivant). */
const snoozed = ref(false);

export const autoActive = computed(() => settings.presentationAuto && privacy.screen && !snoozed.value);
export const presentation = computed(() => manual.value || autoActive.value);

/** Un seul interrupteur gère le manuel et le snooze de l'automatique. */
export function setPresentation(on: boolean) {
  if (on) {
    snoozed.value = false;
    manual.value = true;
  } else {
    manual.value = false;
    if (privacy.screen) snoozed.value = true;
  }
}

let timer: number | undefined;

async function poll() {
  const s = await invoke<{ mic: boolean; camera: boolean; screen: boolean }>("privacy_state").catch(() => null);
  if (!s) return;
  if (s.mic !== privacy.mic || s.camera !== privacy.camera || s.screen !== privacy.screen) Object.assign(privacy, s);
}

export function initPresentation() {
  // Fin de l'enregistrement → le snooze tombe.
  watch(
    () => privacy.screen,
    (rec) => {
      if (!rec) snoozed.value = false;
    },
  );
  watch(
    () => settings.privacyDots || settings.presentationAuto,
    (on) => {
      window.clearInterval(timer);
      if (!on) return Object.assign(privacy, { mic: false, camera: false, screen: false });
      void poll();
      timer = window.setInterval(poll, 1500);
    },
    { immediate: true },
  );
}
