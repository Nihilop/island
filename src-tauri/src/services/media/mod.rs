// Service `media` : lecture en cours (GSMTC) + contrôles. Deux surfaces :
//   - commandes `np_*` de l'outil intégré « Lecture » (cœur, non gardées) ;
//   - commandes `media_*` de l'API extensions (gardées par la permission `media`, gelées
//     pendant la refonte v2 mais conservées).
// Impl native dans `windows.rs` ; no-op ailleurs.

use tauri::AppHandle;

#[cfg(target_os = "windows")]
pub(crate) mod windows;

/// État de la lecture émis sur `np://update`.
#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub active: bool,
    pub title: String,
    pub artist: String,
    /// Pochette en data URL (None tant qu'elle n'est pas disponible).
    pub art: Option<String>,
    pub playing: bool,
    /// Position extrapolée (s).
    pub position: f64,
    /// Durée (s), 0 = inconnue / flux.
    pub duration: f64,
    pub can_seek: bool,
    /// Identifiant de l'app source (AUMID).
    pub app_id: String,
}

pub fn start(app: AppHandle) {
    #[cfg(target_os = "windows")]
    windows::start(app);
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

// --- Outil intégré -------------------------------------------------------------------

#[tauri::command]
pub fn np_state() -> Option<NowPlaying> {
    #[cfg(target_os = "windows")]
    return windows::snapshot();
    #[allow(unreachable_code)]
    None
}

#[tauri::command]
pub fn np_toggle() {
    #[cfg(target_os = "windows")]
    windows::toggle();
}

#[tauri::command]
pub fn np_next() {
    #[cfg(target_os = "windows")]
    windows::next();
}

#[tauri::command]
pub fn np_prev() {
    #[cfg(target_os = "windows")]
    windows::prev();
}

#[tauri::command]
pub fn np_seek(seconds: f64) {
    let _ = seconds;
    #[cfg(target_os = "windows")]
    windows::seek(seconds);
}

/// Amène l'app source au premier plan (bloquant → thread dédié).
#[tauri::command]
pub async fn np_open_source() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(windows::open_source)
        .await
        .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    Err("non supporté".into())
}

// --- API extensions (permission `media`) ---------------------------------------------

fn allowed(app: &AppHandle, ext_id: &str) -> bool {
    crate::ext::ext_has_permission(app, ext_id, "media")
}

#[tauri::command]
pub fn media_toggle(app: AppHandle, ext_id: String) {
    if allowed(&app, &ext_id) {
        np_toggle();
    }
}

#[tauri::command]
pub fn media_next(app: AppHandle, ext_id: String) {
    if allowed(&app, &ext_id) {
        np_next();
    }
}

#[tauri::command]
pub fn media_prev(app: AppHandle, ext_id: String) {
    if allowed(&app, &ext_id) {
        np_prev();
    }
}

#[tauri::command]
pub fn media_seek(app: AppHandle, ext_id: String, position_ms: i64) {
    if allowed(&app, &ext_id) {
        np_seek(position_ms as f64 / 1000.0);
    }
}

#[tauri::command]
pub fn media_get_volume(app: AppHandle, ext_id: String) -> f32 {
    if !allowed(&app, &ext_id) {
        return -1.0;
    }
    #[cfg(target_os = "windows")]
    return windows::get_volume();
    #[allow(unreachable_code)]
    -1.0
}

#[tauri::command]
pub fn media_set_volume(app: AppHandle, ext_id: String, level: f32) {
    if !allowed(&app, &ext_id) {
        return;
    }
    let _ = level;
    #[cfg(target_os = "windows")]
    windows::set_volume(level);
}
