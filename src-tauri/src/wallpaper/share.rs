// Partage de scènes (`.islandscene`) côté app : export d'une scène enregistrée, import en
// nouvelle scène, double-clic sur un fichier. Le FORMAT et ses vérifications (taille,
// noms simples, images décodées, scène validée, version) sont dans
// `island_formats::islandscene`, partagé avec le serveur du magasin.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use island_formats::islandscene::{self as format, EXT};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub id: String,
    pub name: String,
}

// --- Export ------------------------------------------------------------------------------

/// Écrit la scène `id` dans `path` (qui doit finir par `.islandscene`).
pub fn export(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    if path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() != Some(EXT) {
        return Err("invalidPath".into());
    }
    let (s, dir) = super::load(app, id)?;
    let mut files = Vec::new();
    if let Some(dir) = &dir {
        for f in s.images() {
            if !files.iter().any(|(n, _): &(String, Vec<u8>)| n == f) {
                files.push((f.clone(), std::fs::read(dir.join(f)).map_err(|_| format!("missingFile:{f}"))?));
            }
        }
    }
    let thumb = format::thumbnail(&s, |f| dir.as_ref().and_then(|d| image::open(d.join(f)).ok()));
    let tmp = path.with_extension("islandscene.part");
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let written = format::write(file, &s, &files, &format::png(&thumb)?, &format!("Island {}", app.package_info().version));
    match written.and_then(|_| std::fs::rename(&tmp, path).map_err(|e| e.to_string())) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

// --- Import ------------------------------------------------------------------------------

fn read(path: &Path) -> Result<format::Contents, String> {
    let len = std::fs::metadata(path).map_err(|_| "notFound")?.len();
    format::read(std::fs::File::open(path).map_err(|e| e.to_string())?, len)
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
    let format::Contents { scene: mut s, files, .. } = read(path)?;
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
