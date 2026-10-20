//! Serveur du magasin d'Island (cf. `server/README.md`). Sans état : tout est dans Postgres
//! et le stockage de fichiers ; on le redéploie ou on en lance plusieurs sans rien gérer.
//!
//!   error     : erreurs de l'API (`{ "error": "<code>" }`, codes stables)
//!   storage   : fichiers (dossier local en développement ; R2 en production, à venir)
//!   auth      : jetons d'API (hachés en base), compte courant
//!   sanitize  : retrait des métadonnées des images (EXIF, GPS, blocs texte), sans recompression
//!   scan      : analyse antivirus ClamAV (`clamd`, optionnelle)
//!   items     : catalogue, fiche, téléchargement, publication, signalements
//!   github    : client GitHub (device flow, OAuth web, identité)
//!   session   : cookie de session signé du panneau d'administration
//!   login     : connexion GitHub, compte courant, jetons, rôles
//!   admin     : modération (signalements, éléments) et administration (comptes, journal)

pub mod admin;
pub mod auth;
pub mod error;
pub mod github;
pub mod items;
pub mod login;
pub mod media;
pub mod sanitize;
pub mod scan;
pub mod session;
pub mod settings;
pub mod storage;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
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
    /// None = connexion GitHub indisponible (`githubNotConfigured`).
    pub github: Option<github::GitHub>,
    /// Identifiant numérique GitHub du super admin.
    pub admin_github_id: Option<i64>,
    /// Clé de signature des cookies (`SESSION_KEY`).
    pub session_key: Arc<Vec<u8>>,
    /// URL publique (retour de l'OAuth web : `<PUBLIC_URL>/admin/callback`).
    pub public_url: String,
    /// Cookies `Secure` (PUBLIC_URL en https).
    pub secure_cookies: bool,
    /// Panneau d'administration construit (`server/admin/dist`) ; None = page d'attente.
    pub admin_dist: Option<std::path::PathBuf>,
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
        .route("/v1/items/{id}/media/{kind}/{position}", get(items::media))
        .route("/v1/items/{id}/reports", post(items::report))
        .route("/v1/me/items", get(items::mine))
        .route("/v1/store/status", get(settings::status))
        .route("/v1/auth/device", post(login::device))
        .route("/v1/auth/token", post(login::token).delete(login::sign_out))
        .route("/v1/me", get(login::me))
        .route("/v1/me/tokens", get(login::tokens))
        .route("/v1/me/tokens/{id}", delete(login::revoke))
        .route("/v1/mod/summary", get(admin::summary))
        .route("/v1/mod/reports", get(admin::reports))
        .route("/v1/mod/reports/{id}/resolve", post(admin::resolve))
        .route("/v1/mod/items", get(admin::items))
        .route("/v1/mod/items/{id}/hide", post(admin::hide))
        .route("/v1/mod/items/{id}/restore", post(admin::restore))
        .route("/v1/mod/publishing", get(settings::get_publishing).put(settings::set_publishing))
        .route("/v1/admin/users", get(admin::users))
        .route("/v1/admin/moderators", post(admin::add_moderator))
        .route("/v1/admin/moderators/{id}", delete(admin::remove_moderator))
        .route("/v1/admin/users/{id}/ban", post(admin::ban))
        .route("/v1/admin/users/{id}/unban", post(admin::unban))
        .route("/v1/admin/audit", get(admin::journal))
        .route("/admin", get(admin_page))
        .route("/admin/login", get(login::admin_login))
        .route("/admin/callback", get(login::admin_callback))
        .route("/admin/logout", post(login::admin_logout))
        .nest_service(
            "/admin/assets",
            tower_http::services::ServeDir::new(state.admin_dist.clone().unwrap_or_default().join("assets")),
        )
        .with_state(state)
}

/// Le panneau (application Vue, routage par `#`) ou, s'il n'est pas construit, la page
/// d'attente avec le lien de connexion.
async fn admin_page(axum::extract::State(st): axum::extract::State<AppState>, user: Option<auth::User>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if let Some(dir) = &st.admin_dist {
        if let Ok(html) = tokio::fs::read_to_string(dir.join("index.html")).await {
            return (
                [(axum::http::header::CACHE_CONTROL, "no-store"), (axum::http::header::X_FRAME_OPTIONS, "DENY")],
                axum::response::Html(html),
            )
                .into_response();
        }
    }
    login::admin_home(user).await.into_response()
}

/// Migrations embarquées (`server/migrations`).
pub async fn migrate(db: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(db).await
}
