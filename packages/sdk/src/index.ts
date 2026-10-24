// SDK des extensions Island v2. Une extension tourne dans une iframe isolée : elle n'a ni
// accès à l'app ni au réseau, et parle à Island uniquement par ce client (canal fourni
// par l'hôte). Les permissions déclarées dans le manifeste sont vérifiées par Island.
//
//   const island = await connect();
//   island.live.set("reviews", { text: "3", color: "orange" });

export type Theme = "dark" | "light";

export interface HttpRequest {
  url: string;
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD";
  headers?: Record<string, string>;
  body?: unknown;
  /** Délai max en secondes (1 à 120, 20 par défaut). */
  timeout?: number;
}

export interface HttpResponse {
  status: number;
  headers: [string, string][];
  body: string;
}

/** Réponse binaire (`http.bytes`) : le corps est un ArrayBuffer. */
export interface HttpBytesResponse {
  status: number;
  headers: [string, string][];
  body: ArrayBuffer;
}

/** Capture (permission `capture`) : coordonnées en pixels physiques, relatives au moniteur. */
export interface CaptureRect {
  x: number;
  y: number;
  w: number;
  h: number;
}
export type CaptureTarget =
  | { kind: "screen"; display: number }
  | { kind: "region"; display: number; rect: CaptureRect }
  | { kind: "window"; hwnd: number };
export interface CaptureDisplay {
  index: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  primary: boolean;
}
/** Écran (px physiques dans le bureau virtuel). */
export interface WallpaperDisplay {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /** Zone de travail [x, y, largeur, hauteur]. */
  work: [number, number, number, number];
  scale: number;
  primary: boolean;
}
export type WallpaperFit = "fill" | "fit" | "stretch" | "center" | "tile" | "span";
export interface RecordOptions {
  fps?: 30 | 60;
  codec?: "h264" | "hevc";
  quality?: "low" | "medium" | "high";
  systemAudio?: boolean;
  mic?: boolean;
}
export interface RecordingState {
  recording: boolean;
  startedAt: number | null;
  target: CaptureTarget | null;
  /** Fin : fichier produit (absent si l'enregistrement a échoué). */
  path?: string;
  durationMs?: number;
  /** Fin anormale : `targetClosed`, `encoderFailed`, `encoderTimeout`. */
  error?: string;
}

/** Activité (permission `activity`). */
export interface ActivityApp {
  pid: number;
  exe: string;
  path: string;
  title: string;
  name: string;
  kind: "game" | "app";
  source: string;
  appId?: string;
  art?: string;
  icon?: string;
}
export interface ActivityState {
  foreground: ActivityApp | null;
  game: { key: string; name: string; source: string; appId?: string; art?: string; pid: number; startedAt: number } | null;
}
export interface ActivityDay {
  date: string;
  games: { key: string; name: string; seconds: number }[];
  apps: { key: string; name: string; seconds: number }[];
}

/** Entrée d'un dossier (permission `files`). */
export interface FileEntry {
  path: string;
  name: string;
  dir: boolean;
  size: number;
  /** Dernière modification (ms depuis 1970). */
  modified: number;
}

/** Commande lancée dans un dossier donné (permission `terminal`). */
export interface RunSpec {
  cwd: string;
  /** `terminal.run` : vide = ouvrir seulement le shell. */
  command: string;
  /** Titre de l'onglet de Windows Terminal. */
  title?: string;
  /** `terminal.run` : shell de l'onglet. `default` (défaut) = profil par défaut de Windows
   *  Terminal (Git Bash, PowerShell… : les scripts y tournent aussi). */
  shell?: "default" | "cmd" | "powershell" | "pwsh" | "gitbash" | "wsl";
}

export interface DockerRequest {
  method?: "GET" | "POST" | "PUT" | "DELETE" | "HEAD";
  /** Chemin de l'API Docker Engine (`/containers/json?all=1`…). Flux sans fin refusés. */
  path: string;
  body?: unknown;
}

export interface LiveSpec {
  /** Chemin d'une icône de l'extension (SVG monochrome, teinté par `color`). */
  icon?: string;
  /** Texte court affiché à droite du notch fermé. */
  text?: string;
  color?: "label" | "green" | "orange" | "red" | "blue";
  /** « high » passe avant le minuteur et la musique. */
  priority?: "high" | "normal";
}

export interface LauncherItem {
  id: string;
  title: string;
  subtitle?: string;
  /** Chemin d'une icône de l'extension. */
  icon?: string;
}

export interface NotifySpec {
  title: string;
  body?: string;
  /** false = historique seulement, pas de bannière. */
  banner?: boolean;
}

export interface Island {
  /** Identifiant de l'extension (donné par Island). */
  readonly id: string;
  readonly locale: string;
  readonly theme: Theme;
  log(...args: unknown[]): Promise<void>;
  storage: {
    get<T = unknown>(key: string): Promise<T | null>;
    set(key: string, value: unknown): Promise<void>;
  };
  /** Permission `secrets`. */
  secrets: {
    get(key: string): Promise<string | null>;
    set(key: string, value: string): Promise<void>;
    delete(key: string): Promise<void>;
  };
  settings: {
    get<T = unknown>(key: string): Promise<T>;
    onChange(cb: (key: string) => void): void;
  };
  /** Permission `network:<domaine>` (HTTPS, domaines déclarés seulement). Avec la
   *  permission `cookies`, Island garde les cookies de session des réponses et les renvoie
   *  aux requêtes suivantes (jamais visibles ici). */
  http: {
    request(req: HttpRequest): Promise<HttpResponse>;
    json<T = unknown>(url: string, init?: Omit<HttpRequest, "url">): Promise<T>;
    /** Corps binaire (vidéo, images, fichiers), transféré sans copie. */
    bytes(req: HttpRequest): Promise<HttpBytesResponse>;
    /** Oublie tous les cookies (permission `cookies`). */
    clearCookies(): Promise<void>;
  };
  /** Permission `windows` : pages de `contributes.windows` ouvertes dans leur fenêtre. */
  windows: {
    /** Ouvre (ou ramène au premier plan) la fenêtre `id` en lui passant `args`. */
    open(id: string, args?: unknown, opts?: { title?: string }): Promise<void>;
    /** Dans une fenêtre : arguments d'ouverture (null ailleurs). */
    args<T = unknown>(): Promise<T | null>;
    /** Dans une fenêtre : rouverte avec de nouveaux arguments. */
    onArgs<T = unknown>(cb: (args: T) => void): void;
    setTitle(title: string): Promise<void>;
    close(): Promise<void>;
  };
  /** Permission `capture` (à haut risque) : Island affiche la sélection, capture et encode. */
  capture: {
    displays(): Promise<CaptureDisplay[]>;
    /** Sélection dessinée par Island (zone, fenêtre ou écran) ; null si annulée. */
    select(mode: "region" | "window" | "screen"): Promise<CaptureTarget | null>;
    /** PNG dans Images\Island (et/ou presse-papiers). */
    screenshot(target: CaptureTarget, opts?: { clipboard?: boolean; save?: boolean }): Promise<{ path: string | null; width: number; height: number }>;
    /** MP4 dans Vidéos\Island (encodeur matériel de Windows). Un seul enregistrement à la fois. */
    startRecording(target: CaptureTarget, opts?: RecordOptions): Promise<void>;
    stopRecording(): Promise<{ path: string; durationMs: number }>;
    state(): Promise<RecordingState>;
    /** Début / fin d'un enregistrement. */
    onRecording(cb: (state: RecordingState) => void): void;
    /** Affiche une capture d'Island dans l'Explorateur. */
    reveal(path: string): Promise<void>;
  };
  /** Permission `wallpaper` : fond d'écran STATIQUE. L'image est fournie en données (Island
   *  la valide, la range et la pose) ; aucun chemin de fichier n'est accepté. */
  wallpaper: {
    displays(): Promise<WallpaperDisplay[]>;
    /** `display` absent = tous les écrans ; `fit` s'applique à tous (réglage Windows global). */
    setImage(image: Blob | ArrayBuffer | Uint8Array, opts?: { display?: string; fit?: WallpaperFit }): Promise<void>;
  };
  /** Permission `activity` (sensible) : jeu en cours, app au premier plan, temps passé. */
  activity: {
    state(): Promise<ActivityState>;
    /** Temps par jour (AAAA-MM-JJ, bornes incluses). */
    stats(from: string, to: string): Promise<ActivityDay[]>;
    onChange(cb: (state: ActivityState) => void): void;
  };
  /** Permission `files` : seulement les dossiers que l'utilisateur a donnés (`pick`). */
  files: {
    /** Boîte de dialogue d'Island : le dossier choisi devient accessible ; null si annulé. */
    pick(title?: string): Promise<string | null>;
    /** Dossiers accessibles. */
    folders(): Promise<string[]>;
    forget(path: string): Promise<void>;
    /** Contenu sur `depth` niveaux (1 à 4, 5000 entrées) ; pas de descente dans `skip`. */
    list(path: string, opts?: { depth?: number; skip?: string[] }): Promise<FileEntry[]>;
    /** Fichier texte (2 Mo au plus). */
    readText(path: string): Promise<string>;
    /** Ouvre avec l'app associée (.uproject, .sln…). Un exécutable demande `terminal`. */
    open(path: string): Promise<void>;
    reveal(path: string): Promise<void>;
  };
  /** Permission `terminal` (confiance maximale) : `cwd` doit être dans un dossier donné. */
  terminal: {
    /** Nouvel onglet de Windows Terminal, laissé ouvert. */
    run(spec: RunSpec): Promise<void>;
    /** Sans console (ex. `code .`). */
    launch(spec: RunSpec): Promise<void>;
  };
  /** Permission `docker` : moteur Docker local. Erreur `dockerUnavailable` s'il est arrêté. */
  docker: {
    request(req: DockerRequest): Promise<{ status: number; body: string }>;
    /** Requête JSON ; lève `HttpError` si le statut est ≥ 400. */
    json<T = unknown>(path: string, init?: Omit<DockerRequest, "path">): Promise<T>;
  };
  tools: {
    /** Ouvre le notch sur un outil de l'extension. */
    open(id: string): Promise<void>;
  };
  /** Événements entre les pages de l'extension (notch, fenêtres, logique). */
  events: {
    emit(name: string, data?: unknown): Promise<void>;
    on<T = unknown>(name: string, cb: (data: T) => void): void;
  };
  /** Permission `notifications`. */
  notify(spec: NotifySpec): Promise<string>;
  openUrl(url: string): Promise<void>;
  /** Permission `clipboard`. */
  clipboard: {
    readText(): Promise<string | null>;
    writeText(text: string): Promise<void>;
  };
  /** Textes traduits (fichiers `locales` du manifeste), `{param}` remplacés. */
  t(key: string, params?: Record<string, string | number>): string;
  live: { set(id: string, spec: LiveSpec | null): Promise<void> };
  launcher: {
    /** Fournit les résultats d'un fournisseur déclaré (`contributes.launcher`). */
    provide(id: string, search: (query: string) => LauncherItem[] | Promise<LauncherItem[]>): void;
    onRun(id: string, run: (itemId: string) => void): void;
  };
  commands: { on(id: string, run: () => void): void };
  onTheme(cb: (theme: Theme) => void): void;
}

export class HttpError extends Error {
  constructor(
    public status: number,
    public body: string,
  ) {
    super(`HTTP ${status}`);
  }
}

/** Se connecte à Island (attend la poignée de main de l'hôte). */
export function connect(): Promise<Island> {
  return new Promise((resolve) => {
    let done = false;
    addEventListener("message", (e: MessageEvent) => {
      if (done || e.data?.t !== "island:connect" || !e.ports?.[0]) return;
      done = true;
      resolve(create(e.ports[0], e.data));
    });
  });
}

async function create(port: MessagePort, info: { id: string; locale: string; theme: Theme }): Promise<Island> {
  let n = 0;
  const pending = new Map<number, { ok: (v: any) => void; ko: (e: Error) => void }>();
  const listeners = new Map<string, ((data: any) => void)[]>();
  port.onmessage = ({ data }) => {
    if (data?.t === "ret") {
      const p = pending.get(data.n);
      pending.delete(data.n);
      if (p) data.ok ? p.ok(data.value) : p.ko(new Error(data.error));
    } else if (data?.t === "event") {
      for (const cb of listeners.get(data.name) ?? []) cb(data.data);
    }
  };
  const call = <T = unknown>(method: string, ...args: unknown[]) =>
    new Promise<T>((ok, ko) => {
      const id = ++n;
      pending.set(id, { ok, ko });
      const msg = { t: "call", n: id, method, args };
      try {
        port.postMessage(msg);
      } catch {
        // Argument non transmissible tel quel (proxy réactif de Vue…) : copie JSON.
        try {
          port.postMessage(JSON.parse(JSON.stringify(msg)));
        } catch (e) {
          pending.delete(id);
          ko(e instanceof Error ? e : new Error(String(e)));
        }
      }
    });
  const on = (name: string, cb: (data: any) => void) => listeners.set(name, [...(listeners.get(name) ?? []), cb]);

  // Thème : classe `light` sur <html> (cf. ui.css), suivi en direct.
  let theme = info.theme;
  const applyTheme = (t: Theme) => document.documentElement.classList.toggle("light", t === "light");
  applyTheme(theme);
  on("theme", (t: Theme) => {
    theme = t;
    applyTheme(t);
  });

  // Traductions.
  let messages = await call<Record<string, string>>("i18n.messages", info.locale).catch(() => ({}) as Record<string, string>);
  on("locale", async (l: string) => {
    messages = await call<Record<string, string>>("i18n.messages", l).catch(() => messages);
  });
  const t = (key: string, params?: Record<string, string | number>) =>
    (messages[key] ?? key).replace(/\{(\w+)\}/g, (_, k: string) => String(params?.[k] ?? `{${k}}`));

  // Launcher : l'hôte envoie la requête, on répond avec le même numéro.
  const providers = new Map<string, (q: string) => LauncherItem[] | Promise<LauncherItem[]>>();
  const runners = new Map<string, (id: string) => void>();
  on("launcher.query", async ({ provider, q, seq }) => {
    const fn = providers.get(provider);
    if (!fn) return;
    const items = await Promise.resolve(fn(q)).catch(() => []);
    await call("launcher.results", provider, seq, items);
  });
  on("launcher.run", ({ provider, id }) => runners.get(provider)?.(id));

  const commands = new Map<string, () => void>();
  on("command", ({ id }) => commands.get(id)?.());

  const bus = new Map<string, ((data: any) => void)[]>();
  on("msg", ({ name, data }) => bus.get(name)?.forEach((cb) => cb(data)));

  const http: Island["http"] = {
    request: (req) => call<HttpResponse>("http.request", req),
    bytes: (req) => call<HttpBytesResponse>("http.bytes", req),
    clearCookies: () => call("http.clearCookies"),
    async json<T>(url: string, init?: Omit<HttpRequest, "url">) {
      const r = await call<HttpResponse>("http.request", { ...init, url, headers: { Accept: "application/json", ...init?.headers } });
      if (r.status >= 400) throw new HttpError(r.status, r.body);
      return JSON.parse(r.body) as T;
    },
  };

  const island: Island = {
    id: info.id,
    locale: info.locale,
    get theme() {
      return theme;
    },
    log: (...args) => call("log", args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" ")),
    storage: { get: (k) => call("storage.get", k), set: (k, v) => call("storage.set", k, v) },
    secrets: { get: (k) => call("secrets.get", k), set: (k, v) => call("secrets.set", k, v), delete: (k) => call("secrets.delete", k) },
    settings: { get: (k) => call("settings.get", k), onChange: (cb) => on("settings.changed", ({ key }) => cb(key)) },
    http,
    windows: {
      open: (id, args, opts) => call("windows.open", id, args, opts),
      args: <T>() => call<T | null>("windows.args").catch(() => null),
      onArgs: (cb) => on("window.args", cb),
      setTitle: (title) => call("windows.setTitle", title),
      close: () => call("windows.close"),
    },
    wallpaper: {
      displays: () => call("wallpaper.displays"),
      setImage: async (image, opts) => call("wallpaper.setImage", await toBase64(image), opts ?? {}),
    },
    activity: {
      state: () => call("activity.state"),
      stats: (from, to) => call<{ days: ActivityDay[] }>("activity.stats", from, to).then((r) => r.days),
      onChange: (cb) => on("activity.changed", cb),
    },
    capture: {
      displays: () => call("capture.displays"),
      select: (mode) => call("capture.select", mode),
      screenshot: (target, opts) => call("capture.screenshot", target, opts),
      startRecording: (target, opts) => call("capture.startRecording", target, opts),
      stopRecording: () => call("capture.stopRecording"),
      state: () => call("capture.state"),
      onRecording: (cb) => on("capture.recording", cb),
      reveal: (path) => call("capture.reveal", path),
    },
    files: {
      pick: (title) => call("files.pick", title ?? null),
      folders: () => call("files.folders"),
      forget: (path) => call("files.forget", path),
      list: (path, opts) => call("files.list", path, opts ?? {}),
      readText: (path) => call("files.readText", path),
      open: (path) => call("files.open", path),
      reveal: (path) => call("files.reveal", path),
    },
    terminal: { run: (spec) => call("terminal.run", spec), launch: (spec) => call("terminal.launch", spec) },
    docker: {
      request: (req) => call("docker.request", req),
      async json<T>(path: string, init?: Omit<DockerRequest, "path">) {
        const r = await call<{ status: number; body: string }>("docker.request", { ...init, path });
        if (r.status >= 400) throw new HttpError(r.status, r.body);
        return (r.body ? JSON.parse(r.body) : null) as T;
      },
    },
    tools: { open: (id) => call("tools.open", id) },
    events: {
      // Copie JSON : un objet réactif (proxy Vue…) ne passerait pas `postMessage`.
      emit: (name, data) => call("events.emit", name, data === undefined ? null : JSON.parse(JSON.stringify(data))),
      on: (name, cb) => void bus.set(name, [...(bus.get(name) ?? []), cb]),
    },
    notify: (spec) => call("notify", spec),
    openUrl: (url) => call("openUrl", url),
    clipboard: { readText: () => call("clipboard.readText"), writeText: (text) => call("clipboard.writeText", text) },
    t,
    live: { set: (id, spec) => call("live.set", id, spec) },
    launcher: {
      provide: (id, fn) => void providers.set(id, fn),
      onRun: (id, fn) => void runners.set(id, fn),
    },
    commands: { on: (id, fn) => void commands.set(id, fn) },
    onTheme: (cb) => on("theme", cb),
  };
  return island;
}

/** Image → base64 (sans préfixe data:), pour la passer au canal hôte. */
function toBase64(image: Blob | ArrayBuffer | Uint8Array): Promise<string> {
  const blob = image instanceof Blob ? image : new Blob([image instanceof Uint8Array ? image : new Uint8Array(image)]);
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).replace(/^data:[^,]*,/, ""));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}
