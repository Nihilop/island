// Format de scène des fonds animés d'Island (v1, à calques) : des DONNÉES, jamais du code.
// Une pile de calques (du bas vers le haut) — image, couleur / dégradé, particules — chacun
// avec sa transformation, son opacité, son mode de fusion, sa profondeur de parallaxe, ses
// effets et ses masques ; plus des effets de SCÈNE.
//
// Partagé par l'app (éditeur, moteur, partage) et le serveur du magasin : une scène est
// validée par les MÊMES règles des deux côtés. Toute valeur numérique est bornée au rendu
// (côté app) ; ici, on rejette ce qui ne peut pas être une scène.

use serde::{Deserialize, Serialize};


/// Version la plus récente que cette build sait lire. Une scène porte la version minimale
/// qu'il faut pour la lire (`Scene::required_version`) : 2 dès qu'elle contient une vidéo
/// ou un texte, sinon 1 — une scène sans vidéo reste lisible par les versions plus anciennes d'Island.
pub const VERSION: u32 = 2;
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
    /// "image" | "video" | "text" | "fill" | "particles".
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
    /// Vidéo en boucle (MP4 / WebM, sans le son ; nom simple dans le dossier de la scène).
    pub video: Option<String>,
    pub playback: Playback,
    /// Texte (contenu avec variables `{time}`…, style) ; placé comme une image « libre ».
    pub text: Option<TextStyle>,
    /// Visible seulement quand… (moment de la journée réel, heures, ciel) ; None = toujours.
    pub when: Option<When>,
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
            video: None,
            playback: Playback::default(),
            text: None,
            when: None,
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
/// Lecture d'un calque vidéo : vitesse, et début de la boucle (les secondes avant sont
/// sautées, à chaque tour).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Playback {
    /// 0.25..4
    pub speed: f32,
    /// Secondes, ≥ 0.
    pub start: f32,
}

impl Default for Playback {
    fn default() -> Self {
        Self { speed: 1.0, start: 0.0 }
    }
}

/// Style d'un calque texte. Les tailles sont relatives à la hauteur de l'écran (la police)
/// ou à la taille de la police (contour, ombre) : le rendu est le même sur tout écran.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct TextStyle {
    /// Texte, avec variables : `{time}`, `{date:EEEE d MMMM}`, `{time@Europe/Paris}`,
    /// `{weather.temp}`… ; `{{` et `}}` pour des accolades.
    pub content: String,
    /// Famille de police installée (sinon la police par défaut du système).
    pub font: String,
    /// 100..900
    pub weight: u16,
    pub italic: bool,
    /// Taille de la police, en fraction de la hauteur de l'écran.
    pub size: f32,
    pub color: String,
    /// "left" | "center" | "right"
    pub align: String,
    /// Interlignage (× la taille).
    pub line_height: f32,
    /// Espacement des lettres (em).
    pub letter_spacing: f32,
    pub uppercase: bool,
    /// Épaisseur du contour (fraction de la taille ; 0 = aucun).
    pub outline: f32,
    pub outline_color: String,
    pub shadow: Option<TextShadow>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            content: "{time}".into(),
            font: String::new(),
            weight: 600,
            italic: false,
            size: 0.12,
            color: "#ffffff".into(),
            align: "center".into(),
            line_height: 1.15,
            letter_spacing: 0.0,
            uppercase: false,
            outline: 0.0,
            outline_color: "#000000".into(),
            shadow: None,
        }
    }
}

/// Ombre (ou lueur, sans décalage) d'un texte ; flou et décalage en fraction de la taille.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct TextShadow {
    pub color: String,
    pub opacity: f32,
    pub blur: f32,
    pub x: f32,
    pub y: f32,
}

impl Default for TextShadow {
    fn default() -> Self {
        Self { color: "#000000".into(), opacity: 0.6, blur: 0.08, x: 0.0, y: 0.04 }
    }
}

pub const MAX_TEXT: usize = 500;

/// Condition d'affichage d'un calque. Tous les critères renseignés doivent être vrais ; le
/// calque apparaît et disparaît en fondu (`fade` secondes).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct When {
    /// "dawn" | "day" | "dusk" | "night", d'après le vrai lever / coucher du soleil (vide =
    /// tout moment). Ignoré si `from` / `to` sont donnés.
    pub moments: Vec<String>,
    /// Plage horaire précise « HH:MM » (heure locale ; elle peut passer minuit).
    pub from: Option<String>,
    pub to: Option<String>,
    /// "clear" | "cloudy" | "fog" | "rain" | "snow" | "storm" (météo réelle ; vide = tout ciel).
    pub sky: Vec<String>,
    /// Durée du fondu, en secondes.
    pub fade: f32,
}

impl Default for When {
    fn default() -> Self {
        Self { moments: Vec::new(), from: None, to: None, sky: Vec::new(), fade: 3.0 }
    }
}

pub const MOMENTS: &[&str] = &["dawn", "day", "dusk", "night"];
pub const SKIES: &[&str] = &["clear", "cloudy", "fog", "rain", "snow", "storm"];

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm"];

fn simple_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 80 && !n.contains(['/', '\\', ':']) && !n.starts_with('.')
}

fn extension(n: &str) -> String {
    n.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// Nom d'une vidéo de calque.
pub fn valid_video_name(n: &str) -> bool {
    simple_name(n) && VIDEO_EXTENSIONS.contains(&extension(n).as_str())
}

/// Nom d'un fichier de scène : image, masque ou vidéo.
pub fn valid_file_name(n: &str) -> bool {
    valid_image_name(n) || valid_video_name(n)
}

/// Le contenu est bien une vidéo du format annoncé par son nom (signature du conteneur) :
/// MP4 (`ftyp` à l'octet 4) ou WebM (en-tête EBML).
pub fn is_video(name: &str, bytes: &[u8]) -> bool {
    match extension(name).as_str() {
        "mp4" => bytes.len() > 12 && &bytes[4..8] == b"ftyp",
        "webm" => bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]),
        _ => false,
    }
}

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
            if let Some(w) = &l.when {
                let hhmm = |v: &Option<String>| v.as_deref().is_none_or(|t| t.is_empty() || t.len() <= 5 && t.split_once(':').is_some_and(|(h, m)| h.parse::<u8>().is_ok_and(|h| h < 24) && m.parse::<u8>().is_ok_and(|m| m < 60)));
                let known = w.moments.iter().all(|m| MOMENTS.contains(&m.as_str())) && w.sky.iter().all(|k| SKIES.contains(&k.as_str()));
                if !known || !hhmm(&w.from) || !hhmm(&w.to) {
                    return Err("invalidCondition".into());
                }
            }
            if let Some(t) = &l.text {
                if t.content.chars().count() > MAX_TEXT || t.font.chars().count() > 80 {
                    return Err("invalidText".into());
                }
            }
            for f in l.files() {
                // La vidéo d'un calque est une vidéo, tout le reste (image, masques) une image.
                let ok = if Some(f) == l.video.as_ref() { valid_video_name(f) } else { valid_image_name(f) };
                if !ok {
                    return Err("invalidImage".into());
                }
            }
        }
        Ok(())
    }

    /// Fichiers référencés par les calques (images, masques et vidéos).
    pub fn images(&self) -> impl Iterator<Item = &String> {
        self.layers.iter().flat_map(|l| l.files())
    }

    /// Vidéos des calques.
    pub fn videos(&self) -> impl Iterator<Item = &String> {
        self.layers.iter().filter_map(|l| l.video.as_ref())
    }

    /// Version minimale d'Island pour lire cette scène (cf. `VERSION`).
    pub fn required_version(&self) -> u32 {
        if self.layers.iter().any(|l| l.kind == "video" || l.kind == "text" || l.when.is_some()) { 2 } else { 1 }
    }
}

impl Layer {
    /// Image, vidéo, masque du calque, masques des effets.
    pub fn files(&self) -> impl Iterator<Item = &String> {
        [
            self.image.as_ref(),
            self.video.as_ref(),
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
