// Notifications : UNE seule API d'affichage, `notify()`, pour toutes les sources :
// - Windows : le centre de notifications (lu par le service Rust `notifs`, rien n'est
//   gardé par Island ; fermer ici ferme dans Windows) ;
// - Island : outils intégrés (mise à jour, …) et services Rust (`notify://post`) ;
// - extensions (plus tard, via le SDK).
// Mêmes règles pour tout le monde : notch FERMÉ → bannière (style « minimal » ou
// « complet », réglable) ; notch ouvert, mode présentation ou mode focus → historique
// seulement. L'historique (outil du dock) fusionne les sources, plus récentes d'abord.

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../../notch/settings";
import { state } from "../../notch/state";
import { presentation } from "../../notch/presentation";
import { t } from "../../notch/i18n";

/** Ce qu'une source passe à `notify()`. */
export interface NotifySpec {
  title: string;
  body?: string;
  /** Nom affiché de l'émetteur (défaut « Island »). */
  app?: string;
  /** Icône : URL ou data URL d'image. */
  icon?: string | null;
  /** false = historique seulement, pas de bannière. */
  banner?: boolean;
  /** Action principale (bouton sur la carte de l'historique). */
  action?: { label: string; run: () => void };
}

export interface Notif {
  /** `w<id Windows>` ou `i<n>` (Island / extensions). */
  id: string;
  source: "windows" | "island";
  app: string;
  icon: string | null;
  title: string;
  body: string;
  time: number;
  /** Windows : id natif + AppUserModelId (ouvrir l'app source). */
  native?: { id: number; aumid: string };
  action?: NotifySpec["action"];
}

/** Forme renvoyée par le service Rust. */
interface WinNotif {
  id: number;
  app: string;
  aumid: string;
  icon: string | null;
  title: string;
  body: string;
  time: number;
}

export type Access = "allowed" | "denied" | "unspecified" | "unsupported";

const fromWindows = (n: WinNotif): Notif => ({
  id: `w${n.id}`,
  source: "windows",
  app: n.app,
  icon: n.icon,
  title: n.title,
  body: n.body,
  time: n.time,
  native: { id: n.id, aumid: n.aumid },
});

export const notifs = reactive({
  access: "unspecified" as Access,
  windows: [] as Notif[],
  island: [] as Notif[],
  /** Bannière en cours (notch fermé). */
  banner: null as Notif | null,
});

/** Historique fusionné, plus récentes d'abord. */
export const history = computed(() => [...notifs.island, ...notifs.windows].sort((a, b) => b.time - a.time));

const BANNER_MS = { minimal: 4000, full: 6000 } as const;
const ISLAND_MAX = 50;
let bannerTimer: number | undefined;
let seq = 0;

const windowsEnabled = computed(() => settings.tools.some((t) => t.id === "notifications" && t.enabled));

// --- Affichage (commun à toutes les sources) -------------------------------------------

export function hideBanner() {
  window.clearTimeout(bannerTimer);
  notifs.banner = null;
}

/** (Re)lance le délai d'effacement de la bannière ; suspendu tant qu'on la survole. */
function armBanner() {
  window.clearTimeout(bannerTimer);
  bannerTimer = window.setTimeout(() => {
    if (state.value === "peek") armBanner();
    else hideBanner();
  }, BANNER_MS[settings.notifStyle]);
}

function showBanner(n: Notif) {
  if (state.value === "open" || presentation.value || settings.focusMode) return;
  notifs.banner = n;
  armBanner();
}

/** Affiche une notification (bannière si le notch est fermé) et l'ajoute à l'historique.
 *  Renvoie son id (pour `dismiss`). */
export function notify(spec: NotifySpec): string {
  const n: Notif = {
    id: `i${++seq}`,
    source: "island",
    app: spec.app ?? "Island",
    icon: spec.icon ?? null,
    title: spec.title,
    body: spec.body ?? "",
    time: Date.now(),
    action: spec.action,
  };
  notifs.island = [n, ...notifs.island].slice(0, ISLAND_MAX);
  if (spec.banner !== false) showBanner(n);
  return n.id;
}

/** Ferme une notification (Windows : aussi dans le centre de notifications). */
export async function dismiss(id: string) {
  if (notifs.banner?.id === id) hideBanner();
  const n = history.value.find((x) => x.id === id);
  notifs.island = notifs.island.filter((x) => x.id !== id);
  notifs.windows = notifs.windows.filter((x) => x.id !== id);
  if (n?.native) await invoke("notifs_dismiss", { id: n.native.id }).catch(() => loadWindows());
}

/** Tout effacer (Windows : comme « Tout effacer » du centre de notifications).
 *  Échec côté Windows → liste relue et erreur renvoyée (null = tout est parti). */
export async function clearAll(): Promise<string | null> {
  hideBanner();
  notifs.island = [];
  if (!notifs.windows.length) return null;
  notifs.windows = [];
  try {
    await invoke("notifs_clear");
    return null;
  } catch (e) {
    await loadWindows();
    return String(e);
  }
}

export const openApp = (n: Notif) => n.native?.aumid && invoke("notifs_open", { aumid: n.native.aumid }).catch(() => {});

// --- Source Windows --------------------------------------------------------------------

export async function refreshAccess() {
  notifs.access = await invoke<{ access: Access }>("notifs_status")
    .then((s) => s.access)
    .catch(() => "unsupported" as Access);
}

export async function requestAccess() {
  notifs.access = await invoke<{ access: Access }>("notifs_request_access")
    .then((s) => s.access)
    .catch(() => "unsupported" as Access);
  await loadWindows();
}

async function loadWindows() {
  notifs.windows = (await invoke<WinNotif[]>("notifs_list").catch(() => [])).map(fromWindows);
  // Fermée dans Windows : la bannière disparaît aussi.
  const b = notifs.banner;
  if (b?.source === "windows" && !notifs.windows.some((n) => n.id === b.id)) hideBanner();
}

// --- Essai depuis les Réglages ----------------------------------------------------------

/** La notification d'essai part dès que le notch se referme. */
export const testPending = ref(false);
export function testBanner() {
  if (testPending.value) return;
  testPending.value = true;
  const stop = watch(state, (s) => {
    if (s !== "closed") return;
    stop();
    testPending.value = false;
    notify({ title: t("notifs.test.title"), body: t("notifs.test.body") });
  });
}

export function initNotifs() {
  void refreshAccess();
  watch(
    windowsEnabled,
    (on) => {
      void invoke("notifs_set_enabled", { enabled: on }).catch(() => {});
      if (!on) notifs.windows = [];
    },
    { immediate: true },
  );
  listen("notifs://changed", () => void loadWindows());
  listen<WinNotif>("notifs://new", (e) => showBanner(fromWindows(e.payload)));
  // Services Rust : `app.emit("notify://post", { title, body, … })`.
  listen<NotifySpec>("notify://post", (e) => void notify(e.payload));
  // Ouvert : la bannière n'a plus lieu d'être.
  watch(state, (s) => s === "open" && hideBanner());
}
