//! Médias de la fiche du magasin — convention COMMUNE aux paquets d'extension (`.island`) et
//! aux scènes (`.islandscene`) : un dossier `store/` à la racine.
//!
//! ```text
//! store/
//!   thumbnail.(png|webp|jpg)       vignette 16:9 (carte du catalogue)
//!   banner.(png|webp|jpg)          bannière ~3:1 (en-tête de la fiche) — facultative
//!   screenshots/<1..6>.(png|webp|jpg)  captures, dans l'ordre — facultatives
//!   README.md                      description longue (Markdown limité) — facultative
//! ```
//!
//! Tout autre fichier sous `store/` est refusé (`badMedia:<chemin>`), de même qu'un doublon
//! (`thumbnail.png` ET `thumbnail.jpg`). Chaque image est décodée par [`Media::check`] ;
//! le serveur les ré-encode ensuite à taille bornée (métadonnées et contenu caché retirés).

pub const PREFIX: &str = "store/";
pub const MAX_SCREENSHOTS: u8 = 6;
/// README : 20 Ko de texte UTF-8.
pub const MAX_README: usize = 20_000;
/// Côté max d'une image de la fiche (décodée pour vérification).
pub const MAX_SIDE: u32 = 8192;

const IMAGE_EXT: &[&str] = &["png", "webp", "jpg", "jpeg"];

/// Rôle d'un fichier de `store/`.
#[derive(Debug, PartialEq)]
pub enum Slot {
    Thumbnail,
    Banner,
    Screenshot(u8),
    Readme,
}

/// Rôle d'un chemin (None : pas un média du magasin, ou nom non reconnu).
pub fn slot(path: &str) -> Option<Slot> {
    let p = path.strip_prefix(PREFIX)?;
    if p == "README.md" {
        return Some(Slot::Readme);
    }
    let (stem, ext) = p.rsplit_once('.')?;
    if !IMAGE_EXT.contains(&ext.to_ascii_lowercase().as_str()) {
        return None;
    }
    match stem {
        "thumbnail" => Some(Slot::Thumbnail),
        "banner" => Some(Slot::Banner),
        _ => {
            let n: u8 = stem.strip_prefix("screenshots/")?.parse().ok()?;
            (1..=MAX_SCREENSHOTS).contains(&n).then_some(Slot::Screenshot(n))
        }
    }
}

/// Médias trouvés dans un paquet : (chemin d'origine, octets).
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Media {
    pub thumbnail: Option<(String, Vec<u8>)>,
    pub banner: Option<(String, Vec<u8>)>,
    /// Triées par numéro.
    pub screenshots: Vec<(u8, String, Vec<u8>)>,
    pub readme: Option<String>,
}

impl Media {
    /// Range un fichier de `store/`. Erreurs : `badMedia:<chemin>`, `duplicateMedia:<chemin>`,
    /// `readmeTooLong`, `readmeNotUtf8`.
    pub fn add(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
        let dup = || format!("duplicateMedia:{path}");
        match slot(path).ok_or_else(|| format!("badMedia:{path}"))? {
            Slot::Thumbnail if self.thumbnail.is_some() => return Err(dup()),
            Slot::Thumbnail => self.thumbnail = Some((path.into(), bytes)),
            Slot::Banner if self.banner.is_some() => return Err(dup()),
            Slot::Banner => self.banner = Some((path.into(), bytes)),
            Slot::Screenshot(n) => {
                if self.screenshots.iter().any(|(m, _, _)| *m == n) {
                    return Err(dup());
                }
                self.screenshots.push((n, path.into(), bytes));
                self.screenshots.sort_by_key(|(m, _, _)| *m);
            }
            Slot::Readme => {
                if bytes.len() > MAX_README {
                    return Err("readmeTooLong".into());
                }
                self.readme = Some(String::from_utf8(bytes).map_err(|_| "readmeNotUtf8")?);
            }
        }
        Ok(())
    }

    /// Images de la fiche, avec leur chemin : vignette, bannière, captures.
    pub fn images(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.thumbnail
            .iter()
            .chain(self.banner.iter())
            .map(|(p, b)| (p.as_str(), b.as_slice()))
            .chain(self.screenshots.iter().map(|(_, p, b)| (p.as_str(), b.as_slice())))
    }

    /// Chaque image se décode et reste dans les bornes (`invalidImage:<chemin>`, `tooLarge`).
    pub fn check(&self) -> Result<(), String> {
        for (path, bytes) in self.images() {
            let img = image::load_from_memory(bytes).map_err(|_| format!("invalidImage:{path}"))?;
            if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
                return Err("tooLarge".into());
            }
        }
        Ok(())
    }

    /// Fichiers à écrire dans un paquet (README compris).
    pub fn entries(&self) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = self.images().map(|(p, b)| (p.to_string(), b.to_vec())).collect();
        if let Some(r) = &self.readme {
            out.push((format!("{PREFIX}README.md"), r.clone().into_bytes()));
        }
        out
    }
}

/// Métadonnées de fiche lues dans un manifeste (`.island` ou `.islandscene`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StoreMeta {
    /// Une phrase (≤ 200 caractères).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(4, 4, image::Rgb([9, 9, 9])).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn recognizes_the_convention() {
        assert_eq!(slot("store/thumbnail.png"), Some(Slot::Thumbnail));
        assert_eq!(slot("store/banner.JPG"), Some(Slot::Banner));
        assert_eq!(slot("store/screenshots/3.webp"), Some(Slot::Screenshot(3)));
        assert_eq!(slot("store/README.md"), Some(Slot::Readme));
        assert_eq!(slot("store/screenshots/7.png"), None);
        assert_eq!(slot("store/thumbnail.gif"), None);
        assert_eq!(slot("store/evil.exe"), None);
        assert_eq!(slot("thumbnail.png"), None);
    }

    #[test]
    fn collects_orders_and_checks() {
        let mut m = Media::default();
        m.add("store/screenshots/2.png", png()).unwrap();
        m.add("store/screenshots/1.png", png()).unwrap();
        m.add("store/thumbnail.png", png()).unwrap();
        m.add("store/README.md", b"# Titre".to_vec()).unwrap();
        assert_eq!(m.screenshots.iter().map(|s| s.0).collect::<Vec<_>>(), [1, 2]);
        assert!(m.add("store/thumbnail.jpg", png()).unwrap_err().starts_with("duplicateMedia"));
        assert!(m.add("store/autre.txt", vec![]).unwrap_err().starts_with("badMedia"));
        m.check().unwrap();
        m.add("store/banner.png", b"pas une image".to_vec()).unwrap();
        assert_eq!(m.check().unwrap_err(), "invalidImage:store/banner.png");
        assert_eq!(m.entries().len(), 5);
    }
}
