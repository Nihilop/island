// Auto-update (GitHub). Vérifie 15 s après le démarrage puis toutes les 24 h si la
// recherche automatique est active ; jamais de téléchargement sans clic. L'installeur
// est SIGNÉ (clé publique de tauri.conf), installé puis l'app relance.
import { reactive } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { notify } from "../tools/notifications/store";
import { t } from "../notch/i18n";

export type UpdateStatus = "idle" | "checking" | "current" | "available" | "downloading" | "error";

/** État partagé (Réglages › Mises à jour, badge de l'engrenage). */
export const updateState = reactive<{ status: UpdateStatus; version: string; notes: string; progress: number; error: string }>({
  status: "idle",
  version: "",
  notes: "",
  progress: 0,
  error: "",
});

let pending: Update | null = null;

/** Télécharge + installe la mise à jour disponible, puis relance. */
export async function applyUpdate(): Promise<void> {
  if (!pending || updateState.status === "downloading") return;
  const update = pending;
  updateState.status = "downloading";
  updateState.progress = 0;
  let total = 0;
  let done = 0;
  try {
    await update.downloadAndInstall((e) => {
      if (e.event === "Started") total = e.data.contentLength ?? 0;
      else if (e.event === "Progress") {
        done += e.data.chunkLength;
        updateState.progress = total ? Math.min(1, done / total) : 0;
      } else updateState.progress = 1;
    });
    await relaunch();
  } catch (e) {
    updateState.status = "error";
    updateState.error = String((e as Error)?.message ?? e);
  }
}

/** Vérifie les mises à jour. `silent` : pas de notification s'il n'y a rien. */
export async function checkForUpdate(silent = true): Promise<void> {
  if (updateState.status === "checking" || updateState.status === "downloading") return;
  updateState.status = "checking";
  updateState.error = "";
  try {
    const update = await check({ timeout: 8000 });
    if (!update) {
      updateState.status = "current";
      return;
    }
    const isNew = pending?.version !== update.version;
    pending = update;
    updateState.status = "available";
    updateState.version = update.version;
    updateState.notes = update.body ?? "";
    if (isNew)
      notify({
        title: `Island ${update.version}`,
        body: t("notifs.update.body"),
        action: { label: t("notifs.update.action"), run: () => void applyUpdate() },
      });
  } catch (e) {
    // En dev / sans release, le endpoint renvoie 404 → on reste discret.
    updateState.status = silent ? "idle" : "error";
    updateState.error = String((e as Error)?.message ?? e);
  }
}

let started = false;

/** Recherche automatique : 15 s après le démarrage, puis toutes les 24 h. */
export function startAutoUpdate(enabled: () => boolean) {
  if (started) return;
  started = true;
  const tick = () => {
    if (enabled()) void checkForUpdate();
  };
  window.setTimeout(tick, 15_000);
  window.setInterval(tick, 24 * 3600_000);
}
