// Launcher, côté front : la recherche se fait en Rust (`launcher_search`) ; ici, saisie
// (délai 60 ms + numéro de séquence pour jeter les réponses périmées), sélection,
// ouverture, icônes à la demande, et le préfixe `c:` (historique du presse-papiers).
//
// Préfixes : `a:` apps, `f:` fichiers, `d:` dossiers, `s:` réglages Windows, `c:` presse-papiers,
// `n:` notes, `t:` tâches, `j:` jeux, `b:` budget (cf. native.ts). Commandes rapides en tête
// (« todo … demain 18h », « timer 25 », « note … », « dépense 12,50 … »).

import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../notch/settings";
import { activeTool, close, hold, open, state } from "../notch/state";
import { locale } from "../sdk/i18n";
import { entries as clipEntries, copy as clipCopy } from "../tools/clipboard/store";
import { PREFIXES, loadGames, nativeResults, quickRow, refreshNative, type NativeItem } from "./native";

export type Kind = "app" | "setting" | "folder" | "file";

export interface Hit {
  id: string;
  kind: Kind;
  title: string;
  subtitle: string;
  path: string | null;
}

/** Une ligne affichée : résultat de l'index, réponse directe ou entrée de presse-papiers. */
export type Row =
  | { type: "hit"; key: string; hit: Hit }
  | { type: "calc"; key: string; value: string }
  | { type: "url"; key: string; url: string }
  | { type: "web"; key: string; query: string }
  | { type: "clip"; key: string; id: number; text: string }
  | { type: "ext"; key: string; source: string; sourceName: string; item: ExternalItem }
  | { type: "native"; key: string; item: NativeItem };

// --- Sources externes (extensions) ---------------------------------------------------
// Une source reçoit la requête (`query`) et répond plus tard par `setExternalResults` ;
// avec un préfixe (« gh ma-recherche »), elle est seule à répondre.

export interface ExternalItem {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string | null;
}

export interface LauncherSource {
  key: string;
  name: string;
  prefix?: string;
  query(q: string): void;
  run(itemId: string): void;
}

const sources = new Map<string, LauncherSource>();
const external = reactive(new Map<string, ExternalItem[]>());

export function addLauncherSource(s: LauncherSource) {
  sources.set(s.key, s);
}

export function removeLauncherSource(key: string) {
  sources.delete(key);
  external.delete(key);
}

export function setExternalResults(key: string, items: ExternalItem[]) {
  if (sources.has(key)) external.set(key, items.slice(0, 40));
}

/** Source désignée par un préfixe en tête de requête (« gh … »), et le reste. */
function prefixed(q: string): { source: LauncherSource; rest: string } | null {
  const low = q.toLowerCase();
  for (const s of sources.values()) {
    if (!s.prefix) continue;
    const pre = s.prefix.toLowerCase();
    // « gh … » (espace obligatoire), ou « p:… » (un préfixe en `:` se colle à la recherche).
    if (low === pre || low.startsWith(pre.endsWith(":") ? pre : `${pre} `)) return { source: s, rest: q.slice(s.prefix.length).trim() };
  }
  return null;
}

function queryExternal(q: string) {
  const p = prefixed(q);
  external.clear();
  if (p) return p.source.query(p.rest);
  if (q.length < 2) return;
  for (const s of sources.values()) s.query(q);
}

export const query = ref("");
export const launcher = reactive({
  hits: [] as Hit[],
  calc: null as string | null,
  url: null as string | null,
  indexing: false,
  selected: 0,
  /** Panneau d'actions (Ctrl+K) ouvert sur la ligne sélectionnée. */
  actions: false,
  /** Ouverture en cours (anti double Entrée). */
  busy: false,
  /** Retour bref après « copier » (clé de ligne). */
  copied: null as string | null,
});

/** Icônes déjà chargées (id → data URL ou null). */
export const icons = reactive(new Map<string, string | null>());

const ENGINES = {
  google: "https://www.google.com/search?q=",
  duckduckgo: "https://duckduckgo.com/?q=",
  bing: "https://www.bing.com/search?q=",
} as const;

const clipMode = computed(() => /^c:/i.test(query.value.trimStart()));

/** Préfixe d'une source native (`n:` notes, `t:` tâches, `j:` jeux, `b:` budget) et le reste. */
const nativePrefix = computed(() => {
  const m = query.value.trimStart().match(/^([ntjb]):\s*(.*)$/i);
  return m ? { source: PREFIXES[m[1].toLowerCase()], rest: m[2] } : null;
});

export const rows = computed<Row[]>(() => {
  const q = query.value.trim();
  if (clipMode.value) {
    const term = q.slice(2).trim().toLowerCase();
    return clipEntries.value
      .filter((e) => (e.kind === "text" || e.kind === "link") && e.preview && (!term || e.preview.toLowerCase().includes(term)))
      .slice(0, 40)
      .map((e) => ({ type: "clip" as const, key: `c${e.id}`, id: e.id, text: e.preview! }));
  }
  const out: Row[] = [];
  const np = nativePrefix.value;
  if (np) return nativeResults(np.rest, np.source).map((item) => ({ type: "native" as const, key: item.key, item }));
  const quick = quickRow(q);
  if (quick) out.push({ type: "native", key: quick.key, item: quick });
  if (launcher.calc) out.push({ type: "calc", key: "calc", value: launcher.calc });
  if (launcher.url) out.push({ type: "url", key: "url", url: launcher.url });
  const only = prefixed(q);
  if (!only) for (const h of launcher.hits) out.push({ type: "hit", key: h.id, hit: h });
  if (!only && !quick) for (const item of nativeResults(q, null)) out.push({ type: "native", key: item.key, item });
  for (const [key, items] of external) {
    const s = sources.get(key);
    if (!s || (only && only.source.key !== key)) continue;
    for (const item of items.slice(0, only ? 40 : 5)) out.push({ type: "ext", key: `${key}|${item.id}`, source: key, sourceName: s.name, item });
  }
  if (q && !only && !/^[afds]:\s*$/i.test(q)) out.push({ type: "web", key: "web", query: q.replace(/^[afds]:/i, "").trim() });
  return out;
});

export const current = computed(() => rows.value[launcher.selected] ?? null);

// --- Recherche ------------------------------------------------------------------------

let seq = 0;
let timer: number | undefined;

async function search() {
  const mine = ++seq;
  queryExternal(query.value.trim());
  const r = await invoke<{ seq: number; items: Hit[]; calc: string | null; url: string | null; indexing: boolean }>("launcher_search", {
    query: query.value,
    limit: 40,
    seq: mine,
  }).catch(() => null);
  if (!r || r.seq !== seq) return;
  launcher.hits = r.items;
  launcher.calc = r.calc;
  launcher.url = r.url;
  launcher.indexing = r.indexing;
  launcher.selected = 0;
  launcher.actions = false;
}

watch(query, (q) => {
  window.clearTimeout(timer);
  // Notes : recherche plein texte côté Rust (asynchrone), sur la requête sans préfixe.
  void refreshNative(nativePrefix.value ? nativePrefix.value.rest : q);
  if (clipMode.value || nativePrefix.value) {
    launcher.selected = 0;
    return;
  }
  timer = window.setTimeout(search, 60);
});

/** Charge les icônes manquantes des lignes visibles (une requête groupée). */
export async function loadIcons(ids: string[]) {
  const missing = ids.filter((id) => !icons.has(id));
  if (!missing.length) return;
  for (const id of missing) icons.set(id, null);
  const list = await invoke<(string | null)[]>("launcher_icons", { ids: missing }).catch(() => []);
  missing.forEach((id, i) => icons.set(id, list[i] ?? null));
}

// --- Actions --------------------------------------------------------------------------

export function move(delta: number) {
  const n = rows.value.length;
  if (!n) return;
  launcher.selected = (launcher.selected + delta + n) % n;
  launcher.actions = false;
}

function flashCopied(key: string) {
  launcher.copied = key;
  window.setTimeout(() => launcher.copied === key && (launcher.copied = null), 1200);
}

/** Exécute une ligne. `mode` : open | admin | reveal | copy | web. Ferme le notch si besoin. */
export async function run(row: Row | null, mode: "open" | "admin" | "reveal" | "copy" | "web" = "open") {
  if (launcher.busy) return;
  const q = query.value.trim();
  if (mode === "web" && q) row = { type: "web", key: "web", query: q };
  if (!row) return;
  launcher.busy = true;
  try {
    switch (row.type) {
      case "calc":
        await navigator.clipboard.writeText(row.value);
        flashCopied(row.key);
        return;
      case "clip":
        if (!(await clipCopy(row.id))) close();
        return;
      case "ext":
        sources.get(row.source)?.run(row.item.id);
        break;
      case "native":
        await row.item.run();
        break;
      case "url":
        await invoke("launcher_open_url", { url: row.url });
        break;
      case "web":
        await invoke("launcher_open_url", { url: ENGINES[settings.launcher.engine] + encodeURIComponent(row.query) });
        break;
      case "hit":
        if (mode === "copy") {
          await navigator.clipboard.writeText(row.hit.path ?? row.hit.title);
          flashCopied(row.key);
          return;
        }
        await invoke("launcher_open", { id: row.hit.id, action: mode === "admin" || mode === "reveal" ? mode : "open" });
        break;
    }
    close();
  } catch {
    // Élément disparu depuis l'indexation : on relance la recherche.
    void search();
  } finally {
    launcher.busy = false;
  }
}

// --- Clavier (champ réel ou frappe captée) -----------------------------------------------

/** Le panneau d'actions (Ctrl+K) a-t-il un sens pour la ligne choisie ? */
const hasActions = computed(() => current.value?.type === "hit");

export interface KeyCmd {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

/** Raccourcis du launcher, communs au champ réel et à la frappe captée. Renvoie true si la
 *  touche a été traitée. `copyOk` : Ctrl+C peut copier le chemin (pas de texte sélectionné). */
export function command(k: KeyCmd, copyOk = true): boolean {
  if (k.key === "ArrowDown" || k.key === "ArrowUp") {
    move(k.key === "ArrowDown" ? 1 : -1);
  } else if (k.key === "Enter") {
    if (k.ctrl && k.shift) void run(current.value, "admin");
    else if (k.alt) void run(current.value, "reveal");
    else if (k.ctrl) void run(null, "web");
    else void run(current.value);
  } else if (k.key === "k" && k.ctrl) {
    if (hasActions.value) launcher.actions = !launcher.actions;
  } else if (k.key === "c" && k.ctrl && copyOk) {
    void run(current.value, "copy");
  } else if (k.key === "Escape" && launcher.actions) {
    launcher.actions = false;
  } else {
    return false;
  }
  return true;
}

// --- Frappe captée (ouverture par la touche Windows) -------------------------------------
// Après un Win seul, le launcher s'ouvre SANS que la fenêtre d'Island devienne active
// (sinon Windows ouvre sa recherche, cf. `services/reserved/windows.rs`) : le hook capte
// la frappe et on édite la recherche ici. Un clic dans le launcher rend le clavier normal.

/** La frappe est captée par le hook (le champ réel n'a pas le focus). */
export const capturing = ref(false);
/** Ctrl+A en capture : la prochaine saisie remplace tout. */
let selectAll = false;
let captureHold: (() => void) | null = null;

const VK: Record<number, string> = {
  0x08: "Backspace",
  0x0d: "Enter",
  0x1b: "Escape",
  0x26: "ArrowUp",
  0x28: "ArrowDown",
  0x41: "a",
  0x43: "c",
  0x4b: "k",
  0x56: "v",
};

interface CapturedKey {
  vk: number;
  text: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

function insert(text: string) {
  query.value = selectAll ? text : query.value + text;
  selectAll = false;
}

async function onCaptured(k: CapturedKey) {
  if (!capturing.value) return;
  const key = VK[k.vk] ?? "";
  if (key === "Escape" && !launcher.actions) return close();
  if (command({ key, ctrl: k.ctrl, shift: k.shift, alt: k.alt })) return;
  if (key === "Backspace") {
    if (selectAll || k.ctrl) query.value = selectAll ? "" : query.value.replace(/\S*\s*$/, "");
    else query.value = query.value.slice(0, -1);
    selectAll = false;
  } else if (key === "a" && k.ctrl) {
    selectAll = true;
  } else if (key === "v" && k.ctrl) {
    const text = await invoke<string | null>("launcher_paste_text").catch(() => null);
    if (text) insert(text.replace(/\s+/g, " "));
  } else if (k.text) {
    insert(k.text);
  }
}

/** Clic dans le launcher : fin de la capture, le champ réel prend le clavier. */
export function stopCapture() {
  if (!capturing.value) return;
  capturing.value = false;
  selectAll = false;
  captureHold?.();
  captureHold = null;
  invoke("launcher_capture", { open: true, capture: false }).catch(() => {});
}

// --- Ouverture ------------------------------------------------------------------------

function reset() {
  query.value = "";
  void loadGames();
  launcher.selected = 0;
  launcher.actions = false;
  invoke("launcher_opened").catch(() => {});
  void search();
}

/** Raccourci global : ouvre le launcher avec le focus (ou le referme s'il est affiché). */
export function toggleLauncher() {
  if (state.value === "open" && activeTool.value === "launcher") return close();
  capturing.value = false;
  reset();
  open({ tool: "launcher", focus: true });
  invoke("launcher_capture", { open: true, capture: false }).catch(() => {});
}

/** Touche Windows : ouvre le launcher SANS activer la fenêtre, frappe captée par le hook
 *  (déjà en cours côté Rust). Tenu ouvert : aucun survol ne doit le refermer. */
function openCaptured() {
  capturing.value = true;
  selectAll = false;
  reset();
  open({ tool: "launcher" });
  captureHold?.();
  captureHold = hold();
}

export const shortcutOk = ref(true);

export function initLauncher() {
  listen("launcher://toggle", toggleLauncher);
  listen<boolean>("launcher://win", (e) => {
    if (e.payload) openCaptured();
    else if (state.value === "open" && activeTool.value === "launcher") close();
  });
  listen<CapturedKey>("launcher://key", (e) => void onCaptured(e.payload));
  // Launcher masqué (fermeture, autre outil) : le hook arrête de capter.
  watch(
    () => state.value === "open" && activeTool.value === "launcher",
    (visible) => {
      if (visible) return;
      capturing.value = false;
      captureHold?.();
      captureHold = null;
      invoke("launcher_capture", { open: false, capture: false }).catch(() => {});
    },
  );
  // Index reconstruit : la recherche en cours est relancée.
  listen("launcher://index", () => {
    if (state.value === "open" && activeTool.value === "launcher" && !clipMode.value) void search();
  });
  watch(
    () => JSON.stringify(settings.launcher),
    async () => {
      const { shortcut, winKey, exclude, fullscreenOff, excludeWhen } = settings.launcher;
      shortcutOk.value = await invoke<boolean>("launcher_config", { config: { shortcut, winKey, exclude, fullscreenOff, excludeWhen } }).catch(() => false);
    },
    { immediate: true },
  );
  watch(locale, (lang) => invoke("launcher_set_lang", { lang }).catch(() => {}), { immediate: true });
}
