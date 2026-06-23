// Service `apps` : indexation + lancement d'applications (launcher type Flow).
// Couche commande cross-platform (gated `apps`) ; impl native (Start Menu .lnk +
// ShellExecute + icônes shell) dans `windows.rs`.

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
mod files;

/// Une application indexée (Win32 / UWP / jeu Steam).
#[derive(serde::Serialize)]
pub struct AppEntry {
    name: String,
    path: String,
}

/// Un fichier ou dossier trouvé par la recherche.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
}

/// Liste les applications installées (Win32 + UWP + Steam). Gated par la permission `apps`.
/// COM requis (énumération AppsFolder) → travail sur un thread bloquant dédié.
#[tauri::command]
pub async fn list_apps(app: AppHandle, ext_id: String) -> Result<Vec<AppEntry>, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(|| {
            use ::windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            let apps = windows::list_apps();
            unsafe {
                CoUninitialize();
            }
            apps
        })
        .await
        .map_err(|e| e.to_string())
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

/// Lance une app / un fichier **en administrateur** (UAC). Gated `apps`.
#[tauri::command]
pub fn launch_admin(app: AppHandle, ext_id: String, path: String) -> Result<(), String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        windows::launch_admin(&path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("apps: Windows uniquement".into())
    }
}

/// Recherche fichiers/dossiers (Everything si dispo, sinon index maison des `roots`).
/// Gated `apps`. Travail sur un thread bloquant (scan FS / pompe IPC).
#[tauri::command]
pub async fn search_files(
    app: AppHandle,
    ext_id: String,
    query: String,
    roots: Vec<String>,
    limit: Option<usize>,
) -> Result<Vec<FileEntry>, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        let limit = limit.unwrap_or(20).clamp(1, 50);
        tauri::async_runtime::spawn_blocking(move || files::search(&query, roots, limit))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (query, roots, limit);
        Ok(Vec::new())
    }
}

/// Statut du moteur de recherche fichiers (pour les réglages) : Everything détecté ?
#[tauri::command]
pub fn files_engine(app: AppHandle, ext_id: String) -> Result<bool, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "apps") {
        return Err("apps: permission « apps » requise".into());
    }
    #[cfg(target_os = "windows")]
    {
        Ok(files::everything_available())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
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
