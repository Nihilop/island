// Apparence de l'île — DEUX axes INDÉPENDANTS, persistés séparément (KV `__app__` + event,
// comme le thème clair/sombre) → Réglages change, l'overlay suit :
//  1. LAYOUT (`islandTheme`) : forme/ancrage. "floating" (pilule flottante) | "topbar"
//     (ancrée au bord haut, congés inversés, tailles distinctes idle/ouvert).
//  2. COULEUR (`islandAccent`) : override de --primary dans l'île (point central, sélection…).
//     Valeur À PART (color picker), indépendante du layout. "" = primary par défaut.
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

export type IslandTheme = "floating" | "topbar";

export interface ThemeDef {
  anchor: "floating" | "topbar"; // le thème ne change que le CHROME (ancrage + congés), pas la taille
}

const THEMES: Record<IslandTheme, ThemeDef> = {
  floating: { anchor: "floating" },
  topbar: { anchor: "topbar" },
};

const APP = "__app__";
const isTheme = (v: unknown): v is IslandTheme => v === "floating" || v === "topbar";

// --- Axe 1 : LAYOUT ---------------------------------------------------------------
export const islandTheme = ref<IslandTheme>("floating");
export const theme = computed<ThemeDef>(() => THEMES[islandTheme.value] ?? THEMES.floating);

// --- Axe 2 : COULEUR d'accent (indépendant du layout) -----------------------------
export const islandAccent = ref<string>("");

let inited = false;
export async function initIslandTheme() {
  if (inited) return;
  inited = true;
  const [t, a] = await Promise.all([
    invoke<string | null>("storage_get", { ext: APP, key: "islandTheme" }).catch(() => null),
    invoke<string | null>("storage_get", { ext: APP, key: "islandAccent" }).catch(() => null),
  ]);
  if (isTheme(t)) islandTheme.value = t;
  if (typeof a === "string") islandAccent.value = a;
  await listen<IslandTheme>("island-theme://changed", (e) => { if (isTheme(e.payload)) islandTheme.value = e.payload; });
  await listen<string>("island-accent://changed", (e) => { islandAccent.value = e.payload ?? ""; });
}

export function setIslandTheme(t: IslandTheme) {
  islandTheme.value = t;
  invoke("storage_set", { ext: APP, key: "islandTheme", value: t }).catch(() => {});
  emit("island-theme://changed", t).catch(() => {});
}
export function setIslandAccent(c: string) {
  islandAccent.value = c;
  invoke("storage_set", { ext: APP, key: "islandAccent", value: c }).catch(() => {});
  emit("island-accent://changed", c).catch(() => {});
}
export function cycleIslandTheme() {
  setIslandTheme(islandTheme.value === "floating" ? "topbar" : "floating");
}
