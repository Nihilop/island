//! `island-store` : serveur du magasin d'Island.
//!
//!   island-store                    lance l'API (variables d'environnement, cf. README)
//!   island-store token <login> [--role user|moderator|admin] [--name <nom>]
//!                                   crée le compte s'il n'existe pas et affiche un jeton
//!                                   d'API (développement, avant la connexion GitHub)

use std::net::SocketAddr;
use std::sync::Arc;

use island_store::{app, migrate, AppState, Storage};
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("island-store : {msg}");
    std::process::exit(1)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "island_store=info,tower_http=info".into()))
        .init();

    let url = env("DATABASE_URL").unwrap_or_else(|| fail("DATABASE_URL manquant"));
    let db = PgPoolOptions::new().max_connections(10).connect(&url).await.unwrap_or_else(|e| fail(format!("base : {e}")));
    migrate(&db).await.unwrap_or_else(|e| fail(format!("migrations : {e}")));

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("token") {
        return token(&db, &args[1..]).await;
    }

    let storage = Storage::from_config(&env("STORAGE").unwrap_or_else(|| "local:./data/files".into())).unwrap_or_else(|e| fail(e));
    let state = AppState { db, storage: Arc::new(storage), clamav: env("CLAMAV_URL"), trust_proxy: env("TRUST_PROXY").as_deref() == Some("1") };
    if state.clamav.is_none() {
        tracing::warn!("CLAMAV_URL absent : les fichiers publiés ne sont pas analysés (scan: skipped)");
    }
    let bind = env("BIND").unwrap_or_else(|| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| fail(format!("{bind} : {e}")));
    tracing::info!("magasin d'Island sur http://{bind}");
    let router = app(state).layer(TraceLayer::new_for_http());
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap_or_else(|e| fail(e));
}

/// `token <login> [--role r] [--name n]` : compte de développement + jeton d'API.
async fn token(db: &sqlx::PgPool, args: &[String]) {
    let login = args.first().filter(|l| !l.starts_with("--")).unwrap_or_else(|| fail("usage : island-store token <login> [--role user|moderator|admin] [--name <nom>]"));
    let opt = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let role = opt("--role").unwrap_or_else(|| "user".into());
    if !matches!(role.as_str(), "user" | "moderator" | "admin") {
        fail("rôle invalide (user, moderator, admin)");
    }
    let name = opt("--name").unwrap_or_else(|| "développement".into());
    let id: i64 = sqlx::query_scalar("INSERT INTO users (login, role) VALUES ($1, $2) ON CONFLICT (login) DO UPDATE SET role = $2 RETURNING id")
        .bind(login)
        .bind(&role)
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| fail(e));
    let token = island_store::auth::create_token(db, id, &name).await.unwrap_or_else(|e| fail(e));
    println!("{token}");
    eprintln!("jeton pour {login} ({role}) — il ne sera plus affiché.");
}
