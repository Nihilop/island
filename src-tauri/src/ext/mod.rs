// Système d'extensions : helpers partagés (résolution de dossier, permissions,
// garde anti-path-traversal) + sous-modules storage / install / scaffold.
//
// Modèle de confiance : une extension ne peut accéder QU'À son propre dossier
// (`resolve_in_ext`) et QU'AUX services dont elle a déclaré la permission dans son
// manifeste (`ext_has_permission`) — défense en profondeur, en plus du consentement
// à l'install.

use tauri::{AppHandle, Manager};

pub mod install;
pub mod scaffold;
pub mod storage;

/// Garde de PROVENANCE : rejette l'appel s'il ne vient pas de la fenêtre `expected`.
/// Les extensions tournent dans la webview `overlay` → elles ne peuvent donc PAS atteindre
/// les commandes réservées aux fenêtres hôte (install / settings / create). Le param
/// `WebviewWindow` est injecté par Tauri (invisible côté JS → aucun changement d'appel).
pub(crate) fn require_window(window: &tauri::WebviewWindow, expected: &str) -> Result<(), String> {
    if window.label() == expected {
        Ok(())
    } else {
        Err(format!("commande réservée à la fenêtre « {expected} »"))
    }
}

/// Normalise un id d'extension en nom de dossier sûr (alphanum + `.-_`).
pub(crate) fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Dossier d'une extension installée (`%APPDATA%/<id>/extensions/<sanitize(id)>`).
pub(crate) fn ext_dir(app: &AppHandle, id: &str) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("extensions")
        .join(sanitize(id)))
}

// --- Permissions FIGÉES (source de vérité = `permissions.json`, hors d'atteinte des ext) ---
// Les permissions consenties sont gelées À L'INSTALL dans `<app_config>/permissions.json`.
// Ce fichier est à la RACINE du dossier de config : aucune commande accessible à une
// extension ne peut l'écrire (storage est confiné à `storage/<id>.json`, fetchBinary /
// resolve_in_ext au dossier de l'ext, pack/install gardés par fenêtre). Une extension ne
// peut donc plus s'auto-octroyer un droit en réécrivant son propre `manifest.json`.

fn perms_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("permissions.json"))
}
fn read_perms(app: &AppHandle) -> serde_json::Map<String, serde_json::Value> {
    perms_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn write_perms(app: &AppHandle, map: &serde_json::Map<String, serde_json::Value>) {
    if let Some(p) = perms_path(app) {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, serde_json::to_string(map).unwrap_or_default());
    }
}
/// Tableau `permissions` du manifeste sur disque (pour le seed de migration).
fn manifest_perms(app: &AppHandle, id: &str) -> Vec<serde_json::Value> {
    let path = match ext_dir(app, id) {
        Ok(d) => d.join("manifest.json"),
        Err(_) => return Vec::new(),
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("permissions").and_then(|p| p.as_array()).cloned())
        .unwrap_or_default()
}

/// FIGE les permissions consenties (appelé à l'install, AVANT que l'ext ne tourne jamais).
pub(crate) fn snapshot_perms(app: &AppHandle, id: &str, manifest: &serde_json::Value) {
    let perms = manifest.get("permissions").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    let mut m = read_perms(app);
    m.insert(id.to_string(), serde_json::Value::Array(perms));
    write_perms(app, &m);
}

/// L'extension a-t-elle `perm` ?
/// - **DEV** (dossier source avec `package.json`) → on lit le manifeste à CHAQUE fois (le dev
///   change ses permissions librement, pas de gel).
/// - **INSTALLÉE** → permissions FIGÉES à l'install (`permissions.json`, hors d'atteinte de
///   l'ext). Si pas d'entrée (install legacy d'avant le gel) → fallback manifeste.
/// Les ext installées via `install_island` ont TOUJOURS un snapshot → leur gel tient ; seuls
/// les paquets `.island` (manifest + dist, sans package.json) comptent comme « installés ».
pub(crate) fn ext_has_permission(app: &AppHandle, id: &str, perm: &str) -> bool {
    let dir = match ext_dir(app, id) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let list = if dir.join("package.json").exists() {
        manifest_perms(app, id) // dev : source de vérité = manifeste
    } else {
        match read_perms(app).remove(id) {
            Some(serde_json::Value::Array(a)) => a,
            _ => manifest_perms(app, id), // legacy sans snapshot
        }
    };
    list.iter().any(|x| x.as_str() == Some(perm))
}

/// Résout un chemin RELATIF dans le dossier de l'extension (interdit l'évasion :
/// `..`, chemins absolus, lettres de lecteur). L'extension ne peut donc lancer/écrire
/// QUE des fichiers de son propre dossier — pas un programme système.
pub(crate) fn resolve_in_ext(app: &AppHandle, id: &str, rel: &str) -> Result<std::path::PathBuf, String> {
    if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') || rel.contains(':') {
        return Err("chemin invalide".into());
    }
    let dir = ext_dir(app, id)?;
    let p = dir.join(rel);
    if !p.starts_with(&dir) {
        return Err("chemin hors de l'extension".into());
    }
    Ok(p)
}

/// Vérifie qu'une extension a déclaré une permission, sinon `return Err(...)`.
/// `$app: &AppHandle`, `$ext: &str`, `$perm: littéral`. Pour les commandes renvoyant
/// `Result<_, String>`.
macro_rules! require_perm {
    ($app:expr, $ext:expr, $perm:literal) => {
        if !$crate::ext::ext_has_permission($app, $ext, $perm) {
            return Err(concat!("permission « ", $perm, " » requise").into());
        }
    };
}
pub(crate) use require_perm;
