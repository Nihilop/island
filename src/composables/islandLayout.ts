// Placement horizontal de l'île (déplaçable) + mode ÉDITION.
//  - `islandOffsetX` : décalage horizontal depuis le centre, en px CSS, PERSISTÉ.
//  - `editMode` : runtime. En édition, l'overlay passe plein écran (repère stable pour
//    dragger), l'île suit via translateX ; à la sortie, le décalage passe sur la boîte.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const APP = "__app__";
export const islandOffsetX = ref(0); // px CSS (0 = centré)
export const editMode = ref(false);

let inited = false;
export async function initLayout() {
  if (inited) return;
  inited = true;
  const v = await invoke<number | null>("storage_get", { ext: APP, key: "islandOffsetX" }).catch(() => null);
  if (typeof v === "number") islandOffsetX.value = v;
}

export function setOffsetX(v: number) {
  islandOffsetX.value = v;
}
export function enterEdit() {
  editMode.value = true;
}
export function exitEdit() {
  if (!editMode.value) return;
  editMode.value = false;
  invoke("storage_set", { ext: APP, key: "islandOffsetX", value: Math.round(islandOffsetX.value) }).catch(() => {});
}
