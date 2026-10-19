// Partage de scènes : un fichier `.islandscene` (zip) qui contient TOUT — la scène, ses
// images et ses masques, une vignette — pour l'envoyer à quelqu'un qui pourra l'appliquer
// et la modifier.
//
//   manifest.json   { "format": "islandscene", "version": 1, "name", "app" }
//   scene.json      la scène (format de `scene.rs`)
//   thumbnail.png   vignette 480 × 270 (le calque de fond)
//   <fichiers>      images et masques référencés, à plat (noms simples)
//
// Aucun code : pas de signature (contrairement aux extensions), mais un fichier reçu peut
// être piégé, donc tout est vérifié à l'import : taille du fichier et taille décompressée
// bornées, noms simples uniquement (rien ne s'écrit hors du dossier de la scène), chaque
// image réellement décodée, scène validée comme les autres, format trop récent refusé.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::scene::{self, Scene};

pub const EXT: &str = "islandscene";
const FORMAT: &str = "islandscene";
const FORMAT_VERSION: u32 = 1;

/// Fichier reçu : 256 Mo ; décompressé : 384 Mo au total, 96 Mo par fichier ; 64 entrées.
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 384 * 1024 * 1024;
const MAX_ENTRY: u64 = 96 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;
/// Côté max d'une image importée (décodée pour vérification).
const MAX_SIDE: u32 = 16384;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    app: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub id: String,
    pub name: String,
}

// --- Export ------------------------------------------------------------------------------

/// Vignette 480 × 270 : le calque de fond (le plus bas des calques visibles), image recadrée
/// ou couleur / dégradé.
fn thumbnail(s: &Scene, dir: Option<&Path>) -> image::RgbaImage {
    let (w, h) = (480u32, 270u32);
    let bottom = s.layers.iter().find(|l| l.visible && (l.kind == "fill" || (l.kind == "image" && l.image.is_some())));
    if let (Some(l), Some(dir)) = (bottom, dir) {
        if l.kind == "image" {
            if let Some(img) = l.image.as_ref().and_then(|f| image::open(dir.join(f)).ok()) {
                return img.resize_to_fill(w, h, image::imageops::FilterType::Triangle).to_rgba8();
            }
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

fn png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Écrit la scène `id` dans `path` (qui doit finir par `.islandscene`).
pub fn export(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    if path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() != Some(EXT) {
        return Err("invalidPath".into());
    }
    let (s, dir) = super::load(app, id)?;
    let manifest = Manifest { format: FORMAT.into(), version: FORMAT_VERSION, name: s.name.clone(), app: format!("Island {}", app.package_info().version) };
    let tmp = path.with_extension("islandscene.part");
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // Images et masques sont déjà compressés : stockés tels quels.
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut put = |name: &str, bytes: &[u8], opts| -> Result<(), String> {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())
    };
    put("manifest.json", &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?, deflate)?;
    put("scene.json", &serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?, deflate)?;
    put("thumbnail.png", &png(&thumbnail(&s, dir.as_deref()))?, stored)?;
    if let Some(dir) = &dir {
        let mut seen = std::collections::HashSet::new();
        for f in s.images() {
            if seen.insert(f.clone()) {
                let bytes = std::fs::read(dir.join(f)).map_err(|_| format!("missingFile:{f}"))?;
                put(f, &bytes, stored)?;
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

// --- Import ------------------------------------------------------------------------------

/// Lit et vérifie un `.islandscene` ; renvoie la scène et ses fichiers (déjà validés).
fn read(path: &Path) -> Result<(Scene, Vec<(String, Vec<u8>)>), String> {
    let meta = std::fs::metadata(path).map_err(|_| "notFound")?;
    if meta.len() > MAX_FILE {
        return Err("tooLarge".into());
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?).map_err(|_| "corrupt")?;
    if zip.len() > MAX_ENTRIES {
        return Err("corrupt".into());
    }
    let mut total = 0u64;
    let (mut manifest, mut scene_json) = (None, None);
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
            "thumbnail.png" => {}
            _ => files.push((name, bytes)),
        }
    }
    let m: Manifest = serde_json::from_slice(&manifest.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    if m.format != FORMAT {
        return Err("corrupt".into());
    }
    if m.version > FORMAT_VERSION {
        return Err("sceneTooNew".into());
    }
    let s: Scene = serde_json::from_slice(&scene_json.ok_or("corrupt")?).map_err(|_| "corrupt")?;
    s.check()?;
    // Tout fichier référencé doit être là, et être une vraie image ; le reste est ignoré.
    let needed: std::collections::HashSet<&String> = s.images().collect();
    for f in &needed {
        let (_, bytes) = files.iter().find(|(n, _)| n == *f).ok_or_else(|| format!("missingFile:{f}"))?;
        let img = image::load_from_memory(bytes).map_err(|_| format!("invalidImage:{f}"))?;
        if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
            return Err("tooLarge".into());
        }
    }
    files.retain(|(n, _)| needed.contains(n));
    Ok((s, files))
}

fn new_id() -> String {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("s-{stamp:x}-{:08x}", u32::from_le_bytes(b))
}

/// « Nom », ou « Nom (2) »… si une de tes scènes porte déjà ce nom.
fn unique_name(app: &AppHandle, name: &str) -> String {
    let base: String = name.trim().chars().take(70).collect();
    let base = if base.is_empty() { "Scène".to_string() } else { base };
    let taken: Vec<String> = super::wallpaper_list(app.clone()).map(|l| l.into_iter().map(|e| e.scene.name).collect()).unwrap_or_default();
    if !taken.contains(&base) {
        return base;
    }
    (2..1000).map(|n| format!("{base} ({n})")).find(|n| !taken.contains(n)).unwrap_or(base)
}

/// Importe un `.islandscene` : nouvelle scène (à toi, modifiable). Renvoie son id et son nom.
pub fn import(app: &AppHandle, path: &Path) -> Result<Imported, String> {
    let (mut s, files) = read(path)?;
    s.name = unique_name(app, &s.name);
    let id = new_id();
    let dir = super::scene_dir(app, &id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let write = || -> Result<(), String> {
        for (name, bytes) in &files {
            std::fs::write(dir.join(name), bytes).map_err(|e| e.to_string())?;
        }
        std::fs::write(dir.join("scene.json"), serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    let _ = app.emit("wallpaper://changed", ());
    Ok(Imported { id, name: s.name })
}

// --- Commandes ---------------------------------------------------------------------------

#[tauri::command]
pub async fn wallpaper_export(app: AppHandle, id: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || export(&app, &id, &PathBuf::from(path))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wallpaper_import(app: AppHandle, path: String) -> Result<Imported, String> {
    tauri::async_runtime::spawn_blocking(move || import(&app, &PathBuf::from(path))).await.map_err(|e| e.to_string())?
}

// --- Double-clic sur un fichier ------------------------------------------------------------

/// Résultats d'imports faits avant que le front soit prêt (ouverture au démarrage).
static PENDING: Mutex<Vec<Result<Imported, String>>> = Mutex::new(Vec::new());

/// argv (démarrage ou seconde instance) : importe les `.islandscene` reçus, puis prévient le
/// front (`wallpaper://imported`), qui affiche une notification.
pub fn handle_argv(app: &AppHandle, argv: &[String]) {
    for path in argv.iter().filter(|a| a.to_lowercase().ends_with(&format!(".{EXT}"))) {
        let r = import(app, Path::new(path));
        PENDING.lock().unwrap_or_else(|p| p.into_inner()).push(r.clone());
        let _ = app.emit("wallpaper://imported", ());
    }
}

/// Imports en attente (le front les vide au démarrage et à chaque `wallpaper://imported`).
#[tauri::command]
pub fn wallpaper_take_imported() -> Vec<Result<Imported, String>> {
    std::mem::take(&mut *PENDING.lock().unwrap_or_else(|p| p.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, b) in entries {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }

    fn tiny_png() -> Vec<u8> {
        png(&image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))).unwrap()
    }

    fn manifest() -> Vec<u8> {
        br#"{"format":"islandscene","version":1,"name":"t"}"#.to_vec()
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("island-test-{}-{name}.islandscene", std::process::id()))
    }

    #[test]
    fn reads_a_valid_scene_and_drops_unreferenced_files() {
        let mut s = Scene::default();
        s.layers.push(scene::Layer { id: "a".into(), kind: "image".into(), image: Some("a.png".into()), ..Default::default() });
        let p = tmp("ok");
        let json = serde_json::to_vec(&s).unwrap();
        write_zip(&p, &[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", &tiny_png()), ("extra.png", &tiny_png())]);
        let (scene, files) = read(&p).unwrap();
        assert_eq!(scene.layers.len(), 1);
        assert_eq!(files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a.png"]);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn rejects_paths_missing_files_and_fake_images() {
        let mut s = Scene::default();
        s.layers.push(scene::Layer { id: "a".into(), kind: "image".into(), image: Some("a.png".into()), ..Default::default() });
        let json = serde_json::to_vec(&s).unwrap();
        // Chemin dans l'archive (tentative d'écrire ailleurs).
        let p = tmp("path");
        write_zip(&p, &[("manifest.json", &manifest()), ("scene.json", &json), ("../a.png", &tiny_png())]);
        assert!(read(&p).is_err());
        // Fichier référencé absent.
        write_zip(&p, &[("manifest.json", &manifest()), ("scene.json", &json)]);
        assert!(read(&p).unwrap_err().starts_with("missingFile"));
        // « Image » qui n'en est pas une.
        write_zip(&p, &[("manifest.json", &manifest()), ("scene.json", &json), ("a.png", b"pas une image")]);
        assert!(read(&p).unwrap_err().starts_with("invalidImage"));
        // Format trop récent.
        write_zip(&p, &[("manifest.json", br#"{"format":"islandscene","version":99}"#), ("scene.json", &json), ("a.png", &tiny_png())]);
        assert_eq!(read(&p).unwrap_err(), "sceneTooNew");
        let _ = std::fs::remove_file(p);
    }
}
