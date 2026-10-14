// Historique du presse-papiers (tenu en mémoire par Rust). Le relevé n'est actif que si
// l'outil Presse-papiers est activé (sinon Rust ne lit rien et vide l'historique).

import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../../notch/settings";

export interface ClipEntry {
  id: number;
  kind: "text" | "link" | "image" | "files";
  at: number;
  preview: string | null;
  files: string[];
  thumb: string | null;
  width: number | null;
  height: number | null;
}

export const entries = ref<ClipEntry[]>([]);
export type Filter = "all" | "text" | "link" | "image" | "files";
export const filter = ref<Filter>("all");

export const visible = computed(() =>
  entries.value.filter((e) => {
    if (filter.value === "all") return true;
    // « Images » inclut les fichiers ayant une vignette.
    if (filter.value === "image") return e.kind === "image" || (e.kind === "files" && !!e.thumb);
    return e.kind === filter.value;
  }),
);

/** Recopie une entrée ; renvoie le message d'erreur éventuel. */
export const copy = (id: number) =>
  invoke("clip_copy", { id })
    .then(() => null)
    .catch((e) => {
      console.error("[clipboard] recopie impossible", e);
      return String(e);
    });
export const remove = (id: number) => invoke("clip_remove", { id }).catch(() => {});
export const clearAll = () => invoke("clip_clear").catch(() => {});
export const preview = (id: number) => invoke<string>("clip_preview", { id });

export async function initClipboard() {
  listen<ClipEntry[]>("clip://changed", (e) => (entries.value = e.payload));
  entries.value = await invoke<ClipEntry[]>("clip_list").catch(() => []);
  watch(
    () => settings.tools.find((t) => t.id === "clipboard")?.enabled ?? false,
    (on) => invoke("clip_set_mode", { mode: on ? "all" : "off" }).catch(() => {}),
    { immediate: true },
  );
}
