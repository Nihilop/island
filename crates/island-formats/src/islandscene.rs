//! Fichier de partage d'une scène, `.islandscene` (zip) : TOUT ce qu'il faut pour appliquer
//! et modifier la scène ailleurs.
//!
//! ```text
//! manifest.json   { "format": "islandscene", "version": 1, "name", "app",
//!                   "summary"?, "tags"?, "homepage"?, "repository"?, "license"? }
//! scene.json      la scène (format de `scene`)
//! store/…         médias de la fiche (convention de `media` : vignette = rendu de la scène)
//! <fichiers>      images et masques référencés, à plat (noms simples)
//! ```
//!
//! Compatibilité : un fichier de la 2.5.0 a sa vignette en `thumbnail.png` à la racine ;
//! elle est lue comme `store/thumbnail.png`.
//!
//! Aucun code : pas de signature, mais un fichier reçu peut être piégé, donc [`read`] vérifie
//! tout : taille du fichier et taille décompressée bornées, noms simples uniquement (rien ne
//! peut s'écrire hors du dossier de la scène), chaque image réellement décodée, scène validée,
//! format trop récent refusé. Codes d'erreur : `tooLarge`, `corrupt`, `sceneTooNew`,
//! `missingFile:<nom>`, `invalidImage:<nom>`, plus ceux de [`Scene::check`].

use std::collections::HashSet;
use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize};

use crate::media::{self, Media, StoreMeta};
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
    /// Résumé, étiquettes, liens, licence (fiche du magasin).
    #[serde(flatten)]
    pub store: StoreMeta,
}

/// Contenu vérifié d'un `.islandscene`.
pub struct Contents {
    pub manifest: Manifest,
    pub scene: Scene,
    /// Images et masques référencés par la scène (les autres fichiers sont ignorés).
    pub files: Vec<(String, Vec<u8>)>,
    /// Médias de la fiche (images décodées et vérifiées).
    pub media: Media,
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
    let (mut manifest, mut scene_json, mut legacy_thumbnail) = (None, None, None);
    let mut media = Media::default();
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|_| "corrupt")?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let known = matches!(name.as_str(), "manifest.json" | "scene.json" | "thumbnail.png");
        // Noms simples seulement (ou un média de `store/`, reconnu par la convention) : rien
        // ne peut s'écrire ailleurs que dans le dossier de la scène.
        let is_media = name.starts_with(media::PREFIX);
        if is_media && media::slot(&name).is_none() {
            return Err(format!("badMedia:{name}"));
        }
        if !known && !is_media && !scene::valid_file_name(&name) {
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
            "thumbnail.png" => legacy_thumbnail = Some(bytes),
            _ if is_media => media.add(&name, bytes)?,
            _ => files.push((name, bytes)),
        }
    }
    if media.thumbnail.is_none() {
        media.thumbnail = legacy_thumbnail.map(|b| (format!("{}thumbnail.png", media::PREFIX), b));
    }
    media.check()?;
    let manifest: Manifest = serde_json::from_slice(&manifest.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    if manifest.format != FORMAT {
        return Err("corrupt".into());
    }
    if manifest.version > FORMAT_VERSION {
        return Err("sceneTooNew".into());
    }
    let scene: Scene = serde_json::from_slice(&scene_json.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    scene.check()?;
    // Tout fichier référencé doit être là, et être une vraie image (ou une vraie vidéo) ; le
    // reste est ignoré.
    let needed: HashSet<String> = scene.images().cloned().collect();
    let videos: HashSet<&String> = scene.videos().collect();
    for f in &needed {
        let (_, bytes) = files.iter().find(|(n, _)| n == f).ok_or_else(|| format!("missingFile:{f}"))?;
        if videos.contains(f) {
            if !scene::is_video(f, bytes) {
                return Err(format!("invalidVideo:{f}"));
            }
            continue;
        }
        let img = image::load_from_memory(bytes).map_err(|_| format!("invalidImage:{f}"))?;
        if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
            return Err("tooLarge".into());
        }
    }
    files.retain(|(n, _)| needed.contains(n));
    Ok(Contents { manifest, scene, files, media })
}

/// Écrit un `.islandscene`. `files` : les images et masques de la scène (nom, octets) ;
/// `media` : médias de la fiche (`store/…`) ; `app` : « Island 2.5.1 ».
pub fn write<W: Write + Seek>(writer: W, scene: &Scene, files: &[(String, Vec<u8>)], media: &Media, store: &StoreMeta, app: &str) -> Result<(), String> {
    let manifest = Manifest { format: FORMAT.into(), version: FORMAT_VERSION, name: scene.name.clone(), app: app.into(), store: store.clone() };
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
    for (path, bytes) in media.entries() {
        put(&path, &bytes, if path.ends_with(".md") { deflate } else { stored })?;
    }
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

/// Vignette 480 × 270 approchée, quand la scène n'a pas de rendu de l'éditeur : les calques
/// visibles composés de bas en haut (couleurs, dégradés, images placées), avec opacité,
/// masque, mode de fusion et vignettage ; sans les particules ni les effets animés. `image`
/// charge un fichier de la scène par son nom.
pub fn thumbnail(s: &Scene, image: impl Fn(&str) -> Option<image::DynamicImage>) -> image::RgbaImage {
    let (w, h) = (480u32, 270u32);
    let mut px = vec![[0f32; 3]; (w * h) as usize];
    // De bas (`layers[0]`) en haut, comme le moteur ; les particules (animées) sont ignorées.
    for l in s.layers.iter().filter(|l| l.visible && l.opacity > 0.0) {
        let opacity = l.opacity.clamp(0.0, 1.0);
        match l.kind.as_str() {
            "fill" => {
                let mask = l.mask.as_deref().and_then(&image).map(|m| m.resize_exact(w, h, image::imageops::FilterType::Triangle).to_luma8());
                let color = fill_color(&l.fill);
                for y in 0..h {
                    for x in 0..w {
                        let a = opacity * mask.as_ref().map_or(1.0, |m| m.get_pixel(x, y).0[0] as f32 / 255.0);
                        let c = color(x as f32 / w as f32, y as f32 / h as f32);
                        blend(&mut px[(y * w + x) as usize], c, a, &l.blend);
                    }
                }
            }
            "image" => {
                let Some(img) = l.image.as_deref().and_then(&image) else { continue };
                let (cx, cy, hw, hh) = place(&l.transform, w as f32, h as f32, img.width() as f32, img.height() as f32);
                let (bw, bh) = ((hw * 2.0).round().max(1.0) as u32, (hh * 2.0).round().max(1.0) as u32);
                if bw > w * 6 || bh > h * 6 {
                    continue;
                }
                let tex = img.resize_exact(bw, bh, image::imageops::FilterType::Triangle).to_rgba8();
                let mask = l.mask.as_deref().and_then(&image).map(|m| m.resize_exact(bw, bh, image::imageops::FilterType::Triangle).to_luma8());
                let (x0, y0) = ((cx - hw).round() as i64, (cy - hh).round() as i64);
                for ty in 0..bh {
                    let y = y0 + ty as i64;
                    if y < 0 || y >= h as i64 {
                        continue;
                    }
                    for tx in 0..bw {
                        let x = x0 + tx as i64;
                        if x < 0 || x >= w as i64 {
                            continue;
                        }
                        let p = tex.get_pixel(tx, ty).0;
                        let m = mask.as_ref().map_or(1.0, |m| m.get_pixel(tx, ty).0[0] as f32 / 255.0);
                        let a = p[3] as f32 / 255.0 * opacity * m;
                        if a > 0.0 {
                            let c = [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0];
                            blend(&mut px[(y as u32 * w + x as u32) as usize], c, a, &l.blend);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Vignettage de la scène, comme le rendu final.
    let vignette = s.vignette.unwrap_or(0.0).clamp(0.0, 1.0);
    image::RgbaImage::from_fn(w, h, |x, y| {
        let c = px[(y * w + x) as usize];
        let (dx, dy) = (x as f32 / w as f32 - 0.5, y as f32 / h as f32 - 0.5);
        let v = 1.0 - vignette * ((dx * dx + dy * dy).sqrt() * 1.4).powf(2.0).min(1.0);
        let q = |i: usize| ((c[i] * v).clamp(0.0, 1.0) * 255.0).round() as u8;
        image::Rgba([q(0), q(1), q(2), 255])
    })
}

/// Couleur d'un calque couleur / dégradé en (u, v) ∈ [0,1]² (0° = vers la droite).
fn fill_color(fill: &scene::Fill) -> impl Fn(f32, f32) -> [f32; 3] {
    let (a, b) = (scene::hex(&fill.colors[0]), scene::hex(&fill.colors[1]));
    let solid = fill.mode == "solid";
    let ang = fill.angle.to_radians();
    let (dx, dy) = (ang.cos(), ang.sin());
    move |u, v| {
        let g = if solid { 0.0 } else { ((u - 0.5) * dx + (v - 0.5) * dy + 0.5).clamp(0.0, 1.0) };
        let g = g * g * (3.0 - 2.0 * g);
        [a[0] + (b[0] - a[0]) * g, a[1] + (b[1] - a[1]) * g, a[2] + (b[2] - a[2]) * g]
    }
}

/// Boîte d'un calque image (centre, demi-tailles), comme `scene.ts::layerBox` sans parallaxe.
fn place(t: &scene::Transform, w: f32, h: f32, iw: f32, ih: f32) -> (f32, f32, f32, f32) {
    let (iw, ih) = (iw.max(1.0), ih.max(1.0));
    match t.fit.as_str() {
        "cover" | "contain" => {
            let k = if t.fit == "cover" { (w / iw).max(h / ih) } else { (w / iw).min(h / ih) };
            (w * 0.5, h * 0.5, iw * k * 0.5, ih * k * 0.5)
        }
        "free" => {
            let hw = t.width.clamp(0.01, 4.0) * w * 0.5;
            let hh = hw * ih / iw;
            let a = &t.anchor;
            let ax = if a.ends_with('l') { 0.0 } else if a.ends_with('r') { 1.0 } else { 0.5 };
            let ay = if a.starts_with('t') { 0.0 } else if a.starts_with('b') { 1.0 } else { 0.5 };
            (ax * w + t.x.clamp(-2.0, 2.0) * w + (0.5 - ax) * 2.0 * hw, ay * h + t.y.clamp(-2.0, 2.0) * h + (0.5 - ay) * 2.0 * hh, hw, hh)
        }
        _ => (w * 0.5, h * 0.5, w * 0.5, h * 0.5),
    }
}

/// Mélange `src` (opacité `a`) sur `dst` selon le mode de fusion du calque.
fn blend(dst: &mut [f32; 3], src: [f32; 3], a: f32, mode: &str) {
    for i in 0..3 {
        let (d, s) = (dst[i], src[i]);
        let r = match mode {
            "add" => (d + s).min(1.0),
            "screen" => 1.0 - (1.0 - d) * (1.0 - s),
            "multiply" => d * s,
            _ => s,
        };
        dst[i] = d + (r - d) * a;
    }
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

    fn media_with(thumb: Vec<u8>) -> Media {
        Media { thumbnail: Some(("store/thumbnail.png".into(), thumb)), ..Default::default() }
    }

    fn read_bytes(b: &[u8]) -> Result<Contents, String> {
        read(Cursor::new(b), b.len() as u64)
    }

    fn tiny_png() -> Vec<u8> {
        png(&image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))).unwrap()
    }

    #[test]
    fn thumbnail_composites_layers_bottom_up() {
        use crate::scene::{Layer, Scene};
        let fill = Layer { kind: "fill".into(), fill: crate::scene::Fill { mode: "solid".into(), colors: ["#ff0000".into(), "#ff0000".into()], angle: 0.0 }, ..Default::default() };
        let mut img = Layer { kind: "image".into(), image: Some("demon.png".into()), ..Default::default() };
        img.transform.fit = "contain".into();
        let s = Scene { layers: vec![fill, img], ..Default::default() };
        // Image carrée bleue en « contain » : bleu au centre, fond rouge sur les côtés.
        let t = thumbnail(&s, |n| (n == "demon.png").then(|| image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 255, 255])))));
        assert_eq!(t.get_pixel(240, 135).0, [0, 0, 255, 255]);
        assert_eq!(t.get_pixel(10, 135).0, [255, 0, 0, 255]);
        // Un calque à moitié transparent par-dessus laisse voir ce qu'il y a dessous.
        let mut top = Layer { kind: "fill".into(), opacity: 0.5, ..Default::default() };
        top.fill = crate::scene::Fill { mode: "solid".into(), colors: ["#000000".into(), "#000000".into()], angle: 0.0 };
        let s = Scene { layers: vec![s.layers[0].clone(), s.layers[1].clone(), top], ..Default::default() };
        let t = thumbnail(&s, |n| (n == "demon.png").then(|| image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 255, 255])))));
        assert_eq!(t.get_pixel(240, 135).0, [0, 0, 128, 255]);
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
    fn video_layers_carry_a_real_video_and_need_a_newer_island() {
        let mut l = crate::scene::Layer { kind: "video".into(), video: Some("clip.mp4".into()), ..Default::default() };
        l.playback.speed = 0.5;
        let s = Scene { layers: vec![l], ..Default::default() };
        assert_eq!(s.required_version(), 2);
        assert_eq!(image_scene().required_version(), 1);
        let json = serde_json::to_vec(&s).unwrap();
        // En-tête MP4 minimal : taille de boîte, puis `ftyp`.
        let mp4 = b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2".to_vec();
        let r = read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("clip.mp4", &mp4)])).unwrap();
        assert_eq!(r.files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["clip.mp4"]);
        assert_eq!(r.scene.layers[0].playback.speed, 0.5);
        // Une « vidéo » qui n'en est pas une, ou une image déguisée en vidéo.
        assert!(read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("clip.mp4", b"pas une video")])).err().unwrap().starts_with("invalidVideo"));
        assert!(read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("clip.mp4", &tiny_png())])).err().unwrap().starts_with("invalidVideo"));
        // Une vidéo ne peut pas servir d'image (ni l'inverse).
        let bad = Scene { layers: vec![crate::scene::Layer { kind: "image".into(), image: Some("clip.mp4".into()), ..Default::default() }], ..Default::default() };
        assert_eq!(bad.check().err().unwrap(), "invalidImage");
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
        let meta = StoreMeta { summary: Some("Une scène".into()), tags: vec!["nuit".into()], ..Default::default() };
        let mut media = media_with(tiny_png());
        media.readme = Some("# Description".into());
        write(&mut out, &s, &files, &media, &meta, "Island test").unwrap();
        let bytes = out.into_inner();
        let r = read_bytes(&bytes).unwrap();
        assert_eq!(r.manifest.app, "Island test");
        assert_eq!(r.manifest.store, meta);
        assert_eq!(r.files, files);
        assert_eq!(r.media, media);
    }

    #[test]
    fn reads_a_2_5_0_file_and_refuses_unknown_media() {
        let json = serde_json::to_vec(&image_scene()).unwrap();
        // 2.5.0 : vignette à la racine → lue comme store/thumbnail.png.
        let r = read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", &tiny_png()), ("thumbnail.png", &tiny_png())])).unwrap();
        assert_eq!(r.media.thumbnail.unwrap().0, "store/thumbnail.png");
        let bad = read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", &tiny_png()), ("store/x.exe", b"MZ")]));
        assert_eq!(bad.err().unwrap(), "badMedia:store/x.exe");
        let fake = read_bytes(&zip_of(&[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", &tiny_png()), ("store/banner.png", b"faux")]));
        assert_eq!(fake.err().unwrap(), "invalidImage:store/banner.png");
    }
}
