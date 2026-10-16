// Moteur du ciel météo : un seul shader WebGL plein cadre (aucune bibliothèque). Couches,
// de l'arrière vers l'avant : dégradé du ciel → étoiles → soleil / lune → deux couches de
// nuages (fbm à domaine déformé, éclairés par le dessus) → brouillard → pluie (3 plans,
// inclinée par le vent) ou neige (3 plans, flocons qui ondulent) → éclair → brume de chaleur.
//
// Les paramètres (couleurs, densités) se calculent en JS à partir de l'ambiance et glissent
// vers leur cible à chaque image : changer d'ambiance donne un fondu de ~1 s. Rendu plafonné
// à 30 images/s, densité de pixels ≤ 1,5. Animations réduites : une image fixe.

import type { Mood, Phase, Sky } from "./sky";

type RGB = [number, number, number];
const hex = (h: string): RGB => [parseInt(h.slice(1, 3), 16) / 255, parseInt(h.slice(3, 5), 16) / 255, parseInt(h.slice(5, 7), 16) / 255];
const mix = (a: RGB, b: RGB, t: number): RGB => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const scale = (a: RGB, k: number): RGB => [a[0] * k, a[1] * k, a[2] * k];
const clamp = (n: number, a = 0, b = 1) => Math.min(b, Math.max(a, n));

// --- Palettes -------------------------------------------------------------------------------

/** Ciel dégagé par moment : haut, milieu, horizon. */
const CLEAR: Record<Phase, [string, string, string]> = {
  night: ["#03050d", "#0a1330", "#1b2b55"],
  dawn: ["#17204a", "#6c4a80", "#f39a6a"],
  day: ["#0b4699", "#2b78cc", "#86bde9"],
  dusk: ["#111641", "#6a3166", "#f2744a"],
};

/** Voile de l'ambiance sur le ciel : couleur (jour, nuit) et force. */
const VEIL: Record<Mood, { day: string; night: string; k: number }> = {
  clear: { day: "#000000", night: "#000000", k: 0 },
  partly: { day: "#5d6b7d", night: "#0d111a", k: 0.12 },
  cloudy: { day: "#4d5868", night: "#10141c", k: 0.6 },
  fog: { day: "#8b939e", night: "#22272f", k: 0.7 },
  drizzle: { day: "#4a5564", night: "#0e121a", k: 0.62 },
  rain: { day: "#2c3542", night: "#090c12", k: 0.75 },
  storm: { day: "#1d1b2b", night: "#060609", k: 0.86 },
  snow: { day: "#8c98aa", night: "#1d2434", k: 0.62 },
  heat: { day: "#d9924a", night: "#2a1a1a", k: 0.55 },
};

/** Nuages : face éclairée et ombre par moment. */
const CLOUD_LIT: Record<Phase, string> = { night: "#2f3a52", dawn: "#f7c6ae", day: "#f6f8fc", dusk: "#f2ae94" };
const CLOUD_SHADE: Record<Phase, string> = { night: "#0c1120", dawn: "#6a587a", day: "#a5b1c3", dusk: "#563c5d" };
/** Assombrissement des nuages par ambiance. */
const CLOUD_DARK: Record<Mood, number> = { clear: 0, heat: 0, partly: 0, cloudy: 0.22, fog: 0.08, drizzle: 0.38, rain: 0.55, snow: 0.12, storm: 0.78 };
const COVER: Record<Mood, (i: number) => number> = {
  clear: (i) => 0.06 + i * 0.12,
  heat: () => 0.04,
  partly: (i) => 0.34 + i * 0.26,
  cloudy: (i) => 0.72 + i * 0.22,
  fog: () => 0.45,
  drizzle: (i) => 0.74 + i * 0.16,
  rain: (i) => 0.84 + i * 0.14,
  storm: () => 1,
  snow: (i) => 0.7 + i * 0.2,
};
const SUN_VIS: Record<Mood, number> = { clear: 1, heat: 1, partly: 0.95, cloudy: 0.3, fog: 0.4, drizzle: 0.18, rain: 0.06, storm: 0, snow: 0.22 };
const SUN_COL: Record<Phase, string> = { night: "#dfe6ff", dawn: "#ffad78", day: "#fff0cc", dusk: "#ff8550" };

/** Valeurs envoyées au shader (toutes interpolées). */
interface Params {
  top: RGB;
  mid: RGB;
  bot: RGB;
  lit: RGB;
  shade: RGB;
  sunCol: RGB;
  sunX: number;
  sunY: number;
  sun: number;
  moon: number;
  sunR: number;
  stars: number;
  cover: number;
  rain: number;
  snow: number;
  fog: number;
  heat: number;
  storm: number;
  wind: number;
  light: number;
}

export type SkyParams = Params;

export function targetFor(sky: Sky, light: boolean): Params {
  const { mood, phase, intensity: i } = sky;
  const night = phase === "night";
  const veil = VEIL[mood];
  const k = veil.k * (0.65 + 0.35 * i);
  const veilCol = hex(night ? veil.night : veil.day);
  let [top, mid, bot] = CLEAR[phase].map(hex) as [RGB, RGB, RGB];
  top = mix(top, scale(veilCol, 0.8), k);
  mid = mix(mid, veilCol, k);
  bot = mix(bot, mood === "heat" && !night ? hex("#f5c27c") : scale(veilCol, 1.12), k * (mood === "heat" ? 1 : 0.92));

  const dark = CLOUD_DARK[mood] * (0.7 + 0.3 * i);
  const shadeBase = hex(CLOUD_SHADE[phase]);
  const lit = mix(hex(CLOUD_LIT[phase]), scale(shadeBase, 0.85), dark);
  const shade = scale(shadeBase, 1 - dark * 0.6);

  // Course : arc de gauche (lever) à droite (coucher), point haut à midi.
  const a = clamp(sky.arc);
  const sunX = 0.14 + a * 0.72;
  const sunY = 0.42 + Math.sin(a * Math.PI) * (phase === "day" ? 0.36 : 0.26);
  const vis = SUN_VIS[mood];

  let p: Params = {
    top,
    mid,
    bot,
    lit,
    shade,
    sunCol: hex(mood === "heat" && !night ? "#ffd896" : SUN_COL[phase]),
    sunX,
    sunY,
    sun: night ? 0 : vis * (phase === "day" ? 1 : 0.92),
    moon: night ? vis : 0,
    sunR: mood === "heat" ? 0.085 : phase === "day" ? 0.062 : 0.074,
    stars: night ? (mood === "clear" ? 1 : mood === "partly" ? 0.65 : mood === "heat" ? 0.8 : 0.08) : phase === "dawn" || phase === "dusk" ? 0.12 * vis : 0,
    cover: COVER[mood](i),
    rain: mood === "drizzle" ? 0.2 + 0.2 * i : mood === "rain" ? 0.42 + 0.58 * i : mood === "storm" ? 0.7 + 0.3 * i : 0,
    snow: mood === "snow" ? 0.3 + 0.7 * i : 0,
    fog: mood === "fog" ? 0.5 + 0.5 * i : mood === "drizzle" ? 0.16 : mood === "rain" ? 0.1 : mood === "snow" ? 0.12 : mood === "cloudy" ? 0.05 : 0,
    heat: mood === "heat" && !night ? 0.4 + 0.6 * i : 0,
    storm: mood === "storm" ? 0.5 + 0.5 * i : 0,
    wind: clamp(sky.wind + (mood === "storm" ? 0.3 : 0)),
    light: light ? 1 : 0,
  };
  // Thème clair : même ciel, éclairci (le texte du premier plan est sombre).
  if (light) {
    const w: RGB = [1, 1, 1];
    p = { ...p, top: mix(p.top, w, 0.42), mid: mix(p.mid, w, 0.4), bot: mix(p.bot, w, 0.36), lit: mix(p.lit, w, 0.25), shade: mix(p.shade, w, 0.35) };
  }
  return p;
}

// --- Shader ---------------------------------------------------------------------------------

const VERT = `attribute vec2 a;void main(){gl_Position=vec4(a,0.,1.);}`;

const FRAG = /* glsl */ `
precision highp float;
uniform vec2 uRes;
uniform float uTime;
uniform vec3 uTop, uMid, uBot, uLit, uShade, uSunCol;
uniform vec2 uSunP;
uniform float uSun, uMoon, uSunR, uStars, uCover, uRain, uSnow, uFog, uHeat, uWind, uLight;
uniform float uFlash, uBolt, uBoltSeed;

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3. - 2. * f);
  return mix(mix(hash(i), hash(i + vec2(1., 0.)), u.x), mix(hash(i + vec2(0., 1.)), hash(i + vec2(1., 1.)), u.x), u.y);
}
/** fbm basse fréquence (3 octaves) : modelé des nuages, sans grain. */
float fbm3(vec2 p) {
  float v = 0., a = .5;
  mat2 m = mat2(1.6, 1.2, -1.2, 1.6);
  for (int i = 0; i < 3; i++) { v += a * noise(p); p = m * p; a *= .5; }
  return v;
}
float fbm(vec2 p) {
  float v = 0., a = .5;
  mat2 m = mat2(1.6, 1.2, -1.2, 1.6);
  for (int i = 0; i < 5; i++) { v += a * noise(p); p = m * p; a *= .5; }
  return v;
}

void main() {
  vec2 uv = gl_FragCoord.xy / uRes;          // 0..1, y vers le haut
  float asp = uRes.x / uRes.y;
  vec2 p = vec2(uv.x * asp, uv.y);           // espace isotrope (hauteur = 1)

  // Brume de chaleur : l'air ondule près de l'horizon.
  if (uHeat > .01) {
    float hz = smoothstep(.6, 0., uv.y) * uHeat;
    p.x += sin(p.y * 70. + uTime * 3.2) * .0028 * hz;
    p.y += sin(p.x * 45. - uTime * 2.4) * .0022 * hz;
  }

  // Ciel.
  vec3 col = mix(uBot, uMid, smoothstep(0., .55, uv.y));
  col = mix(col, uTop, smoothstep(.42, 1., uv.y));

  // Étoiles (scintillent, s'effacent vers l'horizon).
  if (uStars > .01) {
    vec2 g = p * 55.;
    vec2 id = floor(g);
    float h = hash(id);
    if (h > .962) {
      vec2 o = (vec2(hash(id + 1.3), hash(id + 2.7)) - .5) * .7;
      float d = length(fract(g) - .5 - o) * uRes.y / 55.;
      float tw = .55 + .45 * sin(uTime * (.6 + h * 2.4) + h * 60.);
      float s = (smoothstep(1.7, 0., d) + exp(-d * 1.2) * .25) * (.45 + .55 * (h - .962) / .038) * tw;
      col += vec3(.88, .92, 1.) * s * uStars * smoothstep(.12, .6, uv.y);
    }
  }

  // Soleil : halo large + cœur + disque ; rayons très doux.
  vec2 sp = vec2(uSunP.x * asp, uSunP.y);
  float d = length(p - sp);
  if (uSun > .01) {
    col += uSunCol * (exp(-d * d * 16.) * .5 + exp(-d * 3.) * .22) * uSun;
    float ang = atan(p.y - sp.y, p.x - sp.x);
    col += uSunCol * uSun * .05 * pow(max(0., sin(ang * 9. + uTime * .12)), 6.) * exp(-d * 2.2);
    float disc = smoothstep(uSunR, uSunR - 3. / uRes.y, d);
    col = mix(col, mix(uSunCol, vec3(1.), .6), disc * uSun);
  }
  // Lune : croissant éclairé, léger relief.
  if (uMoon > .01) {
    float r = uSunR * .78;
    vec3 mc = vec3(.92, .94, 1.) * (.9 + .1 * noise(p * 70.));
    col += vec3(.75, .82, 1.) * (exp(-d * d * 40.) * .2 + exp(-d * 6.) * .07) * uMoon;
    float disc = smoothstep(r, r - 2.5 / uRes.y, d);
    float shadow = smoothstep(r * 1.02, r * .9, length(p - sp - vec2(r * .48, r * .22)));
    col = mix(col, mix(mc, col * 1.08, shadow * .92), disc * uMoon);
  }

  // Nuages : couche lointaine (petite, lente, translucide) puis proche (grande, rapide).
  if (uCover > .01) {
    // Plafond nuageux : au-delà de ~60 % de couverture, une nappe continue (le ciel
    // disparaît), modelée en grandes masses lentes.
    float deck = smoothstep(.55, .95, uCover);
    if (deck > .001) {
      float dn = fbm(p * vec2(1.1, 2.) + vec2(uTime * (.01 + .03 * uWind), 0.));
      vec3 dc = mix(uShade, uLit, .35 + .45 * dn + .15 * uv.y);
      col = mix(col, dc, deck * (.82 + .18 * uv.y));
    }
    // Lumière venue d'en haut, légèrement inclinée vers le soleil (direction lisse partout).
    vec2 lightDir = normalize(vec2(clamp((sp.x - p.x) * .6, -.6, .6), 1.));
    for (int i = 0; i < 2; i++) {
      float fi = float(i);
      float sc = 2.3 - fi * 1.05;
      vec2 q = p * vec2(sc, sc * 1.8);
      q.x += uTime * (.016 + .055 * uWind) * (1. + fi * .9) + fi * 13.7;
      vec2 w = vec2(fbm(q * .55 + vec2(0., uTime * .012)), fbm(q * .55 + vec2(5.2, 1.3 - uTime * .008)));
      float n = fbm(q + w * .55);
      float th = mix(.8, .3, uCover);
      float c = smoothstep(th, th + .2 + .1 * fi, n);
      c *= mix(smoothstep(.1, .65, uv.y), 1., uCover * uCover);
      vec2 qw = q + w * .55;
      float lit = clamp(.58 + (fbm3(qw + lightDir * .14) - fbm3(qw)) * 3.2 + (uv.y - .5) * .25, 0., 1.);
      vec3 cc = mix(uShade, uLit, lit);
      cc += uSunCol * (uSun + uMoon * .4) * .32 * exp(-length(p - sp) * 3.2) * (1. - c * .4);
      col = mix(col, cc, c * (fi < .5 ? .72 : .94));
    }
  }

  // Brouillard : nappes horizontales qui glissent, plus denses en bas.
  if (uFog > .01) {
    float f1 = fbm(vec2(p.x * 1.3 - uTime * .035 * (1. + uWind), p.y * 4. + sin(p.x * .7 + uTime * .05)));
    float f2 = fbm(vec2(p.x * 2.4 + uTime * .05, p.y * 7. - 3.));
    vec3 fc = mix(uLit, uShade, .3);
    float band = smoothstep(1.05, .05, uv.y);
    col = mix(col, fc, clamp(uFog * (.2 + 1.1 * f1 * f2) * band + uFog * .12, 0., .92));
  }

  // Pluie : traînées inclinées par le vent, 3 plans (loin = fin et lent).
  if (uRain > .01) {
    vec3 rc = mix(vec3(.8, .87, 1.), vec3(.2, .28, .4), uLight);
    float slant = .1 + .5 * uWind;
    vec2 r = vec2(p.x + p.y * slant, p.y);
    for (int i = 0; i < 3; i++) {
      float fi = float(i);
      float cols = 46. - fi * 12.;
      vec2 st = vec2(r.x * cols, r.y * (3. - fi * .55) + uTime * (4.6 + fi * 1.9) * (1. + uWind * .5));
      float cx = floor(st.x);
      st.y += hash(vec2(cx, fi * 17.)) * 20.;
      float seg = floor(st.y);
      float fy = fract(st.y);
      float on = step(1. - uRain * (.5 + fi * .16), hash(vec2(cx, seg)));
      float fx = fract(st.x) - .5 - (hash(vec2(cx, seg + 3.)) - .5) * .5;
      float px = abs(fx) * uRes.y / cols;
      float line = smoothstep(.55 + fi * .45, 0., px);
      float streak = smoothstep(0., .06, fy) * smoothstep(.5 - fi * .05, .08, fy);
      col = mix(col, rc, line * streak * on * (.2 + fi * .17));
    }
    vec3 mist = mix(uShade, uLit, .45);
    col = mix(col, mist, uRain * .22 * smoothstep(.42, 0., uv.y) * (.55 + .45 * fbm(vec2(p.x * 3. + uTime * .25, uv.y * 4.))));
  }

  // Neige : flocons ronds sur 3 plans, dérive avec le vent et ondulation.
  if (uSnow > .01) {
    for (int i = 0; i < 3; i++) {
      float fi = float(i);
      float sc = 9. - fi * 2.6;
      vec2 s = p * sc;
      s.x += uTime * (.12 + uWind * .9) * (1. + fi * .5);
      s.y += uTime * (.32 + fi * .16) * (1. + uWind * .3);
      vec2 id = floor(s);
      float h = hash(id + fi * 7.1);
      if (h < uSnow * (.5 + fi * .14)) {
        vec2 o = (vec2(hash(id + 3.1), hash(id + 5.7)) - .5) * .5;
        o.x += sin(uTime * (.8 + h * 1.6) + h * 6.283) * .18;
        float rr = .035 + .05 * hash(id + 9.3);
        float dd = length(fract(s) - .5 - o);
        float flake = smoothstep(rr, rr * .25, dd);
        float glow = exp(-dd * dd / (rr * rr * 5.)) * .22;
        vec3 fcol = mix(vec3(.97, .98, 1.), vec3(1.), fi * .5);
        col = mix(col, fcol, clamp(flake * (.5 + fi * .22) + glow, 0., 1.));
      }
    }
  }

  // Éclair : flash global puis arc ramifié du haut vers le milieu.
  if (uFlash > .001) {
    col += vec3(.6, .64, .86) * uFlash * (.22 + .5 * smoothstep(0., 1., uv.y));
    if (uBolt >= 0.) {
      float y = uv.y;
      float x = uBolt + (fbm(vec2(y * 5., uBoltSeed)) - .5) * .24 + (noise(vec2(y * 38., uBoltSeed * 3.)) - .5) * .035;
      float dpx = abs(uv.x - x) * uRes.x;
      float bolt = smoothstep(1.4, 0., dpx) + exp(-dpx * .22) * .3;
      // Branche : part d'un point de l'arc, dérive sur le côté.
      float by = .62 + hash(vec2(uBoltSeed, 1.)) * .15;
      float bx = x + (by - y) * (hash(vec2(uBoltSeed, 2.)) > .5 ? .35 : -.35) + (noise(vec2(y * 30., uBoltSeed)) - .5) * .03;
      float bd = abs(uv.x - bx) * uRes.x;
      bolt += (smoothstep(1., 0., bd) + exp(-bd * .3) * .2) * step(y, by) * smoothstep(by - .22, by - .05, y) * .7;
      bolt *= smoothstep(.22, .42, y);
      col += vec3(.86, .9, 1.) * bolt * uFlash * 1.5;
    }
  }

  // Chaleur : lumière chaude qui monte de l'horizon.
  col = mix(col, vec3(1., .7, .38), uHeat * .3 * smoothstep(.7, 0., uv.y));

  // Vignette douce et tramage (évite les bandes du dégradé).
  col *= 1. - .14 * pow(length(uv - vec2(.5, .55)) * 1.15, 2.);
  col += (hash(gl_FragCoord.xy + fract(uTime)) - .5) / 255.;
  gl_FragColor = vec4(clamp(col, 0., 1.), 1.);
}
`;

const UNIFORMS = [
  "uRes", "uTime", "uTop", "uMid", "uBot", "uLit", "uShade", "uSunCol", "uSunP", "uSun", "uMoon", "uSunR", "uStars",
  "uCover", "uRain", "uSnow", "uFog", "uHeat", "uWind", "uLight", "uFlash", "uBolt", "uBoltSeed",
] as const;

// --- Rendu ----------------------------------------------------------------------------------

const FPS = 30;
const DPR_MAX = 1.5;
/** Vitesse du fondu entre ambiances (par seconde). */
const EASE = 2.2;

export class SkyRenderer {
  private gl: WebGLRenderingContext;
  private loc = {} as Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;
  private cur: Params | null = null;
  private target: Params | null = null;
  private raf = 0;
  private last = 0;
  private lastDraw = 0;
  private t0 = performance.now();
  private still: boolean;
  // Éclairs.
  private flash = 0;
  private bolt = -1;
  private boltSeed = 0;
  private nextFlash = 0;
  private pulses: number[] = [];

  constructor(private canvas: HTMLCanvasElement, opts: { still: boolean }) {
    const gl = canvas.getContext("webgl", { antialias: false, alpha: false, premultipliedAlpha: false, powerPreference: "low-power" });
    if (!gl) throw new Error("webgl");
    this.gl = gl;
    this.still = opts.still;
    const sh = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
      return s;
    };
    const prog = gl.createProgram()!;
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FRAG));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
    gl.useProgram(prog);
    // Un triangle qui couvre tout l'écran.
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const a = gl.getAttribLocation(prog, "a");
    gl.enableVertexAttribArray(a);
    gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
    for (const u of UNIFORMS) this.loc[u] = gl.getUniformLocation(prog, u);
  }

  /** Nouvelle cible ; `instant` = sans fondu (premier affichage). */
  set(p: Params, instant = false) {
    this.target = p;
    if (instant || !this.cur || this.still) this.cur = structuredClone(p);
    if (this.still) this.draw(performance.now());
  }

  /** Éclair immédiat (debug). */
  strike() {
    this.nextFlash = 0;
  }

  start() {
    if (this.still) return void this.draw(performance.now());
    const loop = (now: number) => {
      this.raf = requestAnimationFrame(loop);
      if (now - this.lastDraw < 1000 / FPS - 2) return;
      this.draw(now);
    };
    this.raf = requestAnimationFrame(loop);
  }

  stop() {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
  }

  dispose() {
    this.stop();
    this.gl.getExtension("WEBGL_lose_context")?.loseContext();
  }

  private resize() {
    const c = this.canvas;
    const dpr = Math.min(window.devicePixelRatio || 1, DPR_MAX);
    const w = Math.max(1, Math.round(c.clientWidth * dpr));
    const h = Math.max(1, Math.round(c.clientHeight * dpr));
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
      this.gl.viewport(0, 0, w, h);
    }
  }

  private ease(dt: number) {
    const cur = this.cur as unknown as Record<string, RGB | number>;
    const tgt = this.target as unknown as Record<string, RGB | number>;
    const k = 1 - Math.exp(-dt * EASE);
    for (const key of Object.keys(tgt)) {
      const a = cur[key];
      const b = tgt[key];
      cur[key] = Array.isArray(a) ? mix(a, b as RGB, k) : a + ((b as number) - a) * k;
    }
  }

  /** Éclairs : rafales irrégulières (double ou triple impulsion), plus fréquentes si fort. */
  private lightning(now: number, dt: number) {
    const storm = this.cur!.storm;
    if (storm < 0.05 || this.still) {
      this.flash = 0;
      return;
    }
    if (now >= this.nextFlash) {
      const n = Math.random() < 0.55 ? 2 : Math.random() < 0.5 ? 3 : 1;
      this.pulses = Array.from({ length: n }, (_, i) => now + i * (70 + Math.random() * 90));
      this.bolt = Math.random() < 0.7 ? 0.15 + Math.random() * 0.7 : -1;
      this.boltSeed = Math.random() * 100;
      this.nextFlash = now + (2600 + Math.random() * 6000) / (0.6 + storm * 0.8);
    }
    while (this.pulses.length && now >= this.pulses[0]) {
      this.pulses.shift();
      this.flash = Math.max(this.flash, 0.65 + Math.random() * 0.35);
    }
    this.flash *= Math.exp(-dt * 9);
    if (this.flash < 0.002) this.flash = 0;
  }

  private draw(now: number) {
    if (!this.cur || !this.target) return;
    const dt = this.last ? Math.min(0.1, (now - this.last) / 1000) : 0;
    this.last = now;
    this.lastDraw = now;
    this.resize();
    if (!this.still) this.ease(dt);
    this.lightning(now, dt);
    const { gl, loc } = this;
    const p = this.cur;
    gl.uniform2f(loc.uRes, this.canvas.width, this.canvas.height);
    // Animations réduites : instant figé choisi pour être joli (nuages bien formés).
    gl.uniform1f(loc.uTime, this.still ? 42 : (now - this.t0) / 1000);
    gl.uniform3fv(loc.uTop, p.top);
    gl.uniform3fv(loc.uMid, p.mid);
    gl.uniform3fv(loc.uBot, p.bot);
    gl.uniform3fv(loc.uLit, p.lit);
    gl.uniform3fv(loc.uShade, p.shade);
    gl.uniform3fv(loc.uSunCol, p.sunCol);
    gl.uniform2f(loc.uSunP, p.sunX, p.sunY);
    gl.uniform1f(loc.uSun, p.sun);
    gl.uniform1f(loc.uMoon, p.moon);
    gl.uniform1f(loc.uSunR, p.sunR);
    gl.uniform1f(loc.uStars, p.stars);
    gl.uniform1f(loc.uCover, p.cover);
    gl.uniform1f(loc.uRain, p.rain);
    gl.uniform1f(loc.uSnow, p.snow);
    gl.uniform1f(loc.uFog, p.fog);
    gl.uniform1f(loc.uHeat, p.heat);
    gl.uniform1f(loc.uWind, p.wind);
    gl.uniform1f(loc.uLight, p.light);
    gl.uniform1f(loc.uFlash, this.flash);
    gl.uniform1f(loc.uBolt, this.bolt);
    gl.uniform1f(loc.uBoltSeed, this.boltSeed);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
}

/** Couleur CSS moyenne du ciel (fond de repli sans WebGL, premier affichage). */
export function cssGradient(p: Params): string {
  const c = (v: RGB) => `rgb(${v.map((x) => Math.round(clamp(x) * 255)).join(" ")})`;
  return `linear-gradient(to bottom, ${c(p.top)}, ${c(p.mid)} 55%, ${c(p.bot)})`;
}
