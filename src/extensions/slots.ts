// Emplacements des extensions v2 rendus PAR L'HÔTE (aucun code d'extension dans le
// notch) : activité en direct, résultats du launcher, commandes. Le slot « outil » est
// dans tools.ts, les réglages dans settings.ts.
//
// RPC (extension → hôte) :
//   live.set(id, spec | null)               activité en direct (notch fermé)
//   launcher.results(provider, seq, items)  réponse à l'événement `launcher.query`
// Événements (hôte → extension) :
//   launcher.query { provider, q, seq } · launcher.run { provider, id } · command { id }

import { computed, reactive } from "vue";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { addMethods, emitTo, extUrl, installed, type Manifest } from "./runtime";
import { addLauncherSource, setExternalResults, type ExternalItem } from "../launcher/store";

// --- Activité en direct ----------------------------------------------------------------

export type LiveColor = "label" | "green" | "orange" | "red" | "blue";

export interface LiveSpec {
  /** Icône : chemin d'un fichier de l'extension (SVG teinté par `color`). */
  icon?: string;
  /** Texte court à droite (≤ 12 caractères affichés). */
  text?: string;
  color?: LiveColor;
  /** « high » passe avant le minuteur et la musique ; « normal » après la musique. */
  priority?: "high" | "normal";
}

interface LiveEntry extends LiveSpec {
  ext: Manifest;
  tool: string | null;
  at: number;
}

const lives = reactive(new Map<string, LiveEntry>());

/** Activité d'extension à afficher (la plus prioritaire, puis la plus récente). */
export const extLive = computed(() => {
  let best: LiveEntry | null = null;
  for (const l of lives.values()) {
    const rank = (x: LiveEntry) => (x.priority === "high" ? 1 : 0);
    if (!best || rank(l) > rank(best) || (rank(l) === rank(best) && l.at > best.at)) best = l;
  }
  return best;
});

export const liveIconUrl = (l: LiveEntry) => (l.icon ? extUrl(l.ext.id, l.icon) : l.ext.icon ? extUrl(l.ext.id, l.ext.icon) : null);

addMethods({
  "live.set": (ext, id: unknown, spec: unknown) => {
    const decl = ext.contributes.live.find((l) => l.id === String(id));
    if (!decl) throw new Error(`unknownLive:${String(id)}`);
    const key = `${ext.id}:${decl.id}`;
    if (!spec || typeof spec !== "object") return void lives.delete(key);
    const s = spec as LiveSpec;
    lives.set(key, {
      ext,
      tool: decl.tool ? `ext:${ext.id}:${decl.tool}` : null,
      icon: typeof s.icon === "string" ? s.icon : undefined,
      text: typeof s.text === "string" ? s.text.slice(0, 24) : undefined,
      color: s.color,
      priority: s.priority === "high" ? "high" : "normal",
      at: Date.now(),
    });
  },
});

// --- Launcher ------------------------------------------------------------------------------

/** Dernière requête envoyée par fournisseur (réponses périmées ignorées). */
const lastSeq = new Map<string, number>();
let seq = 0;

addMethods({
  "launcher.results": (ext, provider: unknown, s: unknown, items: unknown) => {
    const key = `${ext.id}:${String(provider)}`;
    if (lastSeq.get(key) !== Number(s) || !Array.isArray(items)) return;
    const clean: ExternalItem[] = items
      .filter((i) => i && typeof i.id === "string" && typeof i.title === "string")
      .slice(0, 40)
      .map((i) => ({
        id: i.id,
        title: String(i.title).slice(0, 200),
        subtitle: typeof i.subtitle === "string" ? i.subtitle.slice(0, 200) : undefined,
        icon: typeof i.icon === "string" ? extUrl(ext.id, i.icon) : ext.icon ? extUrl(ext.id, ext.icon) : null,
      }));
    setExternalResults(key, clean);
  },
});

// --- Commandes -----------------------------------------------------------------------------

/** Raccourcis enregistrés par les extensions (accélérateur → commande). */
const shortcuts = new Map<string, { ext: string; id: string }>();

async function bindShortcut(ext: Manifest, cmd: { id: string; shortcut?: string }) {
  if (!cmd.shortcut || shortcuts.has(cmd.shortcut)) return;
  try {
    await register(cmd.shortcut, (e) => {
      if (e.state === "Pressed") emitTo(ext.id, "command", { id: cmd.id });
    });
    shortcuts.set(cmd.shortcut, { ext: ext.id, id: cmd.id });
  } catch {
    // Pris par une autre app : la commande reste disponible dans le launcher.
  }
}

/** Branche une extension sur les emplacements live / launcher / commandes. */
export function registerExtensionSlots(ext: Manifest) {
  for (const p of ext.contributes.launcher) {
    const key = `${ext.id}:${p.id}`;
    addLauncherSource({
      key,
      name: p.title ?? ext.name,
      prefix: p.prefix,
      query: (q) => {
        const s = ++seq;
        lastSeq.set(key, s);
        emitTo(ext.id, "launcher.query", { provider: p.id, q, seq: s });
      },
      run: (id) => emitTo(ext.id, "launcher.run", { provider: p.id, id }),
    });
  }
  // Commandes : aussi cherchables dans le launcher (titre).
  if (ext.contributes.commands.length) {
    const key = `${ext.id}:#commands`;
    addLauncherSource({
      key,
      name: ext.name,
      query: (q) => {
        const low = q.toLowerCase();
        setExternalResults(
          key,
          ext.contributes.commands
            .filter((c) => c.title.toLowerCase().includes(low) || ext.name.toLowerCase().includes(low))
            .map((c) => ({ id: c.id, title: c.title, subtitle: ext.name, icon: ext.icon ? extUrl(ext.id, ext.icon) : null })),
        );
      },
      run: (id) => emitTo(ext.id, "command", { id }),
    });
    for (const c of ext.contributes.commands) void bindShortcut(ext, c);
  }
}

/** Retire les raccourcis d'extensions (désinstallation, rechargement). */
export async function unbindShortcuts(extId?: string) {
  for (const [accel, c] of [...shortcuts]) {
    if (extId && c.ext !== extId) continue;
    await unregister(accel).catch(() => {});
    shortcuts.delete(accel);
  }
}

export { installed };
