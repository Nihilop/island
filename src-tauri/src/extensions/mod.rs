// Extensions v2 (cf. docs/internal/extensions-v2-spec.md). Domaine distinct de `ext/`
// (système v1, obsolète) : dossier `<app_data>/extensions-v2/<id>/`.
//
// Isolation : le code d'une extension ne tourne QUE dans des iframes sandboxées
// (`allow-scripts`, origine opaque) servies par le protocole `island-ext` (cf. `protocol`).
// Une telle iframe ne peut pas appeler l'IPC Tauri (en-tête Origin `null` → requête
// rejetée par Tauri, et pas de clé d'invocation) : elle ne parle qu'à l'hôte (le front
// d'Island) par un MessageChannel, et c'est l'hôte qui sait quelle extension parle. Les
// commandes ci-dessous ne sont donc appelées que par le code d'Island, jamais par une
// extension.

pub mod install;
pub mod protocol;
pub mod services;
pub mod system;

// Format des paquets (identifiants, chemins, manifeste) : partagé avec le serveur du magasin.
pub use island_formats::package::{valid_host_pattern, valid_id, valid_rel, Manifest};

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Dossier racine des extensions v2.
pub fn root(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("extensions-v2")
}

/// Manifeste valide d'un dossier, ou None.
pub(crate) fn read_manifest_at(dir: &std::path::Path) -> Option<Manifest> {
    install::read_manifest(dir)
}

/// Dossier d'une extension : installée, sinon dossier du mode développeur.
pub fn ext_dir(app: &AppHandle, id: &str) -> Option<PathBuf> {
    if !valid_id(id) {
        return None;
    }
    let installed = root(app).join(id);
    if read_manifest_at(&installed).is_some_and(|m| m.id == id) {
        return Some(installed);
    }
    install::dev_dir(id)
}

/// Extension chargée depuis un dossier de développement ?
pub fn is_dev(app: &AppHandle, id: &str) -> bool {
    ext_dir(app, id).is_some_and(|d| !d.starts_with(root(app)))
}

/// Manifeste EFFECTIF d'une extension : celui du dossier, avec pour une extension
/// installée les permissions réduites à celles consenties à l'installation (aucune si
/// elle n'est pas passée par l'installation). En mode développeur : celles du manifeste.
pub fn manifest(app: &AppHandle, id: &str) -> Option<Manifest> {
    let dir = ext_dir(app, id)?;
    let mut m = read_manifest_at(&dir).filter(|m| m.id == id)?;
    if !is_dev(app, id) {
        let granted = install::state(app, id).map(|s| s.granted).unwrap_or_default();
        m.permissions.retain(|p| granted.contains(p));
    }
    Some(m)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub(crate) manifest: Manifest,
    /// Chargée depuis un dossier de développement.
    pub(crate) dev: bool,
}

/// Extensions v2 : installées, puis dossiers du mode développeur (non masqués).
#[tauri::command]
pub fn ext2_list(app: AppHandle) -> Vec<Installed> {
    let mut ids: Vec<String> = std::fs::read_dir(root(&app))
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    ids.extend(install::dev_folders().iter().filter_map(|f| read_manifest_at(f)).map(|m| m.id));
    ids.sort();
    ids.dedup();
    let mut out: Vec<Installed> = ids
        .iter()
        .filter_map(|id| manifest(&app, id).map(|m| Installed { dev: is_dev(&app, id), manifest: m }))
        .collect();
    out.sort_by(|a, b| a.manifest.name.to_lowercase().cmp(&b.manifest.name.to_lowercase()));
    out
}

/// Stockage clé → valeur PROPRE à une extension (l'identité vient de l'hôte).
#[tauri::command]
pub fn ext2_storage_get(app: AppHandle, id: String, key: String) -> Option<Value> {
    valid_id(&id).then(|| crate::store::storage_get(app, format!("ext2.{id}"), key)).flatten()
}

#[tauri::command]
pub fn ext2_storage_set(app: AppHandle, id: String, key: String, value: Value) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("invalidId".into());
    }
    crate::store::storage_set(app, format!("ext2.{id}"), key, value);
    Ok(())
}

/// Trace d'une extension (console de l'hôte).
#[tauri::command]
pub fn ext2_log(id: String, msg: String) {
    eprintln!("[ext {id}] {}", msg.chars().take(2000).collect::<String>());
}
