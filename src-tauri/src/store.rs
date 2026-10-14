// Stockage clé → valeur persistant : un fichier JSON par espace (`<config>/storage/<nom>.json`).
// `__app__` = réglages d'Island ; `ext2.<id>` = stockage d'une extension v2 (cf.
// extensions/mod.rs). Cross-platform.

use tauri::{AppHandle, Manager};

/// Nom de fichier sûr (caractères hors `[A-Za-z0-9._-]` remplacés).
fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn store_file(app: &AppHandle, ext: &str) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("storage").join(format!("{}.json", sanitize(ext))))
}

pub(crate) fn read_store(app: &AppHandle, ext: &str) -> serde_json::Map<String, serde_json::Value> {
    store_file(app, ext)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn write_store(app: &AppHandle, ext: &str, map: &serde_json::Map<String, serde_json::Value>) {
    if let Some(p) = store_file(app, ext) {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, serde_json::to_string(map).unwrap_or_default());
    }
}

/// Sérialise les lecture-modification-écriture : les commandes Tauri tournent en
/// parallèle et l'hôte Rust écrit aussi dans `__app__` (placement du notch).
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn get(app: &AppHandle, ext: &str, key: &str) -> Option<serde_json::Value> {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    read_store(app, ext).get(key).cloned()
}

pub(crate) fn set(app: &AppHandle, ext: &str, key: &str, value: Option<serde_json::Value>) {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut m = read_store(app, ext);
    match value {
        Some(v) => m.insert(key.to_string(), v),
        None => m.remove(key),
    };
    write_store(app, ext, &m);
}

#[tauri::command]
pub fn storage_get(app: AppHandle, ext: String, key: String) -> Option<serde_json::Value> {
    get(&app, &ext, &key)
}
#[tauri::command]
pub fn storage_set(app: AppHandle, ext: String, key: String, value: serde_json::Value) {
    set(&app, &ext, &key, Some(value));
}
#[tauri::command]
pub fn storage_keys(app: AppHandle, ext: String) -> Vec<String> {
    read_store(&app, &ext).keys().cloned().collect()
}
