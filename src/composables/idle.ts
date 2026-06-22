// Registre de l'île en mode idle.
//  - CENTRE = soit un ÉTAT simple (enum hôte → COULEUR du cercle, ex. recording),
//    soit un COMPOSANT custom monté par une extension (viz complexe : wave Spotify,
//    sphère ThreeJS d'une IA vocale…). Le composant prime sur le cercle coloré.
//  - GAUCHE/DROITE = des ACTIONS (icône + onClick, raccourcis conditionnels libres).
//
// Garde-fou anti-race : clé par contributeur (borné), dédoublonnage, coalescing/frame.
import { ref, computed, markRaw, type Component } from "vue";

// États SIMPLES liés aux services hôte → pilotent juste la couleur/forme du cercle central.
// (Une viz riche passe par un composant via `idle.center`, pas par un état.)
export type IdleState = "idle" | "recording";
export interface IdleAction {
  slot: "left" | "right";
  icon?: string; // SVG (string)
  text?: string; // alternative à l'icône : texte court (ex. compteur "00:12")
  color?: string;
  priority?: number;
  onActivate?: () => void;
}

interface StateEntry { state: IdleState; priority: number }

interface CenterEntry { component: Component; priority: number }

const states = new Map<string, StateEntry>();
const centers = new Map<string, CenterEntry>();
const actions = new Map<string, IdleAction>();
// Tap : si un contributeur enregistre un handler, un clic sur l'île en idle
// l'appelle au lieu d'ouvrir le launcher (ex. ouvrir le contrôle d'enregistrement).
const taps = new Map<string, () => void>();
const tick = ref(0);
let dirty = false;
function schedule() {
  if (dirty) return;
  dirty = true;
  requestAnimationFrame(() => { dirty = false; tick.value++; });
}

export function setIdleState(key: string, state: IdleState | null, priority = 10) {
  if (!state) { if (states.delete(key)) schedule(); return; }
  const prev = states.get(key);
  if (prev && prev.state === state && prev.priority === priority) return;
  states.set(key, { state, priority });
  schedule();
}

// Composant central custom (viz d'extension). `null` = retire la contribution.
export function setIdleCenter(key: string, component: Component | null, priority = 10) {
  if (!component) { if (centers.delete(key)) schedule(); return; }
  const prev = centers.get(key);
  if (prev && prev.component === component && prev.priority === priority) return;
  centers.set(key, { component: markRaw(component), priority });
  schedule();
}

export function setIdleAction(key: string, action: IdleAction | null) {
  if (!action) { if (actions.delete(key)) schedule(); return; }
  const p = actions.get(key);
  if (p && p.slot === action.slot && p.icon === action.icon && p.text === action.text && p.color === action.color
      && (p.priority ?? 0) === (action.priority ?? 0) && p.onActivate === action.onActivate) return;
  actions.set(key, action);
  schedule();
}

export function setIdleTap(key: string, handler: (() => void) | null) {
  if (!handler) { if (taps.delete(key)) schedule(); return; }
  taps.set(key, handler);
  schedule();
}

/** Handler de clic-île actif (le dernier enregistré), ou null. */
export const idleTap = computed<(() => void) | null>(() => {
  void tick.value;
  const arr = [...taps.values()];
  return arr.length ? arr[arr.length - 1] : null;
});

/** État central résolu (plus haute priorité, défaut "idle"). */
export const idleState = computed<IdleState>(() => {
  void tick.value;
  let best: StateEntry | undefined;
  for (const s of states.values()) if (!best || s.priority > best.priority) best = s;
  return best?.state ?? "idle";
});

/** Composant central custom résolu (plus haute priorité), ou null → cercle coloré par état. */
export const idleCenter = computed<Component | null>(() => {
  void tick.value;
  let best: CenterEntry | undefined;
  for (const c of centers.values()) if (!best || c.priority > best.priority) best = c;
  return best?.component ?? null;
});

/** Actions résolues par extrémité — LISTE (cumulables), triées par priorité décroissante. */
export const idleActions = computed(() => {
  void tick.value;
  const bySlot = (slot: "left" | "right") =>
    [...actions.values()].filter((a) => a.slot === slot).sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
  return { left: bySlot("left"), right: bySlot("right") };
});
