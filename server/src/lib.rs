//! Serveur du magasin d'Island (cf. `server/README.md`). Sans état : tout est dans Postgres
//! et le stockage de fichiers ; on le redéploie ou on en lance plusieurs sans rien gérer.
//!
//!   error     : erreurs de l'API (`{ "error": "<code>" }`, codes stables)
//!   storage   : fichiers (dossier local en développement ; R2 en production, à venir)
//!   auth      : jetons d'API (hachés en base), compte courant
//!   sanitize  : retrait des métadonnées des images (EXIF, GPS, blocs texte), sans recompression
//!   scan      : analyse antivirus ClamAV (`clamd`, optionnelle)
//!   items     : catalogue, fiche, téléchargement, publication, signalements

pub mod auth;
pub mod error;
pub mod items;
pub mod sanitize;
pub mod scan;
pub mod storage;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;

pub use storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub storage: Arc<Storage>,
    /// `clamd` (`host:port`) ; None = pas d'analyse (versions marquées `scan: skipped`).
    pub clamav: Option<String>,
    /// Faire confiance à `X-Forwarded-For` (derrière le proxy de la plateforme).
    pub trust_proxy: bool,
}

/// Publication : le fichier entier passe par le serveur (jamais d'envoi direct au stockage).
const UPLOAD_LIMIT: usize = island_formats::islandscene::MAX_FILE as usize + (2 << 20);

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/items", get(items::list).post(items::publish).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/v1/items/{id}", get(items::detail).patch(items::update).delete(items::remove))
        .route("/v1/items/{id}/download", get(items::download))
        .route("/v1/items/{id}/thumbnail", get(items::thumbnail))
        .route("/v1/items/{id}/reports", post(items::report))
        .route("/v1/me/items", get(items::mine))
        .with_state(state)
}

/// Migrations embarquées (`server/migrations`).
pub async fn migrate(db: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(db).await
}
