// Connexion Bitwarden pas à pas. `bw login` demande parfois un code en INVITE interactive :
//   - code 2FA (« Two-step login code ») si le compte a la 2FA ;
//   - code de vérification envoyé par e-mail (« New device verification ») quand Bitwarden
//     voit un nouvel appareil — y compris pour un compte SANS 2FA.
// Ce second code ne peut pas être passé en option : on garde donc le process `bw` vivant
// côté Rust, on détecte l'invite sur stderr, et le front n'affiche un champ de code QUE
// si Bitwarden en demande un. Le notch peut se fermer entre-temps : l'étape est conservée.
//
// Le mot de passe maître reste en mémoire Rust le temps de la connexion (pour relancer
// automatiquement après un code faux), puis il est écrasé. Un journal sans secret
// (`bitwarden/login.log`) trace les échanges avec `bw` pour le diagnostic.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::AppHandle;

use super::bw;

/// Résultat d'une étape de connexion.
pub enum Step {
    /// Connecté : clé de session + e-mail du compte.
    Done { session: String, email: String },
    /// Bitwarden attend un code : `twoFactor` ou `newDevice`. `retry` = le code précédent
    /// était faux, la connexion a été relancée (nouveau code envoyé).
    Code { kind: &'static str, retry: bool },
    /// Plusieurs méthodes 2FA : il faut en choisir une (`--method`).
    ChooseMethod,
    /// Échec : message de `bw`.
    Failed(String),
}

enum Ev {
    Prompt(&'static str),
    StreamDone,
}

struct Pending {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Ev>,
    out: Arc<Mutex<String>>,
    err: Arc<Mutex<String>>,
    step: &'static str,
    email: String,
    password: String,
    method: Option<u8>,
    app: AppHandle,
}

impl Drop for Pending {
    fn drop(&mut self) {
        unsafe { self.password.as_bytes_mut().fill(0) };
    }
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Délai max d'attente d'une réponse de `bw` (réseau compris).
const WAIT: Duration = Duration::from_secs(90);

fn log(app: &AppHandle, line: &str) {
    let path = bw::data_dir(app).join("login.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[{ts}] {}", line.replace(['\r', '\n'], " ⏎ "));
    }
}

fn detect(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    if t.contains("two-step login method") {
        Some("method")
    } else if t.contains("new device verification") {
        Some("newDevice")
    } else if t.contains("two-step login code") {
        Some("twoFactor")
    } else {
        None
    }
}

/// Étape en attente (pour reprendre après une fermeture du notch) : (étape, e-mail).
pub fn pending() -> Option<(&'static str, String)> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|p| (p.step, p.email.clone()))
}

pub fn cancel() {
    if let Some(mut p) = PENDING.lock().unwrap_or_else(|p| p.into_inner()).take() {
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
}

fn spawn(app: &AppHandle, email: &str, password: String, method: Option<u8>) -> Result<Pending, String> {
    let m = method.map(|m| m.to_string());
    let mut args = vec!["login", email, "--passwordenv", "BW_PASSWORD", "--raw"];
    if let Some(m) = m.as_deref() {
        args.extend(["--method", m]);
    }
    log(app, &format!("start login {email} method={method:?}"));
    let mut child = bw::spawn_interactive(app, &args, &[("BW_PASSWORD", password.as_str())])?;
    let stdin = child.stdin.take().ok_or("bw: stdin")?;
    let mut stdout = child.stdout.take().ok_or("bw: stdout")?;
    let mut stderr = child.stderr.take().ok_or("bw: stderr")?;
    let (tx, rx) = channel();
    let out = Arc::new(Mutex::new(String::new()));
    let err = Arc::new(Mutex::new(String::new()));

    {
        let (tx, out) = (tx.clone(), out.clone());
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stdout.read_to_string(&mut s);
            *out.lock().unwrap_or_else(|p| p.into_inner()) = s;
            let _ = tx.send(Ev::StreamDone);
        });
    }
    {
        let (err, app) = (err.clone(), app.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let mut window = String::new();
            loop {
                let n = match stderr.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                log(&app, &format!("stderr: {chunk}"));
                err.lock().unwrap_or_else(|p| p.into_inner()).push_str(&chunk);
                window.push_str(&chunk);
                if let Some(kind) = detect(&window) {
                    window.clear();
                    let _ = tx.send(Ev::Prompt(kind));
                }
            }
            let _ = tx.send(Ev::StreamDone);
        });
    }

    Ok(Pending {
        child,
        stdin,
        rx,
        out,
        err,
        step: "",
        email: email.to_string(),
        password,
        method,
        app: app.clone(),
    })
}

pub fn start(app: &AppHandle, email: &str, password: &str, method: Option<u8>) -> Result<Step, String> {
    cancel();
    let p = spawn(app, email, password.to_string(), method)?;
    drive(p, false)
}

/// Envoie le code demandé et poursuit la connexion.
pub fn submit(code: &str) -> Result<Step, String> {
    let mut p = PENDING.lock().unwrap_or_else(|p| p.into_inner()).take().ok_or("noLogin")?;
    p.err.lock().unwrap_or_else(|p| p.into_inner()).clear();
    log(&p.app, &format!("submit code ({} chars) for step {}", code.trim().len(), p.step));
    // Retour chariot Windows : readline accepte \n comme \r\n.
    write!(p.stdin, "{}\n", code.trim()).map_err(|e| format!("bw: {e}"))?;
    let _ = p.stdin.flush();
    drive(p, true)
}

fn drive(mut p: Pending, after_code: bool) -> Result<Step, String> {
    let mut done = 0;
    loop {
        match p.rx.recv_timeout(WAIT) {
            Ok(Ev::Prompt("method")) => {
                log(&p.app, "prompt: method");
                let _ = p.child.kill();
                let _ = p.child.wait();
                return Ok(Step::ChooseMethod);
            }
            Ok(Ev::Prompt(kind)) => {
                log(&p.app, &format!("prompt: {kind}"));
                // Une nouvelle invite juste après un code = le code était faux et `bw`
                // redemande : on le signale.
                let retry = after_code && p.step == kind;
                p.step = kind;
                *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some(p);
                return Ok(Step::Code { kind, retry });
            }
            Ok(Ev::StreamDone) => {
                done += 1;
                if done < 2 {
                    continue;
                }
                let status = p.child.wait().map_err(|e| format!("bw: {e}"))?;
                let session = p.out.lock().unwrap_or_else(|p| p.into_inner()).trim().to_string();
                if status.success() && !session.is_empty() && !session.contains(char::is_whitespace) {
                    log(&p.app, &format!("done (session {} chars)", session.len()));
                    return Ok(Step::Done { session, email: p.email.clone() });
                }
                let err = p.err.lock().unwrap_or_else(|p| p.into_inner()).trim().to_string();
                let msg = if err.is_empty() { session } else { err };
                let msg = msg.lines().last().unwrap_or_default().trim_start_matches(['?', ' ']).to_string();
                log(&p.app, &format!("exit {:?}: {msg}", status.code()));
                // Code faux : on relance la connexion (Bitwarden renvoie un code).
                if after_code && !p.step.is_empty() {
                    let (app, email, password, method, kind) =
                        (p.app.clone(), p.email.clone(), p.password.clone(), p.method, p.step);
                    drop(p);
                    let next = spawn(&app, &email, password, method)?;
                    return match drive(next, false)? {
                        Step::Code { kind: k, .. } if k == kind => Ok(Step::Code { kind: k, retry: true }),
                        other => Ok(other),
                    };
                }
                return Ok(Step::Failed(msg));
            }
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                log(&p.app, "timeout");
                let _ = p.child.kill();
                let _ = p.child.wait();
                return Err("timeout".into());
            }
        }
    }
}
