//! Connexion par GitHub.
//!
//! - **Device flow** (CLI, app) : `POST /v1/auth/device` → code à saisir sur github.com ;
//!   `POST /v1/auth/token` (appelé à l'intervalle indiqué) → jeton d'API d'Island une fois
//!   validé. Le jeton GitHub ne sert qu'à lire l'identité, puis il est jeté.
//! - **OAuth web** (panneau d'administration) : `/admin/login` → GitHub → `/admin/callback`
//!   (retour vérifié par un `state` signé) → cookie de session signé, réservé aux
//!   modérateurs et à l'admin.
//!
//! Rôles : le compte GitHub `ADMIN_GITHUB_ID` est **admin** à chaque connexion (rôle qui ne
//! peut pas lui être retiré) ; un compte banni ne peut pas se connecter.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::{create_token, User};
use crate::error::{ApiError, ApiResult};
use crate::github::{GitHub, Identity, Poll};
use crate::session;
use crate::AppState;

fn github(st: &AppState) -> ApiResult<&GitHub> {
    st.github.as_ref().ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "githubNotConfigured"))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub id: i64,
    pub login: String,
    pub avatar_url: Option<String>,
    pub role: String,
}

/// Crée ou met à jour le compte d'une identité GitHub. Le nom GitHub peut avoir changé (ou
/// être repris par quelqu'un d'autre) : l'ancien titulaire du nom est renommé `nom~id`.
pub async fn upsert(st: &AppState, who: &Identity) -> ApiResult<Me> {
    let admin = st.admin_github_id == Some(who.id);
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE users SET login = login || '~' || id WHERE login = $1 AND github_id IS DISTINCT FROM $2")
        .bind(&who.login)
        .bind(who.id)
        .execute(&mut *tx)
        .await?;
    let (id, login, role, banned): (i64, String, String, bool) = sqlx::query_as(
        "INSERT INTO users (github_id, login, avatar_url, role, last_seen_at)
         VALUES ($1, $2, $3, CASE WHEN $4 THEN 'admin' ELSE 'user' END, now())
         ON CONFLICT (github_id) DO UPDATE SET login = $2, avatar_url = $3, last_seen_at = now(),
             role = CASE WHEN $4 THEN 'admin' ELSE users.role END
         RETURNING id, login, role, banned",
    )
    .bind(who.id)
    .bind(&who.login)
    .bind(&who.avatar_url)
    .bind(admin)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    if banned {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "banned"));
    }
    Ok(Me { id, login, avatar_url: who.avatar_url.clone(), role })
}

// --- Device flow ----------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

pub async fn device(State(st): State<AppState>) -> ApiResult<Json<DeviceStart>> {
    let d = github(&st)?.device_code().await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("github:{e}")))?;
    Ok(Json(DeviceStart { device_code: d.device_code, user_code: d.user_code, verification_uri: d.verification_uri, expires_in: d.expires_in, interval: d.interval }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBody {
    device_code: String,
    /// Nom du jeton (« Island sur PC-salon », « CLI »…).
    #[serde(default)]
    name: Option<String>,
}

#[derive(Serialize)]
pub struct Issued {
    token: String,
    user: Me,
}

/// 202 `{ "status": "pending" | "slowDown" }` tant que l'utilisateur n'a pas validé ;
/// 200 `{ token, user }` ensuite ; 400 `expired` / `denied`.
pub async fn token(State(st): State<AppState>, Json(b): Json<TokenBody>) -> ApiResult<Response> {
    let gh = github(&st)?;
    let access = match gh.poll(&b.device_code).await {
        Ok(Poll::Token(t)) => t,
        Ok(Poll::Pending) => return Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "status": "pending" }))).into_response()),
        Ok(Poll::SlowDown) => return Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "status": "slowDown" }))).into_response()),
        Err(e) if e == "expired" || e == "denied" => return Err(ApiError::bad(e)),
        Err(e) => return Err(ApiError::new(StatusCode::BAD_GATEWAY, e)),
    };
    let who = gh.identity(&access).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("github:{e}")))?;
    let me = upsert(&st, &who).await?;
    let name: String = b.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Island").chars().take(60).collect();
    let token = create_token(&st.db, me.id, &name).await?;
    tracing::info!("connexion GitHub : {} ({})", me.login, me.role);
    Ok(Json(Issued { token, user: me }).into_response())
}

// --- Compte courant, jetons -------------------------------------------------------------------

pub async fn me(State(st): State<AppState>, user: User) -> ApiResult<Json<Me>> {
    let avatar: Option<String> = sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1").bind(user.id).fetch_one(&st.db).await?;
    Ok(Json(Me { id: user.id, login: user.login, avatar_url: avatar, role: user.role }))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    id: i64,
    name: String,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

pub async fn tokens(State(st): State<AppState>, user: User) -> ApiResult<Json<Vec<TokenInfo>>> {
    let rows = sqlx::query_as::<_, TokenInfo>("SELECT id, name, created_at, last_used_at FROM tokens WHERE user_id = $1 ORDER BY id DESC")
        .bind(user.id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(rows))
}

/// Révoque un de SES jetons.
pub async fn revoke(State(st): State<AppState>, user: User, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let n = sqlx::query("DELETE FROM tokens WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&st.db).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

// --- OAuth web (panneau d'administration) ------------------------------------------------------

fn redirect_uri(st: &AppState) -> String {
    format!("{}/admin/callback", st.public_url.trim_end_matches('/'))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
         <title>{t} — Magasin d'Island</title><style>body{{font:15px system-ui;background:#111;color:#eee;display:grid;place-items:center;min-height:100vh;margin:0}}\
         main{{max-width:420px;padding:24px;text-align:center}}a,button{{color:#fff;background:#2b6cff;border:0;border-radius:8px;padding:8px 14px;text-decoration:none;font:inherit;cursor:pointer}}</style></head>\
         <body><main><h1>{t}</h1>{body}</main></body></html>",
        t = esc(title)
    )
}

/// Début de connexion : `state` aléatoire, signé, gardé 10 min dans un cookie `Lax` (il doit
/// revenir avec la redirection de GitHub, qui est une navigation venue d'un autre site).
pub async fn admin_login(State(st): State<AppState>) -> ApiResult<Response> {
    let gh = github(&st)?;
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("getrandom");
    let state: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let cookie = session::set_cookie(session::STATE_COOKIE, &session::sign(&st.session_key, &state, session::now() + 600), 600, "Lax", st.secure_cookies);
    Ok(([(header::SET_COOKIE, cookie)], Redirect::to(&gh.authorize_url(&redirect_uri(&st), &state))).into_response())
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
}

pub async fn admin_callback(State(st): State<AppState>, headers: HeaderMap, Query(q): Query<Callback>) -> ApiResult<Response> {
    let gh = github(&st)?;
    let expected = session::cookie(&headers, session::STATE_COOKIE).and_then(|c| session::verify(&st.session_key, c));
    let (Some(code), Some(state), Some(expected)) = (q.code, q.state, expected) else {
        return Err(ApiError::bad("invalidState"));
    };
    if state != expected {
        return Err(ApiError::bad("invalidState"));
    }
    let access = gh.exchange(&code, &redirect_uri(&st)).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, e))?;
    let who = gh.identity(&access).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("github:{e}")))?;
    let clear_state = session::set_cookie(session::STATE_COOKIE, "", 0, "Lax", st.secure_cookies);
    let me = match upsert(&st, &who).await {
        Ok(me) => me,
        Err(e) if e.code == "banned" => {
            return Ok((StatusCode::FORBIDDEN, [(header::SET_COOKIE, clear_state)], Html(page("Compte banni", "<p>Ce compte ne peut plus se connecter au magasin.</p>"))).into_response());
        }
        Err(e) => return Err(e),
    };
    if me.role != "moderator" && me.role != "admin" {
        let body = format!("<p>Connecté en tant que <b>{}</b>, mais le panneau est réservé aux modérateurs.</p>", esc(&me.login));
        return Ok((StatusCode::FORBIDDEN, [(header::SET_COOKIE, clear_state)], Html(page("Accès refusé", &body))).into_response());
    }
    let value = session::sign(&st.session_key, &me.id.to_string(), session::now() + session::TTL_SECS);
    let cookie = session::set_cookie(session::COOKIE, &value, session::TTL_SECS, "Strict", st.secure_cookies);
    tracing::info!("panneau d'administration : connexion de {} ({})", me.login, me.role);
    // Le cookie `Strict` n'accompagne pas une redirection venue de GitHub : on repart vers
    // /admin par une NOUVELLE navigation, depuis notre propre page.
    let html = page("Connexion…", "<p>Connexion réussie.</p><meta http-equiv=\"refresh\" content=\"0;url=/admin\"><p><a href=\"/admin\">Continuer</a></p>");
    let mut res = Html(html).into_response();
    res.headers_mut().append(header::SET_COOKIE, cookie.parse().expect("cookie"));
    res.headers_mut().append(header::SET_COOKIE, clear_state.parse().expect("cookie"));
    Ok(res)
}

pub async fn admin_logout(State(st): State<AppState>) -> Response {
    ([(header::SET_COOKIE, session::set_cookie(session::COOKIE, "", 0, "Strict", st.secure_cookies))], StatusCode::NO_CONTENT).into_response()
}

/// Panneau d'administration : page d'attente jusqu'à l'étape 4.
pub async fn admin_home(user: Option<User>) -> Html<String> {
    match user.filter(User::is_moderator) {
        Some(u) => Html(page(
            "Magasin d'Island",
            &format!("<p>Connecté en tant que <b>{}</b> ({}).</p><p>Le panneau d'administration arrive à l'étape 4.</p>", esc(&u.login), esc(&u.role)),
        )),
        None => Html(page("Magasin d'Island", "<p>Panneau réservé aux modérateurs.</p><p><a href=\"/admin/login\">Se connecter avec GitHub</a></p>")),
    }
}
