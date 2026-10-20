// Magasin d'Island côté app : lecture du catalogue et installation des scènes.
//
// Les appels passent par le Rust (pas par le webview) : pas de CORS, un agent identifiable
// (`Island/<version>`, que le filtre anti-robots de Cloudflare laisse passer) et le paquet
// n'est jamais exposé au front. L'installation télécharge le paquet (redirection vers un
// lien signé de R2), puis l'importe comme une scène partagée : mêmes vérifications que
// pour un fichier reçu (cf. `wallpaper::share`).
//
// Adresse du magasin : `ISLAND_STORE_URL` si défini, sinon le serveur local en
// développement et `https://store.streemkit.com` en production.
//
// Scènes installées : `__market__` (stockage clé → valeur d'Island), élément du magasin →
// { scène locale, version } ; sert à afficher « Installée » et, plus tard, les mises à jour.

use std::io::Read;
use std::time::Duration;

use island_formats::islandscene::MAX_FILE;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::wallpaper::share::{self, Imported};

const SPACE: &str = "__market__";

pub fn base_url() -> String {
    if let Ok(u) = std::env::var("ISLAND_STORE_URL") {
        return u.trim_end_matches('/').to_string();
    }
    if cfg!(debug_assertions) { "http://127.0.0.1:8080".into() } else { "https://store.streemkit.com".into() }
}

fn agent(app: &AppHandle, timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(&format!("Island/{}", app.package_info().version))
        .timeout_connect(Duration::from_secs(10))
        .timeout(timeout)
        .build()
}

/// Erreur lisible : le code renvoyé par le serveur (`{"error": "…"}`), sinon `offline`.
fn error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, res) => {
            let body: serde_json::Value = res.into_json().unwrap_or_default();
            body.get("error").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("http{code}"))
        }
        ureq::Error::Transport(_) => "offline".into(),
    }
}

/// Lecture seule de l'API publique (`/v1/items…`, `/v1/store/status`).
#[tauri::command]
pub async fn market_get(app: AppHandle, path: String, query: Option<Vec<(String, String)>>) -> Result<serde_json::Value, String> {
    let allowed = path.starts_with("/v1/items") || path == "/v1/store/status";
    if !allowed || path.contains("..") || path.contains("/download") {
        return Err("invalidPath".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut req = agent(&app, Duration::from_secs(20)).get(&format!("{}{path}", base_url()));
        for (k, v) in query.unwrap_or_default() {
            req = req.query(&k, &v);
        }
        req.call().map_err(error)?.into_json().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn market_base() -> String {
    base_url()
}

#[derive(Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// Scène locale créée par l'installation.
    pub scene: String,
    pub version: String,
}

fn installed_map(app: &AppHandle) -> serde_json::Map<String, serde_json::Value> {
    let mut map = crate::store::read_store(app, SPACE);
    // Scène supprimée depuis : l'élément n'est plus « installé ».
    let scenes: Vec<String> = crate::wallpaper::wallpaper_list(app.clone()).map(|l| l.into_iter().map(|e| e.id).collect()).unwrap_or_default();
    map.retain(|_, v| v.get("scene").and_then(|s| s.as_str()).is_some_and(|s| scenes.iter().any(|id| id == s)));
    map
}

/// Éléments du magasin installés : id de l'élément → { scène, version }.
#[tauri::command]
pub fn market_installed(app: AppHandle) -> serde_json::Map<String, serde_json::Value> {
    installed_map(&app)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Télécharge l'élément `id` (dernière version, ou `version`) et l'installe.
fn install(app: &AppHandle, id: &str, version: Option<&str>) -> Result<Imported, String> {
    if !valid_id(id) || version.is_some_and(|v| !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')) {
        return Err("invalidId".into());
    }
    let agent = agent(app, Duration::from_secs(600));
    let mut req = agent.get(&format!("{}/v1/items/{id}/download", base_url()));
    if let Some(v) = version {
        req = req.query("version", v);
    }
    // ureq suit la redirection vers le lien signé de R2.
    let res = req.call().map_err(error)?;
    let mut bytes = Vec::new();
    res.into_reader().take(MAX_FILE + 1).read_to_end(&mut bytes).map_err(|_| "offline".to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("tooLarge".into());
    }
    // Version effectivement installée (la fiche dit laquelle est la dernière).
    let version = match version {
        Some(v) => v.to_string(),
        None => {
            let d: serde_json::Value = agent.get(&format!("{}/v1/items/{id}", base_url())).call().map_err(error)?.into_json().map_err(|e| e.to_string())?;
            d.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string()
        }
    };
    let tmp = std::env::temp_dir().join(format!("island-market-{id}-{}.islandscene", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    let imported = share::import(app, &tmp);
    let _ = std::fs::remove_file(&tmp);
    let imported = imported?;
    let mut map = installed_map(app);
    map.insert(id.to_string(), serde_json::json!(Installed { scene: imported.id.clone(), version }));
    crate::store::write_store(app, SPACE, &map);
    let _ = app.emit("market://installed", ());
    Ok(imported)
}

#[tauri::command]
pub async fn market_install(app: AppHandle, id: String, version: Option<String>) -> Result<Imported, String> {
    tauri::async_runtime::spawn_blocking(move || install(&app, &id, version.as_deref())).await.map_err(|e| e.to_string())?
}
