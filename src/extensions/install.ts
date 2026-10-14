// Installation et gestion des extensions v2, côté front : fiche de consentement (paquet
// glissé sur le notch, double-cliqué ou choisi), activation, désinstallation, mode
// développeur. La vérification (signature, manifeste, chemins) est faite par Rust, deux
// fois : à l'examen et à l'installation.

import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { settings } from "../notch/settings";
import { interceptDrop } from "../notch/drop";
import { openSettings } from "../tools/settings/nav";
import { hold, open } from "../notch/state";
import { initExtensions, reload, type Manifest } from "./runtime";
import { forgetExtensionTools, registerExtensionTools } from "./tools";
import { registerExtensionSlots } from "./slots";
import "./settings";

export type Signature = "official" | "untrusted" | "unsigned" | "invalid";

export interface Inspect {
  manifest: Manifest;
  signature: Signature;
  publisher: string | null;
  size: number;
  update: { fromVersion: string; newPermissions: string[] } | null;
}

/** Paquet en attente de consentement. */
export const pending = reactive({
  path: null as string | null,
  info: null as Inspect | null,
  error: null as string | null,
  busy: false,
});

/** Examine un paquet et ouvre la fiche (Réglages › Extensions). */
export async function openPackage(path: string) {
  Object.assign(pending, { path, info: null, error: null, busy: true });
  openSettings("extensions");
  open({ tool: "settings", focus: true });
  try {
    pending.info = await invoke<Inspect>("ext2_inspect", { path });
  } catch (e) {
    pending.error = String(e);
  } finally {
    pending.busy = false;
  }
}

/** Bouton « Installer… » : choix d'un fichier `.island`. */
export async function pickPackage() {
  const path = await openDialog({ multiple: false, filters: [{ name: "Island", extensions: ["island"] }] }).catch(() => null);
  if (typeof path === "string") await openPackage(path);
}

export function cancelInstall() {
  Object.assign(pending, { path: null, info: null, error: null, busy: false });
}

/** Installe après consentement. `allowUntrusted` : paquet non officiel (mode dév.). */
export async function confirmInstall(allowUntrusted: boolean) {
  if (!pending.path || pending.busy) return;
  pending.busy = true;
  pending.error = null;
  try {
    const m = await invoke<Manifest>("ext2_install", { path: pending.path, allowUntrusted });
    settings.extensions.disabled = settings.extensions.disabled.filter((x) => x !== m.id);
    await reload([m.id]);
    cancelInstall();
  } catch (e) {
    pending.error = String(e);
    pending.busy = false;
  }
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

/** La fiche de consentement tient le notch ouvert jusqu'à la réponse. */
let pendingHold: (() => void) | null = null;
watch(
  () => pending.path,
  (path) => {
    if (path && !pendingHold) pendingHold = hold();
    else if (!path) {
      pendingHold?.();
      pendingHold = null;
    }
  },
);

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
