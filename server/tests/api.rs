//! Tests de l'API du magasin, contre un vrai Postgres : chaque test reçoit une base neuve
//! (`#[sqlx::test]`, migrations appliquées). Lancer :
//!
//!   docker compose up -d
//!   DATABASE_URL=postgres://island:island@localhost:5433/island_store cargo test

use std::io::Cursor;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use island_formats::islandscene;
use island_formats::scene::{Layer, Scene};
use island_store::{app, AppState, Storage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

// --- Outils -------------------------------------------------------------------------------

fn router(db: PgPool) -> Router {
    let dir = std::env::temp_dir().join(format!("island-store-test-{}", uuid_like()));
    app(AppState {
        db,
        storage: Arc::new(Storage::Local(dir)),
        clamav: None,
        trust_proxy: true,
        github: None,
        admin_github_id: None,
        session_key: Arc::new(b"cle-de-test-cle-de-test-cle-de-test".to_vec()),
        public_url: "http://localhost".into(),
        secure_cookies: false,
        admin_dist: None,
    })
}

fn uuid_like() -> String {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

async fn user(db: &PgPool, login: &str) -> (i64, String) {
    let id: i64 = sqlx::query_scalar("INSERT INTO users (login) VALUES ($1) RETURNING id").bind(login).fetch_one(db).await.unwrap();
    (id, island_store::auth::create_token(db, id, "test").await.unwrap())
}

/// PNG 4×4 avec un bloc texte « GPS » (doit disparaître à la publication).
fn png_with_gps() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(4, 4, image::Rgb([200, 100, 50]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    let clean = out.into_inner();
    let data = b"GPS\0lat=48.8566";
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&[0, 0, 0, 0]);
    let mut dirty = clean[..33].to_vec();
    dirty.extend_from_slice(&chunk);
    dirty.extend_from_slice(&clean[33..]);
    dirty
}

fn scene_file(name: &str) -> Vec<u8> {
    let s = Scene {
        name: name.into(),
        layers: vec![Layer { id: "bg".into(), kind: "image".into(), image: Some("bg.png".into()), ..Default::default() }],
        ..Default::default()
    };
    let mut out = Cursor::new(Vec::new());
    islandscene::write(&mut out, &s, &[("bg.png".into(), png_with_gps())], b"thumb", "Island test").unwrap();
    out.into_inner()
}

fn multipart(fields: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "----island-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        if *name == "file" {
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"x.islandscene\"\r\nContent-Type: application/octet-stream\r\n\r\n");
        } else {
            body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes());
        }
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn send(r: &Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = r.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

async fn get_json(r: &Router, uri: &str) -> (StatusCode, Value) {
    let (s, b) = send(r, Request::get(uri).body(Body::empty()).unwrap()).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn publish(r: &Router, token: Option<&str>, fields: &[(&str, &[u8])]) -> (StatusCode, Value) {
    let (ct, body) = multipart(fields);
    let mut req = Request::post("/v1/items").header("content-type", ct);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let (s, b) = send(r, req.body(Body::from(body)).unwrap()).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn json_req(r: &Router, method: &str, uri: &str, token: Option<&str>, body: Value) -> StatusCode {
    let mut req = Request::builder().method(method).uri(uri).header("content-type", "application/json").header("x-forwarded-for", "203.0.113.7");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    send(r, req.body(Body::from(body.to_string())).unwrap()).await.0
}

// --- Tests ----------------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn publishing_requires_a_valid_token(db: PgPool) {
    let r = router(db);
    let file = scene_file("Aurore");
    assert_eq!(publish(&r, None, &[("file", &file)]).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(publish(&r, Some("ist_faux"), &[("file", &file)]).await.0, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_published_scene_is_listed_downloadable_and_cleaned(db: PgPool) {
    let r = router(db.clone());
    let (_, token) = user(&db, "alice").await;
    let file = scene_file("Lucioles d'été");
    let (s, body) = publish(&r, Some(&token), &[("file", &file), ("summary", "Des lucioles au bord d'un lac".as_bytes()), ("tags", br#"["Nuit","nature"]"#)]).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["version"], "1");
    assert_eq!(body["scan"], "skipped");

    // Catalogue et fiche.
    let (_, page) = get_json(&r, "/v1/items?kind=scene").await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["author"], "alice");
    assert_eq!(page["items"][0]["tags"], json!(["nuit", "nature"]));
    let (s, detail) = get_json(&r, &format!("/v1/items/{id}")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(detail["versions"].as_array().unwrap().len(), 1);

    // Téléchargement : un .islandscene valide, image sans le bloc « GPS », vignette du serveur.
    let (s, bytes) = send(&r, Request::get(format!("/v1/items/{id}/download")).body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    let got = islandscene::read(Cursor::new(&bytes), bytes.len() as u64).unwrap();
    let (_, img) = &got.files[0];
    assert!(!img.windows(11).any(|w| w == b"lat=48.8566"));
    assert!(image::load_from_memory(img).is_ok());
    assert_ne!(got.thumbnail.as_deref(), Some(&b"thumb"[..]));
    let (_, page) = get_json(&r, "/v1/items").await;
    assert_eq!(page["items"][0]["downloads"], 1);

    let (s, thumb) = send(&r, Request::get(format!("/v1/items/{id}/thumbnail")).body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    assert!(image::load_from_memory(&thumb).is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn search_is_french_and_ignores_accents(db: PgPool) {
    let r = router(db.clone());
    let (_, token) = user(&db, "alice").await;
    publish(&r, Some(&token), &[("file", &scene_file("Lucioles d'été")), ("summary", b"Au bord d'un lac")]).await;
    publish(&r, Some(&token), &[("file", &scene_file("Nuit de neige"))]).await;
    let (_, page) = get_json(&r, "/v1/items?q=luciole%20ete").await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["name"], "Lucioles d'été");
    let (_, page) = get_json(&r, "/v1/items?q=lacs").await;
    assert_eq!(page["total"], 1);
    let (_, page) = get_json(&r, "/v1/items?q=pluie").await;
    assert_eq!(page["total"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_the_author_publishes_new_versions(db: PgPool) {
    let r = router(db.clone());
    let (_, alice) = user(&db, "alice").await;
    let (_, bob) = user(&db, "bob").await;
    let (_, body) = publish(&r, Some(&alice), &[("file", &scene_file("Aurore"))]).await;
    let id = body["id"].as_str().unwrap().to_string();
    let (s, body) = publish(&r, Some(&alice), &[("file", &scene_file("Aurore boréale")), ("item", id.as_bytes())]).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(body["version"], "2");
    let (s, _) = publish(&r, Some(&bob), &[("file", &scene_file("Volée")), ("item", id.as_bytes())]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, detail) = get_json(&r, &format!("/v1/items/{id}")).await;
    assert_eq!(detail["name"], "Aurore boréale");
    assert_eq!(detail["version"], "2");
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_files_are_refused_with_a_stable_code(db: PgPool) {
    let r = router(db.clone());
    let (_, token) = user(&db, "alice").await;
    let (s, body) = publish(&r, Some(&token), &[("file", b"pas un zip")]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "corrupt");
    let (s, body) = publish(&r, Some(&token), &[("summary", b"sans fichier")]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "fileRequired");
    let (s, body) = publish(&r, Some(&token), &[("file", &scene_file("x")), ("tags", br#"["avec espace"]"#)]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalidTags");
}

#[sqlx::test(migrations = "./migrations")]
async fn removed_items_and_banned_authors_disappear(db: PgPool) {
    let r = router(db.clone());
    let (alice_id, alice) = user(&db, "alice").await;
    let (_, bob) = user(&db, "bob").await;
    let (_, a) = publish(&r, Some(&alice), &[("file", &scene_file("A"))]).await;
    let (_, b) = publish(&r, Some(&bob), &[("file", &scene_file("B"))]).await;
    let (a, b) = (a["id"].as_str().unwrap().to_string(), b["id"].as_str().unwrap().to_string());

    // Bob ne peut pas retirer la scène d'Alice ; Bob retire la sienne.
    assert_eq!(json_req(&r, "DELETE", &format!("/v1/items/{a}"), Some(&bob), Value::Null).await, StatusCode::FORBIDDEN);
    assert_eq!(json_req(&r, "DELETE", &format!("/v1/items/{b}"), Some(&bob), Value::Null).await, StatusCode::NO_CONTENT);
    assert_eq!(get_json(&r, &format!("/v1/items/{b}")).await.0, StatusCode::NOT_FOUND);

    // Alice bannie : ses scènes disparaissent, son jeton ne marche plus.
    sqlx::query("UPDATE users SET banned = TRUE WHERE id = $1").bind(alice_id).execute(&db).await.unwrap();
    assert_eq!(get_json(&r, "/v1/items").await.1["total"], 0);
    assert_eq!(publish(&r, Some(&alice), &[("file", &scene_file("C"))]).await.0, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn reports_are_validated_and_rate_limited(db: PgPool) {
    let r = router(db.clone());
    let (_, token) = user(&db, "alice").await;
    let (_, body) = publish(&r, Some(&token), &[("file", &scene_file("A"))]).await;
    let uri = format!("/v1/items/{}/reports", body["id"].as_str().unwrap());
    assert_eq!(json_req(&r, "POST", &uri, None, json!({ "reason": "spam" })).await, StatusCode::BAD_REQUEST);
    for _ in 0..5 {
        assert_eq!(json_req(&r, "POST", &uri, None, json!({ "reason": "broken", "details": "écran noir" })).await, StatusCode::CREATED);
    }
    assert_eq!(json_req(&r, "POST", &uri, None, json!({ "reason": "broken" })).await, StatusCode::TOO_MANY_REQUESTS);
    // L'IP n'est jamais stockée, seulement son empreinte.
    let ips: Vec<String> = sqlx::query_scalar("SELECT ip_hash FROM reports").fetch_all(&db).await.unwrap();
    assert!(ips.iter().all(|h| h.len() == 64 && !h.contains("203.0.113.7")));
}
