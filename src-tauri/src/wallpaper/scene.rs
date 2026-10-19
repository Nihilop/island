// Format de scène (v1) : des DONNÉES, jamais du code. Un fond (image ou dégradé), des
// effets et des interactions, tous optionnels, rendus par un seul shader (`scene.wgsl`).
// Le même JSON sert à l'éditeur, au moteur, aux préréglages et, plus tard, à l'IA et aux
// packs de scènes. Toute valeur est bornée ici : une scène malformée ne peut rien faire
// d'autre qu'un fond moche.
//
// `pack` convertit une scène en uniformes ; son jumeau TypeScript est
// `src/wallpaper/scene.ts::pack` (même ordre, mêmes bornes).

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Scene {
    pub version: u32,
    pub name: String,
    pub background: Background,
    pub effects: Effects,
    pub interactions: Interactions,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: VERSION, name: String::new(), background: Background::default(), effects: Effects::default(), interactions: Interactions::default() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Background {
    /// "image" | "gradient".
    pub kind: String,
    /// Fichier image dans le dossier de la scène (nom simple, sans chemin).
    pub image: Option<String>,
    /// "cover" | "contain" | "stretch".
    pub fit: String,
    /// Dégradé (et bandes de « contenir ») : deux couleurs `#rrggbb`.
    pub colors: [String; 2],
    /// Angle du dégradé (degrés).
    pub angle: f32,
}

impl Default for Background {
    fn default() -> Self {
        Self { kind: "gradient".into(), image: None, fit: "cover".into(), colors: ["#1b1f3a".into(), "#3a1b4a".into()], angle: 90.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Effects {
    pub water: Option<Water>,
    pub parallax: Option<Parallax>,
    pub particles: Option<Particles>,
    pub color: Option<ColorAdjust>,
    /// 0..1
    pub grain: Option<f32>,
    /// 0..1
    pub vignette: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Water {
    pub strength: f32,
    pub speed: f32,
    pub scale: f32,
}
impl Default for Water {
    fn default() -> Self {
        Self { strength: 0.4, speed: 0.4, scale: 0.4 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Parallax {
    /// Suivi de la souris (0..1).
    pub mouse: f32,
    /// Dérive lente automatique (0..1).
    pub drift: f32,
    /// Zoom (1..1.3) : marge pour que le décalage ne montre pas les bords.
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
    /// Forme : "dot" (point doux), "streak" (trait étiré dans la direction), "glow" (halo
    /// lumineux, ajouté à l'image). `kind` : ancien nom (snow/rain/fireflies/embers),
    /// converti par `shape_code`. Neige, pluie, lucioles… sont des préréglages de l'éditeur.
    #[serde(alias = "kind")]
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
        Self {
            shape: "dot".into(),
            density: 0.4,
            speed: 0.35,
            size: 1.0,
            color: "#ffffff".into(),
            opacity: 0.8,
            direction: 90.0,
            spread: 0.2,
            turbulence: 0.4,
            flicker: 0.0,
        }
    }
}

impl Particles {
    /// 1 point, 2 trait, 3 halo (les anciens types y sont ramenés).
    pub fn shape_code(&self) -> f32 {
        match self.shape.as_str() {
            "streak" | "rain" => 2.0,
            "glow" | "fireflies" | "embers" => 3.0,
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

impl Scene {
    /// Rejette ce qui ne peut pas être une scène (le reste est borné par `pack`).
    pub fn check(&self) -> Result<(), String> {
        if self.version > VERSION {
            return Err("sceneTooNew".into());
        }
        if self.name.chars().count() > 80 {
            return Err("invalidName".into());
        }
        if let Some(img) = &self.background.image {
            if !valid_image_name(img) {
                return Err("invalidImage".into());
            }
        }
        Ok(())
    }
}

// --- Uniformes ---------------------------------------------------------------------------

pub const RIPPLES: usize = 8;
pub const TRAIL: usize = 32;
/// Nombre de vec4 de `struct U` (15 champs + ondulations + traînée).
pub const VEC4S: usize = 15 + RIPPLES + TRAIL;

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
    pub image: (f32, f32),
    pub time: f32,
    /// Position de la souris (px de la surface) si elle est sur le bureau.
    pub mouse: Option<(f32, f32)>,
    /// (x, y, t0) — ondulations récentes.
    pub ripples: &'a [(f32, f32, f32)],
    /// (x, y, t) — plus récent en premier.
    pub trail: &'a [(f32, f32, f32)],
}

fn c01(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { lo }
}

pub fn pack(s: &Scene, f: &Frame) -> [[f32; 4]; VEC4S] {
    let mut u = [[0f32; 4]; VEC4S];
    let b = &s.background;
    let fit = match b.fit.as_str() {
        "contain" => 1.0,
        "stretch" => 2.0,
        _ => 0.0,
    };
    let gradient = b.kind != "image" || b.image.is_none();
    u[0] = [f.width, f.height, f.image.0.max(1.0), f.image.1.max(1.0)];
    u[1] = [f.time, fit, if gradient { 1.0 } else { 0.0 }, c01(b.angle, -360.0, 360.0).to_radians()];
    if let Some((x, y)) = f.mouse {
        u[2] = [x, y, 1.0, 0.0];
    }
    let (a, bb) = (hex(&b.colors[0]), hex(&b.colors[1]));
    u[3] = [a[0], a[1], a[2], 1.0];
    u[4] = [bb[0], bb[1], bb[2], 1.0];
    let e = &s.effects;
    if let Some(w) = &e.water {
        u[5] = [c01(w.strength, 0.0, 1.0), c01(w.speed, 0.0, 1.0), c01(w.scale, 0.0, 1.0), 0.0];
    }
    u[6] = match &e.parallax {
        Some(p) => [c01(p.mouse, 0.0, 1.0), c01(p.drift, 0.0, 1.0), c01(p.zoom, 1.0, 1.3), 0.0],
        None => [0.0, 0.0, 1.0, 0.0],
    };
    if let Some(p) = &e.particles {
        u[7] = [p.shape_code(), c01(p.density, 0.0, 1.0), c01(p.speed, 0.0, 1.0), c01(p.size, 0.2, 4.0)];
        let c = hex(&p.color);
        u[8] = [c[0], c[1], c[2], c01(p.opacity, 0.0, 1.0)];
        u[14] = [c01(p.direction, -360.0, 360.0).to_radians(), c01(p.turbulence, 0.0, 1.0), c01(p.spread, 0.0, 1.0), c01(p.flicker, 0.0, 1.0)];
    }
    u[9] = match &e.color {
        Some(c) => [c01(c.hue, -180.0, 180.0), c01(c.saturation, 0.0, 2.0), c01(c.brightness, 0.0, 2.0), c01(c.contrast, 0.0, 2.0)],
        None => [0.0, 1.0, 1.0, 1.0],
    };
    u[10] = [c01(e.grain.unwrap_or(0.0), 0.0, 1.0), c01(e.vignette.unwrap_or(0.0), 0.0, 1.0), 0.0, 0.0];
    let it = &s.interactions;
    if let Some(r) = &it.ripple {
        u[11] = [1.0, c01(r.radius, 20.0, 1200.0), c01(r.strength, 0.0, 2.0), c01(r.duration, 0.2, 5.0)];
    }
    if let Some(t) = &it.trail {
        u[12] = [1.0, c01(t.width, 1.0, 80.0), c01(t.fade, 0.05, 3.0), 0.0];
        let c = hex(&t.color);
        u[13] = [c[0], c[1], c[2], c01(t.intensity, 0.0, 2.0)];
    }
    for (i, (x, y, t0)) in f.ripples.iter().take(RIPPLES).enumerate() {
        u[15 + i] = [*x, *y, *t0, 1.0];
    }
    for (i, (x, y, t)) in f.trail.iter().take(TRAIL).enumerate() {
        u[15 + RIPPLES + i] = [*x, *y, *t, 1.0];
    }
    u
}

// --- Préréglages (sans image : rien à télécharger) ----------------------------------------

pub fn presets() -> Vec<(&'static str, Scene)> {
    let g = |name: &str, a: &str, b: &str, angle: f32| Scene {
        name: name.into(),
        background: Background { kind: "gradient".into(), image: None, fit: "cover".into(), colors: [a.into(), b.into()], angle },
        ..Default::default()
    };
    let mut aurora = g("Aurore", "#06121f", "#1d3b4f", 70.0);
    aurora.effects.particles = Some(Particles { shape: "glow".into(), density: 0.35, speed: 0.08, size: 1.6, color: "#7affc8".into(), opacity: 0.7, direction: 270.0, spread: 0.6, turbulence: 0.9, flicker: 0.6 });
    aurora.effects.parallax = Some(Parallax { mouse: 0.4, drift: 0.5, zoom: 1.05 });
    aurora.effects.vignette = Some(0.5);
    aurora.interactions.trail = Some(Trail { color: "#7affc8".into(), ..Default::default() });

    let mut snow = g("Nuit de neige", "#0b1426", "#2a3a5c", 100.0);
    snow.effects.particles = Some(Particles::default());
    snow.effects.vignette = Some(0.4);
    snow.effects.grain = Some(0.15);
    snow.interactions.ripple = Some(Ripple::default());

    let mut rain = g("Pluie", "#10141a", "#2b3442", 95.0);
    rain.effects.particles = Some(Particles { shape: "streak".into(), density: 0.55, speed: 0.92, size: 1.0, color: "#9fb3c8".into(), opacity: 0.45, direction: 100.0, spread: 0.05, turbulence: 0.0, flicker: 0.0 });
    rain.effects.water = Some(Water { strength: 0.25, speed: 0.3, scale: 0.6 });
    rain.effects.grain = Some(0.2);
    rain.interactions.ripple = Some(Ripple { radius: 200.0, strength: 0.8, duration: 1.2 });

    let mut dusk = g("Crépuscule", "#ff7a59", "#3a1c5c", 120.0);
    dusk.effects.water = Some(Water { strength: 0.5, speed: 0.25, scale: 0.3 });
    dusk.effects.particles = Some(Particles { shape: "glow".into(), density: 0.25, speed: 0.4, size: 1.2, color: "#ffb36b".into(), opacity: 0.8, direction: 265.0, spread: 0.3, turbulence: 0.6, flicker: 0.9 });
    dusk.effects.color = Some(ColorAdjust { hue: 0.0, saturation: 1.15, brightness: 1.0, contrast: 1.05 });
    dusk.interactions.trail = Some(Trail { color: "#ffd2a8".into(), width: 14.0, ..Default::default() });
    vec![("preset-aurora", aurora), ("preset-snow", snow), ("preset-rain", rain), ("preset-dusk", dusk)]
}

#[cfg(test)]
mod tests {
    /// Le shader partagé est du WGSL valide, et `struct U` fait exactement la taille que
    /// `pack` remplit (sinon le moteur et l'aperçu liraient des uniformes décalés).
    #[test]
    fn shader_is_valid_and_matches_pack() {
        let module = naga::front::wgsl::parse_str(include_str!("scene.wgsl")).expect("WGSL invalide");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("shader refusé par le validateur");
        let u = module.types.iter().find(|(_, t)| t.name.as_deref() == Some("U")).expect("struct U");
        let size = u.1.inner.size(module.to_ctx());
        assert_eq!(size as usize, super::VEC4S * 16);
    }
}
