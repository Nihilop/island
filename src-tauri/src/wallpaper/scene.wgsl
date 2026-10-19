// Shader des fonds animés — PARTAGÉ : rendu natif (wgpu, `engine.rs`) et aperçu de
// l'éditeur (WebGPU, `src/wallpaper/preview.ts`). Toute la scène tient dans un seul
// passage plein écran ; les effets sont pilotés par les uniformes (cf. `scene.rs::pack` et
// `src/wallpaper/scene.ts::pack`, à garder identiques).
//
// Coordonnées : pixels de la surface, origine en haut à gauche (souris, ondulations,
// traînée). Les tailles en px sont exprimées pour un écran de 1080 px de haut et mises à
// l'échelle (`k`), pour que l'aperçu réduit ressemble au rendu plein écran.

struct U {
  res: vec4f,         // largeur, hauteur, largeur image, hauteur image
  time: vec4f,        // t (s), ajustement (0 couvrir, 1 contenir, 2 étirer), fond (0 image, 1 dégradé), angle (rad)
  mouse: vec4f,       // x, y (px), sur le bureau (0/1), -
  grad_a: vec4f,      // couleur A du dégradé (aussi bandes de « contenir »)
  grad_b: vec4f,      // couleur B
  water: vec4f,       // intensité (0 = off), vitesse, échelle, -
  parallax: vec4f,    // souris, dérive, zoom, -
  particles: vec4f,   // type (0 off, 1 neige, 2 pluie, 3 lucioles), densité, vitesse, taille
  pcolor: vec4f,      // couleur des particules, opacité
  color: vec4f,       // teinte (°), saturation, luminosité, contraste
  post: vec4f,        // grain, vignette, -, -
  ripple: vec4f,      // actif, rayon (px), force, durée (s)
  trail: vec4f,       // actif, largeur (px), durée (s), -
  trail_color: vec4f, // couleur, intensité
  ripples: array<vec4f, 8>,    // x, y, t0, actif
  trail_pts: array<vec4f, 32>, // x, y, t, actif (plus récent en premier)
}

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var img: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

fn hash21(p: vec2f) -> f32 {
  var q = fract(p * vec2f(123.34, 456.21));
  q += dot(q, q + 45.32);
  return fract(q.x * q.y);
}

fn hash22(p: vec2f) -> vec2f {
  let n = hash21(p);
  return vec2f(n, hash21(p + n * 19.19));
}

fn hue_rotate(c: vec3f, deg: f32) -> vec3f {
  let a = radians(deg);
  let k = vec3f(0.57735);
  let ca = cos(a);
  return c * ca + cross(k, c) * sin(a) + k * dot(k, c) * (1.0 - ca);
}

fn seg_dist(p: vec2f, a: vec2f, b: vec2f) -> f32 {
  let pa = p - a;
  let ba = b - a;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.0001), 0.0, 1.0);
  return length(pa - ba * h);
}

// Position dans l'image (0..1) d'un point de l'écran (0..1), selon l'ajustement.
fn image_uv(uv: vec2f) -> vec2f {
  let r = u.res.xy;
  let s = max(u.res.zw, vec2f(1.0));
  var size = r;
  if (u.time.y < 0.5) {
    size = s * max(r.x / s.x, r.y / s.y);
  } else if (u.time.y < 1.5) {
    size = s * min(r.x / s.x, r.y / s.y);
  }
  return (uv * r - (r - size) * 0.5) / size;
}

fn background(uv: vec2f) -> vec3f {
  if (u.time.z > 0.5) {
    let d = vec2f(cos(u.time.w), sin(u.time.w));
    let t = clamp(dot(uv - 0.5, d) + 0.5, 0.0, 1.0);
    return mix(u.grad_a.rgb, u.grad_b.rgb, smoothstep(0.0, 1.0, t));
  }
  let iu = image_uv(uv);
  let c = textureSampleLevel(img, smp, clamp(iu, vec2f(0.0), vec2f(1.0)), 0.0).rgb;
  let inside = all(iu >= vec2f(0.0)) && all(iu <= vec2f(1.0));
  return select(u.grad_a.rgb, c, inside);
}

fn particles(c: vec3f, px: vec2f, t: f32, k: f32) -> vec3f {
  let kind = u.particles.x;
  if (kind < 0.5) {
    return c;
  }
  let dens = u.particles.y;
  let spd = u.particles.z;
  let size = max(u.particles.w, 0.1);
  var acc = 0.0;
  for (var layer = 0; layer < 3; layer++) {
    let lf = f32(layer);
    let cell = (110.0 - lf * 28.0) * k;
    let depth = 0.45 + lf * 0.35;
    var p = px;
    if (kind < 1.5) {
      p.y -= t * 45.0 * spd * depth * k;
      p.x += sin(t * 0.5 + lf * 1.7) * 25.0 * k;
    } else if (kind < 2.5) {
      p.y -= t * 700.0 * spd * depth * k;
      p.x -= p.y * 0.08;
    } else {
      p += vec2f(sin(t * 0.21 + lf * 2.1), cos(t * 0.17 + lf)) * 40.0 * k;
    }
    let id = floor(p / cell);
    let f = fract(p / cell);
    if (hash21(id + lf * 3.1) > dens) {
      continue;
    }
    let h = hash22(id + lf * 17.0);
    var center = h * 0.8 + 0.1;
    if (kind > 2.5) {
      center += vec2f(sin(t * 0.7 * spd + h.x * 6.28), cos(t * 0.6 * spd + h.y * 6.28)) * 0.15;
    }
    let rad = size * (1.6 + lf * 1.1) * k;
    var a = 0.0;
    if (kind > 1.5 && kind < 2.5) {
      let q = (f - center) * vec2f(1.0, 0.07) * cell;
      a = smoothstep(rad * 0.6, 0.0, length(q)) * 0.6;
    } else if (kind > 2.5) {
      let d = length(f - center) * cell;
      a = exp(-d * d / (rad * rad * 3.0)) * (0.55 + 0.45 * sin(t * 1.7 * spd + h.x * 40.0));
    } else {
      let d = length(f - center) * cell;
      a = smoothstep(rad, rad * 0.25, d);
    }
    acc += a * (0.45 + lf * 0.3);
  }
  let amt = clamp(acc * u.pcolor.a, 0.0, 1.0);
  if (kind > 2.5) {
    return c + u.pcolor.rgb * amt;
  }
  return mix(c, u.pcolor.rgb, amt);
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let r = u.res.xy;
  let t = u.time.x;
  let k = r.y / 1080.0;
  let px = pos.xy;
  var uv = px / r;

  // Parallaxe : léger zoom, puis décalage selon la souris et une dérive lente.
  let zoom = max(u.parallax.z, 1.0);
  var off = vec2f(sin(t * 0.11), cos(t * 0.083)) * u.parallax.y * 0.012;
  if (u.mouse.z > 0.5) {
    off += (u.mouse.xy / r - 0.5) * u.parallax.x * 0.04;
  }
  uv = (uv - 0.5) / zoom + 0.5 - off;

  // Eau : ondulations superposées.
  if (u.water.x > 0.0) {
    let f = 6.0 + u.water.z * 30.0;
    let s = t * (0.3 + u.water.y * 2.0);
    let a = u.water.x * 0.008;
    uv += vec2f(
      sin(uv.y * f + s) + sin(uv.y * f * 0.53 - s * 1.3) * 0.5,
      cos(uv.x * f * 0.9 + s * 1.1) + cos(uv.x * f * 0.37 + s * 0.7) * 0.5,
    ) * a;
  }

  // Ondulations au clic : un front circulaire qui s'éloigne et s'éteint.
  var shade = 0.0;
  if (u.ripple.x > 0.5) {
    let dur = max(u.ripple.w, 0.1);
    for (var i = 0; i < 8; i++) {
      let rp = u.ripples[i];
      let age = t - rp.z;
      if (rp.w < 0.5 || age < 0.0 || age > dur) {
        continue;
      }
      let d = px - rp.xy;
      let dist = length(d);
      let band = dist - age / dur * u.ripple.y * k;
      let env = exp(-band * band / (500.0 * k * k)) * (1.0 - age / dur);
      let w = sin(band * 0.22 / k) * env * u.ripple.z;
      if (dist > 0.001) {
        uv += (d / dist) * w * 8.0 * k / r;
      }
      shade += w * 0.12;
    }
  }

  var c = background(uv) + shade;

  // Couleur.
  c = hue_rotate(c, u.color.x);
  let l = dot(c, vec3f(0.299, 0.587, 0.114));
  c = mix(vec3f(l), c, u.color.y);
  c = (c - 0.5) * u.color.w + 0.5;
  c *= u.color.z;

  c = particles(c, px, t, k);

  // Traînée du curseur : segments entre les derniers points, qui s'amincissent et s'éteignent.
  if (u.trail.x > 0.5) {
    let fade = max(u.trail.z, 0.05);
    var g = 0.0;
    for (var i = 0; i < 31; i++) {
      let a = u.trail_pts[i];
      let b = u.trail_pts[i + 1];
      let age = t - a.z;
      if (a.w < 0.5 || b.w < 0.5 || age > fade) {
        continue;
      }
      let life = 1.0 - age / fade;
      let d = seg_dist(px, a.xy, b.xy);
      let wd = max(u.trail.y * k * life, 1.0);
      g = max(g, exp(-d * d / (wd * wd)) * life);
    }
    c += u.trail_color.rgb * g * u.trail_color.a;
  }

  // Vignette et grain.
  let v = length(pos.xy / r - 0.5) * 1.4142;
  c *= 1.0 - u.post.y * smoothstep(0.35, 1.0, v);
  c += (hash21(px + fract(t * 7.3) * 113.0) - 0.5) * u.post.x * 0.25;
  return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)), 1.0);
}
