// Registre de l'île en mode idle.
//  - CENTRE = un ÉTAT (enum géré par l'hôte → rendu cohérent/joli, API légère).
//  - GAUCHE/DROITE = des ACTIONS (icône + onClick, raccourcis conditionnels libres).
//
// Garde-fou anti-race : clé par contributeur (borné), dédoublonnage, coalescing/frame.
import { ref, computed } from "vue";

export type IdleState = "idle" | "playing" | "busy" | "recording";
export interface IdleAction {
  slot: "left" | "right";
  icon?: string; // SVG (string)
  text?: string; // alternative à l'icône : texte court (ex. compteur "00:12")
  color?: string;
  priority?: number;
  onActivate?: () => void;
}

interface StateEntry { state: IdleState; priority: number }

const states = new Map<string, StateEntry>();
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

/** Actions résolues par extrémité — LISTE (cumulables), triées par priorité décroissante. */
export const idleActions = computed(() => {
  void tick.value;
  const bySlot = (slot: "left" | "right") =>
    [...actions.values()].filter((a) => a.slot === slot).sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
  return { left: bySlot("left"), right: bySlot("right") };
});
