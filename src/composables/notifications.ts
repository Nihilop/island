// Centre de notifications — service hôte.
//  - `stack` : pile transitoire de bannières (l'île se déplie, récentes en haut).
//    Minuteur RESET à chaque notif → un burst reste affiché puis se rétracte.
//  - `unread` : inbox des non-lues. Tant qu'il y en a, une CLOCHE s'affiche à
//    droite de l'île ; ouvrir le centre les lit → elles disparaissent pour de bon.
import { ref, shallowRef, computed } from "vue";
import { listen } from "@tauri-apps/api/event";

export interface NotifAction { id?: string; label: string; onClick?: () => void }
export interface NotifSpec {
  title: string;
  body?: string;
  icon?: string; // SVG (string)
  color?: string;
  source?: string;
  timeout?: number; // 0 = pas de bannière (va direct en non-lue)
  actions?: NotifAction[];
  onClick?: () => void;
}
export interface Notif extends NotifSpec { id: string; ts: number }

export const stack = ref<Notif[]>([]); // bannières affichées
export const unread = ref<Notif[]>([]); // non-lues (alimente la cloche + le centre)
export const unreadCount = computed(() => unread.value.length);
export const lastPosted = shallowRef<Notif | null>(null); // pour la gouttelette DND

// Do Not Disturb : aucune bannière ; les notifs filent en non-lues (+ gouttelette).
let dndMode = false;
export function setDnd(v: boolean) {
  dndMode = v;
  if (v) clearStack(); // coupe une éventuelle bannière en cours
}

const HOLD = 4500;
let timer = 0;
let remaining = 0;
let startedAt = 0;
let paused = false;
let seq = 0;

function arm(ms: number) {
  window.clearTimeout(timer);
  remaining = ms;
  startedAt = Date.now();
  paused = false;
  timer = window.setTimeout(() => (stack.value = []), ms);
}

export function post(spec: NotifSpec): string {
  const n: Notif = { ...spec, id: `n${++seq}`, ts: Date.now() };
  unread.value = [n, ...unread.value].slice(0, 100); // récentes en haut
  lastPosted.value = n; // signale l'arrivée (gouttelette en DND)
  if (!dndMode && (spec.timeout ?? HOLD) !== 0) {
    stack.value = [n, ...stack.value];
    arm(HOLD); // reset à chaque nouvelle notif
  }
  return n.id;
}

export function pauseStack() {
  if (paused || !stack.value.length) return;
  paused = true;
  window.clearTimeout(timer);
  remaining -= Date.now() - startedAt;
}
export function resumeStack() {
  if (!paused || !stack.value.length) return;
  arm(remaining > 400 ? remaining : 1500);
}
export function clearStack() {
  window.clearTimeout(timer);
  stack.value = [];
}

/** Marque une notif comme lue → la retire (bannière + inbox). */
export function markRead(id: string) {
  stack.value = stack.value.filter((n) => n.id !== id);
  unread.value = unread.value.filter((n) => n.id !== id);
}
/** Tout marquer lu (ex. à la fermeture du centre) → disparaissent pour de bon. */
export function clearUnread() {
  unread.value = [];
  clearStack();
}

let inited = false;
export function initNotifications() {
  if (inited) return;
  inited = true;
  listen<NotifSpec>("notify://post", (e) => post(e.payload));
}
