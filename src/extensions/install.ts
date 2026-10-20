// Installation et gestion des extensions v2, côté front : fenêtre d'installation (paquet
// glissé sur le notch, double-cliqué, choisi ou téléchargé du magasin), activation,
// désinstallation, mode développeur. La vérification (signature, manifeste, chemins) est faite par Rust, deux
// fois : à l'examen et à l'installation.

import { watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { settings } from "../notch/settings";
import { interceptDrop } from "../notch/drop";
import { t } from "../notch/i18n";
import { initExtensions, reload, type Manifest } from "./runtime";
import { forgetExtensionTools, registerExtensionTools } from "./tools";
import { registerExtensionSlots } from "./slots";
import "./settings";

/** `store` : relue puis signée par le magasin d'Island. */
export type Signature = "official" | "store" | "untrusted" | "unsigned" | "invalid";

export interface Inspect {
  manifest: Manifest;
  signature: Signature;
  publisher: string | null;
  size: number;
  update: { fromVersion: string; newPermissions: string[] } | null;
}

/**
 * Ouvre la fenêtre d'installation sur un paquet (choisi, glissé sur l'île, double-cliqué ou
 * téléchargé du magasin) : la fiche de consentement s'y lit en grand (InstallWindow.vue).
 */
export async function openPackage(path: string) {
  await invoke("window_open", {
    kind: "install",
    key: "extension",
    title: t("ext.consent.title"),
    size: [560, 660],
    minSize: [440, 480],
    args: { path, devMode: settings.extensions.devMode },
  });
}

/** Bouton « Installer… » : choix d'un fichier `.island`. */
export async function pickPackage() {
  const path = await openDialog({ multiple: false, filters: [{ name: "Island", extensions: ["island"] }] }).catch(() => null);
  if (typeof path === "string") await openPackage(path);
}

/** Après l'installation (fenêtre d'installation) : l'extension est activée et chargée. */
async function installed(id: string) {
  settings.extensions.disabled = settings.extensions.disabled.filter((x) => x !== id);
  await reload([id]);
}

export async function uninstall(id: string) {
  settings.extensions.disabled = settings.extensions.disabled.filter((x) => x !== id);
  await invoke("ext2_uninstall", { id });
  forgetExtensionTools(id);
  await reload();
}

export async function setEnabled(id: string, on: boolean) {
  const d = new Set(settings.extensions.disabled);
  if (on) d.delete(id);
  else d.add(id);
  settings.extensions.disabled = [...d];
  await reload();
}

/** Mode développeur : ajoute un dossier d'extension (manifeste vérifié par Rust). */
export async function addDevFolder(): Promise<string | null> {
  const dir = await openDialog({ directory: true, multiple: false }).catch(() => null);
  if (typeof dir !== "string") return null;
  try {
    await invoke<Manifest>("ext2_dev_inspect", { folder: dir });
  } catch (e) {
    return String(e);
  }
  if (!settings.extensions.devFolders.includes(dir)) settings.extensions.devFolders = [...settings.extensions.devFolders, dir];
  return null;
}

export function removeDevFolder(dir: string) {
  settings.extensions.devFolders = settings.extensions.devFolders.filter((d) => d !== dir);
}

/** Dossier de dev : recharge l'extension (après un nouveau build). */
export const reloadExtension = (id: string) => reload([id]);

async function pushDev() {
  await invoke("ext2_set_dev", { enabled: settings.extensions.devMode, folders: settings.extensions.devFolders }).catch(() => {});
}

export async function initExtensionsHost() {
  await pushDev();
  await initExtensions({ disabled: (id) => settings.extensions.disabled.includes(id) }, registerExtensionTools, registerExtensionSlots);
  watch(
    () => [settings.extensions.devMode, settings.extensions.devFolders.join("|")],
    async () => {
      await pushDev();
      await reload();
    },
  );
  listen<string>("ext2://installed", (e) => void installed(e.payload));
  // Paquet double-cliqué (argv) ou glissé sur le notch.
  listen<string>("ext2://install", (e) => void openPackage(e.payload));
  const startup = await invoke<string | null>("ext2_take_pending").catch(() => null);
  if (startup) void openPackage(startup);
  interceptDrop((paths) => {
    const pkg = paths.find((p) => p.toLowerCase().endsWith(".island"));
    if (pkg) void openPackage(pkg);
    return paths.filter((p) => !p.toLowerCase().endsWith(".island"));
  });
}
