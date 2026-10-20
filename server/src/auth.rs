//! Authentification d'une requête, de deux façons :
//! - **jeton d'API** (`Authorization: Bearer ist_…`, CLI et app) : généré aléatoirement,
//!   montré une seule fois, stocké HACHÉ (SHA-256) ; délivré par la connexion GitHub
//!   (`login.rs`) ou, en développement, par `island-store token` ;
//! - **cookie de session** du panneau d'administration (`session.rs`), réservé aux
//!   modérateurs. Une requête qui MODIFIE quelque chose avec ce cookie doit aussi porter
//!   l'en-tête `X-Island-Admin` (qu'un autre site ne peut pas ajouter) : protection CSRF en
//!   plus du `SameSite=Strict`.
//!
//! Un compte banni n'est jamais authentifié ; le rôle est relu en base à chaque requête.

use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use base64::Engine as _;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::AppState;

/// Compte authentifié.
#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub role: String,
}

impl User {
    pub fn is_moderator(&self) -> bool {
        self.role == "moderator" || self.role == "admin"
    }
}

/// Nouveau jeton : `ist_` + 32 octets aléatoires (base64 URL).
pub fn new_token() -> String {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("getrandom");
    format!("ist_{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Crée un jeton pour un compte ; renvoie le jeton en clair (à montrer une seule fois).
pub async fn create_token(db: &sqlx::PgPool, user_id: i64, name: &str) -> Result<String, sqlx::Error> {
    let token = new_token();
    sqlx::query("INSERT INTO tokens (user_id, name, hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(name)
        .bind(hash_token(&token))
        .execute(db)
        .await?;
    Ok(token)
}

async fn authenticate(parts: &Parts, state: &AppState) -> Result<Option<User>, ApiError> {
    let Some(header) = parts.headers.get(axum::http::header::AUTHORIZATION) else { return session_user(parts, state).await };
    let token = header.to_str().ok().and_then(|h| h.strip_prefix("Bearer ")).ok_or_else(ApiError::unauthorized)?;
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "UPDATE tokens t SET last_used_at = now() FROM users u
         WHERE t.hash = $1 AND u.id = t.user_id AND NOT u.banned
         RETURNING u.id, u.login, u.role",
    )
    .bind(hash_token(token.trim()))
    .fetch_optional(&state.db)
    .await?;
    match row {
        Some((id, login, role)) => Ok(Some(User { id, login, role })),
        // Jeton présent mais inconnu / révoqué : refus explicite (pas « anonyme »).
        None => Err(ApiError::unauthorized()),
    }
}

/// Compte du cookie de session (modérateurs et admin seulement), sinon None.
async fn session_user(parts: &Parts, state: &AppState) -> Result<Option<User>, ApiError> {
    let Some(id) = crate::session::cookie(&parts.headers, crate::session::COOKIE)
        .and_then(|c| crate::session::verify(&state.session_key, c))
        .and_then(|v| v.parse::<i64>().ok())
    else {
        return Ok(None);
    };
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "UPDATE users SET last_seen_at = now() WHERE id = $1 AND NOT banned AND role IN ('moderator', 'admin') RETURNING id, login, role",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, login, role)) = row else { return Ok(None) };
    let safe = matches!(parts.method, axum::http::Method::GET | axum::http::Method::HEAD);
    if !safe && !parts.headers.contains_key("x-island-admin") {
        return Err(ApiError::new(axum::http::StatusCode::FORBIDDEN, "csrf"));
    }
    Ok(Some(User { id, login, role }))
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        authenticate(parts, state).await?.ok_or_else(ApiError::unauthorized)
    }
}

/// `Option<User>` : routes publiques qui reconnaissent un compte s'il est fourni.
impl OptionalFromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Option<Self>, Self::Rejection> {
        authenticate(parts, state).await
    }
}
