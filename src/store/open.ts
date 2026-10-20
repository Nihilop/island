// Ouvre (ou ramène au premier plan) la fenêtre Magasin, éventuellement sur une section ou un
// élément.

import { invoke } from "@tauri-apps/api/core";
import { t } from "../notch/i18n";

export function openStore(args: { section?: "home" | "scenes" | "extensions" | "library"; item?: string } = {}) {
  return invoke("window_open", { kind: "store", key: "main", title: t("store.title"), size: [1180, 760], minSize: [860, 560], args });
}
