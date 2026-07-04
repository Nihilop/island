// Loader d'extensions — activation/désactivation EN RUNTIME (load/unload live).
// Island ne lit QUE le `manifest.json` + le `dist/` buildé de chaque extension
// (%APPDATA%/<identifier>/extensions/<ext>/). La source éventuellement présente
// dans le dossier (le dev y travaille avec son propre `pnpm dev`) est IGNORÉE.
// L'état d'activation est possédé par l'app.
import { ref, markRaw, effectScope, type Component, type EffectScope } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useIsland, type ExtensionDef, type ExtStorage, type ExtSecrets, type ExtensionContext } from "../sdk";
import { cleanupExtListeners } from "../sdk/island";
import { setIdleState, setIdleCenter, setIdleAction, setIdleTap } from "./idle";
import { setLauncherEntry, setLauncherProvider } from "./launcher";
import { busEmit, busOn, busClear } from "./bus";
import { registerShortcut, unregisterShortcut, unregisterShortcutsFor } from "./shortcuts";
import { activeView, closeView, closeDrop } from "./overlay";

function makeStorage(id: string): ExtStorage {
  return {
    get: async <T = unknown>(key: string, fallback?: T) => {
      const v = await invoke<T | null>("storage_get", { ext: id, key }).catch(() => null);
      return (v ?? fallback) as T | undefined;
    },
    set: (key, value) => invoke("storage_set", { ext: id, key, value }).then(() => {}).catch(() => {}),
    delete: (key) => invoke("storage_delete", { ext: id, key }).then(() => {}).catch(() => {}),
    keys: () => invoke<string[]>("storage_keys", { ext: id }).catch(() => []),
  };
}

// Coffre de secrets chiffré (Credential Manager) — isolé par id, comme le storage.
function makeSecrets(id: string): ExtSecrets {
  return {
    get: (key) => invoke<string | null>("secret_get", { ext: id, key }).catch(() => null),
    set: (key, value) => invoke("secret_set", { ext: id, key, value }).then(() => {}).catch(() => {}),
    delete: (key) => invoke("secret_delete", { ext: id, key }).then(() => {}).catch(() => {}),
  };
}

export interface DiscoveredExtension { id: string; name: string; surfaces: string[]; dev: boolean }
export const extensions = ref<DiscoveredExtension[]>([]);

// --- Extensions INSTALLÉES (%APPDATA%/island/extensions, via Rust `list_installed`).
interface InstalledExt { id: string; dir: string; manifest: any; dev: boolean }
let installed: InstalledExt[] = [];
async function loadInstalled() {
  installed = await invoke<InstalledExt[]>("list_installed").catch(() => []);
}

// Chemin /@fs/ : Vite sert+transpile un fichier hors racine (chemins en slashes avant).
function fsUrl(dir: string, entry: string): string {
  return `/@fs/${dir.replace(/\\/g, "/").replace(/\/$/, "")}/${entry}`;
}

const surfaces = new Map<string, Component>();
export function getSurface(extId: string, name: string): Component | undefined {
  return surfaces.get(`${extId}.${name}`);
}

export async function discoverManifests(): Promise<DiscoveredExtension[]> {
  await loadInstalled();
  return installed.map((ins) => {
    const man = ins.manifest;
    return { id: man.id, name: man.name, surfaces: Object.keys(man.surfaces || {}), dev: ins.dev };
  });
}

// --- État d'activation, POSSÉDÉ PAR L'APP (persisté, store __app__ clé "enabled").
const APP = "__app__";
export async function getEnabled(): Promise<string[] | null> {
  return await invoke<string[] | null>("storage_get", { ext: APP, key: "enabled" }).catch(() => null);
}
export async function setEnabled(ids: string[]) {
  await invoke("storage_set", { ext: APP, key: "enabled", value: ids }).catch(() => {});
}

interface Active { def: ExtensionDef; scope: EffectScope; surfaceKeys: string[]; comps: Component[]; cssEl: HTMLElement | null }
const active = new Map<string, Active>();

// ctx scopé : idle/launcher avec des clés DÉTERMINISTES (= id) pour le cleanup.
function makeCtx(id: string): ExtensionContext {
  // Lie l'extId → les services gardés (capture/system/media/network) le joignent
  // automatiquement et l'hôte vérifie la permission déclarée au manifeste.
  const api = useIsland(id);
  const idle = {
    state: (s: any, opts?: { priority?: number }) => setIdleState(`${id}:state`, s, opts?.priority ?? 10),
    center: (c: any, opts?: { priority?: number }) => setIdleCenter(`${id}:center`, c, opts?.priority ?? 10),
    action: (slot: "left" | "right", a: any) => setIdleAction(`${id}:${slot}`, a ? { slot, ...a } : null),
    tap: (h: (() => void) | null) => setIdleTap(`${id}:tap`, h),
  };
  const launcher = {
    register: (e: any) => setLauncherEntry(`${id}:launcher`, { id: `${id}:launcher`, ...e }),
    remove: () => setLauncherEntry(`${id}:launcher`, null),
    provider: (p: any) => setLauncherProvider(`${id}:launcher`, p),
    removeProvider: () => setLauncherProvider(`${id}:launcher`, null),
  };
  const shortcuts = {
    register: (accel: string, handler: () => void) => registerShortcut(`${id}:${accel}`, accel, handler),
    unregister: (accel: string) => unregisterShortcut(`${id}:${accel}`),
  };
  // Bus scopé : les abonnements sont tracés par id → nettoyés à la désactivation.
  const bus = {
    emit: (channel: string, payload?: unknown) => busEmit(channel, payload),
    on: (channel: string, cb: (p: any) => void) => busOn(channel, cb, id),
  };
  return Object.assign(Object.create(api), { id, storage: makeStorage(id), secrets: makeSecrets(id), idle, launcher, shortcuts, bus }) as ExtensionContext;
}

interface Resolved { load: () => Promise<unknown>; ins: InstalledExt }

// Charge TOUJOURS le BUILD (`dist/index.mjs`, vue/@island/sdk externalisés) — jamais
// la source. Le MÉCANISME est auto-détecté :
//  - DEV (pnpm tauri dev) → via /@fs/ (Vite sert + résout vue/sdk vers l'hôte) ;
//  - PROD (exe livré, pas de Vite) → lecture du fichier (commande Rust) + import par
//    Blob URL ; les imports bare `vue`/`@island/sdk` sont résolus par l'import map
//    (index.html → shims → runtime de l'hôte, MÊME instance).
function resolveLoader(id: string): Resolved | null {
  const ins = installed.find((i) => i.id === id);
  if (!ins) return null;
  const main = ins.manifest.main ?? "dist/index.mjs";

  if (import.meta.env.DEV) {
    const stamp = Date.now(); // cache-bust → re-import après rebuild (live-reload)
    return { ins, load: () => import(/* @vite-ignore */ `${fsUrl(ins.dir, main)}?t=${stamp}`) };
  }

  return {
    ins,
    load: async () => {
      const code = await invoke<string>("read_ext_file", { id, file: main });
      const url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
      try {
        return await import(/* @vite-ignore */ url);
      } finally {
        setTimeout(() => URL.revokeObjectURL(url), 4000);
      }
    },
  };
}

// Injecte la feuille de style d'une extension, traçable par id pour la retirer au
// unload. DEV = <link> via /@fs/ ; PROD = <style> (contenu lu côté Rust).
async function injectCss(id: string, ins: InstalledExt): Promise<HTMLElement | null> {
  const styles = ins.manifest.styles ?? "dist/style.css";
  if (import.meta.env.DEV) {
    const el = document.createElement("link");
    el.rel = "stylesheet";
    el.href = `${fsUrl(ins.dir, styles)}?t=${Date.now()}`;
    el.dataset.extCss = id;
    document.head.appendChild(el);
    return el;
  }
  const css = await invoke<string>("read_ext_file", { id, file: styles }).catch(() => "");
  if (!css) return null;
  const el = document.createElement("style");
  el.textContent = css;
  el.dataset.extCss = id;
  document.head.appendChild(el);
  return el;
}

async function activateExtension(id: string) {
  if (active.has(id)) return;
  const resolved = resolveLoader(id);
  if (!resolved) return;
  const cssEl = await injectCss(id, resolved.ins);
  const mod = (await resolved.load()) as { default?: ExtensionDef };
  const def = mod.default;
  if (!def) { cssEl?.remove(); return; }

  const surfaceKeys: string[] = [];
  const comps: Component[] = [];
  if (def.surfaces) {
    for (const [name, comp] of Object.entries(def.surfaces)) {
      const c = markRaw(comp as Component);
      surfaces.set(`${id}.${name}`, c);
      surfaceKeys.push(`${id}.${name}`);
      comps.push(c);
    }
  }
  // effectScope → on pourra stopper tous les watch/watchEffect de l'extension au unload.
  const scope = effectScope();
  scope.run(() => void def.activate?.(makeCtx(id)));
  active.set(id, { def, scope, surfaceKeys, comps, cssEl });
}

function deactivateExtension(id: string) {
  const rec = active.get(id);
  if (!rec) return;
  active.delete(id);
  rec.scope.stop(); // stoppe les effets réactifs de l'extension
  try { void rec.def.deactivate?.(); } catch { /* noop */ }
  // Nettoyage de tout ce que l'extension a contribué.
  setIdleState(`${id}:state`, null);
  setIdleCenter(`${id}:center`, null);
  setIdleAction(`${id}:left`, null);
  setIdleAction(`${id}:right`, null);
  setIdleTap(`${id}:tap`, null);
  setLauncherEntry(`${id}:launcher`, null);
  setLauncherProvider(`${id}:launcher`, null);
  busClear(id); // retire les abonnements bus de l'extension
  cleanupExtListeners(id); // retire les listeners Tauri (on()) — non stoppés par l'effectScope
  for (const k of rec.surfaceKeys) surfaces.delete(k);
  rec.cssEl?.remove(); // retire la feuille de style injectée
  void unregisterShortcutsFor(`${id}:`); // retire les raccourcis globaux de l'extension
  // Si une de ses surfaces est montée (view/goutte), on referme.
  if (activeView.value && rec.comps.includes(activeView.value)) { closeDrop(); closeView(); }
}

/** Aligne l'état chargé sur l'état d'activation persisté (load/unload ce qui a changé). */
async function reconcile() {
  await loadInstalled();
  const ids = installed.map((i) => i.id);
  const enabled = (await getEnabled()) ?? ids;
  for (const id of ids) {
    const on = enabled.includes(id);
    if (on && !active.has(id)) await activateExtension(id);
    else if (!on && active.has(id)) deactivateExtension(id);
  }
  // Désactive aussi ce qui a été désinstallé (plus dans la liste scannée).
  for (const id of [...active.keys()]) {
    if (!ids.includes(id)) deactivateExtension(id);
  }
}

// Live-reload : le watcher Rust émet `ext://changed` quand un `dist/` est rebuildé.
// On recharge UNIQUEMENT l'extension concernée (deactivate → reconcile la réactive
// avec un cache-bust frais → nouveau dist/index.mjs importé).
async function onDistChanged(dir: string) {
  await loadInstalled();
  const ins = installed.find((i) => i.dir === dir);
  // Live-reload réservé aux extensions EN DÉVELOPPEMENT (source/package.json présent).
  // Un client n'a que manifest+dist (jamais rebuildé) → aucun reload, aucune accumulation.
  if (!ins || !ins.dev) return;
  if (active.has(ins.id)) deactivateExtension(ins.id);
  await reconcile();
}

let loaded = false;
export async function loadExtensions() {
  if (loaded) return;
  loaded = true;
  extensions.value = await discoverManifests();
  await reconcile();
  // La fenêtre Réglages (autre webview) émet ceci quand on toggle une extension.
  await listen("ext://reload", () => void reconcile());
  // Le watcher Rust émet ceci quand le build d'une extension change (pnpm dev).
  await listen<{ dir: string }>("ext://changed", (e) => void onDistChanged(e.payload.dir));
}
