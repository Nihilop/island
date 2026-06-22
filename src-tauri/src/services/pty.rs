// Service `terminal` : terminaux interactifs (PTY/ConPTY via portable-pty) + `exec`
// one-shot (capture stdout). Cross-platform (portable-pty abstrait ConPTY/unix).
//
// ⚠️ PERMISSION `terminal` = CONFIANCE MAXIMALE : une extension qui l'a peut lancer
// n'importe quel programme. Gated par `require_perm!`, affichée en évidence à l'install.
// Frontière de confiance la plus large du SDK (équivalent exécution de code arbitraire).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use base64::{engine::general_purpose::STANDARD, Engine};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Deserialize;
use tauri::{AppHandle, Emitter};

use crate::ext::require_perm;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

static SESSIONS: Mutex<Option<HashMap<String, Session>>> = Mutex::new(None);

fn with_sessions<R>(f: impl FnOnce(&mut HashMap<String, Session>) -> R) -> R {
    let mut g = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

fn rand_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("pty{:x}", n)
}

#[derive(Deserialize)]
pub struct SpawnOpts {
    pub cwd: Option<String>,
    pub cmd: Option<String>, // défaut = shell par défaut de l'OS
    pub args: Option<Vec<String>>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

/// Démarre un terminal PTY → renvoie un id de session. La sortie est streamée en
/// base64 (binaire/ANSI safe) via l'event `pty://data {id, b64}` ; `pty://exit {id}`
/// à la fin.
#[tauri::command]
pub fn pty_spawn(app: AppHandle, ext_id: String, opts: SpawnOpts) -> Result<String, String> {
    require_perm!(&app, &ext_id, "terminal");

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: opts.rows.unwrap_or(24),
            cols: opts.cols.unwrap_or(80),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut builder = match opts.cmd {
        Some(c) if !c.is_empty() => CommandBuilder::new(c),
        _ => CommandBuilder::new_default_prog(),
    };
    for a in opts.args.unwrap_or_default() {
        builder.arg(a);
    }
    if let Some(d) = opts.cwd {
        if !d.is_empty() {
            builder.cwd(d);
        }
    }
    // Hérite l'environnement de l'hôte (PATH… → pnpm/git/artisan résolvables).
    for (k, v) in std::env::vars() {
        builder.env(k, v);
    }

    let child = pair.slave.spawn_command(builder).map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    drop(pair.slave); // sinon le master ne reçoit jamais l'EOF à la sortie du process

    let id = rand_id();
    with_sessions(|m| {
        m.insert(
            id.clone(),
            Session { master: pair.master, writer, child },
        )
    });

    // Thread lecteur : streame la sortie jusqu'à EOF.
    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let b64 = STANDARD.encode(&buf[..n]);
                    let _ = app2.emit("pty://data", serde_json::json!({ "id": id2, "b64": b64 }));
                }
            }
        }
        with_sessions(|m| {
            m.remove(&id2);
        });
        let _ = app2.emit("pty://exit", serde_json::json!({ "id": id2 }));
    });

    Ok(id)
}

/// Écrit dans le stdin du terminal (frappes clavier depuis xterm).
#[tauri::command]
pub fn pty_write(app: AppHandle, ext_id: String, id: String, data: String) -> Result<(), String> {
    require_perm!(&app, &ext_id, "terminal");
    with_sessions(|m| match m.get_mut(&id) {
        Some(s) => s
            .writer
            .write_all(data.as_bytes())
            .and_then(|_| s.writer.flush())
            .map_err(|e| e.to_string()),
        None => Err("session inconnue".into()),
    })
}

/// Redimensionne le PTY (au resize de la fenêtre / xterm fit).
#[tauri::command]
pub fn pty_resize(app: AppHandle, ext_id: String, id: String, cols: u16, rows: u16) -> Result<(), String> {
    require_perm!(&app, &ext_id, "terminal");
    with_sessions(|m| match m.get(&id) {
        Some(s) => s
            .master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string()),
        None => Err("session inconnue".into()),
    })
}

/// Tue le process et libère la session.
#[tauri::command]
pub fn pty_kill(app: AppHandle, ext_id: String, id: String) -> Result<(), String> {
    require_perm!(&app, &ext_id, "terminal");
    with_sessions(|m| {
        if let Some(mut s) = m.remove(&id) {
            let _ = s.child.kill();
        }
    });
    Ok(())
}

#[derive(Deserialize)]
pub struct ExecOpts {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
}

/// Exécute une commande one-shot et CAPTURE sa sortie (pour les commandes structurées :
/// `git branch`, `git diff --numstat`, `docker ps`…). Pas de PTY (pas d'interactivité).
#[tauri::command]
pub fn pty_exec(app: AppHandle, ext_id: String, opts: ExecOpts) -> Result<serde_json::Value, String> {
    require_perm!(&app, &ext_id, "terminal");
    let mut c = std::process::Command::new(&opts.cmd);
    if let Some(a) = opts.args {
        c.args(a);
    }
    if let Some(d) = opts.cwd {
        if !d.is_empty() {
            c.current_dir(d);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = c.output().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "code": out.status.code(),
        "stdout": String::from_utf8_lossy(&out.stdout),
        "stderr": String::from_utf8_lossy(&out.stderr),
    }))
}
