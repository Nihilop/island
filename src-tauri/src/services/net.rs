// Service `network` : HTTP natif avec cookie-jar PAR EXTENSION (comme dio + cookie_jar
// de Flutter). Une extension consomme une API tierce avec session par cookie SANS les
// restrictions CORS/SameSite d'un fetch navigateur (requête native, hors webview).
// Le cookie-jar est PERSISTÉ sur disque (1 JSON par extension) → la session survit aux
// redémarrages (plus besoin de se reconnecter). Cross-platform (ureq + cookie_store).
//
// Frontière de confiance : l'extension fournit l'URL COMPLÈTE → surface SSRF. La
// permission `network` + le consentement à l'install encadrent la capacité.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

static HTTP_AGENTS: Mutex<Option<std::collections::HashMap<String, ureq::Agent>>> = Mutex::new(None);

/// Fichier de cookies persistés d'une extension (`%APPDATA%/<id>/cookies/<ext>.json`).
fn cookie_file(app: &AppHandle, ext_id: &str) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("cookies").join(format!("{}.json", crate::ext::sanitize(ext_id))))
}

/// Agent ureq de l'extension, SEED avec ses cookies persistés (session restaurée).
fn ext_agent(app: &AppHandle, ext_id: &str) -> ureq::Agent {
    let mut g = HTTP_AGENTS.lock().unwrap_or_else(|p| p.into_inner());
    let map = g.get_or_insert_with(std::collections::HashMap::new);
    map.entry(ext_id.to_string())
        .or_insert_with(|| {
            let store = cookie_file(app, ext_id)
                .and_then(|p| std::fs::File::open(p).ok())
                .and_then(|f| cookie_store::serde::json::load_all(std::io::BufReader::new(f)).ok())
                .unwrap_or_default();
            ureq::AgentBuilder::new().redirects(5).cookie_store(store).build()
        })
        .clone()
}

/// Écrit le cookie-jar courant de l'agent sur disque (appelé après chaque requête).
fn save_cookies(app: &AppHandle, ext_id: &str, agent: &ureq::Agent) {
    let Some(path) = cookie_file(app, ext_id) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::File::create(&path) {
        // `_incl_expired_and_nonpersistent` : inclut les cookies de SESSION (sans
        // Max-Age) → indispensable pour conserver une session « browser-session ».
        let guard = agent.cookie_store();
        let _ = cookie_store::serde::json::save_incl_expired_and_nonpersistent(&guard, &mut f);
    }
}

#[derive(serde::Serialize)]
pub struct HttpResponse {
    status: u16,
    body: String,
}

/// Requête HTTP native pour une extension, via son cookie-jar de session. URL
/// COMPLÈTE (l'extension gère sa base). Gated par la permission `network`.
#[tauri::command]
pub async fn http_fetch(
    app: AppHandle,
    ext_id: String,
    method: String,
    url: String,
    body: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "network") {
        return Err("http: permission « network » requise".into());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let agent = ext_agent(&app2, &ext_id);
        let mut req = agent
            .request(&method, &url)
            .set("Accept", "application/json")
            .set("X-Requested-With", "XMLHttpRequest");
        if let Some(h) = &headers {
            for (k, v) in h {
                req = req.set(k, v);
            }
        }
        let result = match body {
            Some(b) => req.set("Content-Type", "application/json").send_string(&b),
            None => req.call(),
        };
        let out = match result {
            Ok(r) => {
                let status = r.status();
                let body = r.into_string().unwrap_or_default();
                Ok(HttpResponse { status, body })
            }
            // Un statut 4xx/5xx n'est pas une erreur transport : on renvoie le corps.
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Ok(HttpResponse { status: code, body })
            }
            Err(e) => Err(e.to_string()),
        };
        // Persiste la session (cookies posés/rafraîchis par le serveur).
        save_cookies(&app2, &ext_id, &agent);
        out
    })
    .await
    .map_err(|e| e.to_string())?
}
