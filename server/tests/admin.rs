//! Modération et administration : droits par rôle, modérateurs ajoutés par leur nom GitHub,
//! bannissement, signalements, masquage, journal. Vrai Postgres, faux GitHub (cf. `common`).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use common::router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

/// Compte + jeton ; `github` = identifiant GitHub (None : compte de développement).
async fn account(db: &PgPool, login: &str, role: &str, github: Option<i64>) -> (i64, String) {
    let id: i64 = sqlx::query_scalar("INSERT INTO users (login, role, github_id) VALUES ($1, $2, $3) RETURNING id")
        .bind(login)
        .bind(role)
        .bind(github)
        .fetch_one(db)
        .await
        .unwrap();
    (id, island_store::auth::create_token(db, id, "test").await.unwrap())
}

async fn req(r: &Router, method: &str, uri: &str, token: &str, body: Option<Value>) -> (StatusCode, Value) {
    let b = Request::builder().method(method).uri(uri).header("authorization", format!("Bearer {token}"));
    let req = match body {
        Some(v) => b.header("content-type", "application/json").body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Une scène publiée (directement en base : la publication est testée dans `api.rs`).
async fn scene(db: &PgPool, id: &str, owner: i64) {
    sqlx::query("INSERT INTO items (id, kind, owner_id, name) VALUES ($1, 'scene', $2, $1)").bind(id).bind(owner).execute(db).await.unwrap();
    sqlx::query("INSERT INTO versions (item_id, version, status, file_key, size, sha256, scan) VALUES ($1, '1', 'approved', 'x', 1, 'x', 'skipped')")
        .bind(id)
        .execute(db)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn routes_are_restricted_by_role(db: PgPool) {
    let r = router(db.clone(), Some(1)).await;
    let (_, user) = account(&db, "user9", "user", Some(9)).await;
    let (_, moderator) = account(&db, "user8", "moderator", Some(8)).await;
    let (_, admin) = account(&db, "user1", "admin", Some(1)).await;
    assert_eq!(req(&r, "GET", "/v1/mod/reports", &user, None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(req(&r, "GET", "/v1/mod/reports", &moderator, None).await.0, StatusCode::OK);
    assert_eq!(req(&r, "GET", "/v1/mod/summary", &moderator, None).await.0, StatusCode::OK);
    assert_eq!(req(&r, "GET", "/v1/admin/users", &moderator, None).await.0, StatusCode::FORBIDDEN);
    let (s, users) = req(&r, "GET", "/v1/admin/users", &admin, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(users["total"], 3);
    assert_eq!(users["items"][0]["login"], "user1"); // l'admin d'abord
}

#[sqlx::test(migrations = "./migrations")]
async fn moderators_are_added_by_github_login_and_the_admin_is_untouchable(db: PgPool) {
    let r = router(db.clone(), Some(1)).await;
    let (admin_id, admin) = account(&db, "user1", "admin", Some(1)).await;

    // Ajout par nom GitHub : compte créé avec son identifiant numérique.
    let (s, body) = req(&r, "POST", "/v1/admin/moderators", &admin, Some(json!({ "login": "@user42" }))).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let id = body["id"].as_i64().unwrap();
    let (gid, role): (Option<i64>, String) = sqlx::query_as("SELECT github_id, role FROM users WHERE id = $1").bind(id).fetch_one(&db).await.unwrap();
    assert_eq!((gid, role.as_str()), (Some(42), "moderator"));
    assert_eq!(req(&r, "POST", "/v1/admin/moderators", &admin, Some(json!({ "login": "nobody" }))).await.1["error"], "unknownGithubUser");
    assert_eq!(req(&r, "POST", "/v1/admin/moderators", &admin, Some(json!({ "login": "user1" }))).await.1["error"], "isAdmin");

    // Retrait ; l'admin ne peut être ni rétrogradé ni banni, personne ne se bannit lui-même.
    assert_eq!(req(&r, "DELETE", &format!("/v1/admin/moderators/{id}"), &admin, None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "DELETE", &format!("/v1/admin/moderators/{admin_id}"), &admin, None).await.1["error"], "isAdmin");
    assert_eq!(req(&r, "POST", &format!("/v1/admin/users/{admin_id}/ban"), &admin, None).await.1["error"], "cannotBanSelf");

    // Journal.
    let (_, log) = req(&r, "GET", "/v1/admin/audit", &admin, None).await;
    let actions: Vec<&str> = log["items"].as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
    assert_eq!(actions, ["moderator.remove", "moderator.add"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn banning_revokes_tokens_and_hides_items(db: PgPool) {
    let r = router(db.clone(), Some(1)).await;
    let (_, admin) = account(&db, "user1", "admin", Some(1)).await;
    let (bob_id, bob) = account(&db, "user5", "moderator", Some(5)).await;
    scene(&db, "sc_bob", bob_id).await;
    let (_, page) = req(&r, "GET", "/v1/items", &admin, None).await;
    assert_eq!(page["total"], 1);

    assert_eq!(req(&r, "POST", &format!("/v1/admin/users/{bob_id}/ban"), &admin, None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "GET", "/v1/me", &bob, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(req(&r, "GET", "/v1/items", &admin, None).await.1["total"], 0);
    let (_, banned) = req(&r, "GET", "/v1/admin/users?role=banned", &admin, None).await;
    assert_eq!(banned["items"][0]["login"], "user5");
    assert_eq!(banned["items"][0]["role"], "user"); // plus modérateur

    assert_eq!(req(&r, "POST", &format!("/v1/admin/users/{bob_id}/unban"), &admin, None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "GET", "/v1/items", &admin, None).await.1["total"], 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn reports_are_resolved_and_items_hidden_or_restored(db: PgPool) {
    let r = router(db.clone(), None).await;
    let (owner, _) = account(&db, "auteur", "user", None).await;
    let (_, moderator) = account(&db, "modo", "moderator", None).await;
    scene(&db, "sc_a", owner).await;
    scene(&db, "sc_b", owner).await;
    for (item, reason) in [("sc_a", "broken"), ("sc_b", "offensive"), ("sc_b", "copyright")] {
        sqlx::query("INSERT INTO reports (item_id, reason) VALUES ($1, $2)").bind(item).bind(reason).execute(&db).await.unwrap();
    }
    let (_, open) = req(&r, "GET", "/v1/mod/reports", &moderator, None).await;
    assert_eq!(open["total"], 3);
    let id = |item: &str| open["items"].as_array().unwrap().iter().find(|x| x["itemId"] == item).unwrap()["id"].as_i64().unwrap();

    // Rien à faire sur A ; B masqué : ses deux signalements sont clos d'un coup.
    assert_eq!(req(&r, "POST", &format!("/v1/mod/reports/{}/resolve", id("sc_a")), &moderator, Some(json!({ "action": "dismiss" }))).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "POST", &format!("/v1/mod/reports/{}/resolve", id("sc_b")), &moderator, Some(json!({ "action": "hide" }))).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "GET", "/v1/mod/reports", &moderator, None).await.1["total"], 0);
    assert_eq!(req(&r, "GET", "/v1/mod/reports?status=resolved", &moderator, None).await.1["total"], 3);
    assert_eq!(req(&r, "GET", "/v1/items/sc_b", &moderator, None).await.0, StatusCode::NOT_FOUND);
    let (_, hidden) = req(&r, "GET", "/v1/mod/items?status=hidden", &moderator, None).await;
    assert_eq!(hidden["items"][0]["id"], "sc_b");

    // Rétabli.
    assert_eq!(req(&r, "POST", "/v1/mod/items/sc_b/restore", &moderator, None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(req(&r, "GET", "/v1/items/sc_b", &moderator, None).await.0, StatusCode::OK);
    assert_eq!(req(&r, "POST", "/v1/mod/items/sc_b/restore", &moderator, None).await.0, StatusCode::NOT_FOUND);
}
