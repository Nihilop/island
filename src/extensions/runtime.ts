// Extensions v2, côté hôte (cf. docs/internal/extensions-v2-spec.md).
//
// Chaque extension tourne UNIQUEMENT dans des iframes sandboxées (`allow-scripts`,
// origine opaque) servies par le protocole `island-ext` : une iframe cachée pour sa
// logique (`main`) et une par vue d'outil ouverte. Elles n'ont ni Tauri, ni accès à cette
// page : elles parlent à l'hôte par un MessageChannel créé ICI, et c'est ce canal qui
// porte l'identité de l'extension (jamais une valeur qu'elle fournit). L'hôte vérifie les
// permissions puis appelle Rust.
//
// Protocole des messages (port) :
//   ext → hôte : { t: "call", n, method, args }
//   hôte → ext : { t: "ret", n, ok, value | error } · { t: "event", name, data }
// Poignée de main : l'hôte poste { t: "island:connect", id, locale, theme } + le port.

import { reactive, ref, watch } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { locale } from "../sdk/i18n";
import { notify } from "../tools/notifications/store";

export interface ToolDecl {
  id: string;
  title: string;
  icon: string | null;
  view: string;
  size: [number, number];
}

export interface WindowDecl {
  id: string;
  view: string;
  title: string | null;
  size: [number, number];
  minSize: [number, number] | null;
}

export interface SettingDecl {
  key: string;
  type: "text" | "secret" | "switch" | "select" | "number" | "list";
  label: string;
  help?: string;
  default?: unknown;
  options?: { value: string; label: string }[];
}

export interface Manifest {
  id: string;
  name: string;
  version: string;
  author: string | null;
  description: string | null;
  icon: string | null;
  main: string;
  permissions: string[];
  contributes: {
    tools: ToolDecl[];
    windows: WindowDecl[];
    live: { id: string; tool?: string }[];
    launcher: { id: string; prefix?: string; title?: string }[];
    commands: { id: string; title: string; shortcut?: string }[];
    settings: SettingDecl[];
  };
  locales: Record<string, string>;
}

/** Extensions v2 installées. */
export const installed = ref<Manifest[]>([]);

/** Origine du protocole (`http://island-ext.localhost` sous Windows). */
const ORIGIN = convertFileSrc("", "island-ext").replace(/\/+$/, "");

/** URL d'un fichier d'extension (chemin relatif au dossier de l'extension). */
export const extUrl = (id: string, path: string) => `${ORIGIN}/${id}/${path.split("/").map(encodeURIComponent).join("/")}`;

/** Thème effectif (classe posée sur <html> par notch/theme.ts). */
const theme = () => (document.documentElement.classList.contains("light") ? "light" : "dark");

/** Clé de fenêtre d'extension (`ext-<clé>`) : points de l'id → `_` (absents des id), puis
 *  `__` et l'id de la fenêtre. */
export const windowKey = (extId: string, winId: string) => `${extId.split(".").join("_")}__${winId}`;
export function parseWindowKey(key: string): { extId: string; winId: string } | null {
  const i = key.lastIndexOf("__");
  return i > 0 ? { extId: key.slice(0, i).split("_").join("."), winId: key.slice(i + 2) } : null;
}

/** Libellé de la fenêtre où tourne ce runtime (notch ou fenêtre d'extension). */
const here = (() => {
  try {
    return getCurrentWindow().label;
  } catch {
    return "overlay";
  }
})();

/** Requête HTTP d'une extension → forme attendue par Rust. */
const toReq = (req: any) => ({
  url: String(req?.url ?? ""),
  method: req?.method ? String(req.method) : null,
  headers: Object.entries(req?.headers ?? {}).map(([k, v]) => [String(k), String(v)]),
  body: req?.body == null ? null : typeof req.body === "string" ? req.body : JSON.stringify(req.body),
  timeout: typeof req?.timeout === "number" ? Math.round(req.timeout) : null,
});

const toRun = (spec: any) => ({ cwd: String(spec?.cwd ?? ""), command: String(spec?.command ?? ""), title: spec?.title == null ? null : String(spec.title), shell: spec?.shell == null ? null : String(spec.shell) });

// --- RPC ------------------------------------------------------------------------------

type Handler = (ext: Manifest, ...args: any[]) => unknown;

/** Méthodes exposées aux extensions. Chaque entrée reçoit le manifeste de l'appelant,
 *  déduit du canal. Les permissions sont vérifiées ici (cf. `need`). */
const methods: Record<string, Handler> = {
  whoami: (ext) => ext.id,
  log: (ext, msg: unknown) => invoke("ext2_log", { id: ext.id, msg: String(msg) }),

  // Sans permission : ses propres données, sa langue, ouvrir un lien web.
  "storage.get": (ext, key: unknown) => invoke("ext2_storage_get", { id: ext.id, key: String(key) }),
  "storage.set": (ext, key: unknown, value: unknown) => invoke("ext2_storage_set", { id: ext.id, key: String(key), value: value ?? null }),
  "i18n.messages": async (ext, lang: unknown) => {
    const l = String(lang ?? locale.value);
    const path = ext.locales[l] ?? ext.locales[l.split("-")[0]] ?? ext.locales.en ?? Object.values(ext.locales)[0];
    return path ? JSON.parse(await invoke<string>("ext2_read_text", { id: ext.id, path })) : {};
  },
  openUrl: (_ext, url: unknown) => invoke("open_url", { url: String(url) }),

  // Avec permission.
  "secrets.get": (ext, key: unknown) => (need(ext, "secrets"), invoke("ext2_secret_get", { id: ext.id, key: String(key) })),
  "secrets.set": (ext, key: unknown, value: unknown) => (need(ext, "secrets"), invoke("ext2_secret_set", { id: ext.id, key: String(key), value: String(value) })),
  "secrets.delete": (ext, key: unknown) => (need(ext, "secrets"), invoke("ext2_secret_delete", { id: ext.id, key: String(key) })),
  // Réseau : la liste de domaines est revérifiée côté Rust (manifeste installé).
  "http.request": (ext, req: any) => invoke("ext2_http", { id: ext.id, req: toReq(req) }),
  // Binaire : `[u32 LE taille méta][méta JSON][corps]` → { status, headers, body } ; le
  // corps (ArrayBuffer) est TRANSFÉRÉ à l'iframe (cf. serve).
  "http.bytes": async (ext, req: any) => {
    const buf = await invoke<ArrayBuffer>("ext2_http_bytes", { id: ext.id, req: toReq(req) });
    const len = new DataView(buf).getUint32(0, true);
    const meta = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 4, len)));
    return { status: meta.status, headers: meta.headers, body: buf.slice(4 + len) };
  },
  // Cookies : tenus par Island (permission `cookies`), jamais visibles ; l'extension peut
  // seulement tout oublier (déconnexion).
  "http.clearCookies": (ext) => invoke("ext2_cookies_clear", { id: ext.id }),
  // Fenêtres (permission `windows`) : page déclarée dans `contributes.windows`.
  "windows.open": (ext, winId: unknown, args: unknown, opts: any) => {
    need(ext, "windows");
    const decl = ext.contributes.windows?.find((w) => w.id === String(winId));
    if (!decl) throw new Error(`unknownWindow:${String(winId)}`);
    return invoke("window_open", {
      kind: "ext",
      key: windowKey(ext.id, decl.id),
      title: String(opts?.title ?? decl.title ?? ext.name).slice(0, 120),
      size: decl.size,
      minSize: decl.minSize ?? null,
      args: args === undefined ? null : JSON.parse(JSON.stringify(args)),
    }).then(() => undefined);
  },
  // Capture (permission `capture`, à haut risque) : Island sélectionne, capture et encode ;
  // l'extension pilote seulement (cf. docs/internal/capture-spec.md). Revérifiée côté Rust.
  "capture.displays": (ext) => (need(ext, "capture"), invoke("capture_displays", { id: ext.id })),
  "capture.select": (ext, mode: unknown) => (need(ext, "capture"), invoke("capture_select", { id: ext.id, mode: String(mode) })),
  "capture.screenshot": (ext, target: unknown, opts: any) =>
    (need(ext, "capture"), invoke("capture_screenshot", { id: ext.id, target, opts: { clipboard: opts?.clipboard !== false, save: opts?.save !== false } })),
  "capture.startRecording": (ext, target: unknown, opts: any) =>
    (need(ext, "capture"),
    invoke("capture_record_start", {
      id: ext.id,
      target,
      opts: {
        fps: opts?.fps === 60 ? 60 : 30,
        codec: opts?.codec === "hevc" ? "hevc" : "h264",
        quality: ["low", "medium", "high"].includes(opts?.quality) ? opts.quality : "medium",
        systemAudio: !!opts?.systemAudio,
        mic: !!opts?.mic,
      },
    })),
  "capture.stopRecording": (ext) => (need(ext, "capture"), invoke("capture_record_stop", { id: ext.id })),
  "capture.state": (ext) => (need(ext, "capture"), invoke("capture_record_state", { id: ext.id })),
  // Afficher une capture dans l'Explorateur : seulement un PNG / MP4 rangé par Island dans
  // un dossier « Island » (Images ou Vidéos, où qu'ils aient été déplacés).
  "capture.reveal": (ext, path: unknown) => {
    need(ext, "capture");
    const p = String(path ?? "");
    if (!/[\\/]Island[\\/][^\\/]+\.(png|mp4)$/i.test(p) || p.includes("..")) throw new Error("forbidden");
    return invoke("reveal_path", { path: p });
  },
  // Fond d'écran statique (permission `wallpaper`) : image en base64, validée et rangée par
  // Island (jamais de chemin). Revérifiée côté Rust.
  "wallpaper.displays": (ext) => (need(ext, "wallpaper"), invoke("ext2_wallpaper_displays", { id: ext.id })),
  "wallpaper.setImage": (ext, data: unknown, opts: any) => {
    need(ext, "wallpaper");
    const fit = ["fill", "fit", "stretch", "center", "tile", "span"].includes(opts?.fit) ? opts.fit : null;
    return invoke("ext2_wallpaper_set", { id: ext.id, data: String(data ?? ""), display: typeof opts?.display === "string" ? opts.display : null, fit });
  },
  // Activité (permission `activity`, sensible : jeux et apps utilisés). Revérifiée côté Rust.
  "activity.state": (ext) => (need(ext, "activity"), invoke("activity_ext_state", { id: ext.id })),
  "activity.stats": (ext, from: unknown, to: unknown) =>
    (need(ext, "activity"), invoke("activity_ext_stats", { id: ext.id, from: String(from), to: String(to) })),
  // Bus d'événements entre les pages d'UNE extension (notch, fenêtres) : relayé aux autres
  // fenêtres par un événement Tauri, jamais aux autres extensions.
  "events.emit": (ext, name: unknown, data: unknown) => {
    const msg = { name: String(name), data: data === undefined ? null : JSON.parse(JSON.stringify(data)) };
    emitTo(ext.id, "msg", msg);
    void emit("ext2://msg", { ext: ext.id, from: here, ...msg });
  },
  // Dossiers donnés par l'utilisateur (permission `files`) : l'extension ne voit que ceux
  // choisis dans la boîte de dialogue d'Island. Revérifié côté Rust (cf. extensions/system.rs).
  "files.pick": (ext, title: unknown) => (need(ext, "files"), invoke("ext2_files_pick", { id: ext.id, title: title == null ? null : String(title) })),
  "files.folders": (ext) => (need(ext, "files"), invoke("ext2_files_folders", { id: ext.id })),
  "files.forget": (ext, path: unknown) => (need(ext, "files"), invoke("ext2_files_forget", { id: ext.id, path: String(path) })),
  "files.list": (ext, path: unknown, opts: any) =>
    (need(ext, "files"),
    invoke("ext2_files_list", {
      id: ext.id,
      path: String(path),
      depth: typeof opts?.depth === "number" ? Math.round(opts.depth) : null,
      skip: Array.isArray(opts?.skip) ? opts.skip.map(String) : null,
    })),
  "files.readText": (ext, path: unknown) => (need(ext, "files"), invoke("ext2_files_read", { id: ext.id, path: String(path) })),
  "files.open": (ext, path: unknown) => (need(ext, "files"), invoke("ext2_files_open", { id: ext.id, path: String(path) })),
  "files.reveal": (ext, path: unknown) => (need(ext, "files"), invoke("ext2_files_reveal", { id: ext.id, path: String(path) })),
  // Commandes (permission `terminal`, confiance maximale) : Windows Terminal ou lancement
  // discret, dans un dossier donné.
  "terminal.run": (ext, spec: any) => (need(ext, "terminal"), invoke("ext2_terminal_run", { id: ext.id, spec: toRun(spec) })),
  "terminal.launch": (ext, spec: any) => (need(ext, "terminal"), invoke("ext2_terminal_launch", { id: ext.id, spec: toRun(spec) })),
  // Moteur Docker local (permission `docker`, à haut risque).
  "docker.request": (ext, req: any) =>
    (need(ext, "docker"),
    invoke("ext2_docker", {
      id: ext.id,
      req: { method: String(req?.method ?? "GET"), path: String(req?.path ?? ""), body: req?.body == null ? null : typeof req.body === "string" ? req.body : JSON.stringify(req.body) },
    })),
  // Ouvre le notch sur un outil de l'extension.
  "tools.open": (ext, toolId: unknown) => {
    const tool = ext.contributes.tools.find((x) => x.id === String(toolId));
    if (!tool) throw new Error(`unknownTool:${String(toolId)}`);
    return emit("notch://open", `ext:${ext.id}:${tool.id}`);
  },
  "clipboard.readText": (ext) => invoke("ext2_clipboard_read", { id: ext.id }),
  "clipboard.writeText": (ext, text: unknown) => invoke("ext2_clipboard_write", { id: ext.id, text: String(text) }),
  notify: (ext, spec: any) => {
    need(ext, "notifications");
    return notify({
      title: String(spec?.title ?? ext.name),
      body: spec?.body ? String(spec.body) : undefined,
      app: ext.name,
      icon: ext.icon ? extUrl(ext.id, ext.icon) : null,
      banner: spec?.banner !== false,
    });
  },
};

/** Vérifie une permission déclarée (et, à terme, consentie à l'installation). */
function need(ext: Manifest, perm: string) {
  if (!ext.permissions.includes(perm)) throw new Error(`permission:${perm}`);
}

/** Ajoute des méthodes RPC (modules d'emplacements : live, launcher, réglages…). */
export function addMethods(more: Record<string, Handler>) {
  Object.assign(methods, more);
}

/** Ports ouverts, par extension (logique + vues). */
const ports = new Map<string, Set<MessagePort>>();

/** Envoie un événement à toutes les pages ouvertes d'une extension. */
export function emitTo(extId: string, name: string, data: unknown) {
  for (const p of ports.get(extId) ?? []) p.postMessage({ t: "event", name, data });
}

function serve(ext: Manifest, port: MessagePort, onFirstCall: () => void) {
  if (!ports.has(ext.id)) ports.set(ext.id, new Set());
  ports.get(ext.id)!.add(port);
  port.onmessage = async ({ data }) => {
    onFirstCall();
    if (!data || data.t !== "call" || typeof data.n !== "number" || typeof data.method !== "string") return;
    const fn = methods[data.method];
    try {
      if (!fn) throw new Error(`unknownMethod:${data.method}`);
      const value: any = await fn(ext, ...(Array.isArray(data.args) ? data.args : []));
      const transfer = value?.body instanceof ArrayBuffer ? [value.body] : [];
      port.postMessage({ t: "ret", n: data.n, ok: true, value: value ?? null }, transfer);
    } catch (e) {
      port.postMessage({ t: "ret", n: data.n, ok: false, error: String((e as Error)?.message ?? e) });
    }
  };
}

/** Relie une iframe d'extension à l'hôte (à appeler à chaque chargement de l'iframe). */
export function connect(ext: Manifest, frame: HTMLIFrameElement): () => void {
  const { port1, port2 } = new MessageChannel();
  // Diagnostic : une extension qui ne dit rien dans les 5 s (script bloqué, erreur au
  // chargement…) est signalée dans la console de l'hôte.
  const silent = window.setTimeout(
    () => invoke("ext2_log", { id: ext.id, msg: `aucune réponse 5 s après le chargement de ${frame.src}` }).catch(() => {}),
    5000,
  );
  serve(ext, port1, () => window.clearTimeout(silent));
  frame.contentWindow?.postMessage({ t: "island:connect", id: ext.id, locale: locale.value, theme: theme() }, "*", [port2]);
  return () => {
    window.clearTimeout(silent);
    ports.get(ext.id)?.delete(port1);
    port1.close();
  };
}

let busReady = false;
/** Relais du bus d'événements venu des AUTRES fenêtres (cf. `events.emit`). */
export function initExtBus() {
  if (busReady) return;
  busReady = true;
  // Changement de jeu / d'app au premier plan : extensions qui ont `activity`.
  void listen("activity://changed", (e) => {
    for (const x of installed.value) if (x.permissions.includes("activity")) emitTo(x.id, "activity.changed", e.payload);
  });
  // Début / fin d'un enregistrement d'écran : prévient les extensions qui ont `capture`.
  void listen("capture://recording", (e) => {
    for (const x of installed.value) if (x.permissions.includes("capture")) emitTo(x.id, "capture.recording", e.payload);
  });
  void listen<{ ext: string; from: string; name: string; data: unknown }>("ext2://msg", (e) => {
    if (e.payload.from === here) return;
    emitTo(e.payload.ext, "msg", { name: e.payload.name, data: e.payload.data });
  });
}

export function broadcast(name: string, data: unknown) {
  for (const set of ports.values()) for (const p of set) p.postMessage({ t: "event", name, data });
}

/** Crée une iframe d'extension sandboxée. `allow` : fonctions du navigateur permises
 *  (fenêtres d'extension : plein écran et lecture automatique). */
export function sandboxFrame(src: string, allow = ""): HTMLIFrameElement {
  const f = document.createElement("iframe");
  // `allow-forms` : sans lui, le navigateur bloque un formulaire avant même l'événement
  // `submit` (le JS de la page ne le voit pas) ; la CSP (`form-action 'none'`) interdit
  // toujours tout envoi.
  f.setAttribute("sandbox", "allow-scripts allow-forms");
  f.setAttribute("referrerpolicy", "no-referrer");
  f.setAttribute("allow", allow);
  if (allow.includes("fullscreen")) f.setAttribute("allowfullscreen", "");
  f.src = src;
  return f;
}

// --- Cycle de vie ----------------------------------------------------------------------

export interface Installed {
  manifest: Manifest;
  /** Chargée depuis un dossier de développement. */
  dev: boolean;
}

/** Toutes les extensions connues (actives ou non). */
export const all = ref<Installed[]>([]);

type Setup = (ext: Manifest) => (() => void) | void;
let setups: Setup[] = [];
/** Extensions démarrées : iframe de logique + nettoyages des emplacements. */
const running = new Map<string, { frame: HTMLIFrameElement; disconnect?: () => void; cleanups: (() => void)[] }>();

function start(ext: Manifest) {
  if (running.has(ext.id)) return;
  const cleanups = setups.map((fn) => fn(ext)).filter((c): c is () => void => typeof c === "function");
  const frame = sandboxFrame(extUrl(ext.id, "__main.html"));
  frame.style.display = "none";
  const entry: { frame: HTMLIFrameElement; disconnect?: () => void; cleanups: (() => void)[] } = { frame, cleanups };
  frame.addEventListener("load", () => {
    entry.disconnect?.();
    entry.disconnect = connect(ext, frame);
  });
  running.set(ext.id, entry);
  document.body.appendChild(frame);
}

function stop(id: string) {
  const r = running.get(id);
  if (!r) return;
  r.disconnect?.();
  r.frame.remove();
  r.cleanups.forEach((c) => c());
  for (const p of ports.get(id) ?? []) p.close();
  ports.delete(id);
  running.delete(id);
}

/** Extensions désactivées par l'utilisateur (réglage). */
let isDisabled: (id: string) => boolean = () => false;

/** (Re)lit la liste et aligne ce qui tourne : arrête les disparues / désactivées,
 *  redémarre celles demandées (`restart`), démarre les nouvelles. */
export async function reload(restart: string[] = []) {
  const list = await invoke<Installed[]>("ext2_list").catch(() => []);
  all.value = list;
  const wanted = list.filter((x) => !isDisabled(x.manifest.id)).map((x) => x.manifest);
  for (const id of [...running.keys()]) if (restart.includes(id) || !wanted.some((m) => m.id === id)) stop(id);
  installed.value = wanted;
  for (const ext of wanted) start(ext);
}

export const runtime = reactive({ ready: false });

/** Démarre le runtime ; `setup` branche chaque extension sur les emplacements et renvoie
 *  son nettoyage. */
export async function initExtensions(opts: { disabled: (id: string) => boolean }, ...setup: Setup[]) {
  isDisabled = opts.disabled;
  setups = setup;
  await reload();
  initExtBus();
  runtime.ready = true;
  // Thème / langue suivis en direct par les extensions.
  const obs = new MutationObserver(() => broadcast("theme", theme()));
  obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  watch(locale, (l) => broadcast("locale", l));
}
