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
//
// Compte : connexion GitHub par *device flow* (un code à valider sur github.com, aucun
// secret dans l'app). Le magasin délivre alors un jeton d'Island, rangé dans le trousseau
// du système (`island-core:market`, une entrée par adresse de magasin) ; le front ne le
// voit jamais.

use std::io::Read;
use std::sync::Mutex;
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

// --- Compte ---------------------------------------------------------------------------------

fn token_entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new("island-core:market", &base_url())
}

fn token() -> Option<String> {
    token_entry().ok()?.get_password().ok().filter(|t| !t.is_empty())
}

fn forget_token() {
    if let Ok(e) = token_entry() {
        let _ = e.delete_credential();
    }
}

/// Connexion en attente de validation sur GitHub. L'attente tourne dans le Rust : le
/// panneau d'Island se referme quand le navigateur prend le focus, et ne doit pas l'arrêter.
struct Pending {
    device_code: String,
    start: LoginStart,
    /// Change à chaque nouvelle connexion ou annulation : l'ancienne boucle s'arrête.
    run: u64,
}

static LOGIN: Mutex<Option<Pending>> = Mutex::new(None);
static RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoginStart {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

fn login_start(app: &AppHandle, lang: &str) -> Result<LoginStart, String> {
    let d: serde_json::Value = agent(app, Duration::from_secs(20))
        .post(&format!("{}/v1/auth/device", base_url()))
        .send_json(serde_json::json!({}))
        .map_err(error)?
        .into_json()
        .map_err(|e| e.to_string())?;
    let field = |k: &str| d.get(k).and_then(|v| v.as_str()).map(String::from).ok_or_else(|| "badResponse".to_string());
    let num = |k: &str, def: u64| d.get(k).and_then(|v| v.as_u64()).unwrap_or(def);
    let start = LoginStart { user_code: field("userCode")?, verification_uri: field("verificationUri")?, expires_in: num("expiresIn", 900), interval: num("interval", 5) };
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    *LOGIN.lock().unwrap_or_else(|p| p.into_inner()) = Some(Pending { device_code: field("deviceCode")?, start: start.clone(), run });
    announce(app, &start.user_code, lang);
    let app = app.clone();
    std::thread::spawn(move || wait(&app, run));
    Ok(start)
}

/// Le panneau se referme quand le navigateur prend le focus : le code est copié et annoncé
/// par une notification système, pour être collé sur la page GitHub.
fn announce(app: &AppHandle, code: &str, lang: &str) {
    use tauri_plugin_notification::NotificationExt;
    let copied = arboard::Clipboard::new().and_then(|mut c| c.set_text(code)).is_ok();
    let fr = !lang.starts_with("en");
    let title = if fr { format!("Code GitHub : {code}") } else { format!("GitHub code: {code}") };
    let body = match (fr, copied) {
        (true, true) => "Copié : colle-le sur la page GitHub qui s'ouvre.",
        (true, false) => "Saisis-le sur la page GitHub qui s'ouvre.",
        (false, true) => "Copied: paste it on the GitHub page that is opening.",
        (false, false) => "Enter it on the GitHub page that is opening.",
    };
    let _ = app.notification().builder().title(title).body(body).show();
}

fn device_name() -> String {
    let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    if host.trim().is_empty() { "Island".into() } else { format!("Island sur {}", host.trim()) }
}

/// Fin de la connexion : `market://login` { error: null | code } puis `market://account`.
fn finish(app: &AppHandle, run: u64, error: Option<String>) {
    let mut g = LOGIN.lock().unwrap_or_else(|p| p.into_inner());
    if g.as_ref().is_some_and(|p| p.run == run) {
        *g = None;
    }
    drop(g);
    let _ = app.emit("market://login", serde_json::json!({ "error": error }));
    let _ = app.emit("market://account", ());
}

/// Interroge le magasin au rythme demandé par GitHub jusqu'à validation, refus ou expiration.
fn wait(app: &AppHandle, run: u64) {
    let current = || LOGIN.lock().unwrap_or_else(|p| p.into_inner()).as_ref().filter(|p| p.run == run).map(|p| (p.device_code.clone(), p.start.clone()));
    let Some((code, start)) = current() else { return };
    let deadline = std::time::Instant::now() + Duration::from_secs(start.expires_in);
    let mut interval = start.interval.max(1);
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        if current().is_none() {
            return; // annulée ou remplacée
        }
        if std::time::Instant::now() > deadline {
            return finish(app, run, Some("expired".into()));
        }
        let res = agent(app, Duration::from_secs(20))
            .post(&format!("{}/v1/auth/token", base_url()))
            .send_json(serde_json::json!({ "deviceCode": code, "name": device_name() }));
        let res = match res {
            Ok(r) => r,
            Err(e) => match error(e) {
                // Réseau coupé : on réessaie au prochain tour.
                e if e == "offline" => continue,
                e => return finish(app, run, Some(e)),
            },
        };
        let pending = res.status() == 202;
        let body: serde_json::Value = match res.into_json() {
            Ok(b) => b,
            Err(_) => return finish(app, run, Some("badResponse".into())),
        };
        if pending {
            // GitHub demande de ralentir : +5 s, comme le prévoit le device flow.
            if body.get("status").and_then(|s| s.as_str()) == Some("slowDown") {
                interval += 5;
            }
            continue;
        }
        let Some(token) = body.get("token").and_then(|t| t.as_str()) else { return finish(app, run, Some("badResponse".into())) };
        let saved = token_entry().and_then(|e| e.set_password(token)).map_err(|e| format!("keyring: {e}"));
        return finish(app, run, saved.err());
    }
}

/// Démarre la connexion : code à saisir sur `verificationUri` (copié, notifié).
#[tauri::command]
pub async fn market_login_start(app: AppHandle, lang: Option<String>) -> Result<LoginStart, String> {
    tauri::async_runtime::spawn_blocking(move || login_start(&app, lang.as_deref().unwrap_or("fr"))).await.map_err(|e| e.to_string())?
}

/// Connexion en cours (le panneau la retrouve quand il se rouvre), sinon null.
#[tauri::command]
pub fn market_login_state() -> Option<LoginStart> {
    LOGIN.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|p| p.start.clone())
}

#[tauri::command]
pub fn market_login_cancel() {
    RUN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    *LOGIN.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// Compte connecté (null sans jeton, ou si le magasin l'a révoqué).
fn account(app: &AppHandle) -> Result<Option<serde_json::Value>, String> {
    let Some(t) = token() else { return Ok(None) };
    match agent(app, Duration::from_secs(20)).get(&format!("{}/v1/me", base_url())).set("authorization", &format!("Bearer {t}")).call() {
        Ok(r) => Ok(Some(r.into_json().map_err(|e| e.to_string())?)),
        Err(ureq::Error::Status(401, _)) => {
            forget_token();
            Ok(None)
        }
        Err(e) => Err(error(e)),
    }
}

#[tauri::command]
pub async fn market_account(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || account(&app)).await.map_err(|e| e.to_string())?
}

/// Déconnexion : le jeton est révoqué au magasin (si joignable) et retiré du trousseau.
#[tauri::command]
pub async fn market_logout(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(t) = token() {
            let _ = agent(&app, Duration::from_secs(10)).delete(&format!("{}/v1/auth/token", base_url())).set("authorization", &format!("Bearer {t}")).call();
        }
        forget_token();
        let _ = app.emit("market://account", ());
    })
    .await
    .map_err(|e| e.to_string())
}

// --- Publication ------------------------------------------------------------------------------
//
// Une scène locale publiée est liée à son élément du magasin (`__market_published__` :
// scène → { élément, version }) : la publier à nouveau envoie une NOUVELLE VERSION du même
// élément. Le paquet est celui du partage (`share::export`, avec le rendu de l'éditeur en
// vignette) ; le magasin le revérifie entièrement.

const PUBLISHED: &str = "__market_published__";

#[derive(Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    pub item: String,
    pub version: String,
}

/// Élément du magasin d'une scène déjà publiée (pour proposer « nouvelle version »).
#[tauri::command]
pub fn market_published(app: AppHandle, scene: String) -> Option<Publication> {
    crate::store::read_store(&app, PUBLISHED).get(&scene).and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn multipart(fields: &[(&str, String)], file: &[u8]) -> (String, Vec<u8>) {
    let mut b = [0u8; 12];
    let _ = getrandom::getrandom(&mut b);
    let boundary = format!("island{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>());
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    }
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"scene.islandscene\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn publish(app: &AppHandle, scene: &str, summary: &str, tags: &[String], changelog: &str) -> Result<Publication, String> {
    let token = token().ok_or("notSignedIn")?;
    let tmp = std::env::temp_dir().join(format!("island-publish-{}-{}.islandscene", std::process::id(), scene.replace(|c: char| !c.is_ascii_alphanumeric(), "")));
    share::export(app, scene, &tmp)?;
    let file = std::fs::read(&tmp).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&tmp);
    let file = file?;
    let mut map = crate::store::read_store(app, PUBLISHED);
    let mut item: Option<String> = map.get(scene).and_then(|v| v.get("item")).and_then(|v| v.as_str()).map(String::from);
    loop {
        let mut fields: Vec<(&str, String)> = Vec::new();
        if let Some(i) = &item {
            fields.push(("item", i.clone()));
        }
        if !summary.trim().is_empty() {
            fields.push(("summary", summary.trim().to_string()));
        }
        if !tags.is_empty() {
            fields.push(("tags", serde_json::to_string(tags).unwrap_or_default()));
        }
        if !changelog.trim().is_empty() {
            fields.push(("changelog", changelog.trim().to_string()));
        }
        let (ctype, body) = multipart(&fields, &file);
        let res = agent(app, Duration::from_secs(600))
            .post(&format!("{}/v1/items", base_url()))
            .set("authorization", &format!("Bearer {token}"))
            .set("content-type", &ctype)
            .send_bytes(&body);
        match res {
            Ok(r) => {
                let p: serde_json::Value = r.into_json().map_err(|e| e.to_string())?;
                let publication = Publication {
                    item: p.get("id").and_then(|v| v.as_str()).ok_or("badResponse")?.to_string(),
                    version: p.get("version").and_then(|v| v.as_str()).ok_or("badResponse")?.to_string(),
                };
                map.insert(scene.to_string(), serde_json::json!(publication));
                crate::store::write_store(app, PUBLISHED, &map);
                return Ok(publication);
            }
            // L'élément lié a été supprimé du magasin (ou n'est plus à nous) : on en crée un nouveau.
            Err(ureq::Error::Status(404 | 403, _)) if item.is_some() => {
                item = None;
                map.remove(scene);
            }
            Err(ureq::Error::Status(401, _)) => {
                forget_token();
                let _ = app.emit("market://account", ());
                return Err("unauthorized".into());
            }
            Err(e) => return Err(error(e)),
        }
    }
}

/// Publie la scène (déjà enregistrée) : nouvel élément, ou nouvelle version s'il existe.
#[tauri::command]
pub async fn market_publish(app: AppHandle, scene: String, summary: String, tags: Vec<String>, changelog: String) -> Result<Publication, String> {
    tauri::async_runtime::spawn_blocking(move || publish(&app, &scene, &summary, &tags, &changelog)).await.map_err(|e| e.to_string())?
}
