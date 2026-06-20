// Service `media` : contrôle du média natif (lecture/pause, piste suivante/préc.,
// seek, volume). Couche commande cross-platform ; impl native (SMTC + volume WASAPI)
// dans `windows.rs`. Sur un OS sans impl, les commandes sont des no-op (volume = -1).

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;

/// Démarre l'écoute du média natif (appelé une fois au setup). No-op hors Windows.
pub fn start(app: AppHandle) {
    #[cfg(target_os = "windows")]
    windows::start(app);
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

// Toutes les commandes média sont gardées par la permission `media` (l'extension
// fournit son extId, lié automatiquement côté SDK). Sans permission : no-op.
fn allowed(app: &AppHandle, ext_id: &str) -> bool {
    crate::ext::ext_has_permission(app, ext_id, "media")
}

#[tauri::command]
pub fn media_toggle(app: AppHandle, ext_id: String) {
    if !allowed(&app, &ext_id) {
        return;
    }
    #[cfg(target_os = "windows")]
    windows::toggle();
}

#[tauri::command]
pub fn media_next(app: AppHandle, ext_id: String) {
    if !allowed(&app, &ext_id) {
        return;
    }
    #[cfg(target_os = "windows")]
    windows::next();
}

#[tauri::command]
pub fn media_prev(app: AppHandle, ext_id: String) {
    if !allowed(&app, &ext_id) {
        return;
    }
    #[cfg(target_os = "windows")]
    windows::prev();
}

#[tauri::command]
pub fn media_seek(app: AppHandle, ext_id: String, position_ms: i64) {
    if !allowed(&app, &ext_id) {
        return;
    }
    let _ = position_ms;
    #[cfg(target_os = "windows")]
    windows::seek(position_ms);
}

#[tauri::command]
pub fn media_get_volume(app: AppHandle, ext_id: String) -> f32 {
    if !allowed(&app, &ext_id) {
        return -1.0;
    }
    #[cfg(target_os = "windows")]
    {
        return windows::get_volume();
    }
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
