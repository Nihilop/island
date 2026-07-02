// Raccourcis clavier GLOBAUX (système) pour les extensions.
// Clés scopées par extension (`${id}:${accelerator}`) → cleanup au unload.
// IMPORTANT : rien n'est enregistré tant qu'une extension ne le demande pas
// explicitement (utile pour les jeux : pas de hotkey parasite).
//
// DEUX backends, une seule API :
//  - raccourcis normaux (Ctrl+Alt+X…) → plugin global-shortcut (RegisterHotKey) ;
//  - touches RÉSERVÉES par l'OS que RegisterHotKey refuse (touche Windows seule)
//    → hook clavier natif côté hôte (commande `reserved_key_set` + event
//    `reserved://key`). L'extension ne voit qu'un accélérateur ("Super").
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const byKey = new Map<string, string>(); // key → accelerator enregistré
const byAccel = new Map<string, string>(); // accelerator → key propriétaire (anti-conflit)

// --- Touches réservées (hook natif) ----------------------------------------
// Accélérateurs qui désignent la touche Windows seule → identifiant logique "Super".
const RESERVED: Record<string, string> = { super: "Super", win: "Super", meta: "Super" };
const reservedOf = (accel: string): string | null => RESERVED[accel.trim().toLowerCase()] ?? null;
// extId = la clé sans son suffixe `:${accelerator}` (cf. SDK : `${extId}:${accel}`).
const extIdOf = (key: string, accel: string): string => key.slice(0, Math.max(0, key.length - accel.length - 1));

const reservedHandlers = new Map<string, () => void>(); // key propriétaire → handler
const reservedOwner = new Map<string, string>(); // "Super" → key propriétaire
let reservedWired = false;
async function ensureReservedListener() {
  if (reservedWired) return;
  reservedWired = true;
  // Un seul listener global : l'hôte émet la touche réservée déclenchée.
  await listen<{ key: string }>("reserved://key", (e) => {
    const owner = reservedOwner.get(e.payload?.key);
    if (owner) reservedHandlers.get(owner)?.();
  });
}

// Toutes les opérations (register/unregister) sont SÉRIALISÉES : sinon un
// unregister(ancien) et un register(nouveau) ciblant le même hotkey physique se
// chevauchent (async) et le plugin lève « déjà enregistré » → faux conflit.
let queue: Promise<unknown> = Promise.resolve();
function serial<T>(fn: () => Promise<T>): Promise<T> {
  const run = queue.then(fn, fn);
  queue = run.catch(() => {});
  return run;
}

// Enregistrement OS : route vers le hook réservé (touche Win) ou le plugin. Throw si indispo.
async function osRegister(key: string, accel: string, handler: () => void): Promise<void> {
  const reserved = reservedOf(accel);
  if (reserved) {
    await ensureReservedListener();
    reservedHandlers.set(key, handler);
    reservedOwner.set(reserved, key);
    await invoke("reserved_key_set", { extId: extIdOf(key, accel), key: reserved, enabled: true });
    return;
  }
  await register(accel, (e) => {
    if (e.state === "Pressed") handler();
  });
}
async function osUnregister(key: string, accel: string): Promise<void> {
  const reserved = reservedOf(accel);
  if (reserved) {
    reservedHandlers.delete(key);
    if (reservedOwner.get(reserved) === key) {
      reservedOwner.delete(reserved);
      await invoke("reserved_key_set", { extId: extIdOf(key, accel), key: reserved, enabled: false }).catch(() => {});
    }
    return;
  }
  await unregister(accel);
}

/**
 * Enregistre un raccourci. Renvoie `false` UNIQUEMENT en cas de vrai conflit
 * (accelerator déjà possédé par une AUTRE extension) ou de refus OS persistant.
 */
export function registerShortcut(key: string, accelerator: string, handler: () => void): Promise<boolean> {
  return serial(async () => {
    const owner = byAccel.get(accelerator);
    if (owner && owner !== key) return false; // vrai conflit : autre extension
    if (owner === key) {
      // Déjà à nous → idempotent, mais on rafraîchit le handler (reload d'ext).
      if (reservedOf(accelerator)) reservedHandlers.set(key, handler);
      return true;
    }

    // Si cette clé pointait sur un autre accel, on le libère d'abord.
    const prev = byKey.get(key);
    if (prev && prev !== accelerator) {
      byKey.delete(key);
      if (byAccel.get(prev) === key) byAccel.delete(prev);
      try { await osUnregister(key, prev); } catch { /* noop */ }
    }
    // Nettoie toute trace OS résiduelle de cet accelerator (reloads, crash…).
    try { await osUnregister(key, accelerator); } catch { /* pas enregistré, ok */ }

    try {
      await osRegister(key, accelerator, handler);
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
      await osUnregister(key, acc);
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
