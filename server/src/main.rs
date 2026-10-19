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
    // Développement : variables lues depuis `server/.env` s'il existe (exclu du dépôt).
    let _ = dotenvy::dotenv();
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

    // STORE_ENV=dev : dossiers `media-dev/`, `packages-dev/` (même compartiment que la production).
    let dev = env("STORE_ENV").as_deref() == Some("dev");
    let storage = match env("STORAGE").unwrap_or_else(|| "local:./data/files".into()).as_str() {
        "r2" => {
            let need = |k: &str| env(k).unwrap_or_else(|| fail(format!("{k} manquant (STORAGE=r2)")));
            let s = Storage::r2(
                &need("R2_ACCOUNT_ID"),
                &need("R2_ACCESS_KEY_ID"),
                &need("R2_SECRET_ACCESS_KEY"),
                &need("R2_BUCKET"),
                &need("R2_MEDIA_URL"),
                dev,
            )
            .unwrap_or_else(|e| fail(e));
            tracing::info!("stockage : R2{}", if dev { " (dossiers -dev)" } else { "" });
            s
        }
        other => match other.strip_prefix("local:") {
            Some(dir) => Storage::local(dir.into(), dev),
            None => fail("STORAGE : « r2 » ou « local:<dossier> »"),
        },
    };
    let bind = env("BIND").unwrap_or_else(|| "127.0.0.1:8080".into());
    let public_url = env("PUBLIC_URL").unwrap_or_else(|| format!("http://{bind}"));
    let session_key = match env("SESSION_KEY") {
        Some(k) if k.len() >= 32 => k.into_bytes(),
        Some(_) => fail("SESSION_KEY trop courte (32 caractères au moins)"),
        None => {
            // Développement : clé éphémère (les sessions ne survivent pas à un redémarrage).
            tracing::warn!("SESSION_KEY absent : clé éphémère, sessions perdues au redémarrage");
            let mut b = [0u8; 32];
            getrandom::getrandom(&mut b).expect("getrandom");
            b.to_vec()
        }
    };
    let github = env("GITHUB_CLIENT_ID").map(|id| island_store::github::GitHub::from_env(id, env("GITHUB_CLIENT_SECRET")));
    if github.is_none() {
        tracing::warn!("GITHUB_CLIENT_ID absent : connexion GitHub indisponible (jetons de développement seulement)");
    }
    let admin_github_id = env("ADMIN_GITHUB_ID").map(|v| v.parse::<i64>().unwrap_or_else(|_| fail("ADMIN_GITHUB_ID : identifiant numérique attendu")));
    if admin_github_id.is_none() {
        tracing::warn!("ADMIN_GITHUB_ID absent : aucun super admin");
    }
    let state = AppState {
        db,
        storage: Arc::new(storage),
        clamav: env("CLAMAV_URL"),
        trust_proxy: env("TRUST_PROXY").as_deref() == Some("1"),
        github,
        admin_github_id,
        session_key: Arc::new(session_key),
        secure_cookies: public_url.starts_with("https://"),
        public_url,
        admin_dist: Some(std::path::PathBuf::from(env("ADMIN_DIST").unwrap_or_else(|| "admin/dist".into()))),
    };
    if state.clamav.is_none() {
        tracing::warn!("CLAMAV_URL absent : les fichiers publiés ne sont pas analysés (scan: skipped)");
    }
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
