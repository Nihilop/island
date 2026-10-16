// Réglages du notch. À chaque démarrage on fusionne avec les défauts : outils inconnus retirés, nouveaux ajoutés.
// Le placement (moniteur + décalage) vit côté Rust (store `__app__`, clé `display`) pour que la fenêtre
// soit placée avant même que le front charge.

import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SoundDuration, SoundId } from "../design/sound";

export type OpenMode = "hover" | "click";
export type HoverDelay = "fast" | "normal" | "patient";
export type IdleStyle = "notch" | "line";
export type FocusStyle = "dim" | "line";
export type Visibility = "always" | "hideFullscreen";

export interface ToolPref {
  id: string;
  enabled: boolean;
}

export interface NotchSettings {
  openMode: OpenMode;
  hoverDelay: HoverDelay;
  tools: ToolPref[];
  idleStyle: IdleStyle;
  focusStyle: FocusStyle;
  focusMode: boolean;
  visibility: Visibility;
  /** Raccourci global d'ouverture (accelerator Tauri), "" = aucun. */
  shortcut: string;
  /** Horloge 24 h (sinon 12 h). Défaut : selon la région Windows. */
  clock24: boolean;
  /** Recherche automatique des mises à jour (15 s après le démarrage puis toutes les 24 h). */
  autoUpdate: boolean;
  /** Verrouillage automatique après inactivité (secondes) : Coffre et outils verrouillés par Windows Hello. */
  vaultTimeout: number;
  /** Windows Hello par outil : ouverture rapide du Coffre, verrou de la Note. */
  hello: { vault: boolean; notes: boolean; budget: boolean };
  /** Notification reçue notch fermé : pastille (minimal) ou bannière complète. */
  notifStyle: "minimal" | "full";
  /** Launcher : raccourci, touche Windows, apps qui le suspendent, plein écran, moteur web. */
  /** Extensions v2 : désactivées, mode développeur (dossiers non empaquetés). */
  extensions: { disabled: string[]; devMode: boolean; devFolders: string[] };
  launcher: {
    shortcut: string;
    winKey: boolean;
    exclude: string[];
    fullscreenOff: boolean;
    engine: "google" | "duckduckgo" | "bing";
  };
  /** Coffre : options du générateur de mots de passe. */
  passwordGen: { length: number; digits: boolean; symbols: boolean };
  /** Minuteur : son, volume 0–1, durée (0 = une fois, -1 = jusqu'à l'arrêt). */
  alarm: { sound: SoundId; volume: number; duration: SoundDuration };
  /** Température : Celsius ou Fahrenheit (défaut selon la région Windows). */
  tempUnit: "c" | "f";
  /** Voyants micro/caméra (vert) et écran (rouge) à droite du notch. */
  privacyDots: boolean;
  /** Mode présentation automatique pendant un enregistrement d'écran. */
  presentationAuto: boolean;
  /** Discord : Client ID de l'application du portail développeur (le secret est au coffre Windows). */
  discordClientId: string;
  /** Mode jeu : live activity FPS / CPU / GPU / RAM. */
  gaming: "off" | "fullscreen" | "always";
  /** Ask : fournisseur et réglages par fournisseur (clés au Gestionnaire d'identifiants). */
  ask: {
    provider: AskProvider;
    byProvider: Record<AskProvider, { model: string; effort: "low" | "medium" | "high" | "xhigh"; baseUrl?: string }>;
  };
  /** Quotas IA : fournisseurs affichés. */
  usageProviders: { claude: boolean; chatgpt: boolean };
  /** Thème de l'interface : sombre, clair ou suivre Windows (`prefers-color-scheme`). */
  theme: Theme;
}

export type Theme = "dark" | "light" | "system";

export type AskProvider = "anthropic" | "openai" | "openrouter" | "custom";

export const HOVER_DELAY_MS: Record<HoverDelay, number> = { fast: 90, normal: 220, patient: 480 };

/** Ordre par défaut du dock et activation initiale. */
export const DEFAULT_TOOLS: ToolPref[] = [
  { id: "overview", enabled: true },
  { id: "notifications", enabled: true },
  { id: "media", enabled: true },
  { id: "timer", enabled: true },
  { id: "todos", enabled: true },
  { id: "budget", enabled: true },
  { id: "notes", enabled: true },
  { id: "clipboard", enabled: true },
  { id: "shelf", enabled: true },
  { id: "ask", enabled: true },
  { id: "convert", enabled: true },
  { id: "trading", enabled: false },
  { id: "usage", enabled: true },
  { id: "weather", enabled: true },
  { id: "system", enabled: false },
  { id: "discord", enabled: false },
  { id: "vault", enabled: false },
];

const DEFAULTS: NotchSettings = {
  openMode: "hover",
  hoverDelay: "normal",
  tools: DEFAULT_TOOLS,
  idleStyle: "notch",
  focusStyle: "dim",
  focusMode: false,
  visibility: "always",
  // En dev, Ctrl+Alt+Espace est souvent pris (Claude desktop) : défaut de test.
  shortcut: import.meta.env.DEV ? "Control+Alt+O" : "Control+Alt+Space",
  clock24: !/h1[12]/.test(
    (new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions() as { hourCycle?: string }).hourCycle ?? "",
  ),
  autoUpdate: true,
  vaultTimeout: 15 * 60,
  hello: { vault: false, notes: false, budget: false },
  notifStyle: "minimal",
  extensions: { disabled: [], devMode: false, devFolders: [] },
  launcher: { shortcut: "Control+Space", winKey: false, exclude: [], fullscreenOff: true, engine: "google" },
  passwordGen: { length: 20, digits: true, symbols: true },
  alarm: { sound: "gong", volume: 0.7, duration: 5 },
  privacyDots: false,
  presentationAuto: true,
  discordClientId: "",
  gaming: "off",
  ask: {
    provider: "anthropic",
    byProvider: {
      anthropic: { model: "claude-opus-5-5", effort: "low" },
      openai: { model: "", effort: "medium" },
      openrouter: { model: "", effort: "medium" },
      custom: { model: "", effort: "medium", baseUrl: "" },
    },
  },
  usageProviders: { claude: true, chatgpt: true },
  theme: "dark",
  tempUnit: /^en-US$|-(US|BS|BZ|KY|PW|LR)$/i.test(navigator.language) ? "f" : "c",
};

// Persistance : store JSON de l'hôte (`%APPDATA%/<identifier>/storage/__app__.json`,
// clé `notch`), comme le reste des réglages d'Island.
const STORE = "__app__";
const KEY = "notch";

function migrateTools(saved: unknown): ToolPref[] {
  const known = new Set(DEFAULT_TOOLS.map((t) => t.id));
  const list = Array.isArray(saved)
    ? (saved as ToolPref[])
        // Outils d'extensions (`ext:<id>:<outil>`) gardés : l'extension les réenregistre au
        // démarrage ; désinstallée, l'entrée reste inerte (aucun outil ne la porte).
        .filter((t) => t && (known.has(t.id) || t.id.startsWith("ext:")))
        .map((t) => ({ id: t.id, enabled: !!t.enabled }))
    : [];
  const seen = new Set(list.map((t) => t.id));
  for (const t of DEFAULT_TOOLS) if (!seen.has(t.id)) list.push({ ...t });
  if (!list.some((t) => t.enabled)) list[0].enabled = true; // au moins un outil actif
  return list;
}

function merge(saved: Partial<NotchSettings> | null): NotchSettings {
  const s = saved && typeof saved === "object" ? saved : {};
  return { ...DEFAULTS, ...s, hello: { ...DEFAULTS.hello, ...s.hello }, launcher: { ...DEFAULTS.launcher, ...s.launcher }, extensions: { ...DEFAULTS.extensions, ...s.extensions }, tools: migrateTools(s.tools) };
}

export const settings = reactive<NotchSettings>(merge(null));

let saveTimer: number | undefined;

/** Charge les réglages persistés puis sauvegarde (débouncé) à chaque changement. */
export async function loadSettings() {
  const saved = await invoke<Partial<NotchSettings> | null>("storage_get", { ext: STORE, key: KEY }).catch(() => null);
  Object.assign(settings, merge(saved));
  watch(
    settings,
    (s) => {
      window.clearTimeout(saveTimer);
      saveTimer = window.setTimeout(
        () => invoke("storage_set", { ext: STORE, key: KEY, value: JSON.parse(JSON.stringify(s)) }).catch(() => {}),
        300,
      );
    },
    { deep: true },
  );
}
