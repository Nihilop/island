// Format de scène (v1, à calques) : des DONNÉES, jamais du code. Une pile de calques (du bas
// vers le haut) — image, couleur / dégradé, particules — chacun avec sa transformation, son
// opacité, son mode de fusion, sa profondeur de parallaxe et ses effets ; plus des effets de
// SCÈNE (parallaxe, couleur, grain, vignette, ondulation au clic, traînée du curseur).
// Le même JSON sert à l'éditeur, au moteur, aux préréglages et, plus tard, à l'IA et au
// partage. Toute valeur est bornée ici : une scène malformée ne peut rien faire d'autre
// qu'un fond moche.
//
// `layer_box`, `pack_layer` et `pack_composite` ont des jumeaux TypeScript dans
// `src/wallpaper/scene.ts` (même ordre, mêmes bornes, mêmes calculs).

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_LAYERS: usize = 16;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Scene {
    pub version: u32,
    pub name: String,
    /// Du bas vers le haut.
    pub layers: Vec<Layer>,
    pub parallax: Option<Parallax>,
    pub color: Option<ColorAdjust>,
    /// 0..1
    pub grain: Option<f32>,
    /// 0..1
    pub vignette: Option<f32>,
    pub interactions: Interactions,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: VERSION, name: String::new(), layers: Vec::new(), parallax: None, color: None, grain: None, vignette: None, interactions: Interactions::default() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Layer {
    /// Identifiant dans la scène (`[a-z0-9-]`).
    pub id: String,
    pub name: String,
    /// "image" | "fill" | "particles".
    pub kind: String,
    pub visible: bool,
    /// Verrouillé dans l'éditeur (ni sélection sur l'aperçu, ni déplacement).
    pub locked: bool,
    /// 0..1
    pub opacity: f32,
    /// "normal" | "add" | "screen" | "multiply".
    pub blend: String,
    /// Profondeur de parallaxe (0 = immobile, 1 = suit pleinement).
    pub depth: f32,
    /// Image (nom simple dans le dossier de la scène).
    pub image: Option<String>,
    /// Masque du calque (PNG en niveaux de gris, dans le repère du calque) : blanc = visible.
    /// Sur un calque de particules, c'est leur zone d'apparition.
    pub mask: Option<String>,
    pub fill: Fill,
    pub particles: Option<Particles>,
    pub transform: Transform,
    pub effects: LayerEffects,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            id: "layer".into(),
            name: String::new(),
            kind: "fill".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: "normal".into(),
            depth: 0.0,
            image: None,
            mask: None,
            fill: Fill::default(),
            particles: None,
            transform: Transform::default(),
            effects: LayerEffects::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Fill {
    /// "solid" | "gradient".
    pub mode: String,
    /// `#rrggbb` (la 2e ne sert qu'au dégradé).
    pub colors: [String; 2],
    /// Angle du dégradé (degrés, 0 = vers la droite, 90 = vers le bas).
    pub angle: f32,
}

impl Default for Fill {
    fn default() -> Self {
        Self { mode: "gradient".into(), colors: ["#1b1f3a".into(), "#3a1b4a".into()], angle: 90.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Transform {
    /// "cover" | "contain" | "stretch" (plein écran) | "free" (placé : ancre + décalage).
    pub fit: String,
    /// "tl" "t" "tr" "l" "c" "r" "bl" "b" "br" : coin / bord de l'écran auquel le calque
    /// s'accroche (le même coin du calque s'y aligne). Mode "free" seulement.
    pub anchor: String,
    /// Décalage depuis l'ancre, en fraction de la largeur / hauteur de l'écran.
    pub x: f32,
    pub y: f32,
    /// Largeur en fraction de la largeur de l'écran (hauteur : proportions de l'image).
    pub width: f32,
    /// Degrés.
    pub rotation: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self { fit: "cover".into(), anchor: "c".into(), x: 0.0, y: 0.0, width: 0.3, rotation: 0.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LayerEffects {
    pub water: Option<Water>,
    pub heat: Option<Heat>,
    pub color: Option<ColorAdjust>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Water {
    pub strength: f32,
    pub speed: f32,
    pub scale: f32,
    /// Zone de l'effet (masque en niveaux de gris, repère du calque) ; None = partout.
    pub mask: Option<String>,
}
impl Default for Water {
    fn default() -> Self {
        Self { strength: 0.4, speed: 0.4, scale: 0.4, mask: None }
    }
}

/// Distorsion de chaleur (air au-dessus d'une flamme) : un bruit qui file dans `direction`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Heat {
    pub strength: f32,
    pub speed: f32,
    /// Finesse des remous (0..1).
    pub scale: f32,
    /// Degrés (270 = vers le haut).
    pub direction: f32,
    pub mask: Option<String>,
}
impl Default for Heat {
    fn default() -> Self {
        Self { strength: 0.5, speed: 0.5, scale: 0.5, direction: 270.0, mask: None }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Parallax {
    /// Suivi de la souris (0..1).
    pub mouse: f32,
    /// Dérive lente automatique (0..1).
    pub drift: f32,
    /// Zoom (1..1.3) des calques plein écran, au prorata de leur profondeur : marge pour que
    /// le décalage ne montre pas les bords.
    pub zoom: f32,
}
impl Default for Parallax {
    fn default() -> Self {
        Self { mouse: 0.5, drift: 0.3, zoom: 1.06 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Particles {
    /// "dot" (point doux), "streak" (trait étiré dans la direction), "glow" (halo). Neige,
    /// pluie, lucioles… sont des préréglages de l'éditeur.
    pub shape: String,
    pub density: f32,
    /// 0..1, échelle exponentielle : de ~10 px/s (lucioles) à ~1000 px/s (pluie).
    pub speed: f32,
    pub size: f32,
    pub color: String,
    pub opacity: f32,
    /// Direction du mouvement (degrés, 90 = vers le bas, 270 = vers le haut).
    pub direction: f32,
    /// Écart de direction entre les plans (0..1).
    pub spread: f32,
    /// Balancement / errance de chaque particule (0..1).
    pub turbulence: f32,
    /// Scintillement (0..1).
    pub flicker: f32,
}
impl Default for Particles {
    fn default() -> Self {
        Self { shape: "dot".into(), density: 0.4, speed: 0.35, size: 1.0, color: "#ffffff".into(), opacity: 0.8, direction: 90.0, spread: 0.2, turbulence: 0.4, flicker: 0.0 }
    }
}

impl Particles {
    /// 1 point, 2 trait, 3 halo.
    pub fn shape_code(&self) -> f32 {
        match self.shape.as_str() {
            "streak" => 2.0,
            "glow" => 3.0,
            _ => 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorAdjust {
    /// -180..180
    pub hue: f32,
    /// 0..2
    pub saturation: f32,
    /// 0..2
    pub brightness: f32,
    /// 0..2
    pub contrast: f32,
}
impl Default for ColorAdjust {
    fn default() -> Self {
        Self { hue: 0.0, saturation: 1.0, brightness: 1.0, contrast: 1.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Interactions {
    pub ripple: Option<Ripple>,
    pub trail: Option<Trail>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Ripple {
    /// Rayon atteint (px pour 1080 p).
    pub radius: f32,
    pub strength: f32,
    /// Secondes.
    pub duration: f32,
}
impl Default for Ripple {
    fn default() -> Self {
        Self { radius: 260.0, strength: 0.6, duration: 1.4 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Trail {
    /// Largeur (px pour 1080 p).
    pub width: f32,
    /// Durée de vie d'un point (s).
    pub fade: f32,
    pub color: String,
    pub intensity: f32,
}
impl Default for Trail {
    fn default() -> Self {
        Self { width: 10.0, fade: 0.5, color: "#7cc8ff".into(), intensity: 0.8 }
    }
}

// --- Validation --------------------------------------------------------------------------

/// Nom de fichier simple (pas de chemin) avec une extension d'image.
pub fn valid_image_name(n: &str) -> bool {
    let ext = n.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    !n.is_empty() && n.len() <= 80 && !n.contains(['/', '\\', ':']) && !n.starts_with('.') && ["png", "jpg", "jpeg", "webp", "bmp", "gif"].contains(&ext.as_str())
}

fn valid_layer_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Scene {
    /// Rejette ce qui ne peut pas être une scène (le reste est borné par les `pack_*`).
    pub fn check(&self) -> Result<(), String> {
        if self.version > VERSION {
            return Err("sceneTooNew".into());
        }
        if self.name.chars().count() > 80 {
            return Err("invalidName".into());
        }
        if self.layers.len() > MAX_LAYERS {
            return Err("tooManyLayers".into());
        }
        for l in &self.layers {
            if !valid_layer_id(&l.id) || l.name.chars().count() > 80 {
                return Err("invalidLayer".into());
            }
            for f in l.files() {
                if !valid_image_name(f) {
                    return Err("invalidImage".into());
                }
            }
        }
        Ok(())
    }

    /// Fichiers référencés par les calques (images et masques).
    pub fn images(&self) -> impl Iterator<Item = &String> {
        self.layers.iter().flat_map(|l| l.files())
    }
}

impl Layer {
    /// Image, masque du calque, masques des effets.
    pub fn files(&self) -> impl Iterator<Item = &String> {
        [
            self.image.as_ref(),
            self.mask.as_ref(),
            self.effects.water.as_ref().and_then(|w| w.mask.as_ref()),
            self.effects.heat.as_ref().and_then(|h| h.mask.as_ref()),
        ]
        .into_iter()
        .flatten()
    }
}

// --- Placement et uniformes ----------------------------------------------------------------

pub const RIPPLES: usize = 8;
pub const TRAIL: usize = 32;
/// vec4 de `struct L` (layer.wgsl).
pub const LAYER_VEC4S: usize = 12;
/// vec4 de `struct C` (composite.wgsl).
pub const COMPOSITE_VEC4S: usize = 7 + RIPPLES + TRAIL;

/// `#rrggbb` → rgb 0..1 (espace sRGB, comme dans le navigateur ; pas de conversion linéaire).
pub fn hex(s: &str) -> [f32; 3] {
    let h = s.trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
    if h.len() == 6 { [v(0), v(2), v(4)] } else { [0.0, 0.0, 0.0] }
}

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

    /// Les masques comptent parmi les fichiers de la scène : sinon le ménage à
    /// l'enregistrement les supprimerait.
    #[test]
    fn masks_are_scene_files() {
        let mut l = super::Layer { image: Some("a.png".into()), mask: Some("mask-1.png".into()), ..Default::default() };
        l.effects.heat = Some(super::Heat { mask: Some("mask-2.png".into()), ..Default::default() });
        let s = super::Scene { layers: vec![l], ..Default::default() };
        let files: Vec<&String> = s.images().collect();
        assert_eq!(files, ["a.png", "mask-1.png", "mask-2.png"]);
        s.check().unwrap();
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
