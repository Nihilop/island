// Windows Hello, côté front : état du service (configuré une fois dans Réglages › Outils)
// et verrous de vue par portée (Note aujourd'hui, extensions plus tard). Le Coffre a son
// propre déverrouillage (session Bitwarden scellée côté Rust) mais passe par `prompt`.
//
// Verrou de vue : une portée activée démarre verrouillée ; « déverrouiller » demande Hello
// puis la laisse ouverte tant qu'on s'en sert. Inactivité > délai (Réglages, 15 min par
// défaut), mode présentation ou désactivation de Hello → verrouillée.

import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { hold } from "./state";
import { settings } from "./settings";
import { presentation } from "./presentation";

export type HelloScope = keyof typeof settings.hello;

export const hello = reactive({
  supported: false,
  enrolled: false,
  /** Configuration / désactivation en cours. */
  busy: false,
  /** Une invite Hello est ouverte (ignore les clics répétés). */
  prompting: false,
});

/** Dernière activité des portées déverrouillées (absente = verrouillée). */
const unlocked = reactive(new Map<string, number>());

export async function refreshHello() {
  const s = await invoke<{ supported: boolean; enrolled: boolean }>("hello_status").catch(() => null);
  if (s) Object.assign(hello, s);
}

/** Lance une invite Hello en tenant le notch ouvert (le dialogue système prend le focus). */
export async function prompt<T>(fn: () => Promise<T>): Promise<T> {
  if (hello.prompting) throw "busy";
  hello.prompting = true;
  const release = hold();
  try {
    return await fn();
  } finally {
    release();
    hello.prompting = false;
  }
}

async function busy(fn: () => Promise<unknown>) {
  if (hello.busy) return;
  hello.busy = true;
  try {
    await fn();
  } finally {
    hello.busy = false;
    await refreshHello();
  }
}

/** Configuration (une fois) : crée la clé Hello d'Island. */
export const enrollHello = () => busy(() => prompt(() => invoke("hello_enroll")));

/** Désactivation : confirme par Hello, puis supprime la clé et coupe les outils branchés. */
export const disableHello = () =>
  busy(async () => {
    await prompt(() => invoke("hello_verify"));
    await invoke("hello_disable");
    for (const k of Object.keys(settings.hello) as HelloScope[]) settings.hello[k] = false;
    unlocked.clear();
  });

/** Active / coupe Hello pour un outil. Couper un verrou demande d'abord Hello. */
export async function setScope(scope: HelloScope, on: boolean) {
  if (on === settings.hello[scope]) return;
  if (!on && scope !== "vault") await prompt(() => invoke("hello_verify"));
  settings.hello[scope] = on;
  if (on) unlocked.delete(scope);
}

// --- Verrous de vue -------------------------------------------------------------------

export function isLocked(scope: HelloScope): boolean {
  return settings.hello[scope] && hello.enrolled && !unlocked.has(scope);
}

export async function unlockScope(scope: HelloScope) {
  await prompt(() => invoke("hello_verify"));
  unlocked.set(scope, Date.now());
}

/** Une seule confirmation Windows Hello pour plusieurs portées verrouillées (ex. export
 *  d'une sauvegarde qui contient les notes et le budget). Rien à faire si aucune ne l'est. */
export async function unlockScopes(scopes: HelloScope[]) {
  const locked = scopes.filter(isLocked);
  if (!locked.length) return;
  await prompt(() => invoke("hello_verify"));
  for (const s of locked) unlocked.set(s, Date.now());
}

export function lockScope(scope: HelloScope) {
  unlocked.delete(scope);
}

/** Activité dans une vue déverrouillée : prolonge le délai. */
export function touchScope(scope: HelloScope) {
  if (unlocked.has(scope)) unlocked.set(scope, Date.now());
}

/** Message d'une erreur Hello (codes du service Rust). */
export function helloError(t: (k: string) => string, e: unknown): string | null {
  const code = String(e).replace(/^hello\./, "");
  if (code === "canceled" || code === "busy") return null;
  const key = `hello.err.${code}`;
  const text = t(key);
  return text === key ? t("hello.err.failed") : text;
}

export function initHello() {
  void refreshHello();
  // Délai d'inactivité (vérifié toutes les 5 s).
  window.setInterval(() => {
    const max = settings.vaultTimeout * 1000;
    for (const [scope, last] of unlocked) if (Date.now() - last > max) unlocked.delete(scope);
  }, 5000);
  watch(presentation, (on) => on && unlocked.clear());
  // Notes verrouillées (délai, mode présentation, verrou manuel) : leurs fenêtres
  // détachées se ferment. L'activité dans une fenêtre de note prolonge le délai.
  watch(
    () => isLocked("notes"),
    (locked) => locked && invoke("window_close_kind", { kind: "note" }).catch(() => {}),
  );
  listen<string>("hello://touch", (e) => touchScope(e.payload as HelloScope));
  // Le Coffre garde (ou non) sa session pour Hello côté Rust.
  watch(
    () => settings.hello.vault && hello.enrolled,
    (enabled) => invoke("vault_hello_set", { enabled }).catch(() => {}),
  );
}
