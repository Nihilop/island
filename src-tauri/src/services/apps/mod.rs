// Service `apps` : indexation + lancement d'applications (launcher type Flow).
// Couche commande cross-platform (gated `apps`) ; impl native (Start Menu .lnk +
// ShellExecute + icônes shell) dans `windows.rs`.

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;

/// Une application indexée (raccourci du menu Démarrer).
#[derive(serde::Serialize)]
pub struct AppEntry {
    name: String,
    path: String,
}

/// Liste les applications installées. Gated par la permission `apps`.
#[tauri::command]
pub fn list_apps(app: AppHandle, ext_id: String) -> Result<Vec<AppEntry>, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        Ok(windows::list_apps())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

/// Lance une app / ouvre un fichier (un `.lnk` exécute sa cible). Gated `apps`.
#[tauri::command]
pub fn launch_path(app: AppHandle, ext_id: String, path: String) -> Result<(), String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    // On ne lance que des chemins issus de l'index (le binaire reste hors de l'ext,
    // mais la permission + le consentement à l'install encadrent la capacité).
    #[cfg(target_os = "windows")]
    {
        windows::launch(&path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("apps: Windows uniquement".into())
    }
}

/// Icônes (PNG data URL) des chemins demandés, dans le même ordre (None si échec).
/// À la demande pour les résultats visibles. Gated `apps`. COM sur thread dédié.
#[tauri::command]
pub async fn app_icons(app: AppHandle, ext_id: String, paths: Vec<String>) -> Result<Vec<Option<String>>, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || {
            use ::windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            let icons: Vec<Option<String>> = paths.iter().map(|p| windows::icon_data_url(p, 48)).collect();
            unsafe {
                CoUninitialize();
            }
            icons
        })
        .await
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = paths;
        Ok(Vec::new())
    }
}
