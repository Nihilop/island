//! Éléments du magasin : catalogue (recherche plein texte en français, insensible aux
//! accents), fiche, téléchargement, vignette, publication, modification, retrait,
//! signalements. Étape 2 : les SCÈNES ; une extension est refusée (`extensionsNotYet`).
//!
//! Publication d'une scène, dans l'ordre :
//!   1. `islandscene::read` (tailles, noms simples, images décodées, scène validée) ;
//!   2. métadonnées retirées des images (EXIF, GPS…) ;
//!   3. analyse ClamAV (si configurée) ;
//!   4. vignette RÉGÉNÉRÉE par le serveur (celle du fichier n'est pas fiable) ;
//!   5. fichier réécrit (images nettoyées, vignette du serveur), stocké, puis publié.

use std::io::Cursor;
use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, FromRequestParts, Multipart, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use island_formats::islandscene;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::scan::{self, Scan};
use crate::AppState;

const PAGE: i64 = 24;
const MAX_SUMMARY: usize = 200;
const MAX_DESCRIPTION: usize = 5000;
const MAX_TAGS: usize = 8;
const MAX_TAG: usize = 24;
/// Signalements par IP et par heure.
const REPORTS_PER_HOUR: i64 = 5;

// --- Modèles ------------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub tags: serde_json::Value,
    pub downloads: i64,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub author_avatar: Option<String>,
    /// Dernière version publiée.
    pub version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<Card>,
    pub page: i64,
    pub total: i64,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub size: i64,
    pub changelog: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    #[serde(flatten)]
    pub card: Card,
    pub description: String,
    pub versions: Vec<VersionInfo>,
}

const CARD_SELECT: &str = "SELECT i.id, i.kind, i.name, i.summary, i.tags, i.downloads, i.updated_at,
        u.login AS author, u.avatar_url AS author_avatar,
        (SELECT v.version FROM versions v WHERE v.item_id = i.id AND v.status = 'approved' ORDER BY v.id DESC LIMIT 1) AS version
    FROM items i JOIN users u ON u.id = i.owner_id";

/// Visible publiquement : publié, auteur non banni, au moins une version approuvée.
const VISIBLE: &str = "i.status = 'published' AND NOT u.banned
    AND EXISTS (SELECT 1 FROM versions v WHERE v.item_id = i.id AND v.status = 'approved')";

// --- Catalogue ----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListQuery {
    kind: Option<String>,
    q: Option<String>,
    tag: Option<String>,
    sort: Option<String>,
    page: Option<i64>,
}

pub async fn list(State(st): State<AppState>, Query(p): Query<ListQuery>) -> ApiResult<Json<Page>> {
    let page = p.page.unwrap_or(0).clamp(0, 10_000);
    let q = p.q.as_deref().map(str::trim).filter(|q| !q.is_empty() && q.len() <= 200);
    let mut sql = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT c.*, count(*) OVER () AS total FROM (");
    sql.push(CARD_SELECT);
    sql.push(" WHERE ").push(VISIBLE);
    if let Some(k) = &p.kind {
        if k != "scene" && k != "extension" {
            return Err(ApiError::bad("invalidKind"));
        }
        sql.push(" AND i.kind = ").push_bind(k.clone());
    }
    if let Some(t) = p.tag.as_deref().filter(|t| !t.is_empty()) {
        sql.push(" AND i.tags ? ").push_bind(t.to_lowercase());
    }
    if let Some(q) = q {
        sql.push(" AND i.search @@ websearch_to_tsquery('french', island_unaccent(").push_bind(q.to_string()).push("))");
    }
    // Ordre : pertinence d'abord quand on cherche, puis le tri demandé.
    sql.push(" ORDER BY ");
    if let Some(q) = q {
        sql.push("ts_rank(i.search, websearch_to_tsquery('french', island_unaccent(").push_bind(q.to_string()).push("))) DESC, ");
    }
    sql.push(match p.sort.as_deref() {
        Some("popular") => "i.downloads DESC, i.updated_at DESC",
        _ => "i.updated_at DESC",
    });
    sql.push(", i.id) c LIMIT ").push_bind(PAGE).push(" OFFSET ").push_bind(page * PAGE);

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        card: Card,
        total: i64,
    }
    let rows: Vec<Row> = sql.build_query_as().fetch_all(&st.db).await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok(Json(Page { items: rows.into_iter().map(|r| r.card).collect(), page, total }))
}

async fn visible_card(st: &AppState, id: &str) -> ApiResult<Card> {
    sqlx::query_as::<_, Card>(&format!("{CARD_SELECT} WHERE i.id = $1 AND {VISIBLE}"))
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

pub async fn detail(State(st): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Detail>> {
    let card = visible_card(&st, &id).await?;
    let description: String = sqlx::query_scalar("SELECT description FROM items WHERE id = $1").bind(&id).fetch_one(&st.db).await?;
    let versions = sqlx::query_as::<_, VersionInfo>(
        "SELECT version, size, changelog, created_at FROM versions WHERE item_id = $1 AND status = 'approved' ORDER BY id DESC",
    )
    .bind(&id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(Detail { card, description, versions }))
}

// --- Téléchargement et vignette ------------------------------------------------------------

#[derive(Deserialize)]
pub struct DownloadQuery {
    version: Option<String>,
}

/// Nom de fichier proposé : lettres, chiffres, espaces et tirets (le reste devient « _ »).
fn file_name(name: &str) -> String {
    let n: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let n = n.trim();
    if n.is_empty() { "scene".into() } else { n.chars().take(60).collect() }
}

pub async fn download(State(st): State<AppState>, Path(id): Path<String>, Query(q): Query<DownloadQuery>) -> ApiResult<Response> {
    let card = visible_card(&st, &id).await?;
    let version = match q.version {
        Some(v) => v,
        None => card.version.clone().ok_or_else(ApiError::not_found)?,
    };
    let key: Option<String> = sqlx::query_scalar("SELECT file_key FROM versions WHERE item_id = $1 AND version = $2 AND status = 'approved'")
        .bind(&id)
        .bind(&version)
        .fetch_optional(&st.db)
        .await?;
    let key = key.ok_or_else(ApiError::not_found)?;
    let (file, len) = st.storage.open(&key).await.map_err(ApiError::internal)?.ok_or_else(|| ApiError::internal(format!("fichier absent : {key}")))?;
    sqlx::query("UPDATE items SET downloads = downloads + 1 WHERE id = $1").bind(&id).execute(&st.db).await?;
    let ext = if card.kind == "scene" { islandscene::EXT } else { "island" };
    let name = file_name(&card.name);
    let ascii: String = name.chars().map(|c| if c.is_ascii() { c } else { '_' }).collect();
    let utf8: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{ascii}.{ext}\"; filename*=UTF-8''{utf8}.{ext}")),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}

pub async fn thumbnail(State(st): State<AppState>, Path(id): Path<String>) -> ApiResult<Response> {
    visible_card(&st, &id).await?;
    let key: Option<String> = sqlx::query_scalar("SELECT file_key FROM assets WHERE item_id = $1 AND kind = 'thumbnail' AND position = 0")
        .bind(&id)
        .fetch_optional(&st.db)
        .await?;
    let bytes = match key {
        Some(k) => st.storage.get(&k).await.map_err(ApiError::internal)?,
        None => None,
    };
    let bytes = bytes.ok_or_else(ApiError::not_found)?;
    Ok(([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "public, max-age=300")], bytes).into_response())
}

// --- Publication -----------------------------------------------------------------------------

#[derive(Default)]
struct Form {
    file: Option<Vec<u8>>,
    item: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    changelog: Option<String>,
}

async fn read_form(mut mp: Multipart) -> ApiResult<Form> {
    let mut f = Form::default();
    while let Some(field) = mp.next_field().await.map_err(|_| ApiError::bad("invalidForm"))? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            f.file = Some(field.bytes().await.map_err(|_| ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "tooLarge"))?.to_vec());
            continue;
        }
        let text = field.text().await.map_err(|_| ApiError::bad("invalidForm"))?;
        match name.as_str() {
            "item" => f.item = Some(text),
            "summary" => f.summary = Some(text),
            "description" => f.description = Some(text),
            "changelog" => f.changelog = Some(text),
            "tags" => f.tags = Some(serde_json::from_str(&text).map_err(|_| ApiError::bad("invalidTags"))?),
            _ => {}
        }
    }
    Ok(f)
}

/// Étiquettes : 8 au plus, minuscules `[a-z0-9-]`, 24 caractères, sans doublon.
fn clean_tags(tags: &[String]) -> ApiResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_lowercase();
        if t.is_empty() || t.len() > MAX_TAG || !t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(ApiError::bad("invalidTags"));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(ApiError::bad("invalidTags"));
    }
    Ok(out)
}

fn check_len(v: &Option<String>, max: usize, code: &str) -> ApiResult<()> {
    match v {
        Some(s) if s.chars().count() > max => Err(ApiError::bad(code)),
        _ => Ok(()),
    }
}

fn new_id(prefix: &str) -> String {
    const ALPHA: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut b = [0u8; 10];
    getrandom::getrandom(&mut b).expect("getrandom");
    format!("{prefix}_{}", b.iter().map(|x| ALPHA[*x as usize % ALPHA.len()] as char).collect::<String>())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    pub id: String,
    pub version: String,
    pub scan: &'static str,
}

/// `POST /v1/items` (multipart) : `file` (requis), `item` (nouvelle version d'un élément
/// existant), `summary`, `description`, `tags` (tableau JSON), `changelog`.
pub async fn publish(State(st): State<AppState>, user: User, mp: Multipart) -> ApiResult<(StatusCode, Json<Published>)> {
    let f = read_form(mp).await?;
    let bytes = f.file.as_ref().ok_or_else(|| ApiError::bad("fileRequired"))?;
    check_len(&f.summary, MAX_SUMMARY, "summaryTooLong")?;
    check_len(&f.description, MAX_DESCRIPTION, "descriptionTooLong")?;
    check_len(&f.changelog, MAX_DESCRIPTION, "changelogTooLong")?;
    let tags = f.tags.as_deref().map(clean_tags).transpose()?;
    // Une extension est un zip qui contient un manifest.json : pas avant l'étape 6.
    if island_formats::package::read(Cursor::new(bytes), &[]).is_ok() {
        return Err(ApiError::bad("extensionsNotYet"));
    }

    // 1. Lecture vérifiée.
    let contents = islandscene::read(Cursor::new(bytes), bytes.len() as u64).map_err(|e| {
        if e == "tooLarge" { ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, e) } else { ApiError::bad(e) }
    })?;
    let scene = contents.scene;
    let name = scene.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("nameRequired"));
    }
    // 2. Métadonnées retirées.
    let files: Vec<(String, Vec<u8>)> = contents.files.into_iter().map(|(n, b)| (n, crate::sanitize::strip(&b))).collect();
    // 3. Antivirus (sur le fichier tel que reçu).
    let scan = scan::scan(st.clamav.as_deref(), bytes).await.map_err(ApiError::internal)?;
    if let Scan::Found(threat) = &scan {
        tracing::warn!("publication refusée ({}) : menace {threat}", user.login);
        return Err(ApiError::bad("malware"));
    }
    // 4. Vignette du serveur.
    let thumb = islandscene::thumbnail(&scene, |n| files.iter().find(|(f, _)| f == n).and_then(|(_, b)| image::load_from_memory(b).ok()));
    let thumb = islandscene::png(&thumb).map_err(ApiError::internal)?;
    // 5. Fichier réécrit.
    let mut out = Cursor::new(Vec::new());
    islandscene::write(&mut out, &scene, &files, &thumb, &contents.manifest.app).map_err(ApiError::internal)?;
    let packed = out.into_inner();
    let sha: String = Sha256::digest(&packed).iter().map(|b| format!("{b:02x}")).collect();

    let mut tx = st.db.begin().await?;
    let (id, version) = match &f.item {
        Some(id) => {
            let row: Option<(String, i64)> = sqlx::query_as("SELECT kind, owner_id FROM items WHERE id = $1 AND status <> 'removed' FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
            let (kind, owner) = row.ok_or_else(ApiError::not_found)?;
            if owner != user.id {
                return Err(ApiError::forbidden());
            }
            if kind != "scene" {
                return Err(ApiError::bad("kindMismatch"));
            }
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM versions WHERE item_id = $1").bind(id).fetch_one(&mut *tx).await?;
            sqlx::query(
                "UPDATE items SET name = $2, summary = COALESCE($3, summary), description = COALESCE($4, description),
                 tags = COALESCE($5, tags), updated_at = now() WHERE id = $1",
            )
            .bind(id)
            .bind(&name)
            .bind(&f.summary)
            .bind(&f.description)
            .bind(tags.as_ref().map(|t| serde_json::json!(t)))
            .execute(&mut *tx)
            .await?;
            (id.clone(), (n + 1).to_string())
        }
        None => {
            let id = new_id("sc");
            sqlx::query("INSERT INTO items (id, kind, owner_id, name, summary, description, tags) VALUES ($1, 'scene', $2, $3, $4, $5, $6)")
                .bind(&id)
                .bind(user.id)
                .bind(&name)
                .bind(f.summary.as_deref().unwrap_or(""))
                .bind(f.description.as_deref().unwrap_or(""))
                .bind(serde_json::json!(tags.unwrap_or_default()))
                .execute(&mut *tx)
                .await?;
            (id, "1".to_string())
        }
    };
    let file_key = format!("items/{id}/{version}.{}", islandscene::EXT);
    let thumb_key = format!("items/{id}/thumbnail.png");
    st.storage.put(&file_key, &packed).await.map_err(ApiError::internal)?;
    st.storage.put(&thumb_key, &thumb).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO versions (item_id, version, status, file_key, size, sha256, scan, changelog) VALUES ($1, $2, 'approved', $3, $4, $5, $6, $7)")
        .bind(&id)
        .bind(&version)
        .bind(&file_key)
        .bind(packed.len() as i64)
        .bind(&sha)
        .bind(scan.label())
        .bind(f.changelog.as_deref().unwrap_or(""))
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO assets (item_id, kind, position, file_key) VALUES ($1, 'thumbnail', 0, $2) ON CONFLICT (item_id, kind, position) DO UPDATE SET file_key = $2")
        .bind(&id)
        .bind(&thumb_key)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!("publié : {id} v{version} par {} (analyse : {})", user.login, scan.label());
    Ok((StatusCode::CREATED, Json(Published { id, version, scan: scan.label() })))
}

// --- Modification, retrait ------------------------------------------------------------------

/// L'élément existe (non retiré) et `user` peut le modifier (auteur ou modérateur).
async fn editable(st: &AppState, user: &User, id: &str) -> ApiResult<()> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM items WHERE id = $1 AND status <> 'removed'").bind(id).fetch_optional(&st.db).await?;
    match owner {
        None => Err(ApiError::not_found()),
        Some(o) if o == user.id || user.is_moderator() => Ok(()),
        Some(_) => Err(ApiError::forbidden()),
    }
}

#[derive(Deserialize)]
pub struct Patch {
    name: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
}

pub async fn update(State(st): State<AppState>, user: User, Path(id): Path<String>, Json(p): Json<Patch>) -> ApiResult<StatusCode> {
    editable(&st, &user, &id).await?;
    if let Some(n) = &p.name {
        if n.trim().is_empty() || n.chars().count() > 80 {
            return Err(ApiError::bad("invalidName"));
        }
    }
    check_len(&p.summary, MAX_SUMMARY, "summaryTooLong")?;
    check_len(&p.description, MAX_DESCRIPTION, "descriptionTooLong")?;
    let tags = p.tags.as_deref().map(clean_tags).transpose()?;
    sqlx::query(
        "UPDATE items SET name = COALESCE($2, name), summary = COALESCE($3, summary), description = COALESCE($4, description),
         tags = COALESCE($5, tags), updated_at = now() WHERE id = $1",
    )
    .bind(&id)
    .bind(p.name.as_deref().map(str::trim))
    .bind(&p.summary)
    .bind(&p.description)
    .bind(tags.map(|t| serde_json::json!(t)))
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Retire l'élément du catalogue (les installations existantes ne sont pas touchées).
pub async fn remove(State(st): State<AppState>, user: User, Path(id): Path<String>) -> ApiResult<StatusCode> {
    editable(&st, &user, &id).await?;
    sqlx::query("UPDATE items SET status = 'removed', updated_at = now() WHERE id = $1").bind(&id).execute(&st.db).await?;
    if user.is_moderator() {
        sqlx::query("INSERT INTO audit (actor_id, action, target) VALUES ($1, 'item.remove', $2)").bind(user.id).bind(&id).execute(&st.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// --- Mes éléments ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Mine {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub downloads: i64,
    pub updated_at: DateTime<Utc>,
    pub versions: i64,
}

pub async fn mine(State(st): State<AppState>, user: User) -> ApiResult<Json<Vec<Mine>>> {
    let rows = sqlx::query_as::<_, Mine>(
        "SELECT i.id, i.kind, i.name, i.status, i.downloads, i.updated_at,
                (SELECT count(*) FROM versions v WHERE v.item_id = i.id) AS versions
         FROM items i WHERE i.owner_id = $1 AND i.status <> 'removed' ORDER BY i.updated_at DESC",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

// --- Signalements ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ReportBody {
    reason: String,
    #[serde(default)]
    details: String,
}

/// Adresse de la connexion, si le serveur la connaît (absente dans les tests).
pub struct Peer(Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for Peer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Peer(parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0)))
    }
}

/// IP du client : `X-Forwarded-For` derrière le proxy de la plateforme, sinon la connexion.
fn client_ip(st: &AppState, headers: &HeaderMap, conn: Option<SocketAddr>) -> String {
    if st.trust_proxy {
        if let Some(ip) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()) {
            return ip.trim().to_string();
        }
    }
    conn.map(|a| a.ip().to_string()).unwrap_or_else(|| "unknown".into())
}

pub async fn report(
    State(st): State<AppState>,
    user: Option<User>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Peer(conn): Peer,
    Json(r): Json<ReportBody>,
) -> ApiResult<StatusCode> {
    if !matches!(r.reason.as_str(), "illegal" | "malware" | "copyright" | "offensive" | "broken" | "other") {
        return Err(ApiError::bad("invalidReason"));
    }
    if r.details.chars().count() > 2000 {
        return Err(ApiError::bad("detailsTooLong"));
    }
    visible_card(&st, &id).await?;
    // Empreinte de l'IP (jamais l'IP elle-même), pour limiter les signalements anonymes.
    let ip = client_ip(&st, &headers, conn);
    let ip_hash: String = Sha256::digest(format!("island-store:{ip}").as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM reports WHERE ip_hash = $1 AND created_at > now() - interval '1 hour'")
        .bind(&ip_hash)
        .fetch_one(&st.db)
        .await?;
    if recent >= REPORTS_PER_HOUR {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rateLimited"));
    }
    sqlx::query("INSERT INTO reports (item_id, user_id, ip_hash, reason, details) VALUES ($1, $2, $3, $4, $5)")
        .bind(&id)
        .bind(user.map(|u| u.id))
        .bind(&ip_hash)
        .bind(&r.reason)
        .bind(r.details.trim())
        .execute(&st.db)
        .await?;
    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_normalized_and_bounded() {
        assert_eq!(clean_tags(&["Neige".into(), "neige".into(), "nuit".into()]).unwrap(), ["neige", "nuit"]);
        assert!(clean_tags(&["avec espace".into()]).is_err());
        assert!(clean_tags(&(0..9).map(|i| format!("t{i}")).collect::<Vec<_>>()).is_err());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Nuit de neige"), "Nuit de neige");
        assert_eq!(file_name("../../etc"), "______etc");
        assert_eq!(file_name("   "), "scene");
    }
}
