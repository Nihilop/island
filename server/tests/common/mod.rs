//! Outils partagés des tests : faux GitHub local (mêmes routes que github.com et
//! api.github.com) et routeur du magasin branché dessus.
//!
//! Faux GitHub : `device_code` « pending » → en attente ; « denied » → refusé ; « ok-<id> »
//! → jeton `gh-<id>` ; code web « code-<id> » → jeton `gh-<id>` ; `/user` avec `gh-<id>` →
//! compte GitHub `<id>`, nom `user<id>` ; `/users/user<id>` → ce compte, autre nom → 404.
#![allow(dead_code)]

use std::sync::Arc;

use axum::extract::{Form, Path};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use island_store::github::GitHub;
use island_store::{app, AppState, Storage};
use serde_json::{json, Value};
use sqlx::PgPool;

pub async fn fake_github() -> String {
    async fn device_code() -> Json<Value> {
        Json(json!({ "device_code": "pending", "user_code": "ABCD-1234", "verification_uri": "https://github.com/login/device", "expires_in": 900, "interval": 5 }))
    }
    async fn access_token(Form(f): Form<std::collections::HashMap<String, String>>) -> Json<Value> {
        let token = |id: &str| json!({ "access_token": format!("gh-{id}") });
        if let Some(code) = f.get("code").and_then(|c| c.strip_prefix("code-")) {
            return Json(if f.contains_key("client_secret") { token(code) } else { json!({ "error": "incorrect_client_credentials" }) });
        }
        match f.get("device_code").map(String::as_str) {
            Some("pending") => Json(json!({ "error": "authorization_pending" })),
            Some("denied") => Json(json!({ "error": "access_denied" })),
            Some(d) if d.starts_with("ok-") => Json(token(&d[3..])),
            _ => Json(json!({ "error": "bad_verification_code" })),
        }
    }
    async fn user(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
        let id: i64 = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer gh-"))
            .and_then(|v| v.parse().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(Json(json!({ "id": id, "login": format!("user{id}"), "avatar_url": "https://avatars.example/x.png" })))
    }
    /// Recherche par nom : « user<id> » → compte <id> ; « nobody » → inconnu.
    async fn by_login(Path(login): Path<String>) -> Result<Json<Value>, StatusCode> {
        let id: i64 = login.strip_prefix("user").and_then(|v| v.parse().ok()).ok_or(StatusCode::NOT_FOUND)?;
        Ok(Json(json!({ "id": id, "login": login, "avatar_url": null })))
    }
    let r = Router::new()
        .route("/login/device/code", post(device_code))
        .route("/login/oauth/access_token", post(access_token))
        .route("/user", get(user))
        .route("/users/{login}", get(by_login));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, r).await.unwrap() });
    format!("http://{addr}")
}

pub async fn router(db: PgPool, admin: Option<i64>) -> Router {
    let gh = fake_github().await;
    app(AppState {
        db,
        storage: Arc::new(Storage::local(std::env::temp_dir().join("island-store-login-test"), false)),
        clamav: None,
        trust_proxy: false,
        github: Some(GitHub::new("client-id".into(), Some("secret".into()), gh.clone(), gh)),
        admin_github_id: admin,
        session_key: Arc::new(b"cle-de-test-cle-de-test-cle-de-test".to_vec()),
        public_url: "http://localhost".into(),
        secure_cookies: false,
        admin_dist: None,
        signing_key: Some(Arc::new(ed25519_dalek::SigningKey::from_bytes(&[42; 32]))),
    })
}

