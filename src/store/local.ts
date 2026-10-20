// Fenêtre Magasin : ce qui est installé sur ce PC et les actions sur un élément. Les scènes
// s'installent et se mettent à jour ici (Rust : `market_install`) ; une extension passe
// TOUJOURS par la fenêtre d'installation (consentement aux permissions), que l'overlay ouvre
// sur `ext2://install`.

import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { install as installScene, market, marketError, refreshInstalled, sceneModified, updateFor, watchInstalled, watchUpdates, type Card } from "../market/api";
import { t } from "../notch/i18n";

interface ExtInstalled {
  manifest: { id: string; version: string };
  dev: boolean;
}

/** Version installée de chaque extension (hors dossiers de développement). */
export const local = reactive({ extensions: new Map<string, string>(), busy: null as string | null, error: null as string | null });

async function refreshExtensions() {
  const list = await invoke<ExtInstalled[]>("ext2_list").catch(() => []);
  local.extensions = new Map(list.filter((x) => !x.dev).map((x) => [x.manifest.id, x.manifest.version]));
}

let watching = false;
export function watchLocal() {
  if (watching) return;
  watching = true;
  watchInstalled();
  watchUpdates();
  void refreshExtensions();
  void listen("ext2://installed", () => void refreshExtensions());
}

export type State = "none" | "installed" | "update";

export function stateOf(c: Pick<Card, "id" | "kind">): State {
  if (updateFor(c.id)) return "update";
  if (c.kind === "scene") return market.installed[c.id] ? "installed" : "none";
  return local.extensions.has(c.id) ? "installed" : "none";
}

async function run(id: string, fn: () => Promise<unknown>) {
  if (local.busy) return;
  local.busy = id;
  local.error = null;
  try {
    await fn();
  } catch (e) {
    local.error = marketError(e);
  } finally {
    local.busy = null;
  }
}

/** Installer, ou mettre à jour. */
export const get = (c: Pick<Card, "id" | "kind">) =>
  run(c.id, async () => {
    if (c.kind === "scene") {
      if (stateOf(c) === "update" && (await sceneModified(c.id).catch(() => false)) && !window.confirm(t("market.updateOverwrite"))) return;
      await installScene(c.id);
      await refreshInstalled();
    } else {
      const path = await invoke<string>("market_download_extension", { id: c.id });
      await emit("ext2://install", path);
    }
  });

/** Scène installée : l'appliquer à tous les écrans. */
export const apply = (id: string) =>
  run(id, async () => {
    const scene = market.installed[id]?.scene;
    if (scene) await invoke("wallpaper_apply", { id: scene, display: null });
  });
