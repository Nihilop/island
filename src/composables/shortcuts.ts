// Raccourcis clavier GLOBAUX (système) pour les extensions, via le plugin Tauri.
// Clés scopées par extension (`${id}:${accelerator}`) → cleanup au unload.
// IMPORTANT : rien n'est enregistré tant qu'une extension ne le demande pas
// explicitement (utile pour les jeux : pas de hotkey parasite).
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";

const byKey = new Map<string, string>(); // key → accelerator enregistré
const byAccel = new Map<string, string>(); // accelerator → key propriétaire (anti-conflit)

// Toutes les opérations (register/unregister) sont SÉRIALISÉES : sinon un
// unregister(ancien) et un register(nouveau) ciblant le même hotkey physique se
// chevauchent (async) et le plugin lève « déjà enregistré » → faux conflit.
let queue: Promise<unknown> = Promise.resolve();
function serial<T>(fn: () => Promise<T>): Promise<T> {
  const run = queue.then(fn, fn);
  queue = run.catch(() => {});
  return run;
}

/**
 * Enregistre un raccourci. Renvoie `false` UNIQUEMENT en cas de vrai conflit
 * (accelerator déjà possédé par une AUTRE extension) ou de refus OS persistant.
 */
export function registerShortcut(key: string, accelerator: string, handler: () => void): Promise<boolean> {
  return serial(async () => {
    const owner = byAccel.get(accelerator);
    if (owner && owner !== key) return false; // vrai conflit : autre extension
    if (owner === key) return true; // déjà à nous → idempotent

    // Si cette clé pointait sur un autre accel, on le libère d'abord.
    const prev = byKey.get(key);
    if (prev && prev !== accelerator) {
      byKey.delete(key);
      if (byAccel.get(prev) === key) byAccel.delete(prev);
      try { await unregister(prev); } catch { /* noop */ }
    }
    // Nettoie toute trace OS résiduelle de cet accelerator (reloads, crash…).
    try { await unregister(accelerator); } catch { /* pas enregistré, ok */ }

    try {
      await register(accelerator, (e) => {
        if (e.state === "Pressed") handler();
      });
      byKey.set(key, accelerator);
      byAccel.set(accelerator, key);
      return true;
    } catch (err) {
      console.warn("[island] raccourci indisponible:", accelerator, err);
      return false;
    }
  });
}

export function unregisterShortcut(key: string): Promise<void> {
  return serial(async () => {
    const acc = byKey.get(key);
    if (!acc) return;
    byKey.delete(key);
    if (byAccel.get(acc) === key) byAccel.delete(acc);
    try {
      await unregister(acc);
    } catch {
      /* noop */
    }
  });
}

/** Retire tous les raccourcis d'une extension (préfixe `${id}:`). */
export async function unregisterShortcutsFor(prefix: string) {
  for (const key of [...byKey.keys()]) {
    if (key.startsWith(prefix)) await unregisterShortcut(key);
  }
}
