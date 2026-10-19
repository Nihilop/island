// Service `desktop` : le bureau Windows.
//   - volume principal (lecture rapide, sans énumérer les sessions comme le mixer) ;
//   - fond d'écran statique (`IDesktopWallpaper`) : outils intégrés, automatisations et
//     extensions (permission `wallpaper`, image fournie en données, jamais un chemin) ;
//   - calque du fond (`WorkerW`) : fenêtre derrière les icônes où les fonds animés
//     (`crate::wallpaper`) se dessinent, et « le curseur est-il sur le bureau ? ».
// Le mode Focus de Windows (`FocusSessionManager`) n'est pas utilisable : réservé par
// Microsoft aux apps packagées (« Feature … focussessionmanager … is not available »).
// La tuile Focus de la vue d'ensemble pilote donc le mode focus du notch.
// L'implémentation native vit dans `windows.rs`.

use base64::Engine;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[derive(serde::Serialize)]
pub struct Volume {
    level: f32,
    muted: bool,
}

#[tauri::command]
pub async fn master_volume() -> Option<Volume> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(|| crate::services::system::master_volume())
        .await
        .ok()
        .flatten()
        .map(|(level, muted)| Volume { level, muted });
    #[allow(unreachable_code)]
    None
}

// --- Fond d'écran statique ---------------------------------------------------------------

/// Fond actuel d'un écran (`display` = id de `hardware::displays`).
#[derive(serde::Serialize, Clone)]
pub struct Wallpaper {
    pub display: String,
    /// Chemin de l'image ("" = couleur unie ou diaporama).
    pub path: String,
}

/// Disposition de l'image (comme les Paramètres Windows). Globale à tous les écrans.
pub const FITS: &[&str] = &["fill", "fit", "stretch", "center", "tile", "span"];

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "bmp", "webp", "gif", "jfif", "tif", "tiff"];

fn check_image_path(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !p.is_absolute() || !p.is_file() || !IMAGE_EXT.contains(&ext.as_str()) {
        return Err("invalidImage".into());
    }
    Ok(())
}

fn check_fit(fit: &Option<String>) -> Result<(), String> {
    match fit {
        Some(f) if !FITS.contains(&f.as_str()) => Err("invalidFit".into()),
        _ => Ok(()),
    }
}

/// Fonds actuels, par écran.
pub fn wallpapers() -> Result<Vec<Wallpaper>, String> {
    #[cfg(target_os = "windows")]
    return windows::wallpapers();
    #[allow(unreachable_code)]
    Err("unsupported".into())
}

/// Pose une image (`display` None = tous les écrans).
pub fn set_wallpaper(path: &str, display: Option<&str>, fit: Option<&str>) -> Result<(), String> {
    check_image_path(path)?;
    #[cfg(target_os = "windows")]
    return windows::set_wallpaper(path, display, fit);
    #[allow(unreachable_code)]
    {
        let _ = (display, fit);
        Err("unsupported".into())
    }
}

#[tauri::command]
pub async fn desktop_wallpaper_get() -> Result<Vec<Wallpaper>, String> {
    tauri::async_runtime::spawn_blocking(wallpapers).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn desktop_wallpaper_set(path: String, display: Option<String>, fit: Option<String>) -> Result<(), String> {
    check_fit(&fit)?;
    tauri::async_runtime::spawn_blocking(move || set_wallpaper(&path, display.as_deref(), fit.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

// --- Extensions (permission `wallpaper`) --------------------------------------------------

/// Image d'une extension : 30 Mo max une fois décodée du base64.
const EXT_MAX_BYTES: usize = 30 * 1024 * 1024;

#[tauri::command]
pub fn ext2_wallpaper_displays(app: AppHandle, id: String) -> Result<Vec<crate::services::hardware::DisplayInfo>, String> {
    crate::extensions::services::require(&app, &id, "wallpaper")?;
    Ok(crate::services::hardware::displays())
}

/// L'extension fournit l'image en base64 : Island la valide (vraie image décodable), la
/// range dans son dossier (`wallpapers/ext/<id>/`) et la pose. Aucun chemin n'est accepté.
#[tauri::command]
pub async fn ext2_wallpaper_set(app: AppHandle, id: String, data: String, display: Option<String>, fit: Option<String>) -> Result<(), String> {
    crate::extensions::services::require(&app, &id, "wallpaper")?;
    check_fit(&fit)?;
    if data.len() > EXT_MAX_BYTES / 3 * 4 + 4 {
        return Err("tooLarge".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "invalidImage")?;
    let format = image::guess_format(&bytes).map_err(|_| "invalidImage")?;
    image::load_from_memory_with_format(&bytes, format).map_err(|_| "invalidImage")?;
    let ext = format.extensions_str().first().copied().unwrap_or("png");
    if let Some(d) = &display {
        if !crate::services::hardware::displays().iter().any(|x| &x.id == d) {
            return Err("invalidDisplay".into());
        }
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("wallpapers").join("ext").join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Un fichier par écran (horodaté : Windows met en cache un même chemin).
    let slot: String = display.as_deref().unwrap_or("all").chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if e.file_name().to_string_lossy().starts_with(&format!("{slot}-")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("{slot}-{stamp}.{ext}"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let p = path.to_string_lossy().to_string();
    tauri::async_runtime::spawn_blocking(move || set_wallpaper(&p, display.as_deref(), fit.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}
