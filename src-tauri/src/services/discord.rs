// Discord (outil intégré) : appel vocal en cours + mute / sourdine / raccrocher, via le RPC
// LOCAL du client Discord (named pipe `\\.\pipe\discord-ipc-N`, trames binaires opcode +
// longueur + JSON).
//
// Discord n'accorde les scopes vocaux qu'à l'application du portail développeur de
// l'utilisateur : il fournit le Client ID (réglages) et le Client Secret (gardé dans le
// Gestionnaire d'identifiants, jamais relu par le front). Flux : handshake → READY →
// refresh token stocké ? échange OAuth : AUTHORIZE (Discord demande une fois) → code →
// échange → AUTHENTICATE. Réessai toutes les 5 s ; un compteur de génération arrête les
// anciennes boucles lors d'une reconfiguration. L'état complet est poussé au front
// (`discord://state`) à chaque changement.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

const SCOPES: [&str; 4] = ["rpc", "rpc.voice.read", "rpc.voice.write", "identify"];
const RETRY: Duration = Duration::from_secs(5);
const AUTHORIZE_WAIT: Duration = Duration::from_secs(120);

static GEN: AtomicU64 = AtomicU64::new(0);
static CMD: Mutex<Option<Sender<Action>>> = Mutex::new(None);
static STATE: Mutex<Option<DiscordState>> = Mutex::new(None);

enum Action {
    Mute,
    Deaf,
    Hangup,
}

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    id: String,
    name: String,
    avatar: String,
    speaking: bool,
    muted: bool,
}

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    channel_id: String,
    channel: String,
    guild: Option<String>,
    guild_icon: Option<String>,
    members: Vec<Member>,
}

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordState {
    /// off | missingConfig | notRunning | connecting | authorizing | ready | error
    status: &'static str,
    error: Option<String>,
    mute: bool,
    deaf: bool,
    /// Notre id utilisateur (pour savoir si c'est nous qui parlons).
    me: Option<String>,
    call: Option<Call>,
}

// --- Secrets (Gestionnaire d'identifiants, jamais exposés au front) ------------------

fn secret(key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new("island-core:discord", key)
}
fn secret_get(key: &str) -> Option<String> {
    secret(key).ok()?.get_password().ok()
}
fn secret_set(key: &str, v: Option<&str>) {
    if let Ok(e) = secret(key) {
        let _ = match v {
            Some(v) if !v.is_empty() => e.set_password(v),
            _ => e.delete_credential(),
        };
    }
}

// --- Pipe ----------------------------------------------------------------------------

struct Pipe(std::fs::File);

impl Pipe {
    fn connect() -> Option<Self> {
        (0..10).find_map(|i| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!(r"\\.\pipe\discord-ipc-{i}"))
                .ok()
                .map(Pipe)
        })
    }

    fn send(&mut self, op: u32, v: &Value) -> std::io::Result<()> {
        let body = v.to_string().into_bytes();
        let mut buf = Vec::with_capacity(8 + body.len());
        buf.extend_from_slice(&op.to_le_bytes());
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.extend_from_slice(&body);
        self.0.write_all(&buf)
    }

    /// Octets disponibles sans bloquer (une lecture bloquante sur un handle synchrone
    /// bloquerait aussi les écritures).
    fn available(&self) -> u32 {
        #[cfg(target_os = "windows")]
        unsafe {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::System::Pipes::PeekNamedPipe;
            let mut avail = 0u32;
            if PeekNamedPipe(HANDLE(self.0.as_raw_handle()), None, 0, None, Some(&mut avail), None).is_err() {
                return u32::MAX; // pipe cassé → la lecture échouera et on reconnectera
            }
            avail
        }
        #[cfg(not(target_os = "windows"))]
        0
    }

    /// Trame suivante si disponible : (opcode, JSON).
    fn try_recv(&mut self) -> std::io::Result<Option<(u32, Value)>> {
        if self.available() < 8 {
            return Ok(None);
        }
        let mut head = [0u8; 8];
        self.0.read_exact(&mut head)?;
        let op = u32::from_le_bytes(head[..4].try_into().unwrap());
        let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
        let mut body = vec![0u8; len];
        self.0.read_exact(&mut body)?;
        Ok(Some((op, serde_json::from_slice(&body).unwrap_or(Value::Null))))
    }
}

struct Rpc {
    pipe: Pipe,
    nonce: u64,
    /// Événements reçus pendant l'attente d'une réponse, traités ensuite.
    events: VecDeque<Value>,
}

impl Rpc {
    /// Envoie une commande et attend SA réponse (même nonce).
    fn call(&mut self, cmd: &str, args: Value, evt: Option<&str>, wait: Duration) -> Result<Value, String> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        let mut msg = json!({ "cmd": cmd, "args": args, "nonce": nonce });
        if let Some(e) = evt {
            msg["evt"] = json!(e);
        }
        self.pipe.send(1, &msg).map_err(|e| e.to_string())?;
        let start = Instant::now();
        loop {
            match self.pipe.try_recv().map_err(|e| e.to_string())? {
                Some((3, v)) => {
                    let _ = self.pipe.send(4, &v); // PING → PONG
                }
                Some((2, v)) => return Err(v["message"].as_str().unwrap_or("closed").to_string()),
                Some((_, v)) if v["nonce"].as_str() == Some(nonce.as_str()) => {
                    if v["evt"].as_str() == Some("ERROR") {
                        return Err(v["data"]["message"].as_str().unwrap_or("error").to_string());
                    }
                    return Ok(v["data"].clone());
                }
                Some((_, v)) => self.events.push_back(v),
                None => {
                    if start.elapsed() > wait {
                        return Err("timeout".into());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

// --- OAuth ---------------------------------------------------------------------------

fn token(client_id: &str, secret: &str, grant: &[(&str, &str)]) -> Result<(String, String), String> {
    let mut form: Vec<(&str, &str)> = vec![("client_id", client_id), ("client_secret", secret)];
    form.extend_from_slice(grant);
    let body = match ureq::post("https://discord.com/api/oauth2/token").send_form(&form) {
        Ok(r) => r.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, r)) => return Err(r.into_string().unwrap_or_default()),
        Err(_) => return Err("network".into()),
    };
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    match (v["access_token"].as_str(), v["refresh_token"].as_str()) {
        (Some(a), Some(r)) => Ok((a.to_string(), r.to_string())),
        _ => Err(v["error_description"].as_str().or(v["error"].as_str()).unwrap_or("oauth").to_string()),
    }
}

/// Jeton d'accès : refresh token stocké, sinon autorisation dans Discord (une fois).
fn access_token(app: &AppHandle, rpc: &mut Rpc, client_id: &str, secret: &str) -> Result<String, String> {
    if let Some(rt) = secret_get("refresh") {
        match token(client_id, secret, &[("grant_type", "refresh_token"), ("refresh_token", &rt)]) {
            Ok((access, refresh)) => {
                secret_set("refresh", Some(&refresh));
                return Ok(access);
            }
            Err(e) if e == "network" => return Err(e),
            Err(_) => secret_set("refresh", None), // refusé → nouvelle autorisation
        }
    }
    update(app, |s| s.status = "authorizing");
    let data = rpc.call("AUTHORIZE", json!({ "client_id": client_id, "scopes": SCOPES }), None, AUTHORIZE_WAIT)?;
    let code = data["code"].as_str().ok_or("authorize")?.to_string();
    // L'échange est tenté sans puis avec la redirection déclarée sur le portail.
    let r = token(client_id, secret, &[("grant_type", "authorization_code"), ("code", &code)]).or_else(|_| {
        token(client_id, secret, &[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", "http://localhost")])
    });
    let (access, refresh) = r.map_err(|e| if e.contains("redirect") { "redirectUri".to_string() } else { e })?;
    secret_set("refresh", Some(&refresh));
    Ok(access)
}

// --- État ----------------------------------------------------------------------------

fn update(app: &AppHandle, f: impl FnOnce(&mut DiscordState)) {
    let snapshot = {
        let mut g = STATE.lock().unwrap_or_else(|p| p.into_inner());
        let s = g.get_or_insert_with(DiscordState::default);
        f(s);
        s.clone()
    };
    let _ = app.emit("discord://state", snapshot);
}

fn member(v: &Value) -> Option<Member> {
    let u = &v["user"];
    let id = u["id"].as_str()?.to_string();
    let avatar = match u["avatar"].as_str() {
        Some(h) => format!("https://cdn.discordapp.com/avatars/{id}/{h}.png?size=64"),
        None => format!("https://cdn.discordapp.com/embed/avatars/{}.png", (id.parse::<u64>().unwrap_or(0) >> 22) % 6),
    };
    let vs = &v["voice_state"];
    Some(Member {
        name: v["nick"]
            .as_str()
            .or(u["global_name"].as_str())
            .or(u["username"].as_str())
            .unwrap_or("?")
            .to_string(),
        muted: vs["self_mute"].as_bool().unwrap_or(false) || vs["mute"].as_bool().unwrap_or(false),
        speaking: false,
        avatar,
        id,
    })
}

const CHANNEL_EVENTS: [&str; 5] = ["SPEAKING_START", "SPEAKING_STOP", "VOICE_STATE_CREATE", "VOICE_STATE_UPDATE", "VOICE_STATE_DELETE"];

/// Charge le salon `channel_id` (ou rien) et (dés)abonne ses événements.
fn select_channel(app: &AppHandle, rpc: &mut Rpc, channel_id: Option<String>) -> Result<(), String> {
    let old = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|s| s.call.as_ref().map(|c| c.channel_id.clone()));
    if old == channel_id {
        return Ok(());
    }
    if let Some(old) = old {
        for e in CHANNEL_EVENTS {
            let _ = rpc.call("UNSUBSCRIBE", json!({ "channel_id": old }), Some(e), Duration::from_secs(5));
        }
    }
    let Some(id) = channel_id else {
        update(app, |s| s.call = None);
        return Ok(());
    };
    let ch = rpc.call("GET_CHANNEL", json!({ "channel_id": id }), None, Duration::from_secs(5))?;
    for e in CHANNEL_EVENTS {
        let _ = rpc.call("SUBSCRIBE", json!({ "channel_id": id }), Some(e), Duration::from_secs(5));
    }
    let (guild, guild_icon) = match ch["guild_id"].as_str() {
        Some(g) => {
            let gd = rpc.call("GET_GUILD", json!({ "guild_id": g }), None, Duration::from_secs(5)).unwrap_or(Value::Null);
            (gd["name"].as_str().map(str::to_string), gd["icon_url"].as_str().map(str::to_string))
        }
        None => (None, None),
    };
    let members = ch["voice_states"].as_array().map(|a| a.iter().filter_map(member).collect()).unwrap_or_default();
    let call = Call {
        channel_id: id,
        channel: ch["name"].as_str().unwrap_or_default().to_string(),
        guild,
        guild_icon,
        members,
    };
    update(app, |s| s.call = Some(call));
    Ok(())
}

fn on_event(app: &AppHandle, rpc: &mut Rpc, v: Value) -> Result<(), String> {
    let d = &v["data"];
    match v["evt"].as_str().unwrap_or_default() {
        "VOICE_CHANNEL_SELECT" => select_channel(app, rpc, d["channel_id"].as_str().map(str::to_string))?,
        "VOICE_SETTINGS_UPDATE" => update(app, |s| {
            s.mute = d["mute"].as_bool().unwrap_or(s.mute);
            s.deaf = d["deaf"].as_bool().unwrap_or(s.deaf);
        }),
        e @ ("SPEAKING_START" | "SPEAKING_STOP") => {
            let speaking = e == "SPEAKING_START";
            let uid = d["user_id"].as_str().unwrap_or_default().to_string();
            update(app, |s| {
                if let Some(m) = s.call.as_mut().and_then(|c| c.members.iter_mut().find(|m| m.id == uid)) {
                    m.speaking = speaking;
                }
            });
        }
        "VOICE_STATE_CREATE" | "VOICE_STATE_UPDATE" => {
            if let Some(m) = member(d) {
                update(app, |s| {
                    if let Some(c) = s.call.as_mut() {
                        match c.members.iter_mut().find(|x| x.id == m.id) {
                            Some(x) => {
                                x.name = m.name;
                                x.muted = m.muted;
                                x.avatar = m.avatar;
                            }
                            None => c.members.push(m),
                        }
                    }
                });
            }
        }
        "VOICE_STATE_DELETE" => {
            let uid = d["user"]["id"].as_str().unwrap_or_default().to_string();
            update(app, |s| {
                if let Some(c) = s.call.as_mut() {
                    c.members.retain(|m| m.id != uid);
                }
            });
        }
        _ => {}
    }
    Ok(())
}

/// Une session : connexion, authentification, puis boucle d'événements.
fn session(app: &AppHandle, gen: u64, client_id: &str, secret: &str, rx: &Receiver<Action>) -> Result<(), String> {
    let pipe = Pipe::connect().ok_or("notRunning")?;
    update(app, |s| s.status = "connecting");
    let mut rpc = Rpc { pipe, nonce: 0, events: VecDeque::new() };
    rpc.pipe.send(0, &json!({ "v": 1, "client_id": client_id })).map_err(|e| e.to_string())?;
    // READY
    let start = Instant::now();
    loop {
        match rpc.pipe.try_recv().map_err(|e| e.to_string())? {
            Some((2, v)) => return Err(v["message"].as_str().unwrap_or("closed").to_string()),
            Some((_, v)) if v["evt"].as_str() == Some("READY") => break,
            Some(_) => {}
            None if start.elapsed() > Duration::from_secs(10) => return Err("timeout".into()),
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let access = access_token(app, &mut rpc, client_id, secret)?;
    let auth = rpc.call("AUTHENTICATE", json!({ "access_token": access }), None, Duration::from_secs(10)).map_err(|e| {
        secret_set("refresh", None);
        e
    })?;
    let me = auth["user"]["id"].as_str().map(str::to_string);

    for e in ["VOICE_CHANNEL_SELECT", "VOICE_SETTINGS_UPDATE"] {
        rpc.call("SUBSCRIBE", json!({}), Some(e), Duration::from_secs(5))?;
    }
    let vs = rpc.call("GET_VOICE_SETTINGS", json!({}), None, Duration::from_secs(5))?;
    update(app, |s| {
        s.status = "ready";
        s.error = None;
        s.me = me;
        s.mute = vs["mute"].as_bool().unwrap_or(false);
        s.deaf = vs["deaf"].as_bool().unwrap_or(false);
        s.call = None;
    });
    let sel = rpc.call("GET_SELECTED_VOICE_CHANNEL", json!({}), None, Duration::from_secs(5))?;
    select_channel(app, &mut rpc, sel["id"].as_str().map(str::to_string))?;

    loop {
        if GEN.load(Ordering::SeqCst) != gen {
            return Ok(());
        }
        while let Ok(a) = rx.try_recv() {
            let (mute, deaf) = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|s| (s.mute, s.deaf)).unwrap_or_default();
            let _ = match a {
                Action::Mute => rpc.call("SET_VOICE_SETTINGS", json!({ "mute": !mute }), None, Duration::from_secs(5)),
                Action::Deaf => rpc.call("SET_VOICE_SETTINGS", json!({ "deaf": !deaf }), None, Duration::from_secs(5)),
                Action::Hangup => rpc.call("SELECT_VOICE_CHANNEL", json!({ "channel_id": null, "force": true }), None, Duration::from_secs(5)),
            };
        }
        while let Some(v) = rpc.events.pop_front() {
            on_event(app, &mut rpc, v)?;
        }
        match rpc.pipe.try_recv().map_err(|e| e.to_string())? {
            Some((3, v)) => rpc.pipe.send(4, &v).map_err(|e| e.to_string())?,
            Some((2, _)) => return Err("notRunning".into()),
            Some((_, v)) => on_event(app, &mut rpc, v)?,
            None => std::thread::sleep(Duration::from_millis(40)),
        }
    }
}

/// (Re)démarre la boucle avec ce Client ID ; `None` = arrêt (outil désactivé).
fn restart(app: AppHandle, client_id: Option<String>) {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = channel();
    *CMD.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx);
    let Some(client_id) = client_id.filter(|c| !c.trim().is_empty()) else {
        update(&app, |s| *s = DiscordState { status: "missingConfig", ..Default::default() });
        return;
    };
    std::thread::spawn(move || {
        while GEN.load(Ordering::SeqCst) == gen {
            let Some(secret) = secret_get("secret") else {
                update(&app, |s| *s = DiscordState { status: "missingConfig", ..Default::default() });
                return;
            };
            match session(&app, gen, client_id.trim(), &secret, &rx) {
                Ok(()) => return,
                Err(e) if GEN.load(Ordering::SeqCst) == gen => update(&app, |s| {
                    s.status = if e == "notRunning" { "notRunning" } else { "error" };
                    s.error = (e != "notRunning").then_some(e);
                    s.call = None;
                }),
                Err(_) => return,
            }
            std::thread::sleep(RETRY);
        }
    });
}

// --- Commandes -----------------------------------------------------------------------

/// Active (Client ID) ou arrête (`None`) la connexion à Discord.
#[tauri::command]
pub fn discord_configure(app: AppHandle, client_id: Option<String>, enabled: bool) {
    if enabled {
        restart(app, client_id);
    } else {
        GEN.fetch_add(1, Ordering::SeqCst);
        update(&app, |s| *s = DiscordState { status: "off", ..Default::default() });
    }
}

#[tauri::command]
pub fn discord_state() -> DiscordState {
    STATE.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or(DiscordState { status: "off", ..Default::default() })
}

/// Enregistre (ou supprime si vide) le Client Secret ; le refresh token est oublié.
#[tauri::command]
pub fn discord_set_secret(secret: String) {
    secret_set("secret", Some(secret.trim()));
    secret_set("refresh", None);
}

#[tauri::command]
pub fn discord_has_secret() -> bool {
    secret_get("secret").is_some()
}

#[tauri::command]
pub fn discord_action(action: String) {
    let a = match action.as_str() {
        "mute" => Action::Mute,
        "deaf" => Action::Deaf,
        "hangup" => Action::Hangup,
        _ => return,
    };
    if let Some(tx) = CMD.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        let _ = tx.send(a);
    }
}
