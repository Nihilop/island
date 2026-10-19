// Accès aux fonds animés (commandes `wallpaper_*` de src-tauri/src/wallpaper/mod.rs).
// Partagé par l'outil du dock, l'éditeur, les réglages et les automatisations.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { reactive } from "vue";
import { normalize, type Scene, type SceneEntry } from "./scene";
import { t } from "../notch/i18n";

export interface WallpaperState {
  /** Scène de « Tous les écrans » (null = fond Windows). */
  all: string | null;
  /** Écrans réglés à part : id d'écran → scène ("" = fond Windows sur cet écran). */
  screens: Record<string, string>;
  fps: number;
  pauseInGame: boolean;
  running: boolean;
  paused: boolean;
  error: string | null;
}

export const wallpaper = reactive({
  scenes: [] as SceneEntry[],
  state: { all: null, screens: {}, fps: 30, pauseInGame: true, running: false, paused: false, error: null } as WallpaperState,
  loaded: false,
});

export async function refresh() {
  const [list, state] = await Promise.all([invoke<SceneEntry[]>("wallpaper_list"), invoke<WallpaperState>("wallpaper_state")]);
  // Préréglages : nom traduit (le Rust les nomme en français).
  wallpaper.scenes = list.map((e) => {
    const scene = normalize(e.scene);
    if (e.preset) {
      const k = `wall.presetName.${e.id}`;
      const name = t(k);
      if (name !== k) scene.name = name;
    }
    return { ...e, scene };
  });
  wallpaper.state = state;
  wallpaper.loaded = true;
}

let watching = false;
/** Charge la liste et suit les changements (autre fenêtre, moteur). */
export function watchWallpaper() {
  if (watching) return;
  watching = true;
  void refresh().catch(() => {});
  void listen("wallpaper://changed", () => void refresh().catch(() => {}));
  void listen<Pick<WallpaperState, "running" | "paused" | "error">>("wallpaper://status", (e) => Object.assign(wallpaper.state, e.payload));
}

/** `display` absent = tous les écrans (les réglages par écran sont oubliés). */
export const apply = (id: string | null, display: string | null = null) => invoke("wallpaper_apply", { id, display });

/** Scène affichée sur un écran (null = fond Windows). */
export function sceneOn(display: string): string | null {
  const s = wallpaper.state.screens[display];
  return s === undefined ? wallpaper.state.all : s || null;
}

export interface Display {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  work: [number, number, number, number];
  scale: number;
  primary: boolean;
}
export const displays = () => invoke<Display[]>("hw_displays");
export const getScene = (id: string) => invoke<Scene>("wallpaper_get", { id }).then(normalize);
export const saveScene = (id: string, scene: Scene) => invoke("wallpaper_save", { id, scene });
export const deleteScene = (id: string) => invoke("wallpaper_delete", { id });
export const importImage = (id: string, path: string) => invoke<string>("wallpaper_import_image", { id, path });
export const setOptions = (fps: number, pauseInGame: boolean) => invoke("wallpaper_set_options", { fps, pauseInGame });

/** Identifiant d'une nouvelle scène (`[a-z0-9-]`, cf. `valid_id`). */
export const newId = () => `s-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;

const urls = new Map<string, string>();
/** URL locale (blob) de l'image d'une scène, mise en cache — vignettes. */
export async function imageUrl(id: string, name: string): Promise<string> {
  const key = `${id}/${name}`;
  const hit = urls.get(key);
  if (hit) return hit;
  const buf = await invoke<ArrayBuffer>("wallpaper_image", { id, name });
  const url = URL.createObjectURL(new Blob([buf]));
  urls.set(key, url);
  return url;
}

export async function imageBitmap(id: string, name: string): Promise<ImageBitmap> {
  const buf = await invoke<ArrayBuffer>("wallpaper_image", { id, name });
  return createImageBitmap(new Blob([buf]));
}

/** Fond CSS d'une vignette de dégradé (même angle que le shader : 0° = vers la droite). */
export function gradientCss(s: Scene): string {
  const [a, b] = s.background.colors;
  return `linear-gradient(${s.background.angle + 90}deg, ${a}, ${b})`;
}

/** Ouvre l'éditeur (fenêtre unique) sur une scène, ou sur une nouvelle. */
export function openEditor(title: string, id?: string) {
  return invoke("window_open", { kind: "wallpaper", key: "editor", title, size: [1120, 700], minSize: [860, 540], args: { id: id ?? null } });
}
