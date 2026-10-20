//! Extensions au magasin : dépôt en attente, relecture, approbation signée par le magasin,
//! refus motivé, id réservé à son auteur, version qui doit augmenter. Vrai Postgres.

use std::io::Cursor;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use island_formats::package::{self, Signature};
use island_store::{app, AppState, Storage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

const STORE_KEY: [u8; 32] = [42; 32];

fn router(db: PgPool) -> Router {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).unwrap();
    let dir = std::env::temp_dir().join(format!("island-store-ext-{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>()));
    app(AppState {
        db,
        storage: Arc::new(Storage::local(dir, false)),
        clamav: None,
        trust_proxy: false,
        github: None,
        admin_github_id: None,
        session_key: Arc::new(b"cle-de-test-cle-de-test-cle-de-test".to_vec()),
        public_url: "http://localhost".into(),
        secure_cookies: false,
        admin_dist: None,
        signing_key: Some(Arc::new(ed25519_dalek::SigningKey::from_bytes(&STORE_KEY))),
    })
}

async fn account(db: &PgPool, login: &str, role: &str) -> String {
    let id: i64 = sqlx::query_scalar("INSERT INTO users (login, role) VALUES ($1, $2) RETURNING id").bind(login).bind(role).fetch_one(db).await.unwrap();
    island_store::auth::create_token(db, id, "test").await.unwrap()
}

/// Paquet `.island` minimal ; `extra` = fichiers en plus.
fn extension(id: &str, version: &str, permissions: &[&str], extra: &[(&str, &[u8])]) -> Vec<u8> {
    let manifest = json!({ "id": id, "name": "Météo", "version": version, "main": "dist/main.js", "permissions": permissions, "description": "La météo dans l'île" });
    let mut files = vec![("manifest.json".to_string(), manifest.to_string().into_bytes()), ("dist/main.js".to_string(), format!("// v{version}").into_bytes())];
    files.extend(extra.iter().map(|(n, b)| (n.to_string(), b.to_vec())));
    let mut out = Cursor::new(Vec::new());
    package::write(&mut out, &files).unwrap();
    out.into_inner()
}

async fn send(r: &Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    (s, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

async fn call(r: &Router, method: &str, uri: &str, token: &str, body: Option<Value>) -> (StatusCode, Value) {
    let b = Request::builder().method(method).uri(uri).header("authorization", format!("Bearer {token}"));
    let req = match body {
        Some(v) => b.header("content-type", "application/json").body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let (s, bytes) = send(r, req).await;
    (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn publish(r: &Router, token: &str, file: &[u8]) -> (StatusCode, Value) {
    publish_with(r, token, file, &[]).await
}

async fn publish_with(r: &Router, token: &str, file: &[u8], fields: &[(&str, &str)]) -> (StatusCode, Value) {
    let boundary = "x-island";
    let mut body = Vec::new();
    for (k, v) in fields {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"e.island\"\r\n\r\n").as_bytes());
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = Request::post("/v1/items")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    let (s, b) = send(r, req).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn pending_id(r: &Router, moderator: &str) -> i64 {
    let (s, q) = call(r, "GET", "/v1/mod/reviews", moderator, None).await;
    assert_eq!(s, StatusCode::OK);
    q.as_array().unwrap().last().expect("une version en attente")["id"].as_i64().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn an_extension_waits_for_review_then_ships_signed_by_the_store(db: PgPool) {
    let r = router(db.clone());
    let author = account(&db, "auteur", "user").await;
    let moderator = account(&db, "modo", "moderator").await;

    let fields = [
        ("summary", "Météo locale"),
        ("tags", r#"["meteo","ciel"]"#),
        ("homepage", "https://example.com/meteo"),
        ("repository", "javascript:alert(1)"),
        ("license", "MIT"),
    ];
    let (s, p) = publish_with(&r, &author, &extension("dev.example.meteo", "1.0.0", &["network:api.example.com"], &[]), &fields).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{p}");
    assert_eq!((p["id"].as_str(), p["status"].as_str()), (Some("dev.example.meteo"), Some("pending")));
    // Rien de public avant l'approbation.
    assert_eq!(call(&r, "GET", "/v1/items/dev.example.meteo", &author, None).await.0, StatusCode::NOT_FOUND);
    let (_, list) = call(&r, "GET", "/v1/items?kind=extension", &author, None).await;
    assert_eq!(list["total"], 0);
    let (_, mine) = call(&r, "GET", "/v1/me/items", &author, None).await;
    assert_eq!((mine[0]["lastVersion"].as_str(), mine[0]["lastStatus"].as_str()), (Some("1.0.0"), Some("pending")));
    // Un simple compte ne relit pas.
    assert_eq!(call(&r, "GET", "/v1/mod/reviews", &author, None).await.0, StatusCode::FORBIDDEN);

    // Relecture : fichiers, permissions, lecture d'un fichier en texte.
    let vid = pending_id(&r, &moderator).await;
    let (s, d) = call(&r, "GET", &format!("/v1/mod/reviews/{vid}"), &moderator, None).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!(d["addedPermissions"], json!(["network:api.example.com"]));
    assert!(d["files"].as_array().unwrap().iter().any(|f| f["path"] == "dist/main.js" && f["change"] == "added"));
    let req = Request::get(format!("/v1/mod/reviews/{vid}/file?path=dist/main.js")).header("authorization", format!("Bearer {moderator}")).body(Body::empty()).unwrap();
    let res = r.clone().oneshot(req).await.unwrap();
    assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
    assert_eq!(res.headers()["content-security-policy"], "default-src 'none'; sandbox");
    assert_eq!(res.into_body().collect().await.unwrap().to_bytes().as_ref(), b"// v1.0.0");

    // Approbation : en ligne, et le paquet distribué est signé par le magasin.
    assert_eq!(call(&r, "POST", &format!("/v1/mod/reviews/{vid}/approve"), &moderator, None).await.0, StatusCode::NO_CONTENT);
    let (s, item) = call(&r, "GET", "/v1/items/dev.example.meteo", &author, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((item["kind"].as_str(), item["version"].as_str(), item["summary"].as_str()), (Some("extension"), Some("1.0.0"), Some("Météo locale")));
    assert_eq!((item["homepage"].as_str(), item["license"].as_str(), item["tags"].clone()), (Some("https://example.com/meteo"), Some("MIT"), json!(["meteo", "ciel"])));
    // Lien non https : ignoré.
    assert!(item["repository"].is_null());
    let (s, bytes) = send(&r, Request::get("/v1/items/dev.example.meteo/download").body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    use base64::Engine as _;
    let store_pub = base64::engine::general_purpose::STANDARD.encode(ed25519_dalek::SigningKey::from_bytes(&STORE_KEY).verifying_key().to_bytes());
    let pkg = package::read_with(Cursor::new(bytes), &[], &[&store_pub]).unwrap();
    assert!(pkg.signature == Signature::Store);
    let (_, status) = call(&r, "GET", "/v1/store/status", &author, None).await;
    assert_eq!(status["signingKey"], store_pub);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = 'version.approve'").fetch_one(&db).await.unwrap();
    assert_eq!(n, 1);

    // Mises à jour : seulement ce qui est connu ET plus récent que la version installée.
    let check = |items: Value| call(&r, "POST", "/v1/updates", &author, Some(json!({ "items": items })));
    let (s, u) = check(json!([{ "id": "dev.example.meteo", "version": "0.9.0" }, { "id": "dev.inconnue.x", "version": "1.0.0" }])).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(u.as_array().unwrap().len(), 1);
    assert_eq!((u[0]["version"].as_str(), u[0]["kind"].as_str()), (Some("1.0.0"), Some("extension")));
    assert_eq!(u[0]["permissions"], json!(["network:api.example.com"]));
    let (_, u) = check(json!([{ "id": "dev.example.meteo", "version": "1.0.0" }])).await;
    assert_eq!(u, json!([]));
}

#[sqlx::test(migrations = "./migrations")]
async fn updates_must_grow_and_ids_belong_to_their_author(db: PgPool) {
    let r = router(db.clone());
    let author = account(&db, "auteur", "user").await;
    let other = account(&db, "autre", "user").await;
    let moderator = account(&db, "modo", "moderator").await;
    assert_eq!(publish(&r, &author, &extension("dev.example.meteo", "1.0.0", &[], &[])).await.0, StatusCode::ACCEPTED);
    let vid = pending_id(&r, &moderator).await;
    call(&r, "POST", &format!("/v1/mod/reviews/{vid}/approve"), &moderator, None).await;

    // Même id par quelqu'un d'autre : réservé.
    let (s, e) = publish(&r, &other, &extension("dev.example.meteo", "9.0.0", &[], &[])).await;
    assert_eq!((s, e["error"].as_str()), (StatusCode::CONFLICT, Some("idTaken")));
    // La version doit augmenter.
    let (s, e) = publish(&r, &author, &extension("dev.example.meteo", "1.0.0", &[], &[])).await;
    assert_eq!((s, e["error"].as_str()), (StatusCode::BAD_REQUEST, Some("versionNotGreater")));
    // Préfixe officiel réservé à l'admin.
    let (s, e) = publish(&r, &other, &extension("dev.island.fake", "1.0.0", &[], &[])).await;
    assert_eq!((s, e["error"].as_str()), (StatusCode::FORBIDDEN, Some("reservedId")));
    // Exécutable refusé.
    let (s, e) = publish(&r, &author, &extension("dev.example.meteo", "1.1.0", &[], &[("bin/run.exe", b"MZ")])).await;
    assert_eq!((s, e["error"].as_str()), (StatusCode::BAD_REQUEST, Some("fileType:bin/run.exe")));

    // Mise à jour : la relecture montre la nouvelle permission et le fichier modifié.
    assert_eq!(publish(&r, &author, &extension("dev.example.meteo", "1.1.0", &["notifications"], &[])).await.0, StatusCode::ACCEPTED);
    // Une seconde mise à jour remplace celle qui attendait.
    assert_eq!(publish(&r, &author, &extension("dev.example.meteo", "1.2.0", &["notifications"], &[])).await.0, StatusCode::ACCEPTED);
    let (_, q) = call(&r, "GET", "/v1/mod/reviews", &moderator, None).await;
    assert_eq!(q.as_array().unwrap().len(), 1);
    assert_eq!((q[0]["version"].as_str(), q[0]["previousVersion"].as_str()), (Some("1.2.0"), Some("1.0.0")));
    let vid = q[0]["id"].as_i64().unwrap();
    let (_, d) = call(&r, "GET", &format!("/v1/mod/reviews/{vid}"), &moderator, None).await;
    assert_eq!(d["addedPermissions"], json!(["notifications"]));
    assert!(d["files"].as_array().unwrap().iter().any(|f| f["path"] == "dist/main.js" && f["change"] == "modified"));
    // La fiche publique reste celle de la version approuvée.
    let (_, item) = call(&r, "GET", "/v1/items/dev.example.meteo", &author, None).await;
    assert_eq!(item["version"], "1.0.0");

    // Refus motivé : l'auteur voit le motif.
    assert_eq!(call(&r, "POST", &format!("/v1/mod/reviews/{vid}/reject"), &moderator, Some(json!({ "reason": "  " }))).await.0, StatusCode::BAD_REQUEST);
    let why = "La permission notifications n'est pas expliquée dans la description.";
    assert_eq!(call(&r, "POST", &format!("/v1/mod/reviews/{vid}/reject"), &moderator, Some(json!({ "reason": why }))).await.0, StatusCode::NO_CONTENT);
    let (_, mine) = call(&r, "GET", "/v1/me/items", &author, None).await;
    assert_eq!((mine[0]["lastStatus"].as_str(), mine[0]["rejectReason"].as_str()), (Some("rejected"), Some(why)));
}
