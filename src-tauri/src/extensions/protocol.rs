// Protocole `island-ext` : sert les fichiers d'une extension v2 aux iframes de l'hôte.
// Sous Windows l'URL est `http://island-ext.localhost/<id>/<chemin>`.
//
// ⚠️ Tauri considère comme « locale » (= mêmes droits que l'app) toute URL d'un protocole
// enregistré. La sûreté repose donc sur l'ISOLATION DU DOCUMENT, garantie deux fois :
//   1. l'hôte charge ces pages dans des iframes `sandbox="allow-scripts"` ;
//   2. chaque réponse porte la directive CSP `sandbox allow-scripts` : même ouverte hors
//      iframe, la page a une origine opaque → en-tête Origin `null` → Tauri rejette tout
//      appel IPC. La CSP interdit aussi tout réseau direct (`connect-src 'none'`) et ne
//      permet de charger que les fichiers de CETTE extension.
//
// Chemins : canonicalisés puis vérifiés dans le dossier de l'extension (liens et
// jonctions compris). `__main.html` = page générée qui charge le module `main`.

use std::borrow::Cow;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, UriSchemeContext, Wry};

use super::{manifest, root};

pub const SCHEME: &str = "island-ext";

/// Origine VUE PAR LA PAGE (CSP, URL du module principal). Sous Windows, la WebView charge
/// `http://island-ext.localhost/…` mais Tauri transmet la requête au gestionnaire sous la
/// forme `island-ext://localhost/…` : on ne peut donc pas la déduire de la requête.
#[cfg(windows)]
const ORIGIN: &str = "http://island-ext.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "island-ext://localhost";

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response<Cow<'static, [u8]>> {
    Response::builder().status(code).body(Cow::Borrowed(&[][..])).unwrap()
}

/// CSP d'une page d'extension : isolée, sans réseau, ressources de l'extension seulement.
fn csp(origin: &str, id: &str) -> String {
    let own = format!("{origin}/{id}/");
    format!(
        "sandbox allow-scripts; default-src 'none'; script-src {own}; style-src {own} 'unsafe-inline'; \
         img-src {own} data: blob:; font-src {own} data:; media-src {own} data: blob:; connect-src 'none'; \
         frame-src 'none'; worker-src 'none'; form-action 'none'; base-uri 'none'"
    )
}

pub fn handle(ctx: UriSchemeContext<'_, Wry>, req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    serve(ctx.app_handle(), &req)
}

fn serve(app: &AppHandle, req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let uri = req.uri();
    let origin = ORIGIN;
    let path = percent_decode(uri.path().trim_start_matches('/'));
    let Some((id, rel)) = path.split_once('/') else { return status(StatusCode::NOT_FOUND) };
    let Some(m) = manifest(app, id) else { return status(StatusCode::NOT_FOUND) };

    let body: Vec<u8> = if rel == "__main.html" {
        format!(
            "<!doctype html><meta charset=\"utf-8\"><title>{}</title><script type=\"module\" src=\"{origin}/{id}/{}\"></script>",
            m.id, m.main
        )
        .into_bytes()
    } else {
        let dir = root(app).join(id);
        let Ok(base) = dir.canonicalize() else { return status(StatusCode::NOT_FOUND) };
        let Ok(file) = dir.join(rel).canonicalize() else { return status(StatusCode::NOT_FOUND) };
        if !file.starts_with(&base) || !file.is_file() {
            return status(StatusCode::FORBIDDEN);
        }
        match std::fs::read(&file) {
            Ok(b) => b,
            Err(_) => return status(StatusCode::NOT_FOUND),
        }
    };
    let kind = if rel == "__main.html" { "text/html; charset=utf-8" } else { mime(rel) };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind)
        .header("Content-Security-Policy", csp(&origin, id))
        // Les modules JS d'une page à origine opaque sont chargés en mode CORS.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Cow::Owned(body))
        .unwrap()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let (Some(h), Some(l)) = (hex(b.get(i + 1)), hex(b.get(i + 2))) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: Option<&u8>) -> Option<u8> {
    match c? {
        c @ b'0'..=b'9' => Some(c - b'0'),
        c @ b'a'..=b'f' => Some(c - b'a' + 10),
        c @ b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
