// Présence physique : réserve un bandeau haut (AppBar Windows) pour que l'île reste
// visible sous les fenêtres MAXIMISÉES. Réglage persisté (`__app__`), appliqué par
// l'OVERLAY (fenêtre toujours vivante), suspendu temporairement en jeu plein écran.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

const APP = "__app__";
const RESERVE_H = 48; // px CSS réservés en haut (couvre l'île au repos ; ajustable)

export const physicalPresence = ref(false);
let suspended = false; // vrai quand un jeu/app est en plein écran → on libère l'espace

// Bumpé à chaque application : au SETPOS de l'AppBar, Windows « pousse » les fenêtres hors
// du bandeau (dont notre overlay). Island.vue écoute ce signal pour RE-FORCER l'overlay en
// haut (l'île doit rester fixe à y=0, jamais soumise à la réservation).
export const reassertSignal = ref(0);

function apply() {
  invoke("set_physical_presence", { enabled: physicalPresence.value && !suspended, height: RESERVE_H })
    .then(() => reassertSignal.value++)
    .catch(() => {});
}

// À appeler DANS L'OVERLAY : charge le réglage, applique, et gère la suspension (jeu
// plein écran) + la synchro si Réglages change la valeur.
let inited = false;
export async function initPresence() {
  if (inited) return;
  inited = true;
  physicalPresence.value = (await invoke<boolean | null>("storage_get", { ext: APP, key: "physicalPresence" }).catch(() => null)) ?? false;
  apply();
  await listen<{ active: boolean }>("fullscreen://changed", (e) => {
    suspended = !!e.payload?.active;
    apply();
  });
  await listen<boolean>("presence://changed", (e) => {
    physicalPresence.value = !!e.payload;
    apply();
  });
}

// Réglages : charge la valeur courante pour l'affichage du toggle.
export async function loadPresence() {
  physicalPresence.value = (await invoke<boolean | null>("storage_get", { ext: APP, key: "physicalPresence" }).catch(() => null)) ?? false;
}

// Réglages : persiste + diffuse ; c'est l'OVERLAY qui applique réellement (il connaît la
// suspension jeu, et il est toujours vivant même si Réglages se ferme).
export function setPhysicalPresence(v: boolean) {
  physicalPresence.value = v;
  invoke("storage_set", { ext: APP, key: "physicalPresence", value: v }).catch(() => {});
  emit("presence://changed", v).catch(() => {});
}
