// Auto-update (GitHub). Vérifie une nouvelle version ; si dispo, propose l'installation
// via une notification de l'île. Au clic → télécharge l'installeur SIGNÉ, l'installe et
// relance. La signature est vérifiée par l'updater avec la clé publique de tauri.conf.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { post as postNotif } from "./notifications";

let installing = false;

async function applyUpdate(update: Update): Promise<void> {
  if (installing) return;
  installing = true;
  postNotif({ title: "Mise à jour en cours…", body: `Installation de la v${update.version}`, source: "Mise à jour", color: "#3b82f6" });
  try {
    await update.downloadAndInstall();
    await relaunch();
  } catch (e) {
    installing = false;
    postNotif({ title: "Échec de la mise à jour", body: String((e as Error)?.message ?? e), source: "Mise à jour", color: "#ff453a" });
  }
}

/** Vérifie les mises à jour. `silent` : ne notifie pas s'il n'y a rien (défaut). */
export async function checkForUpdate(silent = true): Promise<void> {
  try {
    const update = await check();
    if (!update) {
      if (!silent) postNotif({ title: "Island est à jour", source: "Mise à jour", timeout: 3000 });
      return;
    }
    postNotif({
      title: `Mise à jour disponible — v${update.version}`,
      body: update.body || "Clique pour installer et redémarrer.",
      source: "Mise à jour",
      color: "#3b82f6",
      timeout: 0, // reste dans l'historique tant qu'on n'agit pas
      onClick: () => void applyUpdate(update),
      actions: [{ id: "install", label: "Installer", onClick: () => void applyUpdate(update) }],
    });
  } catch (e) {
    // En dev / sans release, le endpoint renvoie 404 → on ignore en mode silencieux.
    if (!silent) postNotif({ title: "Vérification de mise à jour impossible", body: String((e as Error)?.message ?? e), source: "Mise à jour" });
  }
}
