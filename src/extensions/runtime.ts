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
import { locale } from "../sdk/i18n";
import { notify } from "../tools/notifications/store";

export interface ToolDecl {
  id: string;
  title: string;
  icon: string | null;
  view: string;
  size: [number, number];
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
  "http.request": (ext, req: any) =>
    invoke("ext2_http", {
      id: ext.id,
      req: {
        url: String(req?.url ?? ""),
        method: req?.method ? String(req.method) : null,
        headers: Object.entries(req?.headers ?? {}).map(([k, v]) => [String(k), String(v)]),
        body: req?.body == null ? null : typeof req.body === "string" ? req.body : JSON.stringify(req.body),
      },
    }),
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
      const value = await fn(ext, ...(Array.isArray(data.args) ? data.args : []));
      port.postMessage({ t: "ret", n: data.n, ok: true, value: value ?? null });
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

function broadcast(name: string, data: unknown) {
  for (const set of ports.values()) for (const p of set) p.postMessage({ t: "event", name, data });
}

/** Crée une iframe d'extension sandboxée. */
export function sandboxFrame(src: string): HTMLIFrameElement {
  const f = document.createElement("iframe");
  f.setAttribute("sandbox", "allow-scripts");
  f.setAttribute("referrerpolicy", "no-referrer");
  f.setAttribute("allow", "");
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
  runtime.ready = true;
  // Thème / langue suivis en direct par les extensions.
  const obs = new MutationObserver(() => broadcast("theme", theme()));
  obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  watch(locale, (l) => broadcast("locale", l));
}
