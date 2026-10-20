//! Réglages du magasin modifiables à chaud (table `settings`).
//!
//! `publishing` : interrupteur d'urgence. Fermé, le serveur refuse toute publication
//! (nouvel élément, nouvelle version, donc tout envoi de fichier ou de média) et toute
//! modification de fiche, AVANT de lire le corps de la requête : une vague d'envois ne coûte
//! presque rien. Le catalogue, les téléchargements et les signalements continuent. Lu en
//! base à chaque publication (plusieurs instances du serveur restent d'accord).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

const MAX_MESSAGE: usize = 300;

#[derive(Serialize, Deserialize, Clone)]
pub struct Publishing {
    pub open: bool,
    /// Explication montrée aux auteurs quand c'est fermé.
    #[serde(default)]
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishingState {
    #[serde(flatten)]
    pub publishing: Publishing,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

pub async fn publishing(st: &AppState) -> ApiResult<PublishingState> {
    let (value, updated_at, updated_by): (serde_json::Value, DateTime<Utc>, Option<String>) = sqlx::query_as(
        "SELECT s.value, s.updated_at, u.login FROM settings s LEFT JOIN users u ON u.id = s.updated_by WHERE s.key = 'publishing'",
    )
    .fetch_one(&st.db)
    .await?;
    let publishing = serde_json::from_value(value).unwrap_or(Publishing { open: true, message: String::new() });
    Ok(PublishingState { publishing, updated_at, updated_by })
}

/// Refuse (503 `publishingPaused`) si les publications sont fermées.
pub async fn ensure_open(st: &AppState) -> ApiResult<()> {
    if publishing(st).await?.publishing.open { Ok(()) } else { Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "publishingPaused")) }
}

/// `GET /v1/store/status` (public) : l'app sait si elle peut proposer de publier.
pub async fn status(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let p = publishing(&st).await?.publishing;
    Ok(Json(serde_json::json!({ "publishing": p })))
}

/// `GET /v1/mod/publishing`.
pub async fn get_publishing(State(st): State<AppState>, user: User) -> ApiResult<Json<PublishingState>> {
    if !user.is_moderator() {
        return Err(ApiError::forbidden());
    }
    Ok(Json(publishing(&st).await?))
}

/// `PUT /v1/mod/publishing` : ouvrir ou fermer (modérateurs et admin : c'est une urgence).
pub async fn set_publishing(State(st): State<AppState>, user: User, Json(mut p): Json<Publishing>) -> ApiResult<Json<PublishingState>> {
    if !user.is_moderator() {
        return Err(ApiError::forbidden());
    }
    p.message = p.message.trim().chars().take(MAX_MESSAGE).collect();
    if p.open {
        p.message.clear();
    }
    sqlx::query("UPDATE settings SET value = $1, updated_at = now(), updated_by = $2 WHERE key = 'publishing'")
        .bind(serde_json::json!(p))
        .bind(user.id)
        .execute(&st.db)
        .await?;
    let action = if p.open { "publishing.open" } else { "publishing.pause" };
    crate::admin::audit(&st, &user, action, "store", serde_json::json!({ "message": p.message })).await?;
    tracing::warn!("publications {} par {}", if p.open { "rouvertes" } else { "FERMÉES" }, user.login);
    Ok(Json(publishing(&st).await?))
}
