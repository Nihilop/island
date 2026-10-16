// Services des extensions v2 qui touchent au monde extérieur. Appelés UNIQUEMENT par
// l'hôte (front d'Island), qui fournit l'identité de l'extension déduite de son canal ;
// les permissions sensibles sont revérifiées ici à partir du manifeste installé.

use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::{ext_dir, manifest, valid_id};

// --- Secrets (trousseau du système, cloisonné par extension) --------------------------

fn secret(id: &str, key: &str) -> Result<Entry, String> {
    if !valid_id(id) || key.is_empty() || key.len() > 100 {
        return Err("invalidKey".into());
    }
    Entry::new(&format!("island-ext2:{id}"), key).map_err(|e| e.to_string())
}

/// Clés réservées à l'hôte (bocal à cookies…) : jamais lisibles ni modifiables par
/// l'extension elle-même.
fn user_key(key: &str) -> Result<(), String> {
    if key.starts_with("__island") { Err("reservedKey".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn ext2_secret_get(id: String, key: String) -> Result<Option<String>, String> {
    user_key(&key)?;
    secret_get(&id, &key)
}

fn secret_get(id: &str, key: &str) -> Result<Option<String>, String> {
    match secret(id, key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn ext2_secret_set(app: AppHandle, id: String, key: String, value: String) -> Result<(), String> {
    user_key(&key)?;
    secret_set(&app, &id, key, &value)
}

fn secret_set(app: &AppHandle, id: &str, key: String, value: &str) -> Result<(), String> {
    secret(id, &key)?.set_password(value).map_err(|e| e.to_string())?;
    // Clé retenue pour l'effacer à la désinstallation (le trousseau ne s'énumère pas).
    let mut st = super::install::state(app, id).unwrap_or_default();
    if st.secrets.insert(key) {
        super::install::save_state(app, id, &st);
    }
    Ok(())
}

#[tauri::command]
pub fn ext2_secret_delete(id: String, key: String) -> Result<(), String> {
    user_key(&key)?;
    secret_delete(&id, &key)
}

pub(crate) fn secret_delete(id: &str, key: &str) -> Result<(), String> {
    match secret(id, key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// --- Fichiers de l'extension (traductions…) -------------------------------------------

/// Lit un fichier texte de l'extension (≤ 1 Mo), chemin canonicalisé dans son dossier.
#[tauri::command]
pub fn ext2_read_text(app: AppHandle, id: String, path: String) -> Result<String, String> {
    manifest(&app, &id).ok_or("notFound")?;
    if !super::valid_rel(&path) {
        return Err("invalidPath".into());
    }
    let dir = ext_dir(&app, &id).ok_or("notFound")?;
    let base = dir.canonicalize().map_err(|_| "notFound")?;
    let file = dir.join(&path).canonicalize().map_err(|_| "notFound")?;
    if !file.starts_with(&base) || std::fs::metadata(&file).map(|m| m.len() > 1 << 20).unwrap_or(true) {
        return Err("forbidden".into());
    }
    std::fs::read_to_string(file).map_err(|e| e.to_string())
}

// --- Presse-papiers (permission `clipboard`) ---------------------------------------------

fn require(app: &AppHandle, id: &str, perm: &str) -> Result<(), String> {
    let m = manifest(app, id).ok_or("notFound")?;
    if m.permissions.iter().any(|p| p == perm) { Ok(()) } else { Err(format!("permission:{perm}")) }
}

#[tauri::command]
pub fn ext2_clipboard_read(app: AppHandle, id: String) -> Result<Option<String>, String> {
    require(&app, &id, "clipboard")?;
    Ok(arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok())
}

#[tauri::command]
pub fn ext2_clipboard_write(app: AppHandle, id: String, text: String) -> Result<(), String> {
    require(&app, &id, "clipboard")?;
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())
}

// --- Réseau (permission `network:<domaine>`) ---------------------------------------------

/// Adresse interdite : bouclage, privée, lien-local, CGNAT, multicast, non spécifiée…
/// (une extension ne parle qu'à Internet, jamais au PC ni au réseau local).
fn forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1])) // CGNAT
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // bancs d'essai
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // ULA
                || (s[0] & 0xffc0) == 0xfe80 // lien-local
                || v6.to_ipv4_mapped().is_some_and(|v4| forbidden_ip(IpAddr::V4(v4)))
        }
    }
}

/// Résolveur qui refuse toute adresse interdite AU MOMENT DE LA CONNEXION (pas de
/// contournement par une réponse DNS qui change entre la vérification et l'appel).
struct PublicOnly;

impl ureq::Resolver for PublicOnly {
    fn resolve(&self, netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
        let addrs: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
        if addrs.is_empty() || addrs.iter().any(|a| forbidden_ip(a.ip())) {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "adresse interdite"));
        }
        Ok(addrs)
    }
}

/// Hôte autorisé par une permission `network:<domaine>` (`*.exemple.com` = sous-domaines).
fn host_allowed(perms: &[String], host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    perms.iter().filter_map(|p| p.strip_prefix("network:")).any(|d| {
        let d = d.to_ascii_lowercase();
        match d.strip_prefix("*.") {
            Some(base) => host.ends_with(&format!(".{base}")),
            None => host == d,
        }
    })
}

// --- Cookies (permission `cookies`) -------------------------------------------------------
//
// Une extension qui se connecte à un service à session (cookie HttpOnly) : Island tient le
// bocal à cookies À SA PLACE. Les `Set-Cookie` des réponses sont retenus et les cookies
// renvoyés aux requêtes suivantes vers le même domaine ; le code de l'extension ne les voit
// jamais (`Set-Cookie` retiré des réponses, `Cookie` refusé en entrée). Bocal cloisonné par
// extension, limité à ses domaines déclarés (`network:`), persistant dans le trousseau du
// système pour les cookies à expiration (ceux de session vivent en mémoire), effacé à la
// désinstallation.

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct Cookie {
    name: String,
    value: String,
    /// Domaine (sans point initial).
    domain: String,
    /// `true` = hôte exact (pas d'attribut Domain), sinon sous-domaines compris.
    host_only: bool,
    path: String,
    /// Expiration (ms depuis l'époque), None = cookie de session.
    expires: Option<i64>,
}

/// Clé du trousseau qui garde le bocal (dans l'espace des secrets de l'extension).
pub(crate) const COOKIE_KEY: &str = "__island_cookies";

static JARS: std::sync::Mutex<Option<std::collections::HashMap<String, Vec<Cookie>>>> = std::sync::Mutex::new(None);

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Date HTTP (`Wed, 21 Oct 2015 07:28:00 GMT`) → ms depuis l'époque.
fn parse_http_date(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split([' ', ',']).filter(|p| !p.is_empty()).collect();
    // [jour, JJ, Mois, AAAA, HH:MM:SS, GMT] (le nom du jour est facultatif).
    let p = if parts.len() >= 6 { &parts[1..] } else { &parts[..] };
    let (d, mon, y, hms) = (p.first()?.parse::<i64>().ok()?, *p.get(1)?, p.get(2)?.parse::<i64>().ok()?, *p.get(3)?);
    let m = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].iter().position(|x| mon.to_ascii_lowercase().starts_with(x))? as i64 + 1;
    let t: Vec<i64> = hms.split(':').filter_map(|x| x.parse().ok()).collect();
    if t.len() != 3 {
        return None;
    }
    // Jours depuis l'époque (algorithme « days from civil »).
    let (yy, mm) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400) + t[0] * 3600 + t[1] * 60 + t[2]) * 1000)
}

/// Lit un en-tête `Set-Cookie` reçu de `host` (attributs Domain, Path, Max-Age, Expires).
fn parse_set_cookie(raw: &str, host: &str, default_path: &str, now: i64) -> Option<Cookie> {
    let host = host.to_ascii_lowercase();
    let mut parts = raw.split(';');
    let (name, value) = parts.next()?.split_once('=')?;
    let (name, value) = (name.trim(), value.trim().trim_matches('"'));
    if name.is_empty() {
        return None;
    }
    let mut c = Cookie { name: name.into(), value: value.into(), domain: host.clone(), host_only: true, path: default_path.into(), expires: None };
    let mut max_age: Option<i64> = None;
    for attr in parts {
        let (k, v) = attr.split_once('=').map(|(k, v)| (k.trim(), v.trim())).unwrap_or((attr.trim(), ""));
        match k.to_ascii_lowercase().as_str() {
            "domain" if !v.is_empty() => {
                let d = v.trim_start_matches('.').to_ascii_lowercase();
                // Un serveur ne pose de cookie que pour lui-même ou un domaine parent.
                if host != d && !host.ends_with(&format!(".{d}")) {
                    return None;
                }
                c.domain = d;
                c.host_only = false;
            }
            "path" if v.starts_with('/') => c.path = v.into(),
            "max-age" => max_age = v.parse().ok(),
            "expires" => c.expires = c.expires.or_else(|| parse_http_date(v)),
            _ => {}
        }
    }
    // Max-Age l'emporte sur Expires.
    if let Some(s) = max_age {
        c.expires = Some(now + s.saturating_mul(1000));
    }
    Some(c)
}

fn domain_match(c: &Cookie, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if c.host_only { host == c.domain } else { host == c.domain || host.ends_with(&format!(".{}", c.domain)) }
}

fn path_match(c: &Cookie, path: &str) -> bool {
    path == c.path || (path.starts_with(&c.path) && (c.path.ends_with('/') || path[c.path.len()..].starts_with('/')))
}

/// Bocal d'une extension (chargé depuis le trousseau au premier accès, expirés retirés).
fn with_jar<T>(id: &str, f: impl FnOnce(&mut Vec<Cookie>) -> T) -> T {
    let mut g = JARS.lock().unwrap_or_else(|p| p.into_inner());
    let jars = g.get_or_insert_with(Default::default);
    let jar = jars.entry(id.to_string()).or_insert_with(|| {
        secret_get(id, COOKIE_KEY).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    });
    let now = now_ms();
    jar.retain(|c| c.expires.is_none_or(|e| e > now));
    f(jar)
}

/// Persiste les cookies à expiration (les cookies de session restent en mémoire).
fn save_jar(app: &AppHandle, id: &str, jar: &[Cookie]) {
    let lasting: Vec<&Cookie> = jar.iter().filter(|c| c.expires.is_some()).collect();
    if lasting.is_empty() {
        let _ = secret_delete(id, COOKIE_KEY);
    } else if let Ok(s) = serde_json::to_string(&lasting) {
        let _ = secret_set(app, id, COOKIE_KEY.into(), &s);
    }
}

/// En-tête `Cookie` d'une requête.
fn cookie_header(id: &str, url: &tauri::Url) -> Option<String> {
    let host = url.host_str()?;
    let path = url.path();
    with_jar(id, |jar| {
        let v: Vec<String> = jar.iter().filter(|c| domain_match(c, host) && path_match(c, path)).map(|c| format!("{}={}", c.name, c.value)).collect();
        (!v.is_empty()).then(|| v.join("; "))
    })
}

/// Retient les `Set-Cookie` d'une réponse (un cookie expiré efface le précédent).
fn store_cookies(app: &AppHandle, id: &str, url: &tauri::Url, set: &[String]) {
    let Some(host) = url.host_str() else { return };
    let default_path = match url.path().rsplit_once('/') {
        Some((p, _)) if !p.is_empty() => p.to_string(),
        _ => "/".to_string(),
    };
    let now = now_ms();
    let changed = with_jar(id, |jar| {
        let mut changed = false;
        for raw in set {
            let Some(c) = parse_set_cookie(raw, host, &default_path, now) else { continue };
            jar.retain(|x| !(x.name == c.name && x.domain == c.domain && x.path == c.path));
            if c.expires.is_none_or(|e| e > now) {
                jar.push(c);
            }
            changed = true;
        }
        changed.then(|| jar.clone())
    });
    if let Some(jar) = changed {
        save_jar(app, id, &jar);
    }
}

/// Vide le bocal d'une extension (déconnexion forcée côté extension).
#[tauri::command]
pub fn ext2_cookies_clear(app: AppHandle, id: String) -> Result<(), String> {
    require(&app, &id, "cookies")?;
    forget_cookies(&id);
    Ok(())
}

/// Oublie le bocal en mémoire et dans le trousseau (désinstallation).
pub(crate) fn forget_cookies(id: &str) {
    if let Some(j) = JARS.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        j.remove(id);
    }
    let _ = secret_delete(id, COOKIE_KEY);
}

// --- Requêtes ----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct HttpRequest {
    url: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body: Option<String>,
    /// Délai max (s), 1 à 120 ; 20 par défaut (segments vidéo transcodés à la volée : long).
    #[serde(default)]
    timeout: Option<u64>,
}

#[derive(Serialize)]
pub struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

/// Corps texte max ; corps binaire max (segments vidéo…).
const MAX_BODY: u64 = 5 << 20;
const MAX_BYTES: u64 = 64 << 20;

/// Envoie une requête : HTTPS, domaine déclaré, adresses publiques seulement, cookies du
/// bocal si la permission `cookies` est accordée.
fn send(app: &AppHandle, m: &super::Manifest, req: HttpRequest) -> Result<ureq::Response, String> {
    let url = tauri::Url::parse(&req.url).map_err(|_| "invalidUrl")?;
    if url.scheme() != "https" {
        return Err("httpsOnly".to_string());
    }
    let host = url.host_str().ok_or("invalidUrl")?;
    if !host_allowed(&m.permissions, host) {
        return Err(format!("permission:network:{host}"));
    }
    let method = req.method.as_deref().unwrap_or("GET").to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err("invalidMethod".into());
    }
    let jar = m.permissions.iter().any(|p| p == "cookies");
    let agent = ureq::AgentBuilder::new()
        .resolver(PublicOnly)
        .redirects(0)
        .https_only(true)
        .timeout(Duration::from_secs(req.timeout.unwrap_or(20).clamp(1, 120)))
        .user_agent(&format!("Island-Extension/{} ({})", m.version, m.id))
        .build();
    let mut r = agent.request(&method, url.as_str());
    for (k, v) in &req.headers {
        // En-têtes gérés par l'hôte / dangereux : ignorés.
        if matches!(k.to_ascii_lowercase().as_str(), "host" | "cookie" | "connection" | "content-length" | "transfer-encoding" | "proxy-authorization") {
            continue;
        }
        r = r.set(k, v);
    }
    if jar {
        if let Some(c) = cookie_header(&m.id, &url) {
            r = r.set("Cookie", &c);
        }
    }
    let res = match req.body {
        Some(b) => r.send_string(&b),
        None => r.call(),
    };
    let res = match res {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(format!("network:{e}")),
    };
    if jar {
        let set: Vec<String> = res.all("set-cookie").into_iter().map(str::to_string).collect();
        if !set.is_empty() {
            store_cookies(app, &m.id, &url, &set);
        }
    }
    Ok(res)
}

/// En-têtes renvoyés à l'extension (jamais `Set-Cookie`).
fn visible_headers(res: &ureq::Response) -> Vec<(String, String)> {
    res.headers_names()
        .into_iter()
        .filter(|n| !n.eq_ignore_ascii_case("set-cookie"))
        .filter_map(|n| res.header(&n).map(|v| (n.clone(), v.to_string())))
        .collect()
}

#[tauri::command]
pub async fn ext2_http(app: AppHandle, id: String, req: HttpRequest) -> Result<HttpResponse, String> {
    let m = manifest(&app, &id).ok_or("notFound")?;
    tauri::async_runtime::spawn_blocking(move || {
        let res = send(&app, &m, req)?;
        let status = res.status();
        let headers = visible_headers(&res);
        let mut body = String::new();
        res.into_reader().take(MAX_BODY).read_to_string(&mut body).map_err(|e| e.to_string())?;
        Ok(HttpResponse { status, headers, body })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Requête binaire (segments vidéo, sous-titres, images…) : réponse brute
/// `[u32 LE taille de la méta][méta JSON { status, headers }][corps]`, sans passer par
/// du JSON ; l'hôte la transfère telle quelle à l'iframe (sans copie).
#[tauri::command]
pub async fn ext2_http_bytes(app: AppHandle, id: String, req: HttpRequest) -> Result<tauri::ipc::Response, String> {
    let m = manifest(&app, &id).ok_or("notFound")?;
    tauri::async_runtime::spawn_blocking(move || {
        let res = send(&app, &m, req)?;
        let meta = serde_json::json!({ "status": res.status(), "headers": visible_headers(&res) }).to_string();
        let mut out = Vec::with_capacity(4 + meta.len() + 1024 * 1024);
        out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        out.extend_from_slice(meta.as_bytes());
        res.into_reader().take(MAX_BYTES).read_to_end(&mut out).map_err(|e| e.to_string())?;
        Ok(tauri::ipc::Response::new(out))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_ranges() {
        for ip in ["127.0.0.1", "10.1.2.3", "192.168.1.10", "172.16.0.1", "169.254.1.1", "100.64.0.1", "0.0.0.0", "::1", "fd00::1", "fe80::1", "::ffff:127.0.0.1"] {
            assert!(forbidden_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["140.82.112.3", "1.1.1.1", "2606:4700::1111"] {
            assert!(!forbidden_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn cookies_parse_and_match() {
        let now = 1_000_000;
        let c = parse_set_cookie("atlas_user_session=abc123; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800; Secure", "app.example.com", "/api/users/v1", now).unwrap();
        assert_eq!((c.name.as_str(), c.value.as_str(), c.path.as_str()), ("atlas_user_session", "abc123", "/"));
        assert!(c.host_only);
        assert_eq!(c.expires, Some(now + 604_800_000));
        assert!(domain_match(&c, "app.example.com"));
        assert!(!domain_match(&c, "evil.example.com"));
        assert!(path_match(&c, "/api/video/hls/x/seg0001.ts"));
        // Effacement : Max-Age=0 → expiré.
        let gone = parse_set_cookie("atlas_user_session=; Path=/; Max-Age=0", "app.example.com", "/", now).unwrap();
        assert!(gone.expires.unwrap() <= now);
        // Domaine parent accepté, domaine étranger refusé.
        let parent = parse_set_cookie("a=1; Domain=.example.com", "app.example.com", "/", now).unwrap();
        assert!(!parent.host_only && domain_match(&parent, "cdn.example.com"));
        assert!(parse_set_cookie("a=1; Domain=evil.com", "app.example.com", "/", now).is_none());
        // Chemins.
        let scoped = parse_set_cookie("b=2; Path=/api", "x.com", "/", now).unwrap();
        assert!(path_match(&scoped, "/api") && path_match(&scoped, "/api/x") && !path_match(&scoped, "/apix"));
    }

    #[test]
    fn http_dates() {
        assert_eq!(parse_http_date("Wed, 21 Oct 2015 07:28:00 GMT"), Some(1_445_412_480_000));
        assert_eq!(parse_http_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(parse_http_date("n'importe quoi"), None);
    }

    #[test]
    fn domain_allowlist() {
        let p = vec!["network:api.github.com".to_string(), "network:*.example.org".to_string(), "storage".to_string()];
        assert!(host_allowed(&p, "api.github.com"));
        assert!(host_allowed(&p, "API.GitHub.com"));
        assert!(!host_allowed(&p, "github.com"));
        assert!(!host_allowed(&p, "api.github.com.evil.io"));
        assert!(host_allowed(&p, "cdn.example.org"));
        assert!(!host_allowed(&p, "example.org"));
    }
}
