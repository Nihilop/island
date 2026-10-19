// Format de scène des fonds animés d'Island (v1, à calques) : des DONNÉES, jamais du code.
// Une pile de calques (du bas vers le haut) — image, couleur / dégradé, particules — chacun
// avec sa transformation, son opacité, son mode de fusion, sa profondeur de parallaxe, ses
// effets et ses masques ; plus des effets de SCÈNE.
//
// Partagé par l'app (éditeur, moteur, partage) et le serveur du magasin : une scène est
// validée par les MÊMES règles des deux côtés. Toute valeur numérique est bornée au rendu
// (côté app) ; ici, on rejette ce qui ne peut pas être une scène.

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

/// `#rrggbb` → rgb 0..1 (espace sRGB, comme dans le navigateur ; pas de conversion linéaire).
pub fn hex(s: &str) -> [f32; 3] {
    let h = s.trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
    if h.len() == 6 { [v(0), v(2), v(4)] } else { [0.0, 0.0, 0.0] }
}

#[cfg(test)]
mod tests {
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
    fn rejects_paths_in_file_names() {
        assert!(super::valid_image_name("layer-1.png"));
        assert!(!super::valid_image_name("../a.png"));
        assert!(!super::valid_image_name("a/b.png"));
        assert!(!super::valid_image_name("C:a.png"));
        assert!(!super::valid_image_name(".png"));
        assert!(!super::valid_image_name("a.exe"));
    }
}
