//! Erreurs de l'API : `{ "error": "<code>" }` avec un statut HTTP. Les codes sont stables
//! (l'app les traduit) : ceux d'`island-formats` (`corrupt`, `tooLarge`, `missingFile:…`…)
//! et ceux du serveur (`unauthorized`, `forbidden`, `notFound`, `rateLimited`…).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>) -> Self {
        Self { status, code: code.into() }
    }
    pub fn bad(code: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code)
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "notFound")
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }
    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }
    /// Erreur interne : détaillée dans les journaux, jamais dans la réponse.
    pub fn internal(e: impl std::fmt::Display) -> Self {
        tracing::error!("erreur interne : {e}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.code }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(e)
    }
}
