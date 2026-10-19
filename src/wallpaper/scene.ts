// Format de scène v1 — jumeau de `src-tauri/src/wallpaper/scene.rs`. `pack` doit rester
// IDENTIQUE à `scene.rs::pack` (même ordre, mêmes bornes) : l'aperçu de l'éditeur et le
// fond natif partagent le même shader (`scene.wgsl`).

export interface Background {
  kind: "image" | "gradient";
  image: string | null;
  fit: "cover" | "contain" | "stretch";
  colors: [string, string];
  angle: number;
}
export interface Water { strength: number; speed: number; scale: number }
export interface Parallax { mouse: number; drift: number; zoom: number }
export type ParticleShape = "dot" | "streak" | "glow";
export interface Particles {
  shape: ParticleShape;
  density: number;
  /** 0..1, échelle exponentielle (~10 → ~1000 px/s). */
  speed: number;
  size: number;
  color: string;
  opacity: number;
  /** Degrés : 90 = vers le bas, 270 = vers le haut. */
  direction: number;
  spread: number;
  turbulence: number;
  flicker: number;
}

/** Préréglages de particules de l'éditeur : de simples jeux de réglages. */
export const PARTICLE_PRESETS: Record<"snow" | "rain" | "fireflies" | "embers", Omit<Particles, "density" | "opacity">> = {
  snow: { shape: "dot", speed: 0.35, size: 1, color: "#ffffff", direction: 90, spread: 0.2, turbulence: 0.4, flicker: 0 },
  rain: { shape: "streak", speed: 0.92, size: 1, color: "#9fb3c8", direction: 100, spread: 0.05, turbulence: 0, flicker: 0 },
  fireflies: { shape: "glow", speed: 0.08, size: 1.6, color: "#7affc8", direction: 270, spread: 0.6, turbulence: 0.9, flicker: 0.6 },
  embers: { shape: "glow", speed: 0.4, size: 1.2, color: "#ffb36b", direction: 265, spread: 0.3, turbulence: 0.6, flicker: 0.9 },
};
export interface ColorAdjust { hue: number; saturation: number; brightness: number; contrast: number }
export interface Ripple { radius: number; strength: number; duration: number }
export interface Trail { width: number; fade: number; color: string; intensity: number }

export interface Scene {
  version: number;
  name: string;
  background: Background;
  effects: {
    water: Water | null;
    parallax: Parallax | null;
    particles: Particles | null;
    color: ColorAdjust | null;
    grain: number | null;
    vignette: number | null;
  };
  interactions: { ripple: Ripple | null; trail: Trail | null };
}

export interface SceneEntry {
  id: string;
  preset: boolean;
  scene: Scene;
  updated: number;
}

/** Valeurs par défaut quand on active un effet (mêmes que `scene.rs`). */
export const DEFAULTS = {
  water: (): Water => ({ strength: 0.4, speed: 0.4, scale: 0.4 }),
  parallax: (): Parallax => ({ mouse: 0.5, drift: 0.3, zoom: 1.06 }),
  particles: (): Particles => ({ ...PARTICLE_PRESETS.snow, density: 0.4, opacity: 0.8 }),
  color: (): ColorAdjust => ({ hue: 0, saturation: 1, brightness: 1, contrast: 1 }),
  ripple: (): Ripple => ({ radius: 260, strength: 0.6, duration: 1.4 }),
  trail: (): Trail => ({ width: 10, fade: 0.5, color: "#7cc8ff", intensity: 0.8 }),
};

export function newScene(name: string): Scene {
  return {
    version: 1,
    name,
    background: { kind: "gradient", image: null, fit: "cover", colors: ["#1b1f3a", "#3a1b4a"], angle: 90 },
    effects: { water: null, parallax: null, particles: null, color: null, grain: null, vignette: null },
    interactions: { ripple: null, trail: null },
  };
}

/** Forme ramenée à dot / streak / glow (le Rust lit l'ancien `kind` — snow, rain… — dans `shape`). */
function oldParticles(p: Particles): Particles {
  return { ...p, shape: (["dot", "streak", "glow"] as const)[shapeCode(p.shape) - 1] };
}

/** Complète une scène lue (champs absents = désactivés), comme `serde(default)`. */
export function normalize(s: Partial<Scene>): Scene {
  const base = newScene(s.name ?? "");
  return {
    ...base,
    ...s,
    background: { ...base.background, ...s.background },
    effects: {
      ...base.effects,
      ...s.effects,
      particles: s.effects?.particles ? oldParticles({ ...DEFAULTS.particles(), ...s.effects.particles }) : null,
    },
    interactions: { ...base.interactions, ...s.interactions },
  } as Scene;
}

// --- Uniformes ---------------------------------------------------------------------------

export const RIPPLES = 8;
export const TRAIL = 32;
export const VEC4S = 15 + RIPPLES + TRAIL;

export function hex(s: string): [number, number, number] {
  const h = s.replace(/^#/, "");
  if (h.length !== 6) return [0, 0, 0];
  return [0, 2, 4].map((i) => (parseInt(h.slice(i, i + 2), 16) || 0) / 255) as [number, number, number];
}

export interface Frame {
  width: number;
  height: number;
  image: [number, number];
  time: number;
  mouse: [number, number] | null;
  ripples: [number, number, number][];
  trail: [number, number, number][];
}

const c = (v: number | undefined | null, lo: number, hi: number) => (Number.isFinite(v) ? Math.min(hi, Math.max(lo, v as number)) : lo);

/** Comme `Particles::shape_code` (Rust) : 1 point, 2 trait, 3 halo. */
function shapeCode(s: string): number {
  return s === "streak" || s === "rain" ? 2 : s === "glow" || s === "fireflies" || s === "embers" ? 3 : 1;
}

export function pack(s: Scene, f: Frame): Float32Array {
  const u = new Float32Array(VEC4S * 4);
  const set = (i: number, v: number[]) => u.set(v, i * 4);
  const b = s.background;
  const fit = b.fit === "contain" ? 1 : b.fit === "stretch" ? 2 : 0;
  const gradient = b.kind !== "image" || !b.image;
  set(0, [f.width, f.height, Math.max(f.image[0], 1), Math.max(f.image[1], 1)]);
  set(1, [f.time, fit, gradient ? 1 : 0, (c(b.angle, -360, 360) * Math.PI) / 180]);
  if (f.mouse) set(2, [f.mouse[0], f.mouse[1], 1, 0]);
  set(3, [...hex(b.colors[0]), 1]);
  set(4, [...hex(b.colors[1]), 1]);
  const e = s.effects;
  if (e.water) set(5, [c(e.water.strength, 0, 1), c(e.water.speed, 0, 1), c(e.water.scale, 0, 1), 0]);
  set(6, e.parallax ? [c(e.parallax.mouse, 0, 1), c(e.parallax.drift, 0, 1), c(e.parallax.zoom, 1, 1.3), 0] : [0, 0, 1, 0]);
  if (e.particles) {
    const p = e.particles;
    set(7, [shapeCode(p.shape), c(p.density, 0, 1), c(p.speed, 0, 1), c(p.size, 0.2, 4)]);
    set(8, [...hex(p.color), c(p.opacity, 0, 1)]);
    set(14, [(c(p.direction, -360, 360) * Math.PI) / 180, c(p.turbulence, 0, 1), c(p.spread, 0, 1), c(p.flicker, 0, 1)]);
  }
  set(9, e.color ? [c(e.color.hue, -180, 180), c(e.color.saturation, 0, 2), c(e.color.brightness, 0, 2), c(e.color.contrast, 0, 2)] : [0, 1, 1, 1]);
  set(10, [c(e.grain ?? 0, 0, 1), c(e.vignette ?? 0, 0, 1), 0, 0]);
  const it = s.interactions;
  if (it.ripple) set(11, [1, c(it.ripple.radius, 20, 1200), c(it.ripple.strength, 0, 2), c(it.ripple.duration, 0.2, 5)]);
  if (it.trail) {
    set(12, [1, c(it.trail.width, 1, 80), c(it.trail.fade, 0.05, 3), 0]);
    set(13, [...hex(it.trail.color), c(it.trail.intensity, 0, 2)]);
  }
  f.ripples.slice(0, RIPPLES).forEach(([x, y, t0], i) => set(15 + i, [x, y, t0, 1]));
  f.trail.slice(0, TRAIL).forEach(([x, y, t], i) => set(15 + RIPPLES + i, [x, y, t, 1]));
  return u;
}
