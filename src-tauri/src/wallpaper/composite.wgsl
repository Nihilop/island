// Passe finale des fonds animés — PARTAGÉ (moteur natif + aperçu). Lit l'image composée des
// calques et applique les effets de SCÈNE : ondulations au clic, couleur globale, traînée
// du curseur, vignette, grain. Uniformes : `scene.rs::pack_composite` / `scene.ts::
// packComposite`.

struct C {
  res: vec4f,         // largeur, hauteur, -, -
  time: vec4f,        // t (s), k = hauteur / 1080, -, -
  color: vec4f,       // teinte (°), saturation, luminosité, contraste
  post: vec4f,        // grain, vignette, -, -
  ripple: vec4f,      // actif, rayon (px), force, durée (s)
  trail: vec4f,       // actif, largeur (px), durée (s), -
  trail_color: vec4f, // couleur, intensité
  ripples: array<vec4f, 8>,    // x, y, t0, actif
  trail_pts: array<vec4f, 32>, // x, y, t, actif (plus récent en premier)
}

@group(0) @binding(0) var<uniform> u: C;
@group(0) @binding(1) var scene_tex: texture_2d<f32>;
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

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let r = u.res.xy;
  let t = u.time.x;
  let k = u.time.y;
  let px = pos.xy;
  var uv = px / r;

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

  var c = textureSampleLevel(scene_tex, smp, clamp(uv, vec2f(0.0), vec2f(1.0)), 0.0).rgb + shade;

  // Couleur de la scène.
  c = hue_rotate(c, u.color.x);
  let l = dot(c, vec3f(0.299, 0.587, 0.114));
  c = mix(vec3f(l), c, u.color.y);
  c = (c - 0.5) * u.color.w + 0.5;
  c *= u.color.z;

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
  let v = length(px / r - 0.5) * 1.4142;
  c *= 1.0 - u.post.y * smoothstep(0.35, 1.0, v);
  c += (hash21(px + fract(t * 7.3) * 113.0) - 0.5) * u.post.x * 0.25;
  return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)), 1.0);
}
