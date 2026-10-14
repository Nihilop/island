// Slot « réglages » des extensions v2 : le manifeste déclare des champs, l'hôte génère la
// page (Réglages › Extensions) et range les valeurs dans le stockage de l'extension
// (`settings.<clé>`) ou, pour un champ `secret`, dans son trousseau. L'extension lit avec
// `settings.get(clé)` et reçoit l'événement `settings.changed { key }`.

import { invoke } from "@tauri-apps/api/core";
import { addMethods, emitTo, type Manifest, type SettingDecl } from "./runtime";

const storageKey = (key: string) => `settings.${key}`;

function decl(ext: Manifest, key: string): SettingDecl {
  const d = ext.contributes.settings.find((s) => s.key === key);
  if (!d) throw new Error(`unknownSetting:${key}`);
  return d;
}

/** Valeur d'un réglage (défaut du manifeste si jamais réglé). */
export async function readSetting(ext: Manifest, key: string): Promise<unknown> {
  const d = decl(ext, key);
  if (d.type === "secret") return invoke<string | null>("ext2_secret_get", { id: ext.id, key: storageKey(key) });
  const v = await invoke<unknown>("ext2_storage_get", { id: ext.id, key: storageKey(key) });
  return v ?? d.default ?? (d.type === "switch" ? false : d.type === "list" ? [] : null);
}

/** Écrit un réglage (depuis la page de réglages) et prévient l'extension. */
export async function writeSetting(ext: Manifest, key: string, value: unknown) {
  const d = decl(ext, key);
  if (d.type === "secret") {
    if (value) await invoke("ext2_secret_set", { id: ext.id, key: storageKey(key), value: String(value) });
    else await invoke("ext2_secret_delete", { id: ext.id, key: storageKey(key) });
  } else {
    await invoke("ext2_storage_set", { id: ext.id, key: storageKey(key), value: value ?? null });
  }
  emitTo(ext.id, "settings.changed", { key });
}

addMethods({
  "settings.get": (ext, key: unknown) => readSetting(ext, String(key)),
});
