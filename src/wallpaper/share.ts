// Réception des scènes partagées dans l'overlay : `.islandscene` double-cliqué (argv,
// importé par le Rust, cf. share.rs) ou glissé sur le notch. Une notification dit ce qui
// s'est passé ; la scène apparaît dans l'outil « Fonds animés ».

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { interceptDrop } from "../notch/drop";
import { notify } from "../tools/notifications/store";
import { importScene, shareError, type Imported } from "./api";
import { t } from "../notch/i18n";

type Outcome = { Ok: Imported } | { Err: string };

function report(r: Outcome) {
  if ("Ok" in r) notify({ title: t("wall.share.imported", { name: r.Ok.name }), body: t("wall.share.importedBody"), app: t("tool.wallpaper") });
  else notify({ title: t("wall.share.importFailed"), body: shareError(r.Err), app: t("tool.wallpaper") });
}

async function drain() {
  const list = await invoke<Outcome[]>("wallpaper_take_imported").catch(() => []);
  list.forEach(report);
}

export function initWallpaperShare() {
  void listen("wallpaper://imported", () => void drain());
  void drain();
  interceptDrop((paths) => {
    const scenes = paths.filter((p) => p.toLowerCase().endsWith(".islandscene"));
    for (const p of scenes) {
      importScene(p)
        .then((r) => report({ Ok: r }))
        .catch((e) => report({ Err: String(e) }));
    }
    return paths.filter((p) => !p.toLowerCase().endsWith(".islandscene"));
  });
}
