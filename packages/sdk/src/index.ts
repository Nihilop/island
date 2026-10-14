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
}

export interface HttpResponse {
  status: number;
  headers: [string, string][];
  body: string;
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
  /** Permission `network:<domaine>` (HTTPS, domaines déclarés seulement). */
  http: {
    request(req: HttpRequest): Promise<HttpResponse>;
    json<T = unknown>(url: string, init?: Omit<HttpRequest, "url">): Promise<T>;
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
      port.postMessage({ t: "call", n: id, method, args });
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

  const http: Island["http"] = {
    request: (req) => call<HttpResponse>("http.request", req),
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
