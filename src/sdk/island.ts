// @island/sdk — API runtime fournie par l'hôte (livré AVEC Island).
// Les extensions importent depuis "@island/sdk" ; au runtime, tout passe par le
// pont global `window.__ISLAND__` exposé par l'hôte (marche entre instances Vue).
import { reactive, type Component } from "vue";

export interface MediaState {
  title: string;
  artist: string;
  art: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  /** App source du média (ex. "spotify", "msedge", "chrome"). "" si rien. Permet à une
   *  extension dédiée de ne réagir qu'à son app. */
  source: string;
}

export interface ModalRequest {
  title?: string;
  subtitle?: string;
  component?: Component;
  ui?: any[];
}

/** États du centre de l'île en idle — rendus par l'HÔTE (cohérence). Enrichi au fil des updates. */
export type IdleState = "idle" | "playing" | "busy" | "recording";

/** Raccourci conditionnel sur une extrémité de l'île en idle (icône OU texte court). */
export interface IdleAction {
  icon?: string; // SVG (string)
  text?: string; // alternative à l'icône : ex. compteur "00:12"
  color?: string;
  priority?: number;
  onActivate?: () => void;
}

/** Un écran disponible pour la capture. */
export interface Display {
  index: number; // 1-based
  name: string;
  width: number;
  height: number;
  primary: boolean;
}

/** Rectangle de capture (pixels physiques, relatif au moniteur). */
export interface Region {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Encodeur fourni PAR L'EXTENSION (l'hôte reste agnostique). L'extension embarque
 * son binaire et décide du codec → requiert la permission `native-encoder`.
 * L'hôte appelle : `<bin> -y -f rawvideo -pix_fmt bgra -s WxH -framerate fps -i - <args> <sortie>`.
 */
export interface EncoderSpec {
  extId: string; // id de l'extension (pour résoudre son dossier côté hôte)
  bin: string; // binaire RELATIF au dossier de l'extension (ex "binaries/ffmpeg.exe")
  args: string[]; // args d'encodage VIDÉO (codec/filtres/couleurs), entre entrée et sortie
  audioArgs?: string[]; // codec AUDIO pour le mux si `audio` actif (ex ["-c:a","aac","-b:a","160k"])
}

/** Options de capture. `excludeIsland` (défaut true) cache l'overlay de la capture. */
export interface CaptureOptions {
  display?: number; // 1-based ; absent = écran principal
  excludeIsland?: boolean;
  dir?: string; // dossier de destination ; absent = défaut (Images/Vidéos → Island)
  region?: Region; // zone à capturer ; absent = plein écran
  fps?: number; // enregistrement : images/s (défaut 30)
  audio?: boolean; // enregistrement : capturer le son système (WASAPI loopback) + muxer
  encoder?: EncoderSpec; // enregistrement : encodeur fourni par l'extension (requis)
}

/** Stats système (extension Monitoring). Octets pour la mémoire. */
export interface SysStats {
  cpu: number; // %
  cores: number[]; // % par cœur
  memUsed: number;
  memTotal: number;
}

/** Une fenêtre du bureau (conscience des fenêtres). */
export interface WindowInfo {
  id: number; // handle natif opaque
  title: string;
  app: string; // nom d'exe sans extension (ex. "chrome", "spotify")
}

/** Une notification à pousser dans le centre / la bannière de l'île. */
export interface NotificationSpec {
  title: string;
  body?: string;
  icon?: string; // SVG (string)
  color?: string; // accent
  source?: string; // nom affiché de l'émetteur
  timeout?: number; // ms d'affichage de la bannière ; 0 = historique seul
  actions?: { id?: string; label: string; onClick?: () => void }[];
  onClick?: () => void;
}

/** Store clé→valeur persistant, propre à une extension (settings, données…). */
export interface ExtStorage {
  get<T = unknown>(key: string, fallback?: T): Promise<T | undefined>;
  set(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
  keys(): Promise<string[]>;
}

/**
 * Coffre de secrets CHIFFRÉS, propre à une extension (tokens d'API, mots de passe…).
 * Contrairement à `storage` (JSON en clair), les valeurs vivent dans le coffre du
 * système (Windows Credential Manager / Keychain). À réserver aux données sensibles.
 */
export interface ExtSecrets {
  get(key: string): Promise<string | null>;
  set(key: string, value: string): Promise<void>;
  delete(key: string): Promise<void>;
}

/** Un résultat fourni au launcher quand l'utilisateur tape (palette extensible). */
export interface LauncherResult {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string; // SVG (string) ; une icône par défaut est utilisée si absent
  onActivate: () => void;
}

export interface IslandApi {
  media: {
    state: MediaState;
    toggle(): void;
    next(): void;
    prev(): void;
    seek(ms: number): void;
    setVolume(v: number): void;
  };
  /** Contribue à l'île en idle (garde-fou côté hôte). */
  idle: {
    /** Centre : un état (enum géré par l'hôte). `null` = retire la contribution. */
    state(state: IdleState | null, opts?: { priority?: number }): void;
    /** Extrémité : un raccourci d'action (icône ou texte). `null` = retire. */
    action(slot: "left" | "right", action: IdleAction | null): void;
    /** Clic sur TOUTE l'île en idle → ouvre ton UI au lieu du launcher. `null` = retire. */
    tap(handler: (() => void) | null): void;
  };
  /** Monte une vue de l'extension DANS l'île (slot view). */
  view: {
    /**
     * Ouvre une view. `persistent: true` garde la view ouverte même au clic hors
     * de l'île / perte de focus (ex. Monitoring) — elle ne se referme alors que
     * via le « Retour » de l'île ou `view.close()`.
     */
    open(component: Component, size?: { width?: number; height?: number; radius?: number; persistent?: boolean; safeZone?: "relative" | "absolute" | "hidden"; /** @deprecated → safeZone */ safeArea?: boolean }): void;
    close(): void;
    /** Redimensionne la view ACTIVE (l'île morphe en douceur) sans la remonter. */
    resize(size: { width?: number; height?: number; radius?: number }): void;
  };
  /** Goutte : sous-slot d'une view (contenu fourni par l'extension). */
  drop: {
    open(component: Component): void;
    close(): void;
  };
  /**
   * Fenêtre flottante draggable (3ᵉ conteneur, à côté de `view` et `modal`).
   * Panneau libre, persistant, déplaçable (barre minimale + croix), qui héberge un
   * composant — idéal pour un lecteur vidéo / mini-outil. `open` renvoie l'id.
   */
  window: {
    open(component: Component, opts?: { id?: string; title?: string; width?: number; height?: number; x?: number; y?: number; resizable?: boolean }): string;
    close(id?: string): void;
    focus(id: string): void;
  };
  /** Entrée dans le launcher (visible tant que l'extension est active). */
  launcher: {
    register(entry: { label: string; icon: string; onActivate: () => void }): void;
    remove(): void;
    /**
     * Alimente la RECHERCHE du launcher : `onQuery(q)` renvoie des résultats (sync ou
     * async) affichés quand l'utilisateur tape. Transforme le launcher en palette de
     * commandes extensible. Une extension = un provider.
     */
    provider(p: { onQuery: (query: string) => LauncherResult[] | Promise<LauncherResult[]> }): void;
    removeProvider(): void;
  };
  /** Capture d'écran (Windows Graphics Capture, anti-cheat safe). */
  capture: {
    /** Écrans disponibles (pour un picker « Écran 1 / Écran 2 »). */
    listDisplays(): Promise<Display[]>;
    /** Screenshot PNG → renvoie le chemin du fichier. */
    screenshot(opts?: CaptureOptions): Promise<string>;
    /** Démarre l'enregistrement → renvoie le chemin du fichier. `opts.encoder` requis. */
    startRecording(opts: CaptureOptions & { encoder: EncoderSpec }): Promise<string>;
    /** Arrête l'enregistrement, finalise le fichier, renvoie son chemin. */
    stopRecording(): Promise<string>;
    /** Un enregistrement est-il en cours ? */
    isRecording(): Promise<boolean>;
    /**
     * Télécharge un binaire (ex. l'encodeur) DANS le dossier de l'extension.
     * `zipEntry` = suffixe d'entrée si l'URL est un ZIP (ex "bin/ffmpeg.exe").
     * Émet `encoder://download { extId, percent, done }`. Requiert `native-encoder`.
     */
    fetchBinary(spec: { extId: string; url: string; dest: string; zipEntry?: string }): Promise<void>;
    /** Ouvre un sélecteur de DOSSIER → chemin choisi, ou null si annulé. */
    pickFolder(): Promise<string | null>;
    /** Sélection d'une zone à l'écran → rect physique, ou null si annulé (Échap). */
    selectRegion(): Promise<Region | null>;
    /** Affiche un contour persistant sur une zone (ex. pendant un record). `null` = retire. */
    showRegionOutline(region: Region | null): void;
  };
  /** Raccourcis clavier GLOBAUX. Rien n'est enregistré tant qu'on n'appelle pas register. */
  shortcuts: {
    /** Renvoie false si la combinaison est déjà prise (conflit) ou refusée par l'OS. */
    register(accelerator: string, handler: () => void): Promise<boolean>;
    unregister(accelerator: string): Promise<void>;
  };
  /** Stats & capteurs système. Tout requiert la permission `system`. */
  system: {
    stats(): Promise<SysStats>;
    /** Batterie, ou `null` (poste fixe / inconnu). */
    battery(): Promise<{ percent: number; charging: boolean } | null>;
    /** Connecté à un réseau ? */
    online(): Promise<boolean>;
    /** Volume MAÎTRE de la sortie (≠ `media.setVolume` qui pilote l'app média), ou `null`. */
    volume(): Promise<{ level: number; muted: boolean } | null>;
    setVolume(level: number): Promise<void>;
    setMuted(muted: boolean): Promise<void>;
    /** ms depuis la dernière entrée clavier/souris. */
    idleMs(): Promise<number>;
    /**
     * Helper : `onIdle` quand l'inactivité dépasse `ms`, `onActive` au retour.
     * Sonde `idleMs()` périodiquement. Renvoie une fonction d'arrêt.
     */
    onUserIdle(ms: number, onIdle: () => void, onActive?: () => void): () => void;
  };
  /** Conscience des fenêtres du bureau. Requiert la permission `windows`. */
  windows: {
    /** Fenêtre au premier plan, ou `null`. */
    foreground(): Promise<WindowInfo | null>;
    /** Fenêtres top-level visibles (titrées). */
    list(): Promise<WindowInfo[]>;
    /** Met une fenêtre au premier plan. */
    focus(id: number): Promise<void>;
    /** Helper : `cb` à chaque changement de fenêtre active (sonde `foreground`). Renvoie un stop. */
    onForegroundChanged(cb: (w: WindowInfo | null) => void, intervalMs?: number): () => void;
  };
  /** Presse-papiers (texte + image). Requiert la permission `clipboard`. */
  clipboard: {
    readText(): Promise<string>;
    writeText(text: string): Promise<void>;
    /** Image du presse-papiers en PNG data URL, ou `null` s'il n'y en a pas. */
    readImage(): Promise<string | null>;
    /** Écrit une image (PNG data URL, ex. `canvas.toDataURL()`). */
    writeImage(dataUrl: string): Promise<void>;
  };
  /**
   * Thème courant de l'hôte. Utile au rendu canvas/SVG qui ne suit pas les tokens
   * CSS (le HTML normal, lui, hérite des variables — voir le contrat Tailwind).
   * `onChange` renvoie une fonction de désabonnement.
   */
  theme: {
    current(): "dark" | "light";
    onChange(cb: (theme: "dark" | "light") => void): () => void;
  };
  /**
   * HTTP NATIF avec cookie-jar propre à l'extension (comme dio + cookie_jar) →
   * consomme une API tierce avec session par cookie, hors restrictions
   * CORS/SameSite d'un fetch navigateur. Requiert la permission `network`.
   */
  http: {
    request(opts: { extId: string; url: string; method?: string; body?: unknown; headers?: Record<string, string> }): Promise<{ status: number; body: string }>;
  };
  /**
   * Bus pub/sub ENTRE extensions (composition sans couplage). `on` renvoie une
   * fonction de désabonnement. Choisis des noms de canaux préfixés (ex.
   * `"nowplaying:update"`) pour éviter les collisions.
   */
  bus: {
    emit(channel: string, payload?: unknown): void;
    on(channel: string, cb: (payload: any) => void): () => void;
  };
  /** Pousse une notification (bannière + historique). Renvoie son id. */
  notify(spec: NotificationSpec): string;
  notifications: {
    dismiss(id: string): void;
    clear(): void;
  };
  openModal(req: ModalRequest): void;
  closeModal(): void;
  /** Ouvre une URL http(s) dans le navigateur par défaut. */
  openExternal(url: string): Promise<void>;
  /** Synthèse vocale : lit `text` à voix haute (SAPI). */
  speak(text: string): void;
  /** Automatisation d'entrée clavier. ⚠ Tape dans l'application active → requiert `input`. */
  input: {
    typeText(text: string): Promise<void>;
  };
  invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  on(event: string, cb: (payload: any) => void): Promise<() => void>;
}

/** Contexte passé à `activate` : l'API + l'identité de l'extension + son store. */
export type ExtensionContext = IslandApi & {
  id: string;
  storage: ExtStorage;
  secrets: ExtSecrets;
};

export interface ExtensionDef {
  /** Surfaces UI montées par l'hôte (île, modal…). */
  surfaces?: Record<string, Component>;
  activate?(ctx: ExtensionContext): void | Promise<void>;
  deactivate?(): void | Promise<void>;
}

/** Helper de typage : déclare une extension. */
export function defineExtension(def: ExtensionDef): ExtensionDef {
  return def;
}

interface Bridge {
  invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen(event: string, cb: (e: { payload: any }) => void): Promise<() => void>;
  openModal(req: ModalRequest): void;
  closeModal(): void;
  openView(component: Component, size?: { width?: number; height?: number; radius?: number; persistent?: boolean; safeZone?: "relative" | "absolute" | "hidden"; safeArea?: boolean }): void;
  closeView(): void;
  resizeView(size: { width?: number; height?: number; radius?: number }): void;
  openDrop(component: Component): void;
  closeDrop(): void;
  openWindow(component: Component, opts?: { id?: string; title?: string; width?: number; height?: number; x?: number; y?: number; resizable?: boolean }): string;
  closeWindow(id?: string): void;
  focusWindow(id: string): void;
  setIdleState(key: string, state: IdleState | null, priority: number): void;
  setIdleAction(key: string, action: ({ slot: "left" | "right" } & IdleAction) | null): void;
  setIdleTap(key: string, handler: (() => void) | null): void;
  setLauncherEntry(key: string, entry: { id: string; label: string; icon: string; onActivate: () => void } | null): void;
  setLauncherProvider(key: string, provider: { onQuery: (query: string) => LauncherResult[] | Promise<LauncherResult[]> } | null): void;
  registerShortcut(key: string, accelerator: string, handler: () => void): Promise<boolean>;
  unregisterShortcut(key: string): Promise<void>;
  pickFolder(): Promise<string | null>;
  selectRegion(): Promise<Region | null>;
  showRegionOutline(region: Region | null): void;
  notify(spec: NotificationSpec): string;
  dismissNotification(id: string): void;
  clearNotifications(): void;
  busEmit(channel: string, payload: unknown): void;
  busOn(channel: string, handler: (payload: any) => void, owner?: string): () => void;
}
function bridge(): Bridge {
  const b = (globalThis as any).__ISLAND__ as Bridge | undefined;
  if (!b) throw new Error("Island bridge indisponible (window.__ISLAND__)");
  return b;
}

// État média PARTAGÉ : un seul flux `media://update` quelle que soit l'extension.
const mediaState = reactive<MediaState>({
  title: "",
  artist: "",
  art: "",
  playing: false,
  positionMs: 0,
  durationMs: 0,
  source: "",
});
let mediaWired = false;
function ensureMedia(b: Bridge) {
  if (mediaWired) return;
  mediaWired = true;
  b.listen("media://update", (e) => {
    const u = e.payload || {};
    if (!u.active) {
      mediaState.playing = false;
      mediaState.source = "";
      return;
    }
    mediaState.title = u.title ?? "";
    mediaState.artist = u.artist ?? "";
    if (u.art) mediaState.art = u.art;
    mediaState.playing = !!u.isPlaying;
    mediaState.positionMs = u.positionMs ?? 0;
    mediaState.durationMs = u.durationMs ?? 0;
    mediaState.source = u.source ?? "";
  });
}

// Une API par extId → les services GARDÉS (capture/system/media/network) joignent
// automatiquement l'extId pour que l'hôte vérifie la permission déclarée au manifeste.
const apiCache = new Map<string, IslandApi>();

/**
 * Accès à l'API Island. `extId` = identité de l'extension : à fournir pour les services
 * gardés par une permission (capture, system, media, network) — l'hôte vérifie alors le
 * manifeste. Les extensions reçoivent un `ctx` déjà lié ; dans un composant, passe ton
 * `EXT_ID`. Sans extId, les services gardés sont refusés côté hôte.
 */
export function useIsland(extId: string = ""): IslandApi {
  const hit = apiCache.get(extId);
  if (hit) return hit;
  const b = bridge();
  ensureMedia(b);
  // Namespace propre à cette extension → pas de collision de clés idle entre extensions.
  const ns = "ext:" + Math.random().toString(36).slice(2, 8);
  const state = mediaState;

  const cached: IslandApi = {
    media: {
      state,
      toggle: () => void b.invoke("media_toggle", { extId }),
      next: () => void b.invoke("media_next", { extId }),
      prev: () => void b.invoke("media_prev", { extId }),
      seek: (ms) => void b.invoke("media_seek", { extId, positionMs: Math.round(ms) }),
      setVolume: (v) => void b.invoke("media_set_volume", { extId, level: v }),
    },
    idle: {
      state: (s, opts) => b.setIdleState(`${ns}:state`, s, opts?.priority ?? 10),
      action: (slot, a) => b.setIdleAction(`${ns}:${slot}`, a ? { slot, ...a } : null),
      tap: (h) => b.setIdleTap(`${ns}:tap`, h),
    },
    view: {
      open: (component, size) => b.openView(component, size),
      close: () => b.closeView(),
      resize: (size) => b.resizeView(size),
    },
    drop: {
      open: (component) => b.openDrop(component),
      close: () => b.closeDrop(),
    },
    window: {
      open: (component, opts) => b.openWindow(component, opts),
      close: (id) => b.closeWindow(id),
      focus: (id) => b.focusWindow(id),
    },
    launcher: {
      register: (e) => b.setLauncherEntry(`${ns}:launcher`, { id: `${ns}:launcher`, ...e }),
      remove: () => b.setLauncherEntry(`${ns}:launcher`, null),
      provider: (p) => b.setLauncherProvider(`${ns}:launcher`, p),
      removeProvider: () => b.setLauncherProvider(`${ns}:launcher`, null),
    },
    capture: {
      listDisplays: () => b.invoke<Display[]>("capture_list_displays", { extId }),
      screenshot: (opts) =>
        b.invoke<string>("capture_screenshot", {
          extId,
          display: opts?.display ?? null,
          excludeIsland: opts?.excludeIsland ?? true,
          dir: opts?.dir ?? null,
          region: opts?.region ?? null,
        }),
      startRecording: (opts) =>
        b.invoke<string>("capture_start_recording", {
          extId,
          display: opts?.display ?? null,
          excludeIsland: opts?.excludeIsland ?? true,
          dir: opts?.dir ?? null,
          region: opts?.region ?? null,
          fps: opts?.fps ?? 30,
          audio: opts?.audio ?? false,
          encoder: opts?.encoder ?? null,
        }),
      stopRecording: () => b.invoke<string>("capture_stop_recording"),
      isRecording: () => b.invoke<boolean>("capture_is_recording", { extId }),
      fetchBinary: (s) =>
        b.invoke<void>("ext_fetch_binary", {
          extId: s.extId,
          url: s.url,
          dest: s.dest,
          zipEntry: s.zipEntry ?? null,
        }),
      pickFolder: () => b.pickFolder(),
      selectRegion: () => b.selectRegion(),
      showRegionOutline: (r) => b.showRegionOutline(r),
    },
    shortcuts: {
      register: (accel, h) => b.registerShortcut(`${ns}:${accel}`, accel, h),
      unregister: (accel) => b.unregisterShortcut(`${ns}:${accel}`),
    },
    system: {
      stats: () => b.invoke<SysStats>("system_stats", { extId }),
      battery: () => b.invoke<{ percent: number; charging: boolean } | null>("system_battery", { extId }),
      online: () => b.invoke<boolean>("system_online", { extId }),
      volume: () => b.invoke<{ level: number; muted: boolean } | null>("system_volume", { extId }),
      setVolume: (level) => b.invoke<void>("system_set_volume", { extId, level }),
      setMuted: (muted) => b.invoke<void>("system_set_muted", { extId, muted }),
      idleMs: () => b.invoke<number>("system_idle_ms", { extId }),
      onUserIdle: (ms, onIdle, onActive) => {
        let idle = false;
        const period = Math.max(1000, Math.min(ms, 5000));
        const t = setInterval(async () => {
          const cur = await b.invoke<number>("system_idle_ms", { extId }).catch(() => 0);
          if (!idle && cur >= ms) {
            idle = true;
            onIdle();
          } else if (idle && cur < ms) {
            idle = false;
            onActive?.();
          }
        }, period);
        return () => clearInterval(t);
      },
    },
    windows: {
      foreground: () => b.invoke<WindowInfo | null>("window_foreground", { extId }),
      list: () => b.invoke<WindowInfo[]>("window_list", { extId }),
      focus: (id) => b.invoke<void>("window_focus", { extId, id }),
      onForegroundChanged: (cb, intervalMs) => {
        let lastId: number | undefined;
        const period = Math.max(300, intervalMs ?? 800);
        const t = setInterval(async () => {
          const w = await b.invoke<WindowInfo | null>("window_foreground", { extId }).catch(() => null);
          const id = w?.id ?? -1;
          if (id !== lastId) {
            lastId = id;
            cb(w);
          }
        }, period);
        return () => clearInterval(t);
      },
    },
    clipboard: {
      readText: () => b.invoke<string>("clipboard_read_text", { extId }),
      writeText: (text) => b.invoke<void>("clipboard_write_text", { extId, text }),
      readImage: () => b.invoke<string | null>("clipboard_read_image", { extId }),
      writeImage: (dataUrl) => b.invoke<void>("clipboard_write_image", { extId, dataUrl }),
    },
    theme: {
      current: () => (document.documentElement.classList.contains("dark") ? "dark" : "light"),
      onChange: (cb) => {
        const read = (): "dark" | "light" => (document.documentElement.classList.contains("dark") ? "dark" : "light");
        let last = read();
        const obs = new MutationObserver(() => {
          const t = read();
          if (t !== last) {
            last = t;
            cb(t);
          }
        });
        obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
        return () => obs.disconnect();
      },
    },
    http: {
      request: (o) =>
        b.invoke<{ status: number; body: string }>("http_fetch", {
          extId: o.extId,
          method: o.method ?? "GET",
          url: o.url,
          body: o.body === undefined ? null : typeof o.body === "string" ? o.body : JSON.stringify(o.body),
          headers: o.headers ?? null,
        }),
    },
    bus: {
      emit: (channel, payload) => b.busEmit(channel, payload),
      on: (channel, cb) => b.busOn(channel, cb),
    },
    notify: (spec) => b.notify(spec),
    notifications: {
      dismiss: (id) => b.dismissNotification(id),
      clear: () => b.clearNotifications(),
    },
    openModal: (req) => b.openModal(req),
    closeModal: () => b.closeModal(),
    openExternal: (url) => b.invoke<void>("open_url", { url }),
    speak: (text) => void b.invoke("tts_speak", { text }),
    input: {
      typeText: (text) => b.invoke<void>("input_type_text", { extId, text }),
    },
    invoke: (cmd, args) => b.invoke(cmd, args),
    on: (event, cb) => b.listen(event, (e) => cb(e.payload)),
  };
  apiCache.set(extId, cached);
  return cached;
}
