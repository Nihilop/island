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

pub mod protocol;
pub mod services;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Dossier racine des extensions v2.
pub fn root(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("extensions-v2")
}

/// Identifiant valide : nom de domaine inversé, `[a-z0-9-]` par segment, au moins deux
/// segments, pas de segment vide (donc ni `..` ni `.`).
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 100
        && parts.len() >= 2
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// Chemin relatif sûr (pas d'absolu, pas de `..`, pas de lecteur).
pub(crate) fn valid_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && !path.contains(':')
        && path.split(['/', '\\']).all(|s| !s.is_empty() && s != "." && s != "..")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDecl {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub view: String,
    #[serde(default = "default_size")]
    pub size: [u32; 2],
}

fn default_size() -> [u32; 2] {
    [520, 300]
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributes {
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
    #[serde(default)]
    pub live: Vec<Value>,
    #[serde(default)]
    pub launcher: Vec<Value>,
    #[serde(default)]
    pub commands: Vec<Value>,
    #[serde(default)]
    pub settings: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub main: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub locales: std::collections::BTreeMap<String, String>,
}

impl Manifest {
    /// Validation stricte : identifiant, chemins relatifs sûrs, tailles bornées.
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!("identifiant invalide : {}", self.id));
        }
        if self.name.trim().is_empty() || self.version.trim().is_empty() {
            return Err("nom ou version manquant".into());
        }
        let mut paths = vec![self.main.as_str()];
        paths.extend(self.icon.as_deref());
        paths.extend(self.contributes.tools.iter().map(|t| t.view.as_str()));
        paths.extend(self.contributes.tools.iter().filter_map(|t| t.icon.as_deref()));
        paths.extend(self.locales.values().map(String::as_str));
        if let Some(bad) = paths.into_iter().find(|p| !valid_rel(p)) {
            return Err(format!("chemin invalide : {bad}"));
        }
        for t in &self.contributes.tools {
            if !valid_id(&format!("x.{}", t.id)) {
                return Err(format!("outil invalide : {}", t.id));
            }
        }
        Ok(())
    }
}

/// Manifeste d'une extension installée (lu et validé), ou None.
pub fn manifest(app: &AppHandle, id: &str) -> Option<Manifest> {
    if !valid_id(id) {
        return None;
    }
    let txt = std::fs::read_to_string(root(app).join(id).join("manifest.json")).ok()?;
    let m: Manifest = serde_json::from_str(&txt).ok()?;
    (m.id == id && m.validate().is_ok()).then_some(m)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    manifest: Manifest,
}

/// Extensions v2 installées (manifeste valide et dont le dossier porte l'identifiant).
#[tauri::command]
pub fn ext2_list(app: AppHandle) -> Vec<Installed> {
    let Ok(rd) = std::fs::read_dir(root(&app)) else { return Vec::new() };
    let mut out: Vec<Installed> = rd
        .flatten()
        .filter_map(|e| manifest(&app, &e.file_name().to_string_lossy()))
        .map(|manifest| Installed { manifest })
        .collect();
    out.sort_by(|a, b| a.manifest.name.to_lowercase().cmp(&b.manifest.name.to_lowercase()));
    out
}

/// Stockage clé → valeur PROPRE à une extension (l'identité vient de l'hôte).
#[tauri::command]
pub fn ext2_storage_get(app: AppHandle, id: String, key: String) -> Option<Value> {
    valid_id(&id).then(|| crate::ext::storage::storage_get(app, format!("ext2.{id}"), key)).flatten()
}

#[tauri::command]
pub fn ext2_storage_set(app: AppHandle, id: String, key: String, value: Value) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("invalidId".into());
    }
    crate::ext::storage::storage_set(app, format!("ext2.{id}"), key, value);
    Ok(())
}

/// Trace d'une extension (console de l'hôte).
#[tauri::command]
pub fn ext2_log(id: String, msg: String) {
    eprintln!("[ext {id}] {}", msg.chars().take(2000).collect::<String>());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(valid_id("dev.island.github"));
        assert!(valid_id("com.example.my-ext"));
        assert!(!valid_id(".."));
        assert!(!valid_id("a..b"));
        assert!(!valid_id("single"));
        assert!(!valid_id("Dev.Island"));
        assert!(!valid_id("__app__.x"));
    }

    #[test]
    fn rel_paths() {
        assert!(valid_rel("dist/main.js"));
        assert!(!valid_rel("../x"));
        assert!(!valid_rel("/etc/passwd"));
        assert!(!valid_rel("C:\\x"));
        assert!(!valid_rel("dist//x"));
        assert!(!valid_rel("dist/./x"));
    }
}
