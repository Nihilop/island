// Service `apps` : énumération des applications (AppsFolder + Steam), icônes shell,
// lancement et recherche Everything, utilisés par le launcher natif. Implémentation
// native dans `windows.rs` / `files.rs`.

#[cfg(target_os = "windows")]
pub(crate) mod windows;
#[cfg(target_os = "windows")]
pub(crate) mod files;

/// Une application indexée (Win32 / UWP / jeu Steam).
#[derive(serde::Serialize)]
pub struct AppEntry {
    pub(crate) name: String,
    pub(crate) path: String,
}

/// Un fichier ou dossier trouvé par la recherche.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) is_dir: bool,
}
