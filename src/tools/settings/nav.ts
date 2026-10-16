// Navigation des Réglages + liens profonds : n'importe quel outil peut ouvrir « réglages +
// section » → la bonne page s'ouvre puis la section défile en vue.

import { ref } from "vue";
import { selectTool } from "../../notch/state";

export type SettingsPage = "general" | "display" | "tools" | "launcher" | "extensions" | "timer" | "ai" | "connections" | "backup";

/** Section → page qui la contient. */
const SECTION_PAGE: Record<string, SettingsPage> = {
  general: "general",
  weather: "general",
  alarm: "timer",
  discord: "connections",
  ask: "ai",
  usage: "ai",
  updates: "general",
  display: "display",
  presentation: "display",
  gaming: "display",
  tools: "tools",
  launcher: "launcher",
  extensions: "extensions",
  hello: "tools",
  vault: "tools",
  backup: "backup",
  restore: "backup",
};

/** Dernière page ouverte (mémorisée tant que l'app tourne). */
export const page = ref<SettingsPage>("general");
/** Section à faire défiler en vue après l'ouverture. */
export const pendingSection = ref<string | null>(null);

export function openSettings(section?: string) {
  if (section && SECTION_PAGE[section]) page.value = SECTION_PAGE[section];
  pendingSection.value = section ?? null;
  selectTool("settings");
}
