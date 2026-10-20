//! Stockage des fichiers publiés : dossier local (développement) ou Cloudflare R2 (API S3).
//!
//! Deux compartiments, dossiers suffixés `-dev` en développement (`STORE_ENV=dev`) :
//!   `R2_BUCKET` (PUBLIC, domaine `R2_MEDIA_URL`, ex. https://cdn.streemkit.com)
//!       `media/`     vignettes, bannières, captures, servies par le CDN ;
//!   `R2_PACKAGES_BUCKET` (PRIVÉ : ni domaine ni `r2.dev`)
//!       `packages/`  paquets (`.islandscene`, `.island`), distribués par des URL SIGNÉES de
//!                    5 minutes délivrées par le serveur (compteur ; élément masqué = plus
//!                    de lien, et les anciens expirent).
//!
//! Les clés sont produites par le serveur et revérifiées ici : jamais de `..` ni de chemin
//! absolu.

use std::path::PathBuf;
use std::time::Duration;

use rusty_s3::actions::{DeleteObject, GetObject, PutObject, S3Action as _};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use tokio::io::AsyncWriteExt;

/// Durée de vie d'un lien de téléchargement d'un paquet.
pub const DOWNLOAD_TTL: Duration = Duration::from_secs(300);
/// Durée de vie d'une URL signée utilisée par le serveur lui-même (écriture, lecture).
const SERVER_TTL: Duration = Duration::from_secs(120);

pub struct R2 {
    /// Compartiment public (médias).
    media: Bucket,
    /// Compartiment privé (paquets).
    packages: Bucket,
    creds: Credentials,
    http: reqwest::Client,
    /// `https://cdn.streemkit.com` (sans `/` final).
    public_url: String,
}

impl R2 {
    fn bucket(&self, key: &str) -> &Bucket {
        if Storage::is_media(key) { &self.media } else { &self.packages }
    }
}

pub enum Backend {
    Local(PathBuf),
    R2(Box<R2>),
}

pub struct Storage {
    backend: Backend,
    /// `""` en production, `"-dev"` en développement.
    suffix: String,
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 200
        && !key.starts_with('/')
        && key.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
        && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
}

fn check(key: &str) -> Result<(), String> {
    if valid_key(key) { Ok(()) } else { Err(format!("clé de stockage invalide : {key}")) }
}

impl Storage {
    pub fn local(dir: PathBuf, dev: bool) -> Self {
        Self { backend: Backend::Local(dir), suffix: if dev { "-dev".into() } else { String::new() } }
    }

    /// R2 : `account` = identifiant du compte Cloudflare ; `media` = compartiment public
    /// relié au domaine `public_url` ; `packages` = compartiment privé.
    #[allow(clippy::too_many_arguments)]
    pub fn r2(account: &str, key: &str, secret: &str, media: &str, packages: &str, public_url: &str, dev: bool) -> Result<Self, String> {
        if media == packages {
            return Err("R2_PACKAGES_BUCKET doit être un compartiment PRIVÉ, distinct de R2_BUCKET (public)".into());
        }
        let endpoint: url::Url = format!("https://{account}.r2.cloudflarestorage.com").parse().map_err(|e| format!("R2_ACCOUNT_ID : {e}"))?;
        let bucket = |name: &str, var: &str| Bucket::new(endpoint.clone(), UrlStyle::Path, name.to_string(), "auto").map_err(|e| format!("{var} : {e}"));
        let (media, packages) = (bucket(media, "R2_BUCKET")?, bucket(packages, "R2_PACKAGES_BUCKET")?);
        let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().map_err(|e| e.to_string())?;
        let r2 = R2 { media, packages, creds: Credentials::new(key, secret), http, public_url: public_url.trim_end_matches('/').to_string() };
        Ok(Self { backend: Backend::R2(Box::new(r2)), suffix: if dev { "-dev".into() } else { String::new() } })
    }

    /// Les médias vont dans le compartiment public, tout le reste dans le privé.
    fn is_media(key: &str) -> bool {
        key.starts_with("media/") || key.starts_with("media-dev/")
    }

    /// Clé d'un média public (`media[-dev]/<chemin>`).
    pub fn media_key(&self, path: &str) -> String {
        format!("media{}/{path}", self.suffix)
    }

    /// Clé d'un paquet (`packages[-dev]/<chemin>`).
    pub fn package_key(&self, path: &str) -> String {
        format!("packages{}/{path}", self.suffix)
    }

    /// URL publique d'un média (CDN), si le stockage en a une.
    pub fn public_url(&self, key: &str) -> Option<String> {
        match &self.backend {
            Backend::R2(r) if Self::is_media(key) => Some(format!("{}/{key}", r.public_url)),
            _ => None,
        }
    }

    /// Lien de téléchargement signé (5 min) d'un paquet ; `disposition` = `Content-Disposition`
    /// renvoyé par R2 (nom de fichier proposé).
    pub fn signed_download(&self, key: &str, disposition: &str) -> Option<String> {
        match &self.backend {
            Backend::R2(r) => {
                let mut a = GetObject::new(r.bucket(key), Some(&r.creds), key);
                a.query_mut().insert("response-content-disposition", disposition.to_string());
                Some(a.sign(DOWNLOAD_TTL).to_string())
            }
            Backend::Local(_) => None,
        }
    }

    /// Écrit (ou remplace) un fichier.
    pub async fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> Result<(), String> {
        check(key)?;
        match &self.backend {
            Backend::Local(dir) => {
                let path = dir.join(key);
                if let Some(d) = path.parent() {
                    tokio::fs::create_dir_all(d).await.map_err(|e| e.to_string())?;
                }
                let tmp = path.with_extension("part");
                let mut f = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
                f.write_all(bytes).await.map_err(|e| e.to_string())?;
                f.flush().await.map_err(|e| e.to_string())?;
                drop(f);
                tokio::fs::rename(&tmp, &path).await.map_err(|e| e.to_string())
            }
            Backend::R2(r) => {
                let url = PutObject::new(r.bucket(key), Some(&r.creds), key).sign(SERVER_TTL);
                // Médias : mis en cache longtemps (une nouvelle version = un nouveau nom).
                let cache = if Self::is_media(key) { "public, max-age=31536000, immutable" } else { "private, no-store" };
                r.http
                    .put(url)
                    .header("content-type", content_type)
                    .header("cache-control", cache)
                    .body(bytes.to_vec())
                    .send()
                    .await
                    .and_then(|res| res.error_for_status()).map_err(reqwest::Error::without_url)
                    .map(|_| ())
                    .map_err(|e| format!("R2 put {key} : {e}"))
            }
        }
    }

    /// Lit un fichier (None s'il n'existe pas).
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        check(key)?;
        match &self.backend {
            Backend::Local(dir) => match tokio::fs::read(dir.join(key)).await {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            },
            Backend::R2(r) => {
                let url = GetObject::new(r.bucket(key), Some(&r.creds), key).sign(SERVER_TTL);
                let res = r.http.get(url).send().await.map_err(|e| e.without_url().to_string())?;
                if res.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(None);
                }
                let res = res.error_for_status().map_err(|e| format!("R2 get {key} : {}", e.without_url()))?;
                Ok(Some(res.bytes().await.map_err(|e| e.to_string())?.to_vec()))
            }
        }
    }

    /// Fichier local à servir : (fichier, taille). None hors stockage local ou s'il manque.
    pub async fn open(&self, key: &str) -> Result<Option<(tokio::fs::File, u64)>, String> {
        check(key)?;
        let Backend::Local(dir) = &self.backend else { return Ok(None) };
        match tokio::fs::File::open(dir.join(key)).await {
            Ok(f) => {
                let len = f.metadata().await.map_err(|e| e.to_string())?.len();
                Ok(Some((f, len)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Supprime un fichier (absent : pas une erreur).
    pub async fn delete(&self, key: &str) -> Result<(), String> {
        check(key)?;
        match &self.backend {
            Backend::Local(dir) => match tokio::fs::remove_file(dir.join(key)).await {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
            Backend::R2(r) => {
                let url = DeleteObject::new(r.bucket(key), Some(&r.creds), key).sign(SERVER_TTL);
                r.http.delete(url).send().await.and_then(|res| res.error_for_status()).map_err(reqwest::Error::without_url).map(|_| ()).map_err(|e| format!("R2 delete {key} : {e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_never_escape_the_storage() {
        assert!(valid_key("packages/sc_abc/1.islandscene"));
        assert!(!valid_key("../secret"));
        assert!(!valid_key("media/../../x"));
        assert!(!valid_key("/etc/passwd"));
        assert!(!valid_key("media//x"));
        assert!(!valid_key("media/C:x"));
    }

    #[test]
    fn folders_and_urls_follow_the_environment() {
        let dev = Storage::r2("e7149cb0b926dddf52a0f4d30c11ea45", "k", "s", "island-store", "island-packages", "https://cdn.streemkit.com/", true).unwrap();
        assert_eq!(dev.media_key("sc_a/thumbnail-1.jpg"), "media-dev/sc_a/thumbnail-1.jpg");
        assert_eq!(dev.package_key("sc_a/1.islandscene"), "packages-dev/sc_a/1.islandscene");
        assert_eq!(dev.public_url("media-dev/sc_a/t.jpg").as_deref(), Some("https://cdn.streemkit.com/media-dev/sc_a/t.jpg"));
        // Jamais d'URL publique pour un paquet : seulement un lien signé.
        assert_eq!(dev.public_url("packages-dev/sc_a/1.islandscene"), None);
        let link = dev.signed_download("packages-dev/sc_a/1.islandscene", "attachment; filename=\"Aurore.islandscene\"").unwrap();
        assert!(link.starts_with("https://e7149cb0b926dddf52a0f4d30c11ea45.r2.cloudflarestorage.com/island-packages/packages-dev/sc_a/1.islandscene?"));
        assert!(Storage::r2("a", "k", "s", "island-store", "island-store", "https://x", false).is_err());
        assert!(link.contains("X-Amz-Signature=") && link.contains("X-Amz-Expires=300"));
        let prod = Storage::local(PathBuf::from("x"), false);
        assert_eq!(prod.media_key("a.jpg"), "media/a.jpg");
        assert_eq!(prod.public_url("media/a.jpg"), None);
    }
}
