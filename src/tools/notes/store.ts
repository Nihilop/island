// Notes, côté front : liste et dossiers (stockage « façon S3 » côté Rust, cf.
// src-tauri/src/services/notes.rs), ouverture d'une note dans sa propre fenêtre
// (service de fenêtres détachées). La liste se resynchronise sur `notes://changed`.

import { computed, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Note {
  id: string;
  path: string;
  created: number;
  updated: number;
  pinned: boolean;
  trashed: number | null;
  preview: string;
}

/** Vue de la liste : tout, épinglées, un dossier (`folder:<chemin>`) ou la corbeille. */
export type Scope = "all" | "pinned" | "trash" | `folder:${string}`;

export const notes = reactive({
  list: [] as Note[],
  /** Dossiers marqués explicitement (dossiers vides compris). */
  markers: [] as string[],
  loaded: false,
  loading: false,
});

export const nameOf = (path: string) => path.split("/").pop() ?? path;
export const folderOf = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");

/** Tous les dossiers : marqueurs + préfixes des chemins des notes (hors corbeille). */
export const folders = computed(() => {
  const set = new Set<string>(notes.markers);
  for (const n of notes.list) {
    if (n.trashed) continue;
    const parts = folderOf(n.path).split("/").filter(Boolean);
    for (let i = 1; i <= parts.length; i++) set.add(parts.slice(0, i).join("/"));
  }
  return [...set].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
});

/** Notes d'une vue, triées : épinglées d'abord, puis plus récentes. */
export function inScope(scope: Scope): Note[] {
  const list = notes.list.filter((n) => {
    if (scope === "trash") return !!n.trashed;
    if (n.trashed) return false;
    if (scope === "all") return true;
    if (scope === "pinned") return n.pinned;
    const f = scope.slice(7);
    return folderOf(n.path) === f;
  });
  return list.sort((a, b) => (scope === "trash" ? (b.trashed ?? 0) - (a.trashed ?? 0) : Number(b.pinned) - Number(a.pinned) || b.updated - a.updated));
}

export async function load() {
  if (notes.loading) return;
  notes.loading = true;
  try {
    const r = await invoke<{ notes: Note[]; folders: string[] }>("notes_list");
    notes.list = r.notes;
    notes.markers = r.folders;
    notes.loaded = true;
  } finally {
    notes.loading = false;
  }
}

/** Ouvre la note dans sa fenêtre (ou ramène la fenêtre au premier plan). */
export const openNote = (n: Pick<Note, "id" | "path">) => invoke("window_open", { kind: "note", key: n.id, title: nameOf(n.path) });

export async function createNote(folder: string, name: string): Promise<Note> {
  const path = folder ? `${folder}/${name}` : name;
  const meta = await invoke<Note>("notes_create", { path, content: "" });
  await load();
  return { ...meta, preview: "" };
}

export const moveNote = (id: string, path: string) => invoke("notes_move", { id, path });
export const pinNote = (id: string, pinned: boolean) => invoke("notes_pin", { id, pinned });
export const trashNote = (id: string, trash: boolean) => invoke("notes_trash", { id, trash });
export const purgeNote = (id: string) => invoke("notes_purge", { id });
export const createFolder = (path: string) => invoke<string>("notes_folder_create", { path });
export const moveFolder = (from: string, to: string) => invoke("notes_folder_move", { from, to });
export const deleteFolder = (path: string) => invoke("notes_folder_delete", { path });
export const searchNotes = (query: string) => invoke<string[]>("notes_search", { query });

let listening = false;
/** Charge la liste une fois et la tient à jour (écritures depuis les fenêtres de notes). */
export function initNotesList() {
  if (!notes.loaded) void load();
  if (listening) return;
  listening = true;
  listen("notes://changed", () => void load());
}
