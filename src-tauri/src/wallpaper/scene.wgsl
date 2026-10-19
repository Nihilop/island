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
  particles: vec4f,   // forme (0 off, 1 point, 2 trait, 3 halo), densité, vitesse (0..1), taille
  pcolor: vec4f,      // couleur des particules, opacité
  color: vec4f,       // teinte (°), saturation, luminosité, contraste
  post: vec4f,        // grain, vignette, -, -
  ripple: vec4f,      // actif, rayon (px), force, durée (s)
  trail: vec4f,       // actif, largeur (px), durée (s), -
  trail_color: vec4f, // couleur, intensité
  particles2: vec4f,  // direction (rad), turbulence, dispersion, scintillement
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

// Particules procédurales : 3 plans de grilles qui défilent dans la direction choisie, une
// particule (au plus) par case. Forme : point doux, trait étiré dans la direction, ou halo
// lumineux (ajouté à l'image). Neige, pluie, lucioles, braises ne sont que des réglages.
// Chaque pixel regarde sa case ET les 3 voisines du côté le plus proche : un halo qui
// déborde de sa case n'est plus tranché net. Le centre reste dans [0.15, 0.85] de sa case et
// le rayon est borné à 0.2 case, ce qui garantit que ces 4 cases suffisent.
fn particles(c: vec3f, px: vec2f, t: f32, k: f32) -> vec3f {
  let shape = u.particles.x;
  if (shape < 0.5) {
    return c;
  }
  let dens = u.particles.y;
  let size = max(u.particles.w, 0.1);
  let turb = u.particles2.y;
  let spread = u.particles2.z;
  let flicker = u.particles2.w;
  let streak = shape > 1.5 && shape < 2.5;
  let glow = shape > 2.5;
  // Vitesse exponentielle : 0 → 10 px/s, 1 → 1000 px/s (pour 1080 p).
  let speed = 10.0 * pow(100.0, u.particles.z);
  var acc = 0.0;
  for (var layer = 0; layer < 3; layer++) {
    let lf = f32(layer);
    let cell = (110.0 - lf * 28.0) * k;
    let depth = 0.45 + lf * 0.35;
    let ang = u.particles2.x + (lf - 1.0) * spread * 0.6;
    let dir = vec2f(cos(ang), sin(ang));
    let perp = vec2f(-dir.y, dir.x);
    // La grille avance dans `dir` : on échantillonne à rebours.
    let gp = (px - dir * t * speed * depth * k) / cell;
    let id0 = floor(gp);
    let side = select(vec2f(-1.0), vec2f(1.0), fract(gp) >= vec2f(0.5));
    let rad = min(size * (1.6 + lf * 1.1) * k, cell * 0.2);
    for (var n = 0; n < 4; n++) {
      let id = id0 + side * vec2f(f32(n & 1), f32(n >> 1u));
      if (hash21(id + lf * 3.1) > dens) {
        continue;
      }
      let h = hash22(id + lf * 17.0);
      // Errance : balancement de travers et léger va-et-vient dans la direction.
      let w = t * (0.5 + h.y) + h.x * 6.28;
      let local = h * 0.7 + 0.15 + (perp * sin(w) * 0.3 + dir * cos(w * 0.7) * 0.12) * turb;
      let q = (gp - id - clamp(local, vec2f(0.15), vec2f(0.85))) * cell;
      var a = 0.0;
      if (streak) {
        a = smoothstep(rad * 0.6, 0.0, length(vec2f(dot(q, dir) * 0.07, dot(q, perp)))) * 0.6;
      } else if (glow) {
        a = exp(-dot(q, q) / (rad * rad * 0.9));
      } else {
        a = smoothstep(rad, rad * 0.25, length(q));
      }
      let f = mix(1.0, 0.5 + 0.5 * sin(t * (2.0 + h.y * 7.0) + h.x * 50.0) * sin(t * 1.3 + h.y * 20.0), flicker);
      acc += a * f * (0.45 + lf * 0.3);
    }
  }
  let amt = clamp(acc * u.pcolor.a, 0.0, 1.0);
  if (glow) {
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
