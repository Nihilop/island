// Storage : KV persistant par extension (1 JSON par extension). Cross-platform.

use tauri::{AppHandle, Manager};

use super::sanitize;

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

#[tauri::command]
pub fn storage_get(app: AppHandle, ext: String, key: String) -> Option<serde_json::Value> {
    read_store(&app, &ext).get(&key).cloned()
}
#[tauri::command]
pub fn storage_set(app: AppHandle, ext: String, key: String, value: serde_json::Value) {
    let mut m = read_store(&app, &ext);
    m.insert(key, value);
    write_store(&app, &ext, &m);
}
#[tauri::command]
pub fn storage_delete(app: AppHandle, ext: String, key: String) {
    let mut m = read_store(&app, &ext);
    m.remove(&key);
    write_store(&app, &ext, &m);
}
#[tauri::command]
pub fn storage_keys(app: AppHandle, ext: String) -> Vec<String> {
    read_store(&app, &ext).keys().cloned().collect()
}
