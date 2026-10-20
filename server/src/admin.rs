//! Modération (`/v1/mod/*`, modérateurs et admin) et administration (`/v1/admin/*`, admin
//! seulement). Toute action est inscrite au journal (`audit`).
//!
//! - Modération : signalements (clore, avec ou sans masquage de l'élément), éléments de tous
//!   statuts (masquer, rétablir).
//! - Administration : comptes, modérateurs (ajoutés par leur nom GitHub, converti tout de suite
//!   en identifiant numérique), bannissement (jetons révoqués ; ses éléments disparaissent du
//!   catalogue), journal.
//!
//! L'admin (`ADMIN_GITHUB_ID`) ne peut être ni rétrogradé ni banni, et personne ne peut se
//! bannir lui-même.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

const PAGE: i64 = 30;

fn require_mod(u: &User) -> ApiResult<()> {
    if u.is_moderator() { Ok(()) } else { Err(ApiError::forbidden()) }
}

fn require_admin(u: &User) -> ApiResult<()> {
    if u.role == "admin" { Ok(()) } else { Err(ApiError::forbidden()) }
}

pub async fn audit(st: &AppState, actor: &User, action: &str, target: &str, details: serde_json::Value) -> ApiResult<()> {
    sqlx::query("INSERT INTO audit (actor_id, action, target, details) VALUES ($1, $2, $3, $4)")
        .bind(actor.id)
        .bind(action)
        .bind(target)
        .bind(details)
        .execute(&st.db)
        .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct Paging {
    page: Option<i64>,
    q: Option<String>,
    status: Option<String>,
    role: Option<String>,
}

impl Paging {
    fn page(&self) -> i64 {
        self.page.unwrap_or(0).clamp(0, 10_000)
    }
    fn q(&self) -> Option<String> {
        self.q.as_deref().map(str::trim).filter(|q| !q.is_empty() && q.len() <= 100).map(|q| format!("%{}%", q.replace(['%', '_'], "")))
    }
}

#[derive(Serialize)]
pub struct Paged<T> {
    items: Vec<T>,
    page: i64,
    total: i64,
}

// --- Signalements ----------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    id: i64,
    item_id: String,
    item_name: String,
    item_kind: String,
    item_status: String,
    owner: String,
    reason: String,
    details: String,
    reporter: Option<String>,
    created_at: DateTime<Utc>,
    resolved_by: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    /// Signalements encore ouverts sur le même élément.
    open_for_item: i64,
    #[serde(skip)]
    total: i64,
}

pub async fn reports(State(st): State<AppState>, user: User, Query(p): Query<Paging>) -> ApiResult<Json<Paged<ReportRow>>> {
    require_mod(&user)?;
    let open = p.status.as_deref() != Some("resolved");
    let rows = sqlx::query_as::<_, ReportRow>(
        "SELECT r.id, r.item_id, i.name AS item_name, i.kind AS item_kind, i.status AS item_status, o.login AS owner,
                r.reason, r.details, ru.login AS reporter, r.created_at, mu.login AS resolved_by, r.resolved_at,
                (SELECT count(*) FROM reports r2 WHERE r2.item_id = r.item_id AND r2.resolved_at IS NULL) AS open_for_item,
                count(*) OVER () AS total
         FROM reports r
         JOIN items i ON i.id = r.item_id JOIN users o ON o.id = i.owner_id
         LEFT JOIN users ru ON ru.id = r.user_id LEFT JOIN users mu ON mu.id = r.resolved_by
         WHERE (r.resolved_at IS NULL) = $1
         ORDER BY r.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(open)
    .bind(PAGE)
    .bind(p.page() * PAGE)
    .fetch_all(&st.db)
    .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok(Json(Paged { items: rows, page: p.page(), total }))
}

#[derive(Deserialize)]
pub struct Resolve {
    /// `dismiss` (rien à faire) ou `hide` (masquer l'élément).
    action: String,
}

/// Clôt le signalement — et, avec `hide`, tous les signalements ouverts du même élément,
/// qui est masqué.
pub async fn resolve(State(st): State<AppState>, user: User, Path(id): Path<i64>, Json(r): Json<Resolve>) -> ApiResult<StatusCode> {
    require_mod(&user)?;
    if r.action != "dismiss" && r.action != "hide" {
        return Err(ApiError::bad("invalidAction"));
    }
    let item: Option<String> = sqlx::query_scalar("SELECT item_id FROM reports WHERE id = $1").bind(id).fetch_optional(&st.db).await?;
    let item = item.ok_or_else(ApiError::not_found)?;
    if r.action == "hide" {
        sqlx::query("UPDATE items SET status = 'hidden', updated_at = now() WHERE id = $1 AND status = 'published'").bind(&item).execute(&st.db).await?;
        sqlx::query("UPDATE reports SET resolved_by = $1, resolved_at = now() WHERE item_id = $2 AND resolved_at IS NULL")
            .bind(user.id)
            .bind(&item)
            .execute(&st.db)
            .await?;
    } else {
        sqlx::query("UPDATE reports SET resolved_by = $1, resolved_at = now() WHERE id = $2 AND resolved_at IS NULL").bind(user.id).bind(id).execute(&st.db).await?;
    }
    audit(&st, &user, &format!("report.{}", r.action), &item, serde_json::json!({ "report": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Éléments ---------------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ItemRow {
    id: String,
    kind: String,
    name: String,
    status: String,
    owner: String,
    owner_banned: bool,
    downloads: i64,
    versions: i64,
    open_reports: i64,
    updated_at: DateTime<Utc>,
    thumbnail: Option<String>,
    #[serde(skip)]
    total: i64,
}

pub async fn items(State(st): State<AppState>, user: User, Query(p): Query<Paging>) -> ApiResult<Json<Paged<ItemRow>>> {
    require_mod(&user)?;
    let status = p.status.clone().filter(|s| matches!(s.as_str(), "published" | "hidden" | "removed"));
    let rows = sqlx::query_as::<_, ItemRow>(
        "SELECT i.id, i.kind, i.name, i.status, u.login AS owner, u.banned AS owner_banned, i.downloads,
                (SELECT count(*) FROM versions v WHERE v.item_id = i.id) AS versions,
                (SELECT count(*) FROM reports r WHERE r.item_id = i.id AND r.resolved_at IS NULL) AS open_reports,
                i.updated_at, (SELECT a.file_key FROM assets a WHERE a.item_id = i.id AND a.kind = 'thumbnail' AND a.position = 0) AS thumbnail, count(*) OVER () AS total
         FROM items i JOIN users u ON u.id = i.owner_id
         WHERE ($1::text IS NULL OR i.status = $1) AND ($2::text IS NULL OR i.name ILIKE $2 OR u.login ILIKE $2 OR i.id = $3)
         ORDER BY i.updated_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(status)
    .bind(p.q())
    .bind(p.q.clone().unwrap_or_default())
    .bind(PAGE)
    .bind(p.page() * PAGE)
    .fetch_all(&st.db)
    .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    let rows = rows.into_iter().map(|mut r| {
        r.thumbnail = crate::items::thumbnail_url(&st, &r.id, r.thumbnail.take());
        r
    });
    Ok(Json(Paged { items: rows.collect(), page: p.page(), total }))
}

async fn set_item_status(st: &AppState, user: &User, id: &str, from: &str, to: &str, action: &str) -> ApiResult<StatusCode> {
    require_mod(user)?;
    let n = sqlx::query("UPDATE items SET status = $3, updated_at = now() WHERE id = $1 AND status = $2")
        .bind(id)
        .bind(from)
        .bind(to)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::not_found());
    }
    audit(st, user, action, id, serde_json::json!({})).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn hide(State(st): State<AppState>, user: User, Path(id): Path<String>) -> ApiResult<StatusCode> {
    set_item_status(&st, &user, &id, "published", "hidden", "item.hide").await
}

pub async fn restore(State(st): State<AppState>, user: User, Path(id): Path<String>) -> ApiResult<StatusCode> {
    set_item_status(&st, &user, &id, "hidden", "published", "item.restore").await
}

// --- Comptes ------------------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    id: i64,
    github_id: Option<i64>,
    login: String,
    avatar_url: Option<String>,
    role: String,
    banned: bool,
    items: i64,
    created_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    total: i64,
}

pub async fn users(State(st): State<AppState>, user: User, Query(p): Query<Paging>) -> ApiResult<Json<Paged<UserRow>>> {
    require_admin(&user)?;
    let role = p.role.clone().filter(|r| matches!(r.as_str(), "user" | "moderator" | "admin" | "banned"));
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.github_id, u.login, u.avatar_url, u.role, u.banned,
                (SELECT count(*) FROM items i WHERE i.owner_id = u.id AND i.status <> 'removed') AS items,
                u.created_at, u.last_seen_at, count(*) OVER () AS total
         FROM users u
         WHERE ($1::text IS NULL OR ($1 = 'banned' AND u.banned) OR ($1 <> 'banned' AND u.role = $1))
           AND ($2::text IS NULL OR u.login ILIKE $2)
         ORDER BY (u.role = 'admin') DESC, (u.role = 'moderator') DESC, u.last_seen_at DESC NULLS LAST, u.id
         LIMIT $3 OFFSET $4",
    )
    .bind(role)
    .bind(p.q())
    .bind(PAGE)
    .bind(p.page() * PAGE)
    .fetch_all(&st.db)
    .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok(Json(Paged { items: rows, page: p.page(), total }))
}

#[derive(Deserialize)]
pub struct AddModerator {
    login: String,
}

/// Le compte est-il l'admin configuré (ou un admin) ? Il ne peut pas être modifié.
async fn protected(st: &AppState, id: i64) -> ApiResult<(String, Option<i64>, String)> {
    let row: Option<(String, Option<i64>, String)> = sqlx::query_as("SELECT login, github_id, role FROM users WHERE id = $1").bind(id).fetch_optional(&st.db).await?;
    let row = row.ok_or_else(ApiError::not_found)?;
    if row.2 == "admin" || (row.1.is_some() && row.1 == st.admin_github_id) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "isAdmin"));
    }
    Ok(row)
}

/// Ajoute un modérateur à partir de son nom GitHub (converti tout de suite en identifiant
/// numérique : le nom peut changer, l'identifiant non). Sans GitHub configuré
/// (développement), seul un compte local existant peut être promu.
pub async fn add_moderator(State(st): State<AppState>, user: User, Json(b): Json<AddModerator>) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&user)?;
    let login = b.login.trim().trim_start_matches('@');
    if login.is_empty() || login.len() > 39 || !login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(ApiError::bad("invalidLogin"));
    }
    let id: i64 = match &st.github {
        Some(gh) => {
            let who = gh.lookup(login).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("github:{e}")))?.ok_or_else(|| ApiError::bad("unknownGithubUser"))?;
            if st.admin_github_id == Some(who.id) {
                return Err(ApiError::bad("isAdmin"));
            }
            sqlx::query("UPDATE users SET login = login || '~' || id WHERE login = $1 AND github_id IS DISTINCT FROM $2")
                .bind(&who.login)
                .bind(who.id)
                .execute(&st.db)
                .await?;
            sqlx::query_scalar(
                "INSERT INTO users (github_id, login, avatar_url, role) VALUES ($1, $2, $3, 'moderator')
                 ON CONFLICT (github_id) DO UPDATE SET login = $2, avatar_url = $3,
                     role = CASE WHEN users.role = 'admin' THEN 'admin' ELSE 'moderator' END
                 RETURNING id",
            )
            .bind(who.id)
            .bind(&who.login)
            .bind(&who.avatar_url)
            .fetch_one(&st.db)
            .await?
        }
        None => sqlx::query_scalar("UPDATE users SET role = 'moderator' WHERE login = $1 AND role = 'user' RETURNING id")
            .bind(login)
            .fetch_optional(&st.db)
            .await?
            .ok_or_else(|| ApiError::bad("unknownUser"))?,
    };
    audit(&st, &user, "moderator.add", login, serde_json::json!({ "user": id })).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn remove_moderator(State(st): State<AppState>, user: User, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    require_admin(&user)?;
    let (login, _, role) = protected(&st, id).await?;
    if role != "moderator" {
        return Err(ApiError::bad("notModerator"));
    }
    sqlx::query("UPDATE users SET role = 'user' WHERE id = $1").bind(id).execute(&st.db).await?;
    audit(&st, &user, "moderator.remove", &login, serde_json::json!({ "user": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Bannit : connexion refusée, jetons révoqués, éléments retirés du catalogue public.
pub async fn ban(State(st): State<AppState>, user: User, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    require_admin(&user)?;
    if id == user.id {
        return Err(ApiError::bad("cannotBanSelf"));
    }
    let (login, _, _) = protected(&st, id).await?;
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE users SET banned = TRUE, role = 'user' WHERE id = $1").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM tokens WHERE user_id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    audit(&st, &user, "user.ban", &login, serde_json::json!({ "user": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unban(State(st): State<AppState>, user: User, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    require_admin(&user)?;
    let login: Option<String> = sqlx::query_scalar("UPDATE users SET banned = FALSE WHERE id = $1 AND banned RETURNING login").bind(id).fetch_optional(&st.db).await?;
    let login = login.ok_or_else(ApiError::not_found)?;
    audit(&st, &user, "user.unban", &login, serde_json::json!({ "user": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Journal --------------------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuditRow {
    id: i64,
    actor: Option<String>,
    action: String,
    target: String,
    details: serde_json::Value,
    created_at: DateTime<Utc>,
    #[serde(skip)]
    total: i64,
}

pub async fn journal(State(st): State<AppState>, user: User, Query(p): Query<Paging>) -> ApiResult<Json<Paged<AuditRow>>> {
    require_admin(&user)?;
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT a.id, u.login AS actor, a.action, a.target, a.details, a.created_at, count(*) OVER () AS total
         FROM audit a LEFT JOIN users u ON u.id = a.actor_id ORDER BY a.id DESC LIMIT $1 OFFSET $2",
    )
    .bind(PAGE)
    .bind(p.page() * PAGE)
    .fetch_all(&st.db)
    .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok(Json(Paged { items: rows, page: p.page(), total }))
}

/// Compteurs de l'en-tête du panneau.
pub async fn summary(State(st): State<AppState>, user: User) -> ApiResult<Json<serde_json::Value>> {
    require_mod(&user)?;
    let (reports, hidden, pending): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM reports WHERE resolved_at IS NULL),
                (SELECT count(*) FROM items WHERE status = 'hidden'),
                (SELECT count(*) FROM versions WHERE status = 'pending')",
    )
    .fetch_one(&st.db)
    .await?;
    Ok(Json(serde_json::json!({ "openReports": reports, "hiddenItems": hidden, "pendingVersions": pending })))
}
