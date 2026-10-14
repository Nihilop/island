// Tray : le front renvoie à Rust les libellés traduits et l'état réel des cases.

import { watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { locale } from "../sdk/i18n";
import { settings } from "./settings";
import { formatAccel } from "./accel";
import { t } from "./i18n";

export function syncTray() {
  invoke("tray_sync", {
    labels: {
      open: settings.shortcut ? `${t("tray.open")}\t${formatAccel(settings.shortcut)}` : t("tray.open"),
      settings: t("tray.settings"),
      focus: t("notch.focusMode"),
      autostart: t("tray.autostart"),
      quit: t("tray.quit"),
    },
    focusMode: settings.focusMode,
  }).catch(() => {});
}

export function initTray() {
  watch([locale, () => settings.focusMode, () => settings.shortcut, () => t("tray.open")], syncTray);
  listen("autostart://changed", syncTray);
  syncTray();
}
