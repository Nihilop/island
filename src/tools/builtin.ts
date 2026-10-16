// Outils intégrés. En L1, tous affichent une vue provisoire ; chaque lot remplace la
// vue de ses outils par la vraie.

import {
  PhArrowsLeftRight,
  PhBell,
  PhChartLineUp,
  PhCheckCircle,
  PhWallet,
  PhClipboardText,
  PhCloudSun,
  PhCpu,
  PhDiscordLogo,
  PhGauge,
  PhGear,
  PhLockKey,
  PhMagnifyingGlass,
  PhMusicNotes,
  PhNote,
  PhSparkle,
  PhSquaresFour,
  PhTimer,
  PhTray,
} from "@phosphor-icons/vue";
import type { Component } from "vue";
import { registerTool } from "./registry";
import { DEFAULT_TOOLS } from "../notch/settings";
import { t } from "../notch/i18n";
import ComingSoon from "./ComingSoon.vue";
import MediaView from "./media/MediaView.vue";
import VaultView from "./vault/VaultView.vue";
import OverviewView from "./overview/OverviewView.vue";
import TimerView from "./timer/TimerView.vue";
import TodosView from "./todos/TodosView.vue";
import BudgetView from "./budget/BudgetView.vue";
import NotesView from "./notes/NotesView.vue";
import ConvertView from "./convert/ConvertView.vue";
import WeatherView from "./weather/WeatherView.vue";
import MarketsView from "./markets/MarketsView.vue";
import ClipboardView from "./clipboard/ClipboardView.vue";
import ShelfView from "./shelf/ShelfView.vue";
import SystemView from "./system/SystemView.vue";
import DiscordView from "./discord/DiscordView.vue";
import AskView from "./ask/AskView.vue";
import UsageView from "./usage/UsageView.vue";
import NotificationsView from "./notifications/NotificationsView.vue";
import LauncherView from "../launcher/LauncherView.vue";

/** Vues réelles déjà portées ; les autres restent provisoires. */
const VIEWS: Record<string, Component> = {
  overview: OverviewView,
  notifications: NotificationsView,
  media: MediaView,
  timer: TimerView,
  todos: TodosView,
  budget: BudgetView,
  notes: NotesView,
  convert: ConvertView,
  weather: WeatherView,
  trading: MarketsView,
  clipboard: ClipboardView,
  shelf: ShelfView,
  system: SystemView,
  discord: DiscordView,
  ask: AskView,
  usage: UsageView,
  vault: VaultView,
};
import SettingsView from "./settings/SettingsView.vue";

// Taille ouverte (header + contenu + dock compris), en DIP.
const SPECS: Record<string, { icon: Component; size: [number, number] }> = {
  overview: { icon: PhSquaresFour, size: [520, 252] },
  notifications: { icon: PhBell, size: [520, 320] },
  media: { icon: PhMusicNotes, size: [520, 232] },
  timer: { icon: PhTimer, size: [520, 192] },
  todos: { icon: PhCheckCircle, size: [520, 276] },
  budget: { icon: PhWallet, size: [640, 420] },
  notes: { icon: PhNote, size: [520, 260] },
  clipboard: { icon: PhClipboardText, size: [600, 236] },
  shelf: { icon: PhTray, size: [560, 236] },
  ask: { icon: PhSparkle, size: [560, 440] },
  convert: { icon: PhArrowsLeftRight, size: [520, 290] },
  trading: { icon: PhChartLineUp, size: [560, 380] },
  usage: { icon: PhGauge, size: [600, 320] },
  weather: { icon: PhCloudSun, size: [520, 250] },
  system: { icon: PhCpu, size: [600, 248] },
  discord: { icon: PhDiscordLogo, size: [520, 210] },
  vault: { icon: PhLockKey, size: [560, 324] },
};

export function registerBuiltinTools() {
  for (const { id, enabled } of DEFAULT_TOOLS) {
    const spec = SPECS[id];
    registerTool({
      id,
      icon: spec.icon,
      title: () => t(`tool.${id}`),
      size: { w: spec.size[0], h: spec.size[1] },
      view: VIEWS[id] ?? ComingSoon,
      defaultEnabled: enabled,
    });
  }
  // Launcher : hors dock (raccourci / touche Windows), toute la surface du notch.
  registerTool({
    id: "launcher",
    icon: PhMagnifyingGlass,
    title: () => t("tool.launcher"),
    size: { w: 640, h: 86 },
    view: LauncherView,
    defaultEnabled: true,
    dock: false,
    bare: true,
  });
  registerTool({
    id: "settings",
    icon: PhGear,
    title: () => t("tool.settings"),
    size: { w: 660, h: 460 },
    view: SettingsView,
    defaultEnabled: true,
  });
}
