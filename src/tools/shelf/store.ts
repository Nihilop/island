// Étagère : parking de RÉFÉRENCES de fichiers (les fichiers restent en place). 30 max,
// dédupliqués par chemin, nouveaux en tête. Seuls les chemins sont persistés ; infos et
// vignettes sont relues au démarrage (un fichier disparu sort de l'étagère).

import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { persisted } from "../../notch/persist";
import { onFilesDropped } from "../../notch/drop";

export interface FileInfo {
  path: string;
  name: string;
  ext: string;
  size: number;
  category: string;
  thumb: string | null;
}

const MAX = 30;
const { state: saved, ready } = persisted("shelf", { paths: [] as string[] });
export const items = ref<FileInfo[]>([]);

function persist() {
  saved.paths = items.value.map((i) => i.path);
}

export async function add(paths: string[]) {
  const infos = await invoke<FileInfo[]>("files_probe", { paths }).catch(() => [] as FileInfo[]);
  const fresh = infos.filter((i) => !items.value.some((x) => x.path.toLowerCase() === i.path.toLowerCase()));
  // Les fichiers déjà présents remontent en tête.
  const again = items.value.filter((x) => infos.some((i) => i.path.toLowerCase() === x.path.toLowerCase()));
  const rest = items.value.filter((x) => !again.includes(x));
  items.value = [...fresh, ...again, ...rest].slice(0, MAX);
  persist();
}

export function remove(path: string) {
  items.value = items.value.filter((i) => i.path !== path);
  persist();
}

export function clear() {
  items.value = [];
  persist();
}

/** Ctrl+V : fichiers du presse-papiers (ou capture d'écran enregistrée en PNG). */
export async function pasteFromClipboard(): Promise<boolean> {
  const paths = await invoke<string[]>("files_from_clipboard").catch(() => [] as string[]);
  if (paths.length) await add(paths);
  return paths.length > 0;
}

export const open = (path: string) => invoke("file_open", { path });
export const reveal = (path: string) => invoke("reveal_path", { path });
export const copyFiles = (paths: string[]) => invoke("files_copy", { paths });

export async function initShelf() {
  onFilesDropped("shelf", (paths) => void add(paths));
  await ready;
  if (saved.paths.length) {
    items.value = await invoke<FileInfo[]>("files_probe", { paths: saved.paths }).catch(() => []);
    persist();
  }
}

export function formatSize(b: number): string {
  if (b < 1024) return `${b} o`;
  const units = ["Ko", "Mo", "Go"];
  let v = b / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
