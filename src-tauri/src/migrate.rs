// Ménage de la v2.0.0 — PROVISOIRE : à supprimer en v2.0.1.
//
// Exécuté une seule fois au premier démarrage de la v2 (marqueur `v2-migrated`). Retire ce
// que le système d'extensions v1 a laissé sur le PC :
//   • les extensions v1 (`extensions/`) et leurs données : stockage
//     (`storage/<id>.json`), permissions figées (`permissions.json`), cookies HTTP
//     (`cookies/`), secrets du Gestionnaire d'identifiants (cibles `…island-ext:<id>…`) ;
//   • les clés v1 du store `__app__` (`enabled`, `islandOffsetX`, `islandTheme`,
//     `islandAccent`).
// Ne touche ni aux extensions v2 (`extensions-v2/`, `ext2.*`, `island-ext2:`) ni à
// l'association des fichiers `.island` (réutilisée par la v2).

use std::path::Path;

use tauri::{AppHandle, Manager};

const MARKER: &str = "v2-migrated";
const V1_APP_KEYS: &[&str] = &["enabled", "islandOffsetX", "islandTheme", "islandAccent"];

pub fn run(app: &AppHandle) {
    let (Ok(data), Ok(config)) = (app.path().app_data_dir(), app.path().app_config_dir()) else { return };
    let marker = data.join(MARKER);
    if marker.exists() {
        return;
    }
    let ids = v1_ids(&data.join("extensions"));

    let _ = std::fs::remove_dir_all(data.join("extensions"));
    let _ = std::fs::remove_dir_all(data.join("cookies"));
    let _ = std::fs::remove_file(config.join("permissions.json"));
    for id in &ids {
        let _ = std::fs::remove_file(config.join("storage").join(format!("{}.json", sanitize(id))));
    }
    for key in V1_APP_KEYS {
        crate::store::set(app, "__app__", key, None);
    }
    #[cfg(target_os = "windows")]
    let secrets = delete_v1_secrets();
    #[cfg(not(target_os = "windows"))]
    let secrets = 0;

    eprintln!("[migration v2] {} extension(s) v1 retirée(s), {secrets} secret(s) effacé(s)", ids.len());
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::write(marker, env!("CARGO_PKG_VERSION"));
}

/// Identifiants des extensions v1 installées (manifeste, sinon nom du dossier).
fn v1_ids(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            std::fs::read_to_string(e.path().join("manifest.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|m| m.get("id").and_then(|v| v.as_str()).map(str::to_string))
                .unwrap_or_else(|| e.file_name().to_string_lossy().to_string())
        })
        .collect()
}

/// Même règle que le stockage (`store.rs`).
fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Efface les secrets des extensions v1 (service `island-ext:<id>`). Le trousseau ne
/// s'énumère pas par service : on filtre les cibles du Gestionnaire d'identifiants.
#[cfg(target_os = "windows")]
fn delete_v1_secrets() -> usize {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Security::Credentials::{CredDeleteW, CredEnumerateW, CredFree, CREDENTIALW, CRED_ENUMERATE_FLAGS};

    let mut count = 0u32;
    let mut creds: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let mut targets: Vec<(Vec<u16>, u32)> = Vec::new();
    unsafe {
        if CredEnumerateW(w!("*island-ext:*"), Some(CRED_ENUMERATE_FLAGS(0)), &mut count, &mut creds).is_err() {
            return 0;
        }
        for i in 0..count as usize {
            let c = &**creds.add(i);
            let name = c.TargetName.as_wide().to_vec();
            // Garde-fou : jamais les secrets des extensions v2 (`island-ext2:`).
            if String::from_utf16_lossy(&name).contains("island-ext:") {
                targets.push((name, c.Type.0));
            }
        }
        CredFree(creds as *const _);
        let mut deleted = 0;
        for (mut name, kind) in targets {
            name.push(0);
            if CredDeleteW(PCWSTR(name.as_ptr()), windows::Win32::Security::Credentials::CRED_TYPE(kind), None).is_ok() {
                deleted += 1;
            }
        }
        deleted
    }
}
