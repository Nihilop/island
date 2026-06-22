// Auto-update (GitHub). Vérifie une nouvelle version ; si dispo, propose l'installation
// via une notification de l'île ET via une section dédiée dans les Réglages (état réactif).
// Au clic → télécharge l'installeur SIGNÉ, l'installe et relance. La signature est vérifiée
// par l'updater avec la clé publique de tauri.conf.
import { reactive } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { post as postNotif } from "./notifications";

export type UpdateStatus = "idle" | "checking" | "uptodate" | "available" | "installing" | "error";

/** État partagé (lu par la section « Mises à jour » des Réglages). */
export const updateState = reactive<{ status: UpdateStatus; version: string; error: string }>({
  status: "idle",
  version: "",
  error: "",
});

let pending: Update | null = null;

/** Télécharge + installe la mise à jour disponible, puis relance. */
export async function applyUpdate(): Promise<void> {
  if (!pending || updateState.status === "installing") return;
  const update = pending;
  updateState.status = "installing";
  postNotif({ title: "Mise à jour en cours…", body: `Installation de la v${update.version}`, source: "Mise à jour", color: "#3b82f6" });
  try {
    await update.downloadAndInstall();
    await relaunch();
  } catch (e) {
    updateState.status = "error";
    updateState.error = String((e as Error)?.message ?? e);
    postNotif({ title: "Échec de la mise à jour", body: updateState.error, source: "Mise à jour", color: "#ff453a" });
  }
}

/** Vérifie les mises à jour. `silent` : ne notifie pas s'il n'y a rien (défaut). */
export async function checkForUpdate(silent = true): Promise<void> {
  updateState.status = "checking";
  updateState.error = "";
  try {
    const update = await check();
    if (!update) {
      updateState.status = "uptodate";
      if (!silent) postNotif({ title: "Island est à jour", source: "Mise à jour", timeout: 3000 });
      return;
    }
    pending = update;
    updateState.status = "available";
    updateState.version = update.version;
    postNotif({
      title: `Mise à jour disponible — v${update.version}`,
      body: update.body || "Clique pour installer et redémarrer.",
      source: "Mise à jour",
      color: "#3b82f6",
      timeout: 0, // reste dans l'historique tant qu'on n'agit pas
      onClick: () => void applyUpdate(),
      actions: [{ id: "install", label: "Installer", onClick: () => void applyUpdate() }],
    });
  } catch (e) {
    updateState.status = "error";
    updateState.error = String((e as Error)?.message ?? e);
    // En dev / sans release, le endpoint renvoie 404 → on ignore en mode silencieux.
    if (!silent) postNotif({ title: "Vérification de mise à jour impossible", body: updateState.error, source: "Mise à jour" });
  }
}
