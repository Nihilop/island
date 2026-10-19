// Machine à états du notch : closed → peek (survol) → open.
// Le survol vient de Rust (`overlay://hover`) : une fenêtre click-through ne reçoit pas
// les events pointer. Les « holds » (seek, drag, saisie…) empêchent l'auto-fermeture et
// gardent la fenêtre cliquable ; l'épingle garde le notch ouvert mais laisse passer les
// clics hors zone.

import { computed, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HOVER_DELAY_MS, settings } from "./settings";
import { tools } from "../tools/registry";
import { SHAPES } from "./geometry";

export type NotchState = "closed" | "peek" | "open";

export const state = ref<NotchState>("closed");
export const hovering = ref(false);
export const pinned = ref(false);
/** Le raccourci d'ouverture a-t-il pu être enregistré (sinon : pris ailleurs) ? */
export const shortcutOk = ref(true);
/** Outil de la live activity en cours (ouverture « à froid » dessus). */
export const liveTool = ref<string | null>(null);
/** Outil affiché quand le notch est ouvert. */
export const activeTool = ref<string>("overview");

const CLOSE_GRACE = 180; // ms
const RESUME_WINDOW = 60_000; // revenir au même onglet si fermé depuis moins de 60 s
const MEMORY_LOW_AFTER = 20_000;

let openTimer: number | undefined;
let closeTimer: number | undefined;
let memoryTimer: number | undefined;
let lastClosedAt = 0;
let lastTool = "overview"; // dernier outil hors Réglages (bascule de l'engrenage)
let focusTaken = false;
const holds = ref(0);

/** Outils visibles dans le dock, dans l'ordre des réglages. */
export const dockTools = computed(() =>
  settings.tools
    .filter((p) => p.enabled)
    .map((p) => tools.value.find((t) => t.id === p.id))
    .filter((t): t is NonNullable<typeof t> => !!t && t.id !== "settings"),
);

function clearTimers() {
  window.clearTimeout(openTimer);
  window.clearTimeout(closeTimer);
}

function pickTool(explicit?: string | null): string {
  if (explicit) return explicit;
  const visible = dockTools.value;
  if (Date.now() - lastClosedAt < RESUME_WINDOW && (activeTool.value === "settings" || visible.some((t) => t.id === activeTool.value)))
    return activeTool.value;
  // Sinon, comme la Dynamic Island : on saute sur l'outil de la live activity s'il est visible.
  if (liveTool.value && visible.some((t) => t.id === liveTool.value)) return liveTool.value;
  return visible[0]?.id ?? "settings";
}

/** Outil hors dock (launcher…) : jamais retenu comme « dernier outil » ni chassé du notch. */
const offDock = (id: string) => tools.value.find((t) => t.id === id)?.dock === false;

export function selectTool(id: string) {
  if (id !== "settings" && !offDock(id)) lastTool = id;
  if (id !== activeTool.value) extendGrace();
  activeTool.value = id;
}

/** Garde générique : après un changement d'onglet / de taille, la forme bouge sous le
 *  curseur ; on laisse 1,5 s avant toute fermeture automatique. */
const SWITCH_GRACE = 1500;
let graceUntil = 0;
export function extendGrace(ms = SWITCH_GRACE) {
  graceUntil = Math.max(graceUntil, Date.now() + ms);
}

/** Section des Réglages à faire défiler en vue (liens profonds). */
export const settingsSection = ref<string | null>(null);

/** Ouvre les Réglages, éventuellement sur une section (« updates », « display »…). */
export function openSettings(section?: string) {
  settingsSection.value = section ?? null;
  if (state.value === "open") selectTool("settings");
  else open({ tool: "settings" });
}

/** L'engrenage bascule : outil → Réglages, Réglages → dernier outil. */
export function toggleSettings() {
  selectTool(activeTool.value === "settings" ? lastTool : "settings");
}

export function open(opts: { tool?: string | null; focus?: boolean } = {}) {
  if (settings.focusMode) return;
  clearTimers();
  if (state.value !== "open") selectTool(pickTool(opts.tool));
  else if (opts.tool) selectTool(opts.tool);
  state.value = "open";
  if (opts.focus) {
    focusTaken = true;
    invoke("notch_focus", { take: true }).catch(() => {});
  }
}

/** Donne le focus clavier au notch (saisie) ; il sera rendu à la fenêtre précédente à la
 *  fermeture. */
export function takeKeyboard() {
  if (focusTaken) return;
  focusTaken = true;
  invoke("notch_focus", { take: true }).catch(() => {});
}

export function close() {
  clearTimers();
  pinned.value = false;
  if (state.value === "open") lastClosedAt = Date.now();
  state.value = "closed";
  if (focusTaken) {
    focusTaken = false;
    invoke("notch_focus", { take: false }).catch(() => {});
  }
}

export function toggle() {
  if (state.value === "open") close();
  else open({ focus: true });
}

/** Fermeture « casual » (clic dehors, perte de focus) : ignorée si épinglé ou tenu. */
let blurTimer: number | undefined;

function dismiss() {
  if (state.value !== "open" || pinned.value || holds.value > 0) return;
  close();
}

function scheduleClose() {
  window.clearTimeout(closeTimer);
  const wait = Math.max(CLOSE_GRACE, graceUntil - Date.now());
  closeTimer = window.setTimeout(() => {
    if (hovering.value) return;
    // Un champ texte qui a le focus garde le notch ouvert.
    const el = document.activeElement;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
    dismiss();
  }, wait);
}

function onHover(inside: boolean) {
  hovering.value = inside;
  if (settings.focusMode) return;
  if (inside) {
    window.clearTimeout(closeTimer);
    if (state.value === "closed") {
      state.value = "peek";
      if (settings.openMode === "hover") openTimer = window.setTimeout(open, HOVER_DELAY_MS[settings.hoverDelay]);
    }
    return;
  }
  window.clearTimeout(openTimer);
  if (state.value === "peek") state.value = "closed";
  else if (state.value === "open") scheduleClose();
}

/** Tient le notch ouvert ; renvoie la fonction de relâche (idempotente). */
export function hold(): () => void {
  holds.value++;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    holds.value = Math.max(0, holds.value - 1);
    if (holds.value === 0 && !hovering.value && state.value === "open") scheduleClose();
  };
}

export function togglePin() {
  pinned.value = !pinned.value;
}

export function setFocusMode(on: boolean) {
  settings.focusMode = on;
}

export function initNotchState() {
  listen<boolean>("overlay://hover", (e) => onHover(e.payload));
  listen("overlay://dismiss", dismiss);
  listen("notch://toggle", toggle);
  listen<string | null>("notch://open", (e) => {
    if (settings.focusMode) setFocusMode(false);
    open({ tool: e.payload, focus: true });
  });
  listen("notch://focus-toggle", () => setFocusMode(!settings.focusMode));
  listen("notch://focus-exit", () => setFocusMode(false));
  getCurrentWindow()
    .onFocusChanged(({ payload: focused }) => {
      // Une perte de focus aussitôt reprise (passage au premier plan, la WebView reprend
      // le focus) ne ferme pas le notch : on attend 150 ms avant de conclure.
      window.clearTimeout(blurTimer);
      if (!focused) blurTimer = window.setTimeout(() => !document.hasFocus() && dismiss(), 150);
    })
    .catch(() => {});

  window.addEventListener("keydown", (e) => {
    if (state.value !== "open") return;
    if (e.key === "Escape") return close();
    const el = e.target as HTMLElement | null;
    if (e.ctrlKey || e.altKey || e.metaKey || el?.closest("input, textarea, [contenteditable]")) return;
    const n = Number(e.key);
    if (n >= 1 && n <= 9 && dockTools.value[n - 1]) selectTool(dockTools.value[n - 1].id);
  });

  watch(
    () => settings.shortcut,
    async (accel) => {
      shortcutOk.value = await invoke<boolean>("notch_set_shortcut", { accel: accel || null }).catch(() => false);
    },
    { immediate: true },
  );

  // Holds → Rust garde la fenêtre cliquable même hors zone.
  watch(
    () => holds.value > 0,
    (on) => invoke("notch_set_pinned", { pinned: on }).catch(() => {}),
  );

  // Mode focus : tout laisse passer, le notch se ferme et ignore le survol.
  watch(
    () => settings.focusMode,
    (on) => {
      invoke("notch_set_passthrough", { on }).catch(() => {});
      if (on) close();
    },
    { immediate: true },
  );

  watch(
    () => settings.visibility === "hideFullscreen" && settings.gaming !== "fullscreen",
    (on) => invoke("notch_set_hide_in_fullscreen", { on }).catch(() => {}),
    { immediate: true },
  );

  // Marge réservée : bande de la hauteur du notch fermé (0 = rendue au shell).
  watch(
    () => (settings.reserveSpace ? (settings.idleStyle === "line" ? SHAPES.line.h : SHAPES.closed.h) : 0),
    (height) => invoke("notch_set_reserve", { height }).catch(() => {}),
    { immediate: true },
  );

  // Outil actif désactivé → premier visible.
  watch(dockTools, (list) => {
    if (activeTool.value !== "settings" && !offDock(activeTool.value) && !list.some((t) => t.id === activeTool.value))
      selectTool(list[0]?.id ?? "settings");
  });

  // Mémoire WebView2 basse après 20 s fermé ; normale dès le survol (le délai de hover
  // masque la remontée).
  watch(
    state,
    (s) => {
      window.clearTimeout(memoryTimer);
      if (s === "closed") memoryTimer = window.setTimeout(() => invoke("notch_memory_low", { low: true }).catch(() => {}), MEMORY_LOW_AFTER);
      else invoke("notch_memory_low", { low: false }).catch(() => {});
    },
    { immediate: true },
  );
}
