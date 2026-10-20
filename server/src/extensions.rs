//! Extensions au magasin : dépôt, relecture, signature.
//!
//! Dépôt (`POST /v1/items` avec un `.island`) :
//!   1. `package::read` : chemins sûrs, tailles, manifeste valide, fichiers présents ; une
//!      signature fausse (paquet modifié) est refusée ;
//!   2. fichiers admis : ressources web seulement (`package::allowed_file`) ;
//!   3. antivirus ;
//!   4. l'id du manifeste EST l'id de l'élément : libre, ou déjà à l'auteur (`dev.island.*`
//!      réservé à l'admin : extensions officielles) ; la version
//!      doit augmenter. Une version encore en attente est remplacée par la nouvelle ;
//!   5. paquet gardé tel quel (`submitted_key`), médias ré-encodés, fiche mise de côté
//!      (`meta`) : rien n'est visible avant l'approbation.
//!
//! Relecture (`/v1/mod/reviews/*`, modérateurs) : fichiers et différences avec la dernière
//! version approuvée, permissions ajoutées, lecture des fichiers texte. À l'approbation, la
//! signature de l'auteur est REMPLACÉE par celle du magasin (`STORE_SIGNING_KEY`), le paquet
//! signé devient le paquet distribué et la fiche est appliquée. Un refus garde un motif,
//! que l'auteur voit dans « mes éléments ».

use std::io::Cursor;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use island_formats::package::{self, Package, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::items::{clean_tags, Form, Published, MAX_DESCRIPTION, MAX_SUMMARY, MAX_TAGS};
use crate::scan::{self, Scan};
use crate::AppState;

pub const EXT: &str = "island";
/// Préfixe des extensions officielles.
pub const RESERVED: &str = "dev.island.";

/// Clé publique du magasin (base64), à ajouter aux clés de confiance de l'app.
pub fn public_key(k: &ed25519_dalek::SigningKey) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(k.verifying_key().to_bytes())
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn read(bytes: &[u8]) -> ApiResult<Package> {
    package::read(Cursor::new(bytes), &[]).map_err(|e| if e == "tooLarge" { ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, e) } else { ApiError::bad(e) })
}

/// Le fichier est un paquet d'extension (zip avec un manifest.json valide).
pub fn is_extension(bytes: &[u8]) -> bool {
    package::read(Cursor::new(bytes), &[]).is_ok()
}

/// Vignette par défaut : l'icône de l'extension centrée sur un fond sombre.
fn fallback_thumbnail(pkg: &Package) -> image::RgbaImage {
    let (w, h) = crate::media::THUMBNAIL;
    let mut canvas = image::RgbaImage::from_pixel(w, h, image::Rgba([28, 28, 30, 255]));
    let icon = pkg.manifest.icon.as_ref().and_then(|p| pkg.files.iter().find(|(n, _)| n == p)).and_then(|(_, b)| image::load_from_memory(b).ok());
    if let Some(icon) = icon {
        let icon = icon.resize(256, 256, image::imageops::FilterType::Triangle).to_rgba8();
        let (x, y) = ((w - icon.width()) / 2, (h - icon.height()) / 2);
        image::imageops::overlay(&mut canvas, &icon, x as i64, y as i64);
    }
    canvas
}

/// Fiche mise de côté jusqu'à l'approbation.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    name: String,
    summary: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    homepage: Option<String>,
    repository: Option<String>,
    license: Option<String>,
    /// (type, position, clé de stockage).
    assets: Vec<(String, i32, String)>,
}

pub(crate) async fn publish(st: &AppState, user: &User, f: &Form, bytes: &[u8]) -> ApiResult<(StatusCode, Json<Published>)> {
    // 1-2. Lecture vérifiée, fichiers admis.
    let pkg = read(bytes)?;
    if pkg.signature == Signature::Invalid {
        return Err(ApiError::bad("invalidSignature"));
    }
    if let Some((bad, _)) = pkg.files.iter().find(|(p, _)| !package::allowed_file(p)) {
        return Err(ApiError::bad(format!("fileType:{bad}")));
    }
    let m = &pkg.manifest;
    // `dev.island.*` : extensions officielles, publiées par l'admin seulement.
    if m.id.starts_with(RESERVED) && user.role != "admin" {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "reservedId"));
    }
    if f.item.as_deref().is_some_and(|i| i != m.id) {
        return Err(ApiError::bad("idMismatch"));
    }
    // 3. Antivirus.
    let scan = scan::scan(st.clamav.as_deref(), bytes).await.map_err(ApiError::internal)?;
    if let Scan::Found(threat) = &scan {
        tracing::warn!("extension refusée ({}) : menace {threat}", user.login);
        return Err(ApiError::bad("malware"));
    }
    // Fiche : le formulaire d'abord, puis le paquet (README, manifeste).
    let store = &m.store;
    let readme = pkg.media.readme.clone();
    let summary = f.summary.clone().or(store.summary.clone()).or(m.description.clone()).map(|s| s.chars().take(MAX_SUMMARY).collect());
    let description = f.description.clone().or(readme).map(|s| s.chars().take(MAX_DESCRIPTION).collect());
    let tags = match &f.tags {
        Some(t) => Some(clean_tags(t)?),
        None if !store.tags.is_empty() => {
            let ok: Vec<String> = store.tags.iter().filter(|t| clean_tags(std::slice::from_ref(*t)).is_ok()).take(MAX_TAGS).cloned().collect();
            Some(clean_tags(&ok).unwrap_or_default())
        }
        None => None,
    };
    let link = |v: &Option<String>| v.as_deref().map(str::trim).filter(|u| u.starts_with("https://") && u.len() <= 300).map(String::from);
    let processed = crate::media::process(&pkg.media, || fallback_thumbnail(&pkg)).map_err(ApiError::bad)?;

    // 4. Identifiant et version.
    let id = m.id.clone();
    let version = m.version.trim().to_string();
    let mut tx = st.db.begin().await?;
    let row: Option<(String, i64, String)> = sqlx::query_as("SELECT kind, owner_id, status FROM items WHERE id = $1 FOR UPDATE").bind(&id).fetch_optional(&mut *tx).await?;
    match &row {
        Some((_, owner, _)) if *owner != user.id => return Err(ApiError::new(StatusCode::CONFLICT, "idTaken")),
        Some((kind, _, _)) if kind != "extension" => return Err(ApiError::bad("kindMismatch")),
        Some((_, _, status)) if status == "removed" => return Err(ApiError::forbidden()),
        Some(_) => {
            let last: Option<String> = sqlx::query_scalar("SELECT version FROM versions WHERE item_id = $1 AND status = 'approved' ORDER BY id DESC LIMIT 1")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
            if last.as_deref().is_some_and(|l| !package::newer(&version, l)) {
                return Err(ApiError::bad("versionNotGreater"));
            }
            // Une version encore en attente est remplacée par celle-ci.
            sqlx::query(
                "UPDATE versions SET status = 'rejected', reject_reason = 'superseded', reviewed_at = now()
                 WHERE item_id = $1 AND status = 'pending'",
            )
            .bind(&id)
            .execute(&mut *tx)
            .await?;
            let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM versions WHERE item_id = $1 AND version = $2)").bind(&id).bind(&version).fetch_one(&mut *tx).await?;
            if taken {
                return Err(ApiError::bad("versionNotGreater"));
            }
        }
        None => {
            // Élément créé tout de suite (l'id est réservé), invisible sans version approuvée.
            sqlx::query("INSERT INTO items (id, kind, owner_id, name) VALUES ($1, 'extension', $2, $3)").bind(&id).bind(user.id).bind(m.name.trim()).execute(&mut *tx).await?;
        }
    }

    // 5. Stockage : paquet tel qu'envoyé, médias ; fiche mise de côté.
    let submitted_key = st.storage.package_key(&format!("{id}/{version}.submitted.{EXT}"));
    st.storage.put(&submitted_key, bytes, "application/octet-stream").await.map_err(ApiError::internal)?;
    let mut assets: Vec<(String, i32, String, &[u8])> = vec![("thumbnail".into(), 0, st.storage.media_key(&format!("{id}/thumbnail-{version}.jpg")), &processed.thumbnail)];
    if let Some(b) = &processed.banner {
        assets.push(("banner".into(), 0, st.storage.media_key(&format!("{id}/banner-{version}.jpg")), b));
    }
    for (n, b) in &processed.screenshots {
        assets.push(("screenshot".into(), *n as i32, st.storage.media_key(&format!("{id}/screenshot-{version}-{n}.jpg")), b));
    }
    for (_, _, key, b) in &assets {
        st.storage.put(key, b, "image/jpeg").await.map_err(ApiError::internal)?;
    }
    let meta = Meta {
        name: m.name.trim().chars().take(80).collect(),
        summary,
        description,
        tags,
        homepage: link(&store.homepage),
        repository: link(&store.repository),
        license: store.license.as_deref().map(|l| l.trim().chars().take(60).collect::<String>()).filter(|l| !l.is_empty()),
        assets: assets.iter().map(|(k, p, key, _)| (k.clone(), *p, key.clone())).collect(),
    };
    sqlx::query(
        "INSERT INTO versions (item_id, version, status, file_key, submitted_key, size, sha256, permissions, scan, changelog, meta, author_key)
         VALUES ($1, $2, 'pending', $3, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&id)
    .bind(&version)
    .bind(&submitted_key)
    .bind(bytes.len() as i64)
    .bind(sha(bytes))
    .bind(serde_json::json!(m.permissions))
    .bind(scan.label())
    .bind(f.changelog.as_deref().unwrap_or(""))
    .bind(serde_json::json!(meta))
    .bind(&pkg.key)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!("extension déposée : {id} v{version} par {} (relecture en attente)", user.login);
    Ok((StatusCode::ACCEPTED, Json(Published { id, version, scan: scan.label(), status: "pending" })))
}

// --- Relecture --------------------------------------------------------------------------------

fn require_mod(u: &User) -> ApiResult<()> {
    if u.is_moderator() { Ok(()) } else { Err(ApiError::forbidden()) }
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRow {
    id: i64,
    item_id: String,
    name: String,
    owner: String,
    version: String,
    size: i64,
    scan: String,
    permissions: serde_json::Value,
    previous_version: Option<String>,
    previous_permissions: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

const REVIEW_SELECT: &str = "SELECT v.id, v.item_id, COALESCE(v.meta->>'name', i.name) AS name, u.login AS owner, v.version, v.size, v.scan,
        COALESCE(v.permissions, '[]'::jsonb) AS permissions,
        p.version AS previous_version, p.permissions AS previous_permissions, v.created_at
    FROM versions v JOIN items i ON i.id = v.item_id JOIN users u ON u.id = i.owner_id
    LEFT JOIN LATERAL (SELECT version, permissions FROM versions WHERE item_id = v.item_id AND status = 'approved' ORDER BY id DESC LIMIT 1) p ON true";

/// `GET /v1/mod/reviews` : versions en attente, les plus anciennes d'abord.
pub async fn queue(State(st): State<AppState>, user: User) -> ApiResult<Json<Vec<ReviewRow>>> {
    require_mod(&user)?;
    let rows = sqlx::query_as::<_, ReviewRow>(&format!("{REVIEW_SELECT} WHERE v.status = 'pending' ORDER BY v.created_at"))
        .fetch_all(&st.db)
        .await?;
    Ok(Json(rows))
}

async fn pending(st: &AppState, vid: i64) -> ApiResult<(ReviewRow, String, Option<String>, Option<String>, serde_json::Value, String)> {
    let row = sqlx::query_as::<_, ReviewRow>(&format!("{REVIEW_SELECT} WHERE v.id = $1 AND v.status = 'pending'"))
        .bind(vid)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let (submitted, author_key, previous_key, meta, changelog): (String, Option<String>, Option<String>, Option<serde_json::Value>, String) = sqlx::query_as(
        "SELECT v.submitted_key, v.author_key,
                (SELECT file_key FROM versions WHERE item_id = v.item_id AND status = 'approved' ORDER BY id DESC LIMIT 1),
                v.meta, v.changelog
         FROM versions v WHERE v.id = $1",
    )
    .bind(vid)
    .fetch_one(&st.db)
    .await?;
    Ok((row, submitted, author_key, previous_key, meta.unwrap_or_default(), changelog))
}

async fn load(st: &AppState, key: &str) -> ApiResult<Package> {
    let bytes = st.storage.get(key).await.map_err(ApiError::internal)?.ok_or_else(|| ApiError::internal(format!("paquet absent : {key}")))?;
    read(&bytes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    path: String,
    size: usize,
    /// `added`, `modified`, `unchanged` ou `removed` (par rapport à la version approuvée).
    change: &'static str,
}

/// `GET /v1/mod/reviews/{id}` : manifeste, fichiers et différences, permissions ajoutées.
pub async fn detail(State(st): State<AppState>, user: User, Path(vid): Path<i64>) -> ApiResult<Json<serde_json::Value>> {
    require_mod(&user)?;
    let (row, submitted, author_key, previous_key, meta, changelog) = pending(&st, vid).await?;
    let pkg = load(&st, &submitted).await?;
    let previous = match &previous_key {
        Some(k) => Some(load(&st, k).await?),
        None => None,
    };
    let old: std::collections::HashMap<&str, String> =
        previous.as_ref().map(|p| p.files.iter().filter(|(n, _)| n != package::SIGNATURE_FILE).map(|(n, b)| (n.as_str(), sha(b))).collect()).unwrap_or_default();
    let mut files: Vec<FileChange> = pkg
        .files
        .iter()
        .filter(|(n, _)| n != package::SIGNATURE_FILE)
        .map(|(n, b)| FileChange {
            path: n.clone(),
            size: b.len(),
            change: match old.get(n.as_str()) {
                None => "added",
                Some(h) if *h == sha(b) => "unchanged",
                Some(_) => "modified",
            },
        })
        .collect();
    for (n, _) in old.iter().filter(|(n, _)| !pkg.files.iter().any(|(p, _)| p == *n)) {
        files.push(FileChange { path: n.to_string(), size: 0, change: "removed" });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let before: Vec<String> = previous.as_ref().map(|p| p.manifest.permissions.clone()).unwrap_or_default();
    let added: Vec<&String> = pkg.manifest.permissions.iter().filter(|p| !before.contains(p)).collect();
    Ok(Json(serde_json::json!({
        "review": row,
        "manifest": pkg.manifest,
        "files": files,
        "addedPermissions": added,
        "authorSignature": match pkg.signature { Signature::Unsigned => "unsigned", _ => "signed" },
        "authorKey": author_key.as_deref().map(package::fingerprint),
        "meta": meta,
        "changelog": changelog,
    })))
}

#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
}

const MAX_VIEW: usize = 2 << 20;

/// `GET /v1/mod/reviews/{id}/file?path=` : un fichier du paquet, en TEXTE (jamais rendu en
/// HTML), ou l'image telle quelle.
pub async fn file(State(st): State<AppState>, user: User, Path(vid): Path<i64>, Query(q): Query<FileQuery>) -> ApiResult<Response> {
    require_mod(&user)?;
    let (_, submitted, ..) = pending(&st, vid).await?;
    let pkg = load(&st, &submitted).await?;
    let (_, bytes) = pkg.files.iter().find(|(n, _)| *n == q.path).ok_or_else(ApiError::not_found)?;
    let ext = q.path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ if bytes.len() > MAX_VIEW => return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "tooLargeToView")),
        _ if std::str::from_utf8(bytes).is_err() => return Err(ApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, "binary")),
        _ => "text/plain; charset=utf-8",
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CONTENT_SECURITY_POLICY, "default-src 'none'; sandbox"),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes.clone(),
    )
        .into_response())
}

/// `POST /v1/mod/reviews/{id}/approve` : signature du magasin, puis publication.
pub async fn approve(State(st): State<AppState>, user: User, Path(vid): Path<i64>) -> ApiResult<StatusCode> {
    require_mod(&user)?;
    let key = st.signing_key.clone().ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "noSigningKey"))?;
    let (row, submitted, _, _, meta, _) = pending(&st, vid).await?;
    let pkg = load(&st, &submitted).await?;
    let mut out = Cursor::new(Vec::new());
    package::write(&mut out, &package::sign(&pkg.files, &key)).map_err(ApiError::internal)?;
    let signed = out.into_inner();
    let file_key = st.storage.package_key(&format!("{}/{}.{EXT}", row.item_id, row.version));
    st.storage.put(&file_key, &signed, "application/octet-stream").await.map_err(ApiError::internal)?;
    let meta: Meta = serde_json::from_value(meta).unwrap_or_default();
    let mut tx = st.db.begin().await?;
    let n = sqlx::query(
        "UPDATE versions SET status = 'approved', file_key = $2, size = $3, sha256 = $4, reviewed_by = $5, reviewed_at = now()
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(vid)
    .bind(&file_key)
    .bind(signed.len() as i64)
    .bind(sha(&signed))
    .bind(user.id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::not_found());
    }
    sqlx::query(
        "UPDATE items SET name = $2, summary = COALESCE($3, summary), description = COALESCE($4, description),
         tags = COALESCE($5, tags), homepage = COALESCE($6, homepage), repository = COALESCE($7, repository),
         license = COALESCE($8, license), updated_at = now() WHERE id = $1",
    )
    .bind(&row.item_id)
    .bind(if meta.name.is_empty() { &row.name } else { &meta.name })
    .bind(&meta.summary)
    .bind(&meta.description)
    .bind(meta.tags.as_ref().map(|t| serde_json::json!(t)))
    .bind(&meta.homepage)
    .bind(&meta.repository)
    .bind(&meta.license)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM assets WHERE item_id = $1").bind(&row.item_id).execute(&mut *tx).await?;
    for (kind, position, key) in &meta.assets {
        sqlx::query("INSERT INTO assets (item_id, kind, position, file_key) VALUES ($1, $2, $3, $4)")
            .bind(&row.item_id)
            .bind(kind)
            .bind(position)
            .bind(key)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO audit (actor_id, action, target, details) VALUES ($1, 'version.approve', $2, $3)")
        .bind(user.id)
        .bind(&row.item_id)
        .bind(serde_json::json!({ "version": row.version }))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!("extension approuvée : {} v{} par {}", row.item_id, row.version, user.login);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct RejectBody {
    reason: String,
}

/// `POST /v1/mod/reviews/{id}/reject` : refus motivé (montré à l'auteur).
pub async fn reject(State(st): State<AppState>, user: User, Path(vid): Path<i64>, Json(b): Json<RejectBody>) -> ApiResult<StatusCode> {
    require_mod(&user)?;
    let reason: String = b.reason.trim().chars().take(1000).collect();
    if reason.is_empty() {
        return Err(ApiError::bad("reasonRequired"));
    }
    let row: Option<(String, String)> = sqlx::query_as(
        "UPDATE versions SET status = 'rejected', reject_reason = $2, reviewed_by = $3, reviewed_at = now()
         WHERE id = $1 AND status = 'pending' RETURNING item_id, version",
    )
    .bind(vid)
    .bind(&reason)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?;
    let (item, version) = row.ok_or_else(ApiError::not_found)?;
    crate::admin::audit(&st, &user, "version.reject", &item, serde_json::json!({ "version": version, "reason": reason })).await?;
    Ok(StatusCode::NO_CONTENT)
}
