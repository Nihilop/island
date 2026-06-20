// Service `windows` : conscience des fenêtres du bureau — fenêtre au premier plan,
// liste des fenêtres top-level, focus. Couche commande/permission cross-platform ;
// impl native (Win32) dans `windows.rs`. ⚠ Révèle l'activité (apps/titres ouverts) →
// gardé par la permission `windows`.

use tauri::AppHandle;

use crate::ext::require_perm;

#[cfg(target_os = "windows")]
mod windows;

/// Une fenêtre du bureau.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    id: i64,      // handle natif (HWND), opaque côté extension
    title: String,
    app: String,  // nom d'exécutable sans extension (ex. "chrome", "spotify", "code")
}

/// Fenêtre actuellement au premier plan, ou `None`. Gated `windows`.
#[tauri::command]
pub fn window_foreground(app: AppHandle, ext_id: String) -> Option<WindowInfo> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "windows") {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        windows::foreground().map(|(id, title, a)| WindowInfo { id, title, app: a })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Fenêtres top-level visibles (titrées, hors tool windows). Gated `windows`.
#[tauri::command]
pub fn window_list(app: AppHandle, ext_id: String) -> Vec<WindowInfo> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "windows") {
        return Vec::new();
    }
    #[cfg(target_os = "windows")]
    {
        windows::list()
            .into_iter()
            .map(|(id, title, a)| WindowInfo { id, title, app: a })
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// Met une fenêtre au premier plan (par son `id`). Gated `windows`.
#[tauri::command]
pub fn window_focus(app: AppHandle, ext_id: String, id: i64) -> Result<(), String> {
    require_perm!(&app, &ext_id, "windows");
    #[cfg(target_os = "windows")]
    {
        windows::focus(id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = id;
        Err("windows: Windows uniquement".into())
    }
}
