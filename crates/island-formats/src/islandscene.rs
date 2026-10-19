//! Fichier de partage d'une scène, `.islandscene` (zip) : TOUT ce qu'il faut pour appliquer
//! et modifier la scène ailleurs.
//!
//! ```text
//! manifest.json   { "format": "islandscene", "version": 1, "name", "app" }
//! scene.json      la scène (format de `scene`)
//! thumbnail.png   vignette 480 × 270 (le calque de fond)
//! <fichiers>      images et masques référencés, à plat (noms simples)
//! ```
//!
//! Aucun code : pas de signature, mais un fichier reçu peut être piégé, donc [`read`] vérifie
//! tout : taille du fichier et taille décompressée bornées, noms simples uniquement (rien ne
//! peut s'écrire hors du dossier de la scène), chaque image réellement décodée, scène validée,
//! format trop récent refusé. Codes d'erreur : `tooLarge`, `corrupt`, `sceneTooNew`,
//! `missingFile:<nom>`, `invalidImage:<nom>`, plus ceux de [`Scene::check`].

use std::collections::HashSet;
use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize};

use crate::scene::{self, Scene};

pub const EXT: &str = "islandscene";
pub const FORMAT: &str = "islandscene";
pub const FORMAT_VERSION: u32 = 1;

/// Fichier : 256 Mo ; décompressé : 384 Mo au total, 96 Mo par fichier ; 64 entrées.
pub const MAX_FILE: u64 = 256 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 384 * 1024 * 1024;
pub const MAX_ENTRY: u64 = 96 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 64;
/// Côté max d'une image (décodée pour vérification).
pub const MAX_SIDE: u32 = 16384;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub name: String,
    /// Application qui a écrit le fichier (« Island 2.5.1 »), pour information.
    #[serde(default)]
    pub app: String,
}

/// Contenu vérifié d'un `.islandscene`.
pub struct Contents {
    pub manifest: Manifest,
    pub scene: Scene,
    /// Images et masques référencés par la scène (les autres fichiers sont ignorés).
    pub files: Vec<(String, Vec<u8>)>,
    /// Vignette fournie (non vérifiée : à régénérer si on veut s'y fier).
    pub thumbnail: Option<Vec<u8>>,
}

/// Lit et vérifie un `.islandscene`. `len` = taille du fichier (pour refuser tôt).
pub fn read<R: Read + Seek>(reader: R, len: u64) -> Result<Contents, String> {
    if len > MAX_FILE {
        return Err("tooLarge".into());
    }
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| "corrupt")?;
    if zip.len() > MAX_ENTRIES {
        return Err("corrupt".into());
    }
    let mut total = 0u64;
    let (mut manifest, mut scene_json, mut thumbnail) = (None, None, None);
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|_| "corrupt")?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let known = matches!(name.as_str(), "manifest.json" | "scene.json" | "thumbnail.png");
        // Noms simples seulement : rien ne peut s'écrire ailleurs que dans le dossier de la scène.
        if !known && !scene::valid_image_name(&name) {
            return Err("corrupt".into());
        }
        if e.size() > MAX_ENTRY {
            return Err("tooLarge".into());
        }
        // Taille réelle bornée (la taille annoncée peut mentir).
        let mut bytes = Vec::new();
        (&mut e).take(MAX_ENTRY + 1).read_to_end(&mut bytes).map_err(|_| "corrupt")?;
        total += bytes.len() as u64;
        if bytes.len() as u64 > MAX_ENTRY || total > MAX_TOTAL {
            return Err("tooLarge".into());
        }
        match name.as_str() {
            "manifest.json" => manifest = Some(bytes),
            "scene.json" => scene_json = Some(bytes),
            "thumbnail.png" => thumbnail = Some(bytes),
            _ => files.push((name, bytes)),
        }
    }
    let manifest: Manifest = serde_json::from_slice(&manifest.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    if manifest.format != FORMAT {
        return Err("corrupt".into());
    }
    if manifest.version > FORMAT_VERSION {
        return Err("sceneTooNew".into());
    }
    let scene: Scene = serde_json::from_slice(&scene_json.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    scene.check()?;
    // Tout fichier référencé doit être là, et être une vraie image ; le reste est ignoré.
    let needed: HashSet<String> = scene.images().cloned().collect();
    for f in &needed {
        let (_, bytes) = files.iter().find(|(n, _)| n == f).ok_or_else(|| format!("missingFile:{f}"))?;
        let img = image::load_from_memory(bytes).map_err(|_| format!("invalidImage:{f}"))?;
        if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
            return Err("tooLarge".into());
        }
    }
    files.retain(|(n, _)| needed.contains(n));
    Ok(Contents { manifest, scene, files, thumbnail })
}

/// Écrit un `.islandscene`. `files` : les images et masques de la scène (nom, octets) ;
/// `thumbnail` : PNG (cf. [`thumbnail`]) ; `app` : « Island 2.5.1 ».
pub fn write<W: Write + Seek>(writer: W, scene: &Scene, files: &[(String, Vec<u8>)], thumbnail: &[u8], app: &str) -> Result<(), String> {
    let manifest = Manifest { format: FORMAT.into(), version: FORMAT_VERSION, name: scene.name.clone(), app: app.into() };
    let mut zip = zip::ZipWriter::new(writer);
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // Images et masques sont déjà compressés : stockés tels quels.
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut put = |name: &str, bytes: &[u8], opts| -> Result<(), String> {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())
    };
    put("manifest.json", &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?, deflate)?;
    put("scene.json", &serde_json::to_vec_pretty(scene).map_err(|e| e.to_string())?, deflate)?;
    put("thumbnail.png", thumbnail, stored)?;
    let mut seen = HashSet::new();
    for f in scene.images() {
        if seen.insert(f.clone()) {
            let (_, bytes) = files.iter().find(|(n, _)| n == f).ok_or_else(|| format!("missingFile:{f}"))?;
            put(f, bytes, stored)?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Vignette 480 × 270 : le calque de fond (le plus bas des calques visibles), image
/// recadrée ou couleur / dégradé. `image` charge un fichier de la scène par son nom.
pub fn thumbnail(s: &Scene, image: impl Fn(&str) -> Option<image::DynamicImage>) -> image::RgbaImage {
    let (w, h) = (480u32, 270u32);
    let bottom = s.layers.iter().find(|l| l.visible && (l.kind == "fill" || (l.kind == "image" && l.image.is_some())));
    if let Some(l) = bottom.filter(|l| l.kind == "image") {
        if let Some(img) = l.image.as_deref().and_then(&image) {
            return img.resize_to_fill(w, h, image::imageops::FilterType::Triangle).to_rgba8();
        }
    }
    let fill = bottom.filter(|l| l.kind == "fill").map(|l| l.fill.clone()).unwrap_or_default();
    let (a, b) = (scene::hex(&fill.colors[0]), scene::hex(&fill.colors[1]));
    let solid = fill.mode == "solid";
    let ang = fill.angle.to_radians();
    let (dx, dy) = (ang.cos(), ang.sin());
    image::RgbaImage::from_fn(w, h, |x, y| {
        let g = if solid { 0.0 } else { (((x as f32 / w as f32 - 0.5) * dx + (y as f32 / h as f32 - 0.5) * dy) + 0.5).clamp(0.0, 1.0) };
        let g = g * g * (3.0 - 2.0 * g);
        let c = |i: usize| ((a[i] + (b[i] - a[i]) * g) * 255.0) as u8;
        image::Rgba([c(0), c(1), c(2), 255])
    })
}

/// PNG d'une image (vignette).
pub fn png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut out);
            for (n, b) in entries {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b).unwrap();
            }
            z.finish().unwrap();
        }
        out.into_inner()
    }

    fn read_bytes(b: &[u8]) -> Result<Contents, String> {
        read(Cursor::new(b), b.len() as u64)
    }

    fn tiny_png() -> Vec<u8> {
        png(&image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))).unwrap()
    }

    fn manifest() -> Vec<u8> {
        br#"{"format":"islandscene","version":1,"name":"t"}"#.to_vec()
    }

    fn image_scene() -> Scene {
        let mut s = Scene::default();
        s.layers.push(scene::Layer { id: "a".into(), kind: "image".into(), image: Some("a.png".into()), ..Default::default() });
        s
    }

    #[test]
    fn reads_a_valid_scene_and_drops_unreferenced_files() {
        let json = serde_json::to_vec(&image_scene()).unwrap();
        let r = read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", &tiny_png()), ("extra.png", &tiny_png())])).unwrap();
        assert_eq!(r.scene.layers.len(), 1);
        assert_eq!(r.files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a.png"]);
    }

    #[test]
    fn rejects_paths_missing_files_and_fake_images() {
        let json = serde_json::to_vec(&image_scene()).unwrap();
        // Chemin dans l'archive (tentative d'écrire ailleurs).
        assert!(read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("../a.png", &tiny_png())])).is_err());
        // Fichier référencé absent.
        assert!(read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json)])).err().unwrap().starts_with("missingFile"));
        // « Image » qui n'en est pas une.
        assert!(read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", b"pas une image")])).err().unwrap().starts_with("invalidImage"));
        // Format trop récent.
        let newer = br#"{"format":"islandscene","version":99}"#;
        assert_eq!(read_bytes(&zip_of(&[("manifest.json", newer), ("scene.json", &json), ("a.png", &tiny_png())])).err().unwrap(), "sceneTooNew");
        // Trop gros (annoncé).
        assert_eq!(read(Cursor::new(Vec::new()), MAX_FILE + 1).err().unwrap(), "tooLarge");
    }

    #[test]
    fn write_then_read_round_trips() {
        let s = image_scene();
        let files = vec![("a.png".to_string(), tiny_png())];
        let mut out = Cursor::new(Vec::new());
        write(&mut out, &s, &files, &tiny_png(), "Island test").unwrap();
        let bytes = out.into_inner();
        let r = read_bytes(&bytes).unwrap();
        assert_eq!(r.manifest.app, "Island test");
        assert_eq!(r.files, files);
        assert!(r.thumbnail.is_some());
    }
}
