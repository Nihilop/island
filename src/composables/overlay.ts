// État partagé de l'overlay (fenêtre unique) : view, goutte, modal, régions.
import { ref, reactive, shallowRef, markRaw, type Component } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface Rect { x: number; y: number; w: number; h: number }
export interface ModalSpec { title?: string; subtitle?: string; ui?: any[]; component?: Component }
// Zone haute réservée (poignée de collapse + marge au bord d'écran), 3 états :
//  - relative : réserve la bande en haut, la poignée y vit, le contenu démarre dessous (défaut).
//  - absolute : la poignée FLOTTE par-dessus le contenu (+ scrim) ; le contenu va jusqu'au bord
//    (ex. une view à bannière image — détail animé Aniplex).
//  - hidden   : aucune poignée, aucune réserve (ex. notifications — une notif qui pop n'affiche rien).
export type SafeZone = "relative" | "absolute" | "hidden";
export interface ViewSize {
  width?: number; height?: number; radius?: number; persistent?: boolean;
  safeZone?: SafeZone;
  /** @deprecated alias rétro-compat : true → "relative", false → "absolute". Utiliser `safeZone`. */
  safeArea?: boolean;
}
// Résout l'alias historique `safeArea` vers le nouveau `safeZone`.
function resolveSafeZone(size?: ViewSize): SafeZone {
  if (size?.safeZone) return size.safeZone;
  if (size?.safeArea === false) return "absolute";
  return "relative";
}

// --- View : la surface d'une extension montée DANS l'île (slot générique) ---
export const activeView = shallowRef<Component | null>(null);
export const activeViewSize = ref<{ width: number; height: number; radius: number }>({ width: 440, height: 112, radius: 28 });
// Persistante : reste ouverte malgré un clic hors de l'île / une perte de focus
// (ex. Monitoring : garder les stats visibles en faisant autre chose).
export const activeViewPersistent = ref(false);
// Mode de zone haute de la view active (cf. SafeZone). Défaut "relative".
export const activeViewSafeZone = ref<SafeZone>("relative");
export function openView(component: Component, size?: ViewSize) {
  activeViewSize.value = {
    width: size?.width ?? 440,
    height: size?.height ?? 112,
    radius: size?.radius ?? 28,
  };
  activeViewPersistent.value = size?.persistent ?? false;
  activeViewSafeZone.value = resolveSafeZone(size);
  activeView.value = markRaw(component);
}
export function closeView() { activeView.value = null; activeViewPersistent.value = false; }
// Redimensionne la view ACTIVE sans la remonter (l'île morphe via sa transition CSS).
export function resizeView(size: ViewSize) {
  const cur = activeViewSize.value;
  activeViewSize.value = {
    width: size?.width ?? cur.width,
    height: size?.height ?? cur.height,
    radius: size?.radius ?? cur.radius,
  };
}

// --- Goutte : sous-slot d'une view (rond par défaut, s'élargit selon le contenu) ---
export const dropContent = shallowRef<Component | null>(null);
export function openDrop(component: Component) { dropContent.value = markRaw(component); }
export function closeDrop() { dropContent.value = null; }

// --- Modal partagée (l'île/launcher l'ouvre, le composant Modal la rend) ---
export const modalSpec = ref<ModalSpec | null>(null);
export const isModalOpen = () => modalSpec.value !== null;
// Idempotent : une modal déjà ouverte ne se relance pas (anti double-ouverture).
export function openModal(spec: ModalSpec) {
  if (modalSpec.value !== null) return;
  modalSpec.value = spec;
}
export function closeModal() { modalSpec.value = null; }

// --- Fenêtres flottantes draggables (conteneur "window") --------------------
// 3ᵉ conteneur à côté de `view` (île qui morphe) et `modal` : un panneau libre,
// déplaçable, persistant, qui héberge un composant d'extension (ex. lecteur vidéo).
export interface FloatWindow {
  id: string;
  component: Component;
  title?: string;
  x: number;
  y: number;
  width: number;
  height: number;
  resizable: boolean;
  z: number;
}
export interface WindowOpts {
  id?: string;
  title?: string;
  width?: number;
  height?: number;
  x?: number;
  y?: number;
  resizable?: boolean;
}
export const floatWindows = ref<FloatWindow[]>([]);
let winZ = 50;

export function openWindow(component: Component, opts?: WindowOpts): string {
  const id = opts?.id ?? "win:" + Math.random().toString(36).slice(2, 8);
  const existing = floatWindows.value.find((w) => w.id === id);
  if (existing) { existing.z = ++winZ; return id; } // déjà ouverte → au premier plan
  const width = opts?.width ?? 480;
  const height = opts?.height ?? 320;
  // Centrage sur la taille ÉCRAN (l'overlay passe plein écran de façon async à
  // l'ouverture d'une fenêtre → window.innerWidth peut encore valoir la petite boîte).
  const x = opts?.x ?? Math.max(16, Math.round((window.screen.width - width) / 2));
  const y = opts?.y ?? Math.max(16, Math.round((window.screen.height - height) / 3));
  floatWindows.value.push({
    id, component: markRaw(component), title: opts?.title, x, y, width, height,
    resizable: opts?.resizable ?? false, z: ++winZ,
  });
  return id;
}
export function closeWindow(id?: string) {
  floatWindows.value = id ? floatWindows.value.filter((w) => w.id !== id) : [];
}
export function focusWindow(id: string) {
  const w = floatWindows.value.find((x) => x.id === id);
  if (w) w.z = ++winZ;
}

// --- Sélection de zone à l'écran (primitive hôte) ---------------------------
// L'overlay plein écran capture la souris le temps de la sélection, puis renvoie
// un rectangle en pixels PHYSIQUES relatif au moniteur (prêt pour le crop WGC).
export interface Region { x: number; y: number; w: number; h: number }
export const selecting = ref(false);
let regionResolver: ((r: Region | null) => void) | null = null;

export function selectRegion(): Promise<Region | null> {
  if (selecting.value) return Promise.resolve(null);
  selecting.value = true;
  return new Promise((resolve) => { regionResolver = resolve; });
}
export function finishRegion(r: Region | null) {
  selecting.value = false;
  const fn = regionResolver;
  regionResolver = null;
  fn?.(r);
}

// Contour PERSISTANT d'une zone (ex. pendant un enregistrement). `pointer-events:none`
// → n'intercepte pas la souris ; exclu de la capture (fait partie de l'overlay Island).
export const regionOutline = ref<Region | null>(null);
export function showRegionOutline(r: Region | null) {
  regionOutline.value = r;
}

// --- Régions interactives (click-through géré nativement) ---
// Chaque surface publie son rectangle ; on envoie l'union au Rust, qui décide
// quand la fenêtre overlay laisse passer les clics.
const regions = reactive<Record<string, Rect | null>>({});
let scheduled = false;
function publish() {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    const list = Object.values(regions).filter((r): r is Rect => !!r);
    invoke("set_hit_regions", { regions: list }).catch(() => {});
  });
}
export function setHitRegion(key: string, rect: Rect | null) {
  regions[key] = rect;
  publish();
}
