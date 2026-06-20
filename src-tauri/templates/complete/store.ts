// État partagé entre activate() (index.ts) et les surfaces (View / Config).
// Montre : storage persistant + contribution à l'île en idle + notifications.
import { reactive } from "vue";
import type { ExtensionContext } from "@island/sdk";

export const state = reactive({ count: 0, notifyOnBump: true });

let ctx: ExtensionContext | null = null;

/** Appelé une fois depuis activate() : restaure l'état persistant. */
export async function init(c: ExtensionContext) {
  ctx = c;
  state.count = (await c.storage.get<number>("count", 0)) ?? 0;
  state.notifyOnBump = (await c.storage.get<boolean>("notifyOnBump", true)) ?? true;
  refreshIdle();
}

/** Incrémente le compteur, persiste, met à jour l'île + notifie (si activé). */
export async function bump() {
  if (!ctx) return;
  state.count++;
  await ctx.storage.set("count", state.count);
  refreshIdle();
  if (state.notifyOnBump) {
    ctx.notify({ title: "__EXT_NAME__", body: `Compteur : ${state.count}`, color: "#f59e0b", source: "__EXT_NAME__" });
  }
}

export async function reset() {
  if (!ctx) return;
  state.count = 0;
  await ctx.storage.set("count", 0);
  refreshIdle();
}

export async function setNotify(v: boolean) {
  if (!ctx) return;
  state.notifyOnBump = v;
  await ctx.storage.set("notifyOnBump", v);
}

// Affiche (ou retire) le compteur sur l'extrémité droite de l'île en idle.
function refreshIdle() {
  if (!ctx) return;
  ctx.idle.action("right", state.count ? { text: String(state.count), color: "#f59e0b", priority: 20 } : null);
}
