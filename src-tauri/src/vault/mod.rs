// Coffre = pont Bitwarden. La session (`BW_SESSION`) et les éléments déchiffrés ne vivent
// QUE dans la mémoire Rust, le temps du déverrouillage ; le front ne reçoit que des
// métadonnées (nom, identifiant, URL, présence d'un mot de passe / d'un 2FA). Un secret ne
// sort que par « copier » (copie privée, effacée à 30 s) ou « révéler » explicite ; les
// graines TOTP ne sortent jamais (seuls code + secondes restantes + période).
//
// Verrouillage glissant : chaque action prolonge ; au-delà du délai (réglable) le coffre
// se verrouille (`bw lock`, session et éléments effacés) et `vault://changed` est émis.
//
// Windows Hello (ouverture rapide, option) : la clé de session est scellée par le service
// Hello (`services::hello`, portée « vault ») à chaque déverrouillage par mot de passe.
// Au verrouillage on l'oublie en mémoire mais on NE lance PAS `bw lock` : la session reste
// valide côté `bw` et Hello la rend. Le mot de passe maître n'est jamais stocké.
//
// Erreurs = codes courts traduits côté front : locked, notInstalled, twoFactor,
// invalidCredentials, invalidPassword, newDevice, network, notFound, invalidSecret,
// notTotp, noImage, noQr, noScan, clipboardBusy, missing, bw:<message>.

mod bw;
mod clip;
mod login;
mod totp;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::services::hello;

const HELLO_SCOPE: &str = "vault";

/// Ouverture rapide par Windows Hello activée (réglage, poussé par le front).
static HELLO: AtomicBool = AtomicBool::new(false);

/// Session à garder pour Hello (activé et configuré).
fn hello_on(app: &AppHandle) -> bool {
    HELLO.load(Ordering::Relaxed) && hello::enrolled(app)
}

/// Scelle la session pour Hello si l'option est active (sans invite).
fn hello_keep(app: &AppHandle, session: &str) {
    if hello_on(app) {
        let _ = hello::seal(app, HELLO_SCOPE, session.as_bytes());
    }
}

struct Item {
    id: String,
    name: String,
    username: Option<String>,
    password: Option<String>,
    totp: Option<String>,
    uri: Option<String>,
    /// Objet Bitwarden complet (pour l'édition).
    raw: Value,
}

struct Vault {
    session: Option<String>,
    items: Vec<Item>,
    /// Dernière action (déverrouillage glissant).
    last: Option<Instant>,
    timeout: Duration,
    /// Résultat d'un scan QR en attente d'un nom (jamais renvoyé à l'UI).
    pending_scan: Option<String>,
}

static VAULT: Mutex<Vault> = Mutex::new(Vault {
    session: None,
    items: Vec::new(),
    last: None,
    timeout: Duration::from_secs(15 * 60),
    pending_scan: None,
});

/// État du compte côté `bw` (connecté, e-mail, serveur), mis en CACHE : lancer `bw status`
/// coûte 1–2 s (démarrage de Node). Lu une fois, puis tenu à jour par connexion,
/// déconnexion et changement de serveur.
#[derive(Clone, Default)]
struct Account {
    logged_in: bool,
    email: Option<String>,
    server: Option<String>,
}

static ACCOUNT: Mutex<Option<Account>> = Mutex::new(None);

fn account(app: &AppHandle) -> Account {
    if let Some(a) = ACCOUNT.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return a;
    }
    let st: Value = bw::run(app, &["status"], &[], None)
        .ok()
        .and_then(|o| serde_json::from_str(o.stdout.trim()).ok())
        .unwrap_or(Value::Null);
    let a = Account {
        logged_in: st.get("status").and_then(|v| v.as_str()).is_some_and(|s| s != "unauthenticated"),
        email: st.get("userEmail").and_then(|v| v.as_str()).map(str::to_string),
        server: st.get("serverUrl").and_then(|v| v.as_str()).map(str::to_string),
    };
    *ACCOUNT.lock().unwrap_or_else(|p| p.into_inner()) = Some(a.clone());
    a
}

fn update_account(f: impl FnOnce(&mut Account)) {
    let mut g = ACCOUNT.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(Account::default));
}

/// Lance une commande `bw` sans bloquer l'appelant (lock/logout : l'effet local est
/// immédiat, le CLI se met à jour derrière ; les appels `bw` restent sérialisés).
fn run_detached(app: &AppHandle, args: &'static [&'static str]) {
    let app = app.clone();
    std::thread::spawn(move || {
        if bw::installed(&app) {
            let _ = bw::run(&app, args, &[], None);
        }
    });
}

fn vault() -> std::sync::MutexGuard<'static, Vault> {
    VAULT.lock().unwrap_or_else(|p| p.into_inner())
}

/// Efface la session et les éléments (écrase les secrets avant de libérer).
fn wipe(v: &mut Vault) {
    if let Some(s) = v.session.as_mut() {
        unsafe { s.as_bytes_mut().fill(0) };
    }
    for it in v.items.iter_mut() {
        for s in [&mut it.password, &mut it.totp].into_iter().flatten() {
            unsafe { s.as_bytes_mut().fill(0) };
        }
    }
    v.session = None;
    v.items.clear();
    v.pending_scan = None;
}

/// Session courante (prolonge le déverrouillage) ou `locked`.
fn session() -> Result<String, String> {
    let mut v = vault();
    v.last = Some(Instant::now());
    v.session.clone().ok_or_else(|| "locked".into())
}

fn changed(app: &AppHandle) {
    let _ = app.emit("vault://changed", ());
}

/// Thread de verrouillage automatique (vérifie toutes les 5 s).
pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));
        let expired = {
            let v = vault();
            v.session.is_some() && v.last.is_some_and(|l| l.elapsed() > v.timeout)
        };
        if expired {
            lock_now(&app);
        }
    });
}

fn lock_now(app: &AppHandle) {
    let had = {
        let mut v = vault();
        let had = v.session.is_some();
        wipe(&mut v);
        had
    };
    // Session scellée pour Hello : on la laisse valide côté `bw`.
    if had && !(hello_on(app) && hello::has(app, HELLO_SCOPE)) {
        run_detached(app, &["lock"]);
    }
    changed(app);
}

fn map_err(msg: &str) -> String {
    let m = msg.to_lowercase();
    if m.contains("new device") || m.contains("device verification") {
        "newDevice".into()
    } else if m.contains("two-step") || m.contains("two step") || m.contains("2fa") || m.contains("code is required") {
        "twoFactor".into()
    } else if m.contains("invalid master password") {
        "invalidPassword".into()
    } else if m.contains("username or password is incorrect") || m.contains("invalid_grant") || m.contains("email") && m.contains("invalid") {
        "invalidCredentials".into()
    } else if m.contains("enotfound") || m.contains("econnrefused") || m.contains("network") || m.contains("fetch failed") {
        "network".into()
    } else if m.contains("not found") {
        "notFound".into()
    } else {
        format!("bw:{}", msg.lines().next().unwrap_or(msg))
    }
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())? }
}

fn require_bw(app: &AppHandle) -> Result<(), String> {
    if bw::installed(app) { Ok(()) } else { Err("notInstalled".into()) }
}

// --- Chargement des éléments ---------------------------------------------------------

fn parse_item(v: Value) -> Option<Item> {
    if v.get("type")?.as_i64()? != 1 {
        return None; // logins uniquement
    }
    let login = v.get("login")?;
    let s = |x: Option<&Value>| x.and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(str::to_string);
    Some(Item {
        id: s(v.get("id"))?,
        name: s(v.get("name")).unwrap_or_default(),
        username: s(login.get("username")),
        password: s(login.get("password")),
        totp: s(login.get("totp")),
        uri: login.get("uris").and_then(|u| u.as_array()).and_then(|a| a.first()).and_then(|u| s(u.get("uri"))),
        raw: v,
    })
}

/// Charge les éléments depuis le cache local de `bw` ; `sync` = interroge d'abord le
/// serveur (1 lancement de `bw` de plus).
fn load_items(app: &AppHandle, session: &str, sync: bool) -> Result<(), String> {
    let env = [("BW_SESSION", session)];
    if sync {
        let _ = bw::run(app, &["sync"], &env, None);
    }
    let out = bw::run(app, &["list", "items"], &env, None)?;
    if !out.ok {
        return Err(map_err(&out.message()));
    }
    let list: Vec<Value> = serde_json::from_str(out.stdout.trim()).map_err(|e| e.to_string())?;
    let mut items: Vec<Item> = list.into_iter().filter_map(parse_item).collect();
    items.sort_by_key(|i| i.name.to_lowercase());
    let mut v = vault();
    v.items = items;
    v.last = Some(Instant::now());
    Ok(())
}

/// Après un déverrouillage : la liste s'affiche tout de suite depuis le cache local, puis
/// on synchronise avec le serveur en arrière-plan et on prévient le front.
fn sync_in_background(app: &AppHandle, session: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        if load_items(&app, &session, true).is_ok() && vault().session.as_deref() == Some(session.as_str()) {
            changed(&app);
        }
    });
}

// --- Commandes : état, installation, compte -----------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    installed: bool,
    /// missing | loggedOut | locked | unlocked
    state: &'static str,
    email: Option<String>,
    server: Option<String>,
    /// Connexion en cours qui attend un code : twoFactor | newDevice.
    pending: Option<&'static str>,
    /// Déverrouillage par Windows Hello possible (session scellée).
    hello: bool,
}

#[tauri::command]
pub async fn vault_status(app: AppHandle) -> Result<Status, String> {
    blocking(move || {
        if !bw::installed(&app) {
            return Ok(Status { installed: false, state: "missing", email: None, server: None, pending: None, hello: false });
        }
        let acc = account(&app);
        let pending = login::pending();
        let email = acc.email.or_else(|| pending.as_ref().map(|(_, e)| e.clone()));
        let state = if !acc.logged_in {
            "loggedOut"
        } else if vault().session.is_some() {
            "unlocked"
        } else {
            "locked"
        };
        let hello = acc.logged_in && hello_on(&app) && hello::has(&app, HELLO_SCOPE);
        Ok(Status { installed: true, state, email, server: acc.server, pending: pending.map(|(step, _)| step), hello })
    })
    .await
}

#[tauri::command]
pub async fn vault_install(app: AppHandle) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INSTALLING: AtomicBool = AtomicBool::new(false);
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("installing".into());
    }
    let res = blocking(move || bw::install(&app)).await;
    INSTALLING.store(false, Ordering::SeqCst);
    res
}

/// Serveur Bitwarden : `None` = bitwarden.com, sinon URL (ex. https://vault.bitwarden.eu).
#[tauri::command]
pub async fn vault_set_server(app: AppHandle, url: Option<String>) -> Result<(), String> {
    blocking(move || {
        require_bw(&app)?;
        let url = url.unwrap_or_else(|| "https://vault.bitwarden.com".into());
        // Déjà configuré : on évite un lancement de `bw` (1–2 s).
        if account(&app).server.as_deref().map(|s| s.trim_end_matches('/')) == Some(url.trim_end_matches('/')) {
            return Ok(());
        }
        let out = bw::run(&app, &["config", "server", &url], &[], None)?;
        if !out.ok {
            return Err(map_err(&out.message()));
        }
        update_account(|a| a.server = Some(url));
        Ok(())
    })
    .await
}

/// Issue d'une étape de connexion, côté front. `step` : done | twoFactor | newDevice |
/// method ; `retry` : le code précédent était faux (un nouveau est demandé).
#[derive(serde::Serialize)]
pub struct LoginResult {
    step: &'static str,
    retry: bool,
}

fn finish(app: &AppHandle, step: login::Step, password: Option<&str>) -> Result<LoginResult, String> {
    let r = |step, retry| Ok(LoginResult { step, retry });
    match step {
        login::Step::Done { session, email } => {
            update_account(|a| {
                a.logged_in = true;
                a.email = Some(email);
            });
            vault().session = Some(session.clone());
            hello_keep(app, &session);
            load_items(app, &session, false)?;
            changed(app);
            sync_in_background(app, session);
            r("done", false)
        }
        login::Step::Code { kind, retry } => r(kind, retry),
        login::Step::ChooseMethod => r("method", false),
        login::Step::Failed(msg) => {
            // Compte déjà connecté sur ce PC : un simple déverrouillage suffit.
            if let (true, Some(pw)) = (msg.to_lowercase().contains("already logged in"), password) {
                unlock_with(app, pw)?;
                return r("done", false);
            }
            Err(map_err(&msg))
        }
    }
}

/// Connexion : e-mail + mot de passe maître. Si Bitwarden demande ensuite un code (2FA,
/// ou vérification de nouvel appareil envoyée par e-mail), renvoie l'étape attendue et
/// garde la connexion ouverte jusqu'à `vault_login_code`. `method` (0 application,
/// 1 e-mail, 3 YubiKey) ne sert que si le compte a plusieurs méthodes 2FA.
#[tauri::command]
pub async fn vault_login(app: AppHandle, email: String, password: String, method: Option<u8>) -> Result<LoginResult, String> {
    blocking(move || {
        require_bw(&app)?;
        let step = login::start(&app, email.trim(), &password, method)?;
        finish(&app, step, Some(&password))
    })
    .await
}

#[tauri::command]
pub async fn vault_login_code(app: AppHandle, code: String) -> Result<LoginResult, String> {
    blocking(move || {
        let step = login::submit(&code)?;
        finish(&app, step, None)
    })
    .await
}

#[tauri::command]
pub fn vault_login_cancel() {
    login::cancel();
}

fn unlock_with(app: &AppHandle, password: &str) -> Result<(), String> {
    let out = bw::run(app, &["unlock", "--passwordenv", "BW_PASSWORD", "--raw"], &[("BW_PASSWORD", password)], None)?;
    let session = out.stdout.trim().to_string();
    if !out.ok || session.is_empty() || session.contains(' ') {
        return Err(map_err(&out.message()));
    }
    vault().session = Some(session.clone());
    hello_keep(app, &session);
    load_items(app, &session, false)?;
    changed(app);
    sync_in_background(app, session);
    Ok(())
}

/// Déverrouillage par Windows Hello (invite système). Session périmée (verrouillée
/// ailleurs, expirée) → scellé oublié, `helloExpired` : retour au mot de passe maître.
#[tauri::command]
pub async fn vault_hello_unlock(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        require_bw(&app)?;
        let bytes = hello::open(&app, HELLO_SCOPE).map_err(|e| if e == "changed" || e == "notFound" { "helloExpired".into() } else { format!("hello.{e}") })?;
        let session = String::from_utf8(bytes).map_err(|_| "helloExpired")?;
        vault().session = Some(session.clone());
        if load_items(&app, &session, false).is_err() {
            wipe(&mut vault());
            hello::forget(&app, HELLO_SCOPE);
            changed(&app);
            return Err("helloExpired".into());
        }
        changed(&app);
        sync_in_background(&app, session);
        Ok(())
    })
    .await
}

/// Réglage « Coffre › Windows Hello ». Activé et coffre ouvert : la session courante est
/// scellée tout de suite. Désactivé : scellé oublié, et si le coffre est fermé la session
/// encore valide côté `bw` est verrouillée.
#[tauri::command]
pub async fn vault_hello_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    blocking(move || {
        HELLO.store(enabled, Ordering::Relaxed);
        if enabled {
            if let Some(s) = vault().session.clone() {
                hello_keep(&app, &s);
            }
        } else if hello::has(&app, HELLO_SCOPE) {
            hello::forget(&app, HELLO_SCOPE);
            if vault().session.is_none() {
                run_detached(&app, &["lock"]);
            }
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> Result<(), String> {
    blocking(move || {
        require_bw(&app)?;
        unlock_with(&app, &password)
    })
    .await
}

#[tauri::command]
pub async fn vault_lock(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        lock_now(&app);
        Ok(())
    })
    .await
}

/// « Se déconnecter » : oublie la session et le compte sur ce PC (rien n'est supprimé chez
/// Bitwarden).
#[tauri::command]
pub async fn vault_logout(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        login::cancel();
        wipe(&mut vault());
        hello::forget(&app, HELLO_SCOPE);
        update_account(|a| {
            a.logged_in = false;
            a.email = None;
        });
        run_detached(&app, &["logout"]);
        changed(&app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn vault_sync(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        let s = session()?;
        load_items(&app, &s, true)?;
        changed(&app);
        Ok(())
    })
    .await
}

/// Délai de verrouillage automatique (secondes, glissant).
#[tauri::command]
pub fn vault_set_timeout(seconds: u64) {
    let mut v = vault();
    v.timeout = Duration::from_secs(seconds.clamp(60, 24 * 3600));
}

// --- Commandes : éléments ------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    id: String,
    name: String,
    username: Option<String>,
    uri: Option<String>,
    has_password: bool,
    has_totp: bool,
}

/// Métadonnées des éléments (aucun secret).
#[tauri::command]
pub fn vault_list() -> Result<Vec<Meta>, String> {
    session()?;
    Ok(vault()
        .items
        .iter()
        .map(|i| Meta {
            id: i.id.clone(),
            name: i.name.clone(),
            username: i.username.clone(),
            uri: i.uri.clone(),
            has_password: i.password.is_some(),
            has_totp: i.totp.as_deref().is_some_and(|t| totp::parse(t).is_ok()),
        })
        .collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    id: String,
    code: String,
    remaining: u64,
    period: u64,
}

/// Codes 2FA courants (calculés ici ; les graines ne sortent pas).
#[tauri::command]
pub fn vault_codes() -> Result<Vec<Code>, String> {
    session()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Ok(vault()
        .items
        .iter()
        .filter_map(|i| {
            let otp = totp::parse(i.totp.as_deref()?).ok()?;
            let (code, remaining) = totp::code(&otp, now);
            Some(Code { id: i.id.clone(), code, remaining, period: otp.period })
        })
        .collect())
}

/// Copie privée : `field` = username | password | totp. Mots de passe et codes effacés du
/// presse-papiers après 30 s.
#[tauri::command]
pub fn vault_copy(id: String, field: String) -> Result<(), String> {
    session()?;
    let text = {
        let v = vault();
        let it = v.items.iter().find(|i| i.id == id).ok_or("notFound")?;
        match field.as_str() {
            "username" => it.username.clone(),
            "password" => it.password.clone(),
            "totp" => {
                let otp = totp::parse(it.totp.as_deref().ok_or("notFound")?).map_err(str::to_string)?;
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                Some(totp::code(&otp, now).0)
            }
            _ => None,
        }
        .ok_or("notFound")?
    };
    clip::copy_private(&text, field != "username").map_err(str::to_string)
}

/// Seule opération qui renvoie un secret à l'UI.
#[tauri::command]
pub fn vault_reveal(id: String) -> Result<String, String> {
    session()?;
    vault().items.iter().find(|i| i.id == id).and_then(|i| i.password.clone()).ok_or_else(|| "notFound".into())
}

fn encode(v: &Value) -> String {
    base64::engine::general_purpose::STANDARD.encode(v.to_string())
}

fn save_new(app: &AppHandle, item: Value) -> Result<(), String> {
    let s = session()?;
    let out = bw::run(app, &["create", "item", &encode(&item)], &[("BW_SESSION", s.as_str())], None)?;
    if !out.ok {
        return Err(map_err(&out.message()));
    }
    load_items(app, &s, false)?;
    changed(app);
    Ok(())
}

fn login_item(name: &str, username: Option<String>, password: Option<String>, totp: Option<String>, uri: Option<String>) -> Value {
    json!({
        "type": 1,
        "name": name,
        "notes": null,
        "favorite": false,
        "fields": [],
        "folderId": null,
        "organizationId": null,
        "reprompt": 0,
        "login": {
            "username": username,
            "password": password,
            "totp": totp,
            "uris": uri.map(|u| json!([{ "match": null, "uri": u }])).unwrap_or(json!([])),
        },
    })
}

#[tauri::command]
pub async fn vault_add(
    app: AppHandle,
    name: String,
    username: Option<String>,
    password: String,
    uri: Option<String>,
) -> Result<(), String> {
    blocking(move || {
        if name.trim().is_empty() || password.is_empty() {
            return Err("missing".into());
        }
        save_new(&app, login_item(name.trim(), username.filter(|u| !u.is_empty()), Some(password), None, uri.filter(|u| !u.is_empty())))
    })
    .await
}

#[tauri::command]
pub async fn vault_edit(
    app: AppHandle,
    id: String,
    name: String,
    username: Option<String>,
    password: String,
) -> Result<(), String> {
    blocking(move || {
        let s = session()?;
        let mut raw = vault().items.iter().find(|i| i.id == id).map(|i| i.raw.clone()).ok_or("notFound")?;
        raw["name"] = json!(name.trim());
        raw["login"]["username"] = json!(username.filter(|u| !u.is_empty()));
        raw["login"]["password"] = json!(password);
        let out = bw::run(&app, &["edit", "item", &id, &encode(&raw)], &[("BW_SESSION", s.as_str())], None)?;
        if !out.ok {
            return Err(map_err(&out.message()));
        }
        load_items(&app, &s, false)?;
        changed(&app);
        Ok(())
    })
    .await
}

/// Envoie l'élément à la corbeille Bitwarden (récupérable depuis le coffre web).
#[tauri::command]
pub async fn vault_delete(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || {
        let s = session()?;
        let out = bw::run(&app, &["delete", "item", &id], &[("BW_SESSION", s.as_str())], None)?;
        if !out.ok {
            return Err(map_err(&out.message()));
        }
        vault().items.retain(|i| i.id != id);
        changed(&app);
        Ok(())
    })
    .await
}

#[derive(serde::Serialize)]
pub struct Scan {
    issuer: Option<String>,
    account: Option<String>,
}

/// Scanne un QR dans le presse-papiers. Le contenu reste en attente côté Rust ; seuls
/// l'émetteur et le compte remontent.
#[tauri::command]
pub fn vault_scan_qr() -> Result<Scan, String> {
    session()?;
    let content = clip::scan_qr().map_err(str::to_string)?;
    let otp = totp::parse(&content).map_err(|e| if e == "invalidSecret" { "notTotp".to_string() } else { e.to_string() })?;
    vault().pending_scan = Some(content);
    Ok(Scan { issuer: otp.issuer, account: otp.account })
}

#[tauri::command]
pub fn vault_clear_scan() {
    vault().pending_scan = None;
}

/// Ajoute un 2FA : depuis le dernier scan QR, ou depuis une clé base32 / otpauth:// saisie.
#[tauri::command]
pub async fn vault_add_totp(app: AppHandle, name: String, secret: Option<String>, account: Option<String>) -> Result<(), String> {
    blocking(move || {
        if name.trim().is_empty() {
            return Err("missing".into());
        }
        let seed = match secret.filter(|s| !s.trim().is_empty()) {
            Some(s) => s.trim().to_string(),
            None => vault().pending_scan.take().ok_or("noScan")?,
        };
        let otp = totp::parse(&seed).map_err(str::to_string)?;
        let username = account.filter(|a| !a.is_empty()).or(otp.account);
        save_new(&app, login_item(name.trim(), username, None, Some(seed), None))
    })
    .await
}
