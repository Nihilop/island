//! Jetons d'API (`Authorization: Bearer ist_…`) : générés aléatoirement, montrés une seule
//! fois, stockés HACHÉS (SHA-256). Un compte banni n'est plus authentifié. La connexion
//! GitHub (device flow) qui délivre ces jetons arrive à l'étape 3 ; d'ici là, la commande
//! `island-store token` en crée pour le développement.

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
    let Some(header) = parts.headers.get(axum::http::header::AUTHORIZATION) else { return Ok(None) };
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
