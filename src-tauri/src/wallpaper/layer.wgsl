// Passe d'UN calque des fonds animés — PARTAGÉ : moteur natif (wgpu, `engine.rs`) et aperçu
// de l'éditeur (WebGPU, `src/wallpaper/preview.ts`). Les calques sont dessinés du bas vers
// le haut dans une image intermédiaire (couleur prémultipliée, mode de fusion = état de
// mélange du pipeline), puis `composite.wgsl` applique les effets de scène.
//
// Le placement (ajustement, ancrage, parallaxe, zoom) est calculé côté CPU (`scene.rs::
// layer_box` et son jumeau `scene.ts::layerBox`) : ici, un rectangle centre + demi-taille +
// rotation, en pixels de la surface (origine en haut à gauche).
//
// Masques (niveaux de gris, dans le repère du calque, blanc par défaut) : zone visible du
// calque (zone d'apparition pour des particules), zone de l'eau, zone de la chaleur.

struct L {
  res: vec4f,        // largeur, hauteur (surface), largeur, hauteur (image)
  info: vec4f,       // t (s), type (0 image, 1 couleur, 2 dégradé, 3 particules), opacité, bords étendus (0/1)
  box: vec4f,        // centre x, y (px), demi-largeur, demi-hauteur (px)
  box2: vec4f,       // rotation (rad), angle du dégradé (rad), k = hauteur / 1080, -
  fill_a: vec4f,     // couleur (ou début du dégradé)
  fill_b: vec4f,     // fin du dégradé
  water: vec4f,      // intensité (0 = off), vitesse, échelle, -
  color: vec4f,      // teinte (°), saturation, luminosité, contraste
  particles: vec4f,  // forme (1 point, 2 trait, 3 halo), densité, vitesse (0..1), taille
  pcolor: vec4f,     // couleur des particules, opacité
  particles2: vec4f, // direction (rad), turbulence, dispersion, scintillement
  heat: vec4f,       // intensité (0 = off), vitesse, finesse, direction (rad)
}

@group(0) @binding(0) var<uniform> u: L;
@group(0) @binding(1) var img: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
@group(0) @binding(3) var mask_layer: texture_2d<f32>;
@group(0) @binding(4) var mask_water: texture_2d<f32>;
@group(0) @binding(5) var mask_heat: texture_2d<f32>;

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

fn vnoise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash21(i), hash21(i + vec2f(1.0, 0.0)), s.x), mix(hash21(i + vec2f(0.0, 1.0)), hash21(i + vec2f(1.0, 1.0)), s.x), s.y);
}

fn mask(tex: texture_2d<f32>, uv: vec2f) -> f32 {
  return textureSampleLevel(tex, smp, clamp(uv, vec2f(0.0), vec2f(1.0)), 0.0).r;
}

// Un déplacement en px de la surface, exprimé dans le repère du calque (0..1).
fn px_to_uv(d: vec2f) -> vec2f {
  let r = -u.box2.x;
  let q = vec2f(d.x * cos(r) - d.y * sin(r), d.x * sin(r) + d.y * cos(r));
  return q / max(u.box.zw * 2.0, vec2f(0.0001));
}

// Position dans le calque (0..1) d'un pixel de la surface.
fn local_uv(px: vec2f) -> vec2f {
  let d = px - u.box.xy;
  let r = -u.box2.x;
  let q = vec2f(d.x * cos(r) - d.y * sin(r), d.x * sin(r) + d.y * cos(r));
  return q / max(u.box.zw * 2.0, vec2f(0.0001)) + 0.5;
}

// Particules procédurales : 3 plans de grilles qui défilent dans la direction choisie, une
// particule (au plus) par case. Forme : point doux, trait étiré dans la direction, ou halo.
// Chaque pixel regarde sa case ET les 3 voisines du côté le plus proche : un halo qui
// déborde de sa case n'est pas tranché. Le centre reste dans [0.15, 0.85] de sa case et le
// rayon est borné à 0.2 case, ce qui garantit que ces 4 cases suffisent.
// Renvoie la couverture (0..1).
fn particles(px: vec2f, t: f32, k: f32) -> f32 {
  let shape = u.particles.x;
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
  return clamp(acc * u.pcolor.a, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let r = u.res.xy;
  let t = u.info.x;
  let kind = u.info.y;
  let k = u.box2.z;
  let px = pos.xy;
  let base = local_uv(px);
  var uv = base;

  // Eau : ondulations superposées, sur ce calque seulement (dans la zone de son masque).
  if (u.water.x > 0.0) {
    let f = 6.0 + u.water.z * 30.0;
    let s = t * (0.3 + u.water.y * 2.0);
    let a = u.water.x * 0.008 * mask(mask_water, base);
    uv += vec2f(
      sin(base.y * f + s) + sin(base.y * f * 0.53 - s * 1.3) * 0.5,
      cos(base.x * f * 0.9 + s * 1.1) + cos(base.x * f * 0.37 + s * 0.7) * 0.5,
    ) * a;
  }

  // Chaleur (air au-dessus d'une flamme) : des ondes douces qui filent dans la direction
  // choisie, légèrement tordues en travers, sous une enveloppe lente qui fait aller et venir
  // le tremblement par plaques. Mesurée en hauteur d'écran (même finesse quelle que soit la
  // taille du calque). L'amplitude reste à 8 % de la longueur d'onde : la déformation ne
  // « replie » jamais l'image, donc jamais d'arête nette.
  if (u.heat.x > 0.0) {
    let dir = vec2f(cos(u.heat.w), sin(u.heat.w));
    let perp = vec2f(-dir.y, dir.x);
    let p = px / r.y;
    let lambda = mix(0.14, 0.025, u.heat.z);
    let f = 6.2831853 / lambda;
    let s = t * (0.6 + u.heat.y * 4.0);
    let along = dot(p, dir);
    let across = dot(p, perp);
    let w = sin(along * f - s) * 0.6 + sin(along * f * 1.73 + across * f * 0.41 - s * 1.31) * 0.4;
    let drift = dir * s * 0.02;
    let env = smoothstep(0.2, 0.8, vnoise(p * 3.0 - drift) * 0.6 + vnoise(p * 6.1 + 3.7 - drift * 1.6) * 0.4);
    let amp = u.heat.x * lambda * r.y * 0.08 * env * mask(mask_heat, base);
    uv += px_to_uv((perp + dir * 0.25) * w * amp);
  }

  var c = vec4f(0.0);
  if (kind < 0.5) {
    let inside = all(uv >= vec2f(0.0)) && all(uv <= vec2f(1.0));
    let s = textureSampleLevel(img, smp, clamp(uv, vec2f(0.0), vec2f(1.0)), 0.0);
    c = select(vec4f(0.0), s, inside || u.info.w > 0.5);
  } else if (kind < 1.5) {
    c = vec4f(u.fill_a.rgb, 1.0);
  } else if (kind < 2.5) {
    // Dans le repère du calque (plein écran) : le dégradé suit eau, chaleur et parallaxe.
    let d = vec2f(cos(u.box2.y), sin(u.box2.y));
    let g = clamp(dot(uv - 0.5, d) + 0.5, 0.0, 1.0);
    c = vec4f(mix(u.fill_a.rgb, u.fill_b.rgb, smoothstep(0.0, 1.0, g)), 1.0);
  } else {
    // Les particules suivent le décalage de parallaxe du calque.
    c = vec4f(u.pcolor.rgb, particles(px - (u.box.xy - r * 0.5), t, k));
  }

  // Couleur du calque.
  var rgb = hue_rotate(c.rgb, u.color.x);
  let l = dot(rgb, vec3f(0.299, 0.587, 0.114));
  rgb = mix(vec3f(l), rgb, u.color.y);
  rgb = clamp(((rgb - 0.5) * u.color.w + 0.5) * u.color.z, vec3f(0.0), vec3f(1.0));

  let a = clamp(c.a * u.info.z * mask(mask_layer, base), 0.0, 1.0);
  return vec4f(rgb * a, a);
}
