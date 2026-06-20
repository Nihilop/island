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

/// L'extension déclare-t-elle `perm` dans son manifeste ? (garde-fou côté serveur,
/// en plus du consentement à l'install — défense en profondeur).
pub(crate) fn ext_has_permission(app: &AppHandle, id: &str, perm: &str) -> bool {
    let path = match ext_dir(app, id) {
        Ok(d) => d.join("manifest.json"),
        Err(_) => return false,
    };
    let txt = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    serde_json::from_str::<serde_json::Value>(&txt)
        .ok()
        .and_then(|v| v.get("permissions").and_then(|p| p.as_array()).cloned())
        .map(|a| a.iter().any(|x| x.as_str() == Some(perm)))
        .unwrap_or(false)
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
