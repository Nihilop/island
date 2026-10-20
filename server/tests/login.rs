//! Connexion GitHub (device flow et OAuth web), rôles, jetons, session du panneau — contre
//! un vrai Postgres et un FAUX GitHub local (mêmes routes que github.com / api.github.com).
//! Faux GitHub : cf. `common`.

mod common;

use axum::body::Body;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::Router;
use common::router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

async fn call(r: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let res = r.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

async fn post_json(r: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let (s, _, b) = call(r, Request::post(uri).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn get_with(r: &Router, uri: &str, auth: (&str, &str)) -> (StatusCode, Value) {
    let (s, _, b) = call(r, Request::get(uri).header(auth.0, auth.1).body(Body::empty()).unwrap()).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

/// Device flow complet : renvoie le jeton d'Island.
async fn device_login(r: &Router, github_id: i64) -> (StatusCode, Value) {
    post_json(r, "/v1/auth/token", json!({ "deviceCode": format!("ok-{github_id}"), "name": "test" })).await
}

#[sqlx::test(migrations = "./migrations")]
async fn device_flow_issues_an_island_token(db: PgPool) {
    let r = router(db, None).await;
    let (s, start) = post_json(&r, "/v1/auth/device", json!({})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(start["userCode"], "ABCD-1234");
    assert_eq!(start["interval"], 5);

    // L'utilisateur n'a pas encore validé, puis refuse.
    let (s, body) = post_json(&r, "/v1/auth/token", json!({ "deviceCode": "pending" })).await;
    assert_eq!((s, body["status"].as_str()), (StatusCode::ACCEPTED, Some("pending")));
    let (s, body) = post_json(&r, "/v1/auth/token", json!({ "deviceCode": "denied" })).await;
    assert_eq!((s, body["error"].as_str()), (StatusCode::BAD_REQUEST, Some("denied")));

    // Validé : jeton d'Island, compte créé (rôle user).
    let (s, body) = device_login(&r, 42).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let token = body["token"].as_str().unwrap();
    assert!(token.starts_with("ist_"));
    let (s, me) = get_with(&r, "/v1/me", ("authorization", &format!("Bearer {token}"))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["login"], "user42");
    assert_eq!(me["role"], "user");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_app_can_sign_out_its_own_token(db: PgPool) {
    let r = router(db, None).await;
    let (_, a) = device_login(&r, 42).await;
    let (_, b) = device_login(&r, 42).await;
    let (a, b) = (a["token"].as_str().unwrap().to_string(), b["token"].as_str().unwrap().to_string());
    let out = |t: &str| Request::delete("/v1/auth/token").header("authorization", format!("Bearer {t}")).body(Body::empty()).unwrap();
    assert_eq!(call(&r, out(&a)).await.0, StatusCode::NO_CONTENT);
    // Ce jeton est mort, l'autre appareil reste connecté.
    assert_eq!(get_with(&r, "/v1/me", ("authorization", &format!("Bearer {a}"))).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(get_with(&r, "/v1/me", ("authorization", &format!("Bearer {b}"))).await.0, StatusCode::OK);
    assert_eq!(call(&r, out(&a)).await.0, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_configured_github_account_is_admin_and_stays_admin(db: PgPool) {
    let r = router(db.clone(), Some(1)).await;
    // Un compte de développement a pris le même nom : il est renommé, pas écrasé.
    sqlx::query("INSERT INTO users (login) VALUES ('user1')").execute(&db).await.unwrap();
    let (_, body) = device_login(&r, 1).await;
    assert_eq!(body["user"]["role"], "admin");
    assert_eq!(body["user"]["login"], "user1");
    let renamed: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE login LIKE 'user1~%'").fetch_one(&db).await.unwrap();
    assert_eq!(renamed, 1);
    // Même rétrogradé en base, il redevient admin à la connexion suivante.
    sqlx::query("UPDATE users SET role = 'user' WHERE github_id = 1").execute(&db).await.unwrap();
    assert_eq!(device_login(&r, 1).await.1["user"]["role"], "admin");
    // Un autre compte reste user.
    assert_eq!(device_login(&r, 2).await.1["user"]["role"], "user");
}

#[sqlx::test(migrations = "./migrations")]
async fn banned_accounts_cannot_log_in_and_tokens_can_be_revoked(db: PgPool) {
    let r = router(db.clone(), None).await;
    let (_, a) = device_login(&r, 5).await;
    let (_, b) = device_login(&r, 5).await;
    let (ta, tb) = (a["token"].as_str().unwrap().to_string(), b["token"].as_str().unwrap().to_string());
    let auth = format!("Bearer {ta}");

    // Deux jetons ; on révoque le second.
    let (_, list) = get_with(&r, "/v1/me/tokens", ("authorization", &auth)).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    let second = list[0]["id"].as_i64().unwrap();
    let (s, _, _) = call(&r, Request::delete(format!("/v1/me/tokens/{second}")).header("authorization", &auth).body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(get_with(&r, "/v1/me", ("authorization", &format!("Bearer {tb}"))).await.0, StatusCode::UNAUTHORIZED);

    // Banni : plus de connexion, plus de jeton valide.
    sqlx::query("UPDATE users SET banned = TRUE WHERE github_id = 5").execute(&db).await.unwrap();
    let (s, body) = device_login(&r, 5).await;
    assert_eq!((s, body["error"].as_str()), (StatusCode::FORBIDDEN, Some("banned")));
    assert_eq!(get_with(&r, "/v1/me", ("authorization", &auth)).await.0, StatusCode::UNAUTHORIZED);
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with(&format!("{name}=")))
        .map(|v| v.split(';').next().unwrap()[name.len() + 1..].to_string())
        .filter(|v| !v.is_empty())
}

/// `/admin/login` → (state, cookie de state).
async fn start_web_login(r: &Router) -> (String, String) {
    let (s, headers, _) = call(r, Request::get("/admin/login").body(Body::empty()).unwrap()).await;
    assert!(s.is_redirection());
    let location = headers.get(header::LOCATION).unwrap().to_str().unwrap().to_string();
    assert!(location.contains("/login/oauth/authorize?client_id=client-id"));
    let state = location.split("state=").nth(1).unwrap().to_string();
    (state, cookie_value(&headers, "island_oauth_state").unwrap())
}

async fn callback(r: &Router, code: &str, state: &str, state_cookie: &str) -> (StatusCode, HeaderMap) {
    let req = Request::get(format!("/admin/callback?code={code}&state={state}")).header("cookie", format!("island_oauth_state={state_cookie}")).body(Body::empty()).unwrap();
    let (s, h, _) = call(r, req).await;
    (s, h)
}

#[sqlx::test(migrations = "./migrations")]
async fn web_login_is_for_moderators_and_protected_against_csrf(db: PgPool) {
    let r = router(db.clone(), Some(1)).await;

    // State falsifié : refusé.
    let (_, cookie) = start_web_login(&r).await;
    assert_eq!(callback(&r, "code-9", "autre-state", &cookie).await.0, StatusCode::BAD_REQUEST);

    // Simple utilisateur : connecté à GitHub, mais pas de session.
    let (state, cookie) = start_web_login(&r).await;
    let (s, h) = callback(&r, "code-9", &state, &cookie).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(cookie_value(&h, "island_admin").is_none());

    // L'admin : session.
    let (state, cookie) = start_web_login(&r).await;
    let (s, h) = callback(&r, "code-1", &state, &cookie).await;
    assert_eq!(s, StatusCode::OK);
    let session = cookie_value(&h, "island_admin").expect("cookie de session");
    let auth = ("cookie", format!("island_admin={session}"));
    let (s, me) = get_with(&r, "/v1/me", (auth.0, &auth.1)).await;
    assert_eq!((s, me["role"].as_str()), (StatusCode::OK, Some("admin")));
    let (_, _, page) = call(&r, Request::get("/admin").header(auth.0, &auth.1).body(Body::empty()).unwrap()).await;
    assert!(String::from_utf8_lossy(&page).contains("user1"));

    // Modification par cookie : il faut l'en-tête X-Island-Admin.
    let del = |with_header: bool| {
        let mut b = Request::delete("/v1/me/tokens/999").header(auth.0, &auth.1);
        if with_header {
            b = b.header("x-island-admin", "1");
        }
        b.body(Body::empty()).unwrap()
    };
    assert_eq!(call(&r, del(false)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&r, del(true)).await.0, StatusCode::NOT_FOUND);

    // Cookie falsifié : ignoré.
    let forged = ("cookie", format!("island_admin={}", session.replacen(|c: char| c.is_ascii_digit(), "7", 1)));
    assert_eq!(get_with(&r, "/v1/me", (forged.0, &forged.1)).await.0, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_session_ends_when_the_moderator_role_is_removed(db: PgPool) {
    let r = router(db.clone(), None).await;
    sqlx::query("INSERT INTO users (github_id, login, role) VALUES (3, 'user3', 'moderator')").execute(&db).await.unwrap();
    let (state, cookie) = start_web_login(&r).await;
    let (_, h) = callback(&r, "code-3", &state, &cookie).await;
    let auth = ("cookie", format!("island_admin={}", cookie_value(&h, "island_admin").unwrap()));
    assert_eq!(get_with(&r, "/v1/me", (auth.0, &auth.1)).await.0, StatusCode::OK);
    sqlx::query("UPDATE users SET role = 'user' WHERE github_id = 3").execute(&db).await.unwrap();
    assert_eq!(get_with(&r, "/v1/me", (auth.0, &auth.1)).await.0, StatusCode::UNAUTHORIZED);
}
