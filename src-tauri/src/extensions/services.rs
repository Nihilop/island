// Services des extensions v2 qui touchent au monde extérieur. Appelés UNIQUEMENT par
// l'hôte (front d'Island), qui fournit l'identité de l'extension déduite de son canal ;
// les permissions sensibles sont revérifiées ici à partir du manifeste installé.

use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::{manifest, root, valid_id};

// --- Secrets (trousseau du système, cloisonné par extension) --------------------------

fn secret(id: &str, key: &str) -> Result<Entry, String> {
    if !valid_id(id) || key.is_empty() || key.len() > 100 {
        return Err("invalidKey".into());
    }
    Entry::new(&format!("island-ext2:{id}"), key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ext2_secret_get(id: String, key: String) -> Result<Option<String>, String> {
    match secret(&id, &key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn ext2_secret_set(id: String, key: String, value: String) -> Result<(), String> {
    secret(&id, &key)?.set_password(&value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ext2_secret_delete(id: String, key: String) -> Result<(), String> {
    match secret(&id, &key)?.delete_credential() {
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
    let dir = root(&app).join(&id);
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

#[derive(Deserialize)]
pub struct HttpRequest {
    url: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Serialize)]
pub struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

const MAX_BODY: u64 = 5 << 20;

#[tauri::command]
pub async fn ext2_http(app: AppHandle, id: String, req: HttpRequest) -> Result<HttpResponse, String> {
    let m = manifest(&app, &id).ok_or("notFound")?;
    tauri::async_runtime::spawn_blocking(move || {
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
        let agent = ureq::AgentBuilder::new()
            .resolver(PublicOnly)
            .redirects(0)
            .https_only(true)
            .timeout(Duration::from_secs(20))
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
        let res = match req.body {
            Some(b) => r.send_string(&b),
            None => r.call(),
        };
        let res = match res {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("network:{e}")),
        };
        let status = res.status();
        let headers = res.headers_names().into_iter().filter_map(|n| res.header(&n).map(|v| (n.clone(), v.to_string()))).collect();
        let mut body = String::new();
        res.into_reader().take(MAX_BODY).read_to_string(&mut body).map_err(|e| e.to_string())?;
        Ok(HttpResponse { status, headers, body })
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
