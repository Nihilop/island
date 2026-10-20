// Fonds animés côté moteur : le FORMAT des scènes vit dans `island_formats::scene` (partagé
// avec le serveur du magasin) et est réexporté ici ; ce module garde ce qui est propre au
// rendu — placement des calques, uniformes des shaders, préréglages.
//
// `layer_box`, `pack_layer` et `pack_composite` ont des jumeaux TypeScript dans
// `src/wallpaper/scene.ts` (même ordre, mêmes bornes, mêmes calculs).

pub use island_formats::scene::*;

// --- Placement et uniformes ----------------------------------------------------------------

pub const RIPPLES: usize = 8;
pub const TRAIL: usize = 32;
/// vec4 de `struct L` (layer.wgsl).
pub const LAYER_VEC4S: usize = 12;
/// vec4 de `struct C` (composite.wgsl).
pub const COMPOSITE_VEC4S: usize = 7 + RIPPLES + TRAIL;

/// Entrées dynamiques d'une image : écran, temps, souris, interactions.
pub struct Frame<'a> {
    pub width: f32,
    pub height: f32,
    pub time: f32,
    /// Position de la souris (px de la surface), si connue.
    pub mouse: Option<(f32, f32)>,
    /// (x, y, t0) — ondulations récentes.
    pub ripples: &'a [(f32, f32, f32)],
    /// (x, y, t) — plus récent en premier.
    pub trail: &'a [(f32, f32, f32)],
}

fn c01(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { lo }
}

/// Coefficients de l'ancre : (0, 0.5, 1) en x et y.
pub fn anchor(a: &str) -> (f32, f32) {
    let x = if a.ends_with('l') { 0.0 } else if a.ends_with('r') { 1.0 } else { 0.5 };
    let y = if a.starts_with('t') { 0.0 } else if a.starts_with('b') { 1.0 } else { 0.5 };
    (x, y)
}

/// Rectangle d'un calque : (centre x, centre y, demi-largeur, demi-hauteur) en px.
/// `tex` = taille de l'image du calque (1×1 pour les autres types).
pub fn layer_box(s: &Scene, l: &Layer, f: &Frame, tex: (f32, f32)) -> [f32; 4] {
    let (w, h) = (f.width, f.height);
    let depth = c01(l.depth, 0.0, 1.0);
    // Parallaxe : décalage (souris + dérive) et zoom au prorata de la profondeur.
    let (mut ox, mut oy, mut zoom) = (0.0f32, 0.0f32, 1.0f32);
    if let Some(p) = &s.parallax {
        let (drift, mouse) = (c01(p.drift, 0.0, 1.0), c01(p.mouse, 0.0, 1.0));
        ox = (f.time * 0.11).sin() * drift * 0.012;
        oy = (f.time * 0.083).cos() * drift * 0.012;
        if let Some((mx, my)) = f.mouse {
            ox += (mx / w - 0.5) * mouse * 0.04;
            oy += (my / h - 0.5) * mouse * 0.04;
        }
        ox *= w * depth;
        oy *= h * depth;
        zoom = 1.0 + (c01(p.zoom, 1.0, 1.3) - 1.0) * depth;
    }
    let (iw, ih) = (tex.0.max(1.0), tex.1.max(1.0));
    let t = &l.transform;
    match t.fit.as_str() {
        "contain" | "cover" if l.kind == "image" => {
            let k = if t.fit == "cover" { (w / iw).max(h / ih) } else { (w / iw).min(h / ih) } * zoom;
            [w * 0.5 + ox, h * 0.5 + oy, iw * k * 0.5, ih * k * 0.5]
        }
        "free" if l.kind == "image" => {
            let hw = c01(t.width, 0.01, 4.0) * w * 0.5 * zoom;
            let hh = hw * ih / iw;
            let (ax, ay) = anchor(&t.anchor);
            let cx = ax * w + c01(t.x, -2.0, 2.0) * w + (0.5 - ax) * 2.0 * hw;
            let cy = ay * h + c01(t.y, -2.0, 2.0) * h + (0.5 - ay) * 2.0 * hh;
            [cx + ox, cy + oy, hw, hh]
        }
        // Étirer, et tout calque qui n'est pas une image : l'écran entier.
        _ => [w * 0.5 + ox, h * 0.5 + oy, w * 0.5 * zoom, h * 0.5 * zoom],
    }
}

pub fn pack_layer(s: &Scene, l: &Layer, f: &Frame, tex: (f32, f32)) -> [[f32; 4]; LAYER_VEC4S] {
    let mut u = [[0f32; 4]; LAYER_VEC4S];
    let kind = match l.kind.as_str() {
        "image" if l.image.is_some() => 0.0,
        "particles" if l.particles.is_some() => 3.0,
        "fill" if l.fill.mode == "solid" => 1.0,
        _ => 2.0,
    };
    // Bords étendus : les calques plein écran ne laissent jamais voir de vide sur les bords.
    let edges = if matches!(l.transform.fit.as_str(), "cover" | "stretch") { 1.0 } else { 0.0 };
    let rotation = if l.transform.fit == "free" { c01(l.transform.rotation, -360.0, 360.0).to_radians() } else { 0.0 };
    u[0] = [f.width, f.height, tex.0.max(1.0), tex.1.max(1.0)];
    u[1] = [f.time, kind, c01(l.opacity, 0.0, 1.0), edges];
    u[2] = layer_box(s, l, f, tex);
    u[3] = [rotation, c01(l.fill.angle, -360.0, 360.0).to_radians(), f.height / 1080.0, 0.0];
    let (a, b) = (hex(&l.fill.colors[0]), hex(&l.fill.colors[1]));
    u[4] = [a[0], a[1], a[2], 1.0];
    u[5] = [b[0], b[1], b[2], 1.0];
    if let Some(wt) = &l.effects.water {
        u[6] = [c01(wt.strength, 0.0, 1.0), c01(wt.speed, 0.0, 1.0), c01(wt.scale, 0.0, 1.0), 0.0];
    }
    u[7] = color(&l.effects.color);
    if let Some(p) = &l.particles {
        u[8] = [p.shape_code(), c01(p.density, 0.0, 1.0), c01(p.speed, 0.0, 1.0), c01(p.size, 0.2, 4.0)];
        let c = hex(&p.color);
        u[9] = [c[0], c[1], c[2], c01(p.opacity, 0.0, 1.0)];
        u[10] = [c01(p.direction, -360.0, 360.0).to_radians(), c01(p.turbulence, 0.0, 1.0), c01(p.spread, 0.0, 1.0), c01(p.flicker, 0.0, 1.0)];
    }
    if let Some(h) = &l.effects.heat {
        u[11] = [c01(h.strength, 0.0, 1.0), c01(h.speed, 0.0, 1.0), c01(h.scale, 0.0, 1.0), c01(h.direction, -360.0, 360.0).to_radians()];
    }
    u
}

fn color(c: &Option<ColorAdjust>) -> [f32; 4] {
    match c {
        Some(c) => [c01(c.hue, -180.0, 180.0), c01(c.saturation, 0.0, 2.0), c01(c.brightness, 0.0, 2.0), c01(c.contrast, 0.0, 2.0)],
        None => [0.0, 1.0, 1.0, 1.0],
    }
}

pub fn pack_composite(s: &Scene, f: &Frame) -> [[f32; 4]; COMPOSITE_VEC4S] {
    let mut u = [[0f32; 4]; COMPOSITE_VEC4S];
    u[0] = [f.width, f.height, 0.0, 0.0];
    u[1] = [f.time, f.height / 1080.0, 0.0, 0.0];
    u[2] = color(&s.color);
    u[3] = [c01(s.grain.unwrap_or(0.0), 0.0, 1.0), c01(s.vignette.unwrap_or(0.0), 0.0, 1.0), 0.0, 0.0];
    let it = &s.interactions;
    if let Some(r) = &it.ripple {
        u[4] = [1.0, c01(r.radius, 20.0, 1200.0), c01(r.strength, 0.0, 2.0), c01(r.duration, 0.2, 5.0)];
    }
    if let Some(t) = &it.trail {
        u[5] = [1.0, c01(t.width, 1.0, 80.0), c01(t.fade, 0.05, 3.0), 0.0];
        let c = hex(&t.color);
        u[6] = [c[0], c[1], c[2], c01(t.intensity, 0.0, 2.0)];
    }
    for (i, (x, y, t0)) in f.ripples.iter().take(RIPPLES).enumerate() {
        u[7 + i] = [*x, *y, *t0, 1.0];
    }
    for (i, (x, y, t)) in f.trail.iter().take(TRAIL).enumerate() {
        u[7 + RIPPLES + i] = [*x, *y, *t, 1.0];
    }
    u
}

/// Mode de fusion → index de pipeline (normal, add, screen, multiply).
pub fn blend_index(b: &str) -> usize {
    match b {
        "add" => 1,
        "screen" => 2,
        "multiply" => 3,
        _ => 0,
    }
}

// --- Préréglages (sans image : rien à télécharger) ----------------------------------------

fn fill(id: &str, name: &str, a: &str, b: &str, angle: f32) -> Layer {
    Layer { id: id.into(), name: name.into(), kind: "fill".into(), fill: Fill { mode: "gradient".into(), colors: [a.into(), b.into()], angle }, ..Default::default() }
}

fn parts(id: &str, name: &str, depth: f32, blend: &str, p: Particles) -> Layer {
    Layer { id: id.into(), name: name.into(), kind: "particles".into(), depth, blend: blend.into(), particles: Some(p), ..Default::default() }
}

pub fn presets() -> Vec<(&'static str, Scene)> {
    let scene = |name: &str, layers: Vec<Layer>| Scene { name: name.into(), layers, ..Default::default() };

    let mut aurora = scene(
        "Aurore",
        vec![
            fill("sky", "Ciel", "#06121f", "#1d3b4f", 70.0),
            parts("fireflies", "Lucioles", 0.8, "add", Particles { shape: "glow".into(), density: 0.35, speed: 0.08, size: 1.6, color: "#7affc8".into(), opacity: 0.7, direction: 270.0, spread: 0.6, turbulence: 0.9, flicker: 0.6 }),
        ],
    );
    aurora.parallax = Some(Parallax { mouse: 0.5, drift: 0.5, zoom: 1.05 });
    aurora.vignette = Some(0.5);
    aurora.interactions.trail = Some(Trail { color: "#7affc8".into(), ..Default::default() });

    let mut snow = scene(
        "Nuit de neige",
        vec![
            fill("sky", "Ciel", "#0b1426", "#2a3a5c", 100.0),
            parts("far", "Neige lointaine", 0.2, "normal", Particles { density: 0.5, speed: 0.25, size: 0.6, opacity: 0.5, ..Default::default() }),
            parts("near", "Neige proche", 1.0, "normal", Particles { density: 0.25, speed: 0.4, size: 1.6, opacity: 0.85, ..Default::default() }),
        ],
    );
    snow.parallax = Some(Parallax { mouse: 0.6, drift: 0.2, zoom: 1.05 });
    snow.vignette = Some(0.4);
    snow.grain = Some(0.15);
    snow.interactions.ripple = Some(Ripple::default());

    let mut sky = fill("sky", "Ciel", "#10141a", "#2b3442", 95.0);
    sky.effects.water = Some(Water { strength: 0.25, speed: 0.3, scale: 0.6, mask: None });
    let mut rain = scene(
        "Pluie",
        vec![sky, parts("rain", "Pluie", 0.5, "normal", Particles { shape: "streak".into(), density: 0.55, speed: 0.92, size: 1.0, color: "#9fb3c8".into(), opacity: 0.45, direction: 100.0, spread: 0.05, turbulence: 0.0, flicker: 0.0 })],
    );
    rain.grain = Some(0.2);
    rain.interactions.ripple = Some(Ripple { radius: 200.0, strength: 0.8, duration: 1.2 });

    let mut sea = fill("sea", "Mer", "#ff7a59", "#3a1c5c", 120.0);
    sea.effects.water = Some(Water { strength: 0.5, speed: 0.25, scale: 0.3, mask: None });
    let mut dusk = scene(
        "Crépuscule",
        vec![sea, parts("embers", "Braises", 0.7, "add", Particles { shape: "glow".into(), density: 0.25, speed: 0.4, size: 1.2, color: "#ffb36b".into(), opacity: 0.8, direction: 265.0, spread: 0.3, turbulence: 0.6, flicker: 0.9 })],
    );
    dusk.parallax = Some(Parallax { mouse: 0.4, drift: 0.3, zoom: 1.04 });
    dusk.color = Some(ColorAdjust { hue: 0.0, saturation: 1.15, brightness: 1.0, contrast: 1.05 });
    dusk.interactions.trail = Some(Trail { color: "#ffd2a8".into(), width: 14.0, ..Default::default() });

    vec![("preset-aurora", aurora), ("preset-snow", snow), ("preset-rain", rain), ("preset-dusk", dusk)]
}

#[cfg(test)]
mod tests {
    fn check(src: &str, name: &str, vec4s: usize) {
        let module = naga::front::wgsl::parse_str(src).expect("WGSL invalide");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("shader refusé par le validateur");
        let u = module.types.iter().find(|(_, t)| t.name.as_deref() == Some(name)).expect("struct d'uniformes");
        assert_eq!(u.1.inner.size(module.to_ctx()) as usize, vec4s * 16, "taille de {name}");
    }

    /// Les shaders partagés sont du WGSL valide, et leurs structs font exactement la taille
    /// que `pack_*` remplit (sinon le moteur et l'aperçu liraient des uniformes décalés).
    #[test]
    fn shaders_are_valid_and_match_pack() {
        check(include_str!("layer.wgsl"), "L", super::LAYER_VEC4S);
        check(include_str!("composite.wgsl"), "C", super::COMPOSITE_VEC4S);
    }

    #[test]
    fn presets_are_valid() {
        for (_, s) in super::presets() {
            s.check().unwrap();
        }
    }

    #[test]
    fn free_layer_anchors_bottom_right() {
        let mut l = super::Layer { kind: "image".into(), image: Some("a.png".into()), ..Default::default() };
        l.transform = super::Transform { fit: "free".into(), anchor: "br".into(), x: 0.0, y: 0.0, width: 0.1, rotation: 0.0 };
        let s = super::Scene::default();
        let f = super::Frame { width: 1000.0, height: 500.0, time: 0.0, mouse: None, ripples: &[], trail: &[] };
        let b = super::layer_box(&s, &l, &f, (100.0, 50.0));
        // 10 % de 1000 = 100 px de large, 50 px de haut, coin bas-droit collé à l'écran.
        assert_eq!(b, [950.0, 475.0, 50.0, 25.0]);
    }
}
