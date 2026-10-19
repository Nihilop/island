// Format de scène à calques — jumeau de `src-tauri/src/wallpaper/scene.rs`. `layerBox`,
// `packLayer` et `packComposite` doivent rester IDENTIQUES à leurs versions Rust (même
// ordre, mêmes bornes, mêmes calculs) : l'aperçu de l'éditeur et le fond natif partagent
// les mêmes shaders (`layer.wgsl`, `composite.wgsl`).

export type LayerKind = "image" | "fill" | "particles";
export type Blend = "normal" | "add" | "screen" | "multiply";
export type Fit = "cover" | "contain" | "stretch" | "free";
export type Anchor = "tl" | "t" | "tr" | "l" | "c" | "r" | "bl" | "b" | "br";
export type ParticleShape = "dot" | "streak" | "glow";

export interface Fill { mode: "solid" | "gradient"; colors: [string, string]; angle: number }
export interface Transform { fit: Fit; anchor: Anchor; x: number; y: number; width: number; rotation: number }
export interface Water { strength: number; speed: number; scale: number; mask: string | null }
export interface Heat { strength: number; speed: number; scale: number; direction: number; mask: string | null }
export interface Parallax { mouse: number; drift: number; zoom: number }
export interface ColorAdjust { hue: number; saturation: number; brightness: number; contrast: number }
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
export interface Ripple { radius: number; strength: number; duration: number }
export interface Trail { width: number; fade: number; color: string; intensity: number }

export interface Layer {
  id: string;
  name: string;
  kind: LayerKind;
  visible: boolean;
  locked: boolean;
  opacity: number;
  blend: Blend;
  depth: number;
  image: string | null;
  /** Masque du calque (zone visible ; zone d'apparition pour des particules). */
  mask: string | null;
  fill: Fill;
  particles: Particles | null;
  transform: Transform;
  effects: { water: Water | null; heat: Heat | null; color: ColorAdjust | null };
}

export interface Scene {
  version: number;
  name: string;
  /** Du bas vers le haut. */
  layers: Layer[];
  parallax: Parallax | null;
  color: ColorAdjust | null;
  grain: number | null;
  vignette: number | null;
  interactions: { ripple: Ripple | null; trail: Trail | null };
}

export interface SceneEntry {
  id: string;
  preset: boolean;
  scene: Scene;
  updated: number;
}

export const MAX_LAYERS = 16;

/** Préréglages de particules de l'éditeur : de simples jeux de réglages. */
export const PARTICLE_PRESETS: Record<"snow" | "rain" | "fireflies" | "embers", Omit<Particles, "density" | "opacity">> = {
  snow: { shape: "dot", speed: 0.35, size: 1, color: "#ffffff", direction: 90, spread: 0.2, turbulence: 0.4, flicker: 0 },
  rain: { shape: "streak", speed: 0.92, size: 1, color: "#9fb3c8", direction: 100, spread: 0.05, turbulence: 0, flicker: 0 },
  fireflies: { shape: "glow", speed: 0.08, size: 1.6, color: "#7affc8", direction: 270, spread: 0.6, turbulence: 0.9, flicker: 0.6 },
  embers: { shape: "glow", speed: 0.4, size: 1.2, color: "#ffb36b", direction: 265, spread: 0.3, turbulence: 0.6, flicker: 0.9 },
};

/** Valeurs par défaut quand on active un effet (mêmes que `scene.rs`). */
export const DEFAULTS = {
  water: (): Water => ({ strength: 0.4, speed: 0.4, scale: 0.4, mask: null }),
  heat: (): Heat => ({ strength: 0.5, speed: 0.5, scale: 0.5, direction: 270, mask: null }),
  parallax: (): Parallax => ({ mouse: 0.5, drift: 0.3, zoom: 1.06 }),
  particles: (): Particles => ({ ...PARTICLE_PRESETS.snow, density: 0.4, opacity: 0.8 }),
  color: (): ColorAdjust => ({ hue: 0, saturation: 1, brightness: 1, contrast: 1 }),
  ripple: (): Ripple => ({ radius: 260, strength: 0.6, duration: 1.4 }),
  trail: (): Trail => ({ width: 10, fade: 0.5, color: "#7cc8ff", intensity: 0.8 }),
  transform: (): Transform => ({ fit: "cover", anchor: "c", x: 0, y: 0, width: 0.3, rotation: 0 }),
  fill: (): Fill => ({ mode: "gradient", colors: ["#1b1f3a", "#3a1b4a"], angle: 90 }),
};

export const layerId = () => `l-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;

export function newLayer(kind: LayerKind, name: string): Layer {
  return {
    id: layerId(),
    name,
    kind,
    visible: true,
    locked: false,
    opacity: 1,
    blend: "normal",
    depth: kind === "particles" ? 0.5 : 0,
    image: null,
    mask: null,
    fill: DEFAULTS.fill(),
    particles: kind === "particles" ? DEFAULTS.particles() : null,
    transform: DEFAULTS.transform(),
    effects: { water: null, heat: null, color: null },
  };
}

export function newScene(name: string, background: string): Scene {
  return { version: 1, name, layers: [newLayer("fill", background)], parallax: null, color: null, grain: null, vignette: null, interactions: { ripple: null, trail: null } };
}

/** Complète une scène lue (champs absents = valeurs par défaut), comme `serde(default)`. */
export function normalize(s: Partial<Scene>): Scene {
  const layers = (Array.isArray(s.layers) ? s.layers : []).map((l) => {
    const base = newLayer((l.kind as LayerKind) ?? "fill", l.name ?? "");
    return {
      ...base,
      ...l,
      fill: { ...base.fill, ...l.fill },
      transform: { ...base.transform, ...l.transform },
      particles: l.particles ? { ...DEFAULTS.particles(), ...l.particles } : base.particles,
      effects: {
        water: l.effects?.water ? { ...DEFAULTS.water(), ...l.effects.water } : null,
        heat: l.effects?.heat ? { ...DEFAULTS.heat(), ...l.effects.heat } : null,
        color: l.effects?.color ?? null,
      },
    } as Layer;
  });
  return {
    version: 1,
    name: s.name ?? "",
    layers,
    parallax: s.parallax ?? null,
    color: s.color ?? null,
    grain: s.grain ?? null,
    vignette: s.vignette ?? null,
    interactions: { ripple: null, trail: null, ...s.interactions },
  };
}

// --- Placement et uniformes ----------------------------------------------------------------

export const RIPPLES = 8;
export const TRAIL = 32;
export const LAYER_VEC4S = 12;
export const COMPOSITE_VEC4S = 7 + RIPPLES + TRAIL;

export function hex(s: string): [number, number, number] {
  const h = s.replace(/^#/, "");
  if (h.length !== 6) return [0, 0, 0];
  return [0, 2, 4].map((i) => (parseInt(h.slice(i, i + 2), 16) || 0) / 255) as [number, number, number];
}

export interface Frame {
  width: number;
  height: number;
  time: number;
  mouse: [number, number] | null;
  ripples: [number, number, number][];
  trail: [number, number, number][];
}

const c = (v: number | undefined | null, lo: number, hi: number) => (Number.isFinite(v) ? Math.min(hi, Math.max(lo, v as number)) : lo);
const rad = (d: number) => (d * Math.PI) / 180;

export function anchor(a: string): [number, number] {
  return [a.endsWith("l") ? 0 : a.endsWith("r") ? 1 : 0.5, a.startsWith("t") ? 0 : a.startsWith("b") ? 1 : 0.5];
}

/** (centre x, centre y, demi-largeur, demi-hauteur) en px — comme `scene.rs::layer_box`. */
export function layerBox(s: Scene, l: Layer, f: Frame, tex: [number, number]): [number, number, number, number] {
  const { width: w, height: h } = f;
  const depth = c(l.depth, 0, 1);
  let ox = 0;
  let oy = 0;
  let zoom = 1;
  if (s.parallax) {
    const drift = c(s.parallax.drift, 0, 1);
    const mouse = c(s.parallax.mouse, 0, 1);
    ox = Math.sin(f.time * 0.11) * drift * 0.012;
    oy = Math.cos(f.time * 0.083) * drift * 0.012;
    if (f.mouse) {
      ox += (f.mouse[0] / w - 0.5) * mouse * 0.04;
      oy += (f.mouse[1] / h - 0.5) * mouse * 0.04;
    }
    ox *= w * depth;
    oy *= h * depth;
    zoom = 1 + (c(s.parallax.zoom, 1, 1.3) - 1) * depth;
  }
  const iw = Math.max(tex[0], 1);
  const ih = Math.max(tex[1], 1);
  const t = l.transform;
  if (l.kind === "image" && (t.fit === "cover" || t.fit === "contain")) {
    const k = (t.fit === "cover" ? Math.max(w / iw, h / ih) : Math.min(w / iw, h / ih)) * zoom;
    return [w * 0.5 + ox, h * 0.5 + oy, iw * k * 0.5, ih * k * 0.5];
  }
  if (l.kind === "image" && t.fit === "free") {
    const hw = c(t.width, 0.01, 4) * w * 0.5 * zoom;
    const hh = (hw * ih) / iw;
    const [ax, ay] = anchor(t.anchor);
    const cx = ax * w + c(t.x, -2, 2) * w + (0.5 - ax) * 2 * hw;
    const cy = ay * h + c(t.y, -2, 2) * h + (0.5 - ay) * 2 * hh;
    return [cx + ox, cy + oy, hw, hh];
  }
  return [w * 0.5 + ox, h * 0.5 + oy, w * 0.5 * zoom, h * 0.5 * zoom];
}

function colorAdjust(a: ColorAdjust | null): number[] {
  return a ? [c(a.hue, -180, 180), c(a.saturation, 0, 2), c(a.brightness, 0, 2), c(a.contrast, 0, 2)] : [0, 1, 1, 1];
}

export function packLayer(s: Scene, l: Layer, f: Frame, tex: [number, number]): Float32Array {
  const u = new Float32Array(LAYER_VEC4S * 4);
  const set = (i: number, v: number[]) => u.set(v, i * 4);
  const kind = l.kind === "image" && l.image ? 0 : l.kind === "particles" && l.particles ? 3 : l.kind === "fill" && l.fill.mode === "solid" ? 1 : 2;
  const edges = l.transform.fit === "cover" || l.transform.fit === "stretch" ? 1 : 0;
  const rotation = l.transform.fit === "free" ? rad(c(l.transform.rotation, -360, 360)) : 0;
  set(0, [f.width, f.height, Math.max(tex[0], 1), Math.max(tex[1], 1)]);
  set(1, [f.time, kind, c(l.opacity, 0, 1), edges]);
  set(2, layerBox(s, l, f, tex));
  set(3, [rotation, rad(c(l.fill.angle, -360, 360)), f.height / 1080, 0]);
  set(4, [...hex(l.fill.colors[0]), 1]);
  set(5, [...hex(l.fill.colors[1]), 1]);
  const wt = l.effects.water;
  if (wt) set(6, [c(wt.strength, 0, 1), c(wt.speed, 0, 1), c(wt.scale, 0, 1), 0]);
  set(7, colorAdjust(l.effects.color));
  const p = l.particles;
  if (p) {
    set(8, [p.shape === "streak" ? 2 : p.shape === "glow" ? 3 : 1, c(p.density, 0, 1), c(p.speed, 0, 1), c(p.size, 0.2, 4)]);
    set(9, [...hex(p.color), c(p.opacity, 0, 1)]);
    set(10, [rad(c(p.direction, -360, 360)), c(p.turbulence, 0, 1), c(p.spread, 0, 1), c(p.flicker, 0, 1)]);
  }
  const ht = l.effects.heat;
  if (ht) set(11, [c(ht.strength, 0, 1), c(ht.speed, 0, 1), c(ht.scale, 0, 1), rad(c(ht.direction, -360, 360))]);
  return u;
}

/** Masques d'un calque, dans l'ordre des liaisons du shader (calque, eau, chaleur). */
export const maskNames = (l: Layer): [string | null, string | null, string | null] => [l.mask, l.effects.water?.mask ?? null, l.effects.heat?.mask ?? null];

export function packComposite(s: Scene, f: Frame): Float32Array {
  const u = new Float32Array(COMPOSITE_VEC4S * 4);
  const set = (i: number, v: number[]) => u.set(v, i * 4);
  set(0, [f.width, f.height, 0, 0]);
  set(1, [f.time, f.height / 1080, 0, 0]);
  set(2, colorAdjust(s.color));
  set(3, [c(s.grain ?? 0, 0, 1), c(s.vignette ?? 0, 0, 1), 0, 0]);
  const it = s.interactions;
  if (it.ripple) set(4, [1, c(it.ripple.radius, 20, 1200), c(it.ripple.strength, 0, 2), c(it.ripple.duration, 0.2, 5)]);
  if (it.trail) {
    set(5, [1, c(it.trail.width, 1, 80), c(it.trail.fade, 0.05, 3), 0]);
    set(6, [...hex(it.trail.color), c(it.trail.intensity, 0, 2)]);
  }
  f.ripples.slice(0, RIPPLES).forEach(([x, y, t0], i) => set(7 + i, [x, y, t0, 1]));
  f.trail.slice(0, TRAIL).forEach(([x, y, t], i) => set(7 + RIPPLES + i, [x, y, t, 1]));
  return u;
}

export const BLENDS: Blend[] = ["normal", "add", "screen", "multiply"];
export const blendIndex = (b: Blend) => Math.max(0, BLENDS.indexOf(b));
