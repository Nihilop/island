// Services « système » des extensions v2 : dossiers choisis par l'utilisateur (`files`),
// commandes dans Windows Terminal (`terminal`), Docker Engine (`docker`). Comme le reste de
// `services`, appelés UNIQUEMENT par l'hôte, avec l'identité déduite du canal ; chaque
// permission est revérifiée ici.
//
// `files` : l'extension ne voit QUE les dossiers que l'utilisateur lui a donnés par la boîte
// de dialogue d'Island (`ext2_files_pick`), retenus dans son état (`State.folders`). Tout
// chemin est canonisé (liens résolus) et doit rester dans l'un d'eux. Ouvrir un exécutable
// (.exe, .bat, .ps1, .lnk…) demande en plus `terminal` : sans elle, `files` ne lance rien.
//
// `terminal` (confiance maximale : exécution de commandes) : nouvel onglet de Windows
// Terminal (repli : console cmd), ou lancement discret (`code .`), toujours dans un dossier
// donné. `docker` (à haut risque : Docker pilote la machine) : requêtes HTTP vers le moteur
// local (`\\.\pipe\docker_engine`, `/var/run/docker.sock`), flux sans fin refusés.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::install::{save_state, state};
use super::services::require;

// --- Dossiers donnés par l'utilisateur ----------------------------------------------------

/// Chemin lisible (sans le préfixe `\\?\` de la canonisation sous Windows).
fn plain(p: &Path) -> String {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => s.strip_prefix(r"\\?\").unwrap_or(&s).to_string(),
    }
}

fn folders(app: &AppHandle, id: &str) -> Vec<PathBuf> {
    state(app, id).map(|s| s.folders).unwrap_or_default().into_iter().filter_map(|f| std::fs::canonicalize(f).ok()).collect()
}

/// Chemin canonisé, refusé s'il sort des dossiers donnés.
fn within(app: &AppHandle, id: &str, path: &str) -> Result<PathBuf, String> {
    let p = std::fs::canonicalize(path).map_err(|_| "notFound".to_string())?;
    if folders(app, id).iter().any(|root| p.starts_with(root)) { Ok(p) } else { Err("forbidden".into()) }
}

/// Demande un dossier à l'utilisateur (boîte de dialogue d'Island) et le donne à l'extension.
#[tauri::command]
pub async fn ext2_files_pick(app: AppHandle, id: String, title: Option<String>) -> Result<Option<String>, String> {
    require(&app, &id, "files")?;
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file();
    if let Some(t) = title.filter(|t| !t.trim().is_empty()) {
        dialog = dialog.set_title(t.chars().take(120).collect::<String>());
    }
    let Some(picked) = dialog.blocking_pick_folder() else { return Ok(None) };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let shown = plain(&canon);
    let mut st = state(&app, &id).unwrap_or_default();
    if !st.folders.iter().any(|f| f.eq_ignore_ascii_case(&shown)) {
        st.folders.push(shown.clone());
        save_state(&app, &id, &st);
    }
    Ok(Some(shown))
}

#[tauri::command]
pub fn ext2_files_folders(app: AppHandle, id: String) -> Result<Vec<String>, String> {
    require(&app, &id, "files")?;
    Ok(state(&app, &id).map(|s| s.folders).unwrap_or_default())
}

/// Retire l'accès à un dossier.
#[tauri::command]
pub fn ext2_files_forget(app: AppHandle, id: String, path: String) -> Result<(), String> {
    require(&app, &id, "files")?;
    let mut st = state(&app, &id).unwrap_or_default();
    st.folders.retain(|f| !f.eq_ignore_ascii_case(&path));
    save_state(&app, &id, &st);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    path: String,
    name: String,
    dir: bool,
    size: u64,
    /// Dernière modification (ms depuis 1970).
    modified: i64,
}

const MAX_ENTRIES: usize = 5000;

/// Contenu d'un dossier, sur `depth` niveaux (1 à 4). On ne descend pas dans les dossiers
/// nommés dans `skip` (node_modules, .git…), qui restent listés.
#[tauri::command]
pub async fn ext2_files_list(app: AppHandle, id: String, path: String, depth: Option<u32>, skip: Option<Vec<String>>) -> Result<Vec<Entry>, String> {
    require(&app, &id, "files")?;
    let start = within(&app, &id, &path)?;
    let depth = depth.unwrap_or(1).clamp(1, 4);
    let skip: Vec<String> = skip.unwrap_or_default().into_iter().take(64).map(|s| s.to_lowercase()).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        walk(&start, depth, &skip, &mut out);
        out
    })
    .await
    .map_err(|e| e.to_string())
}

fn walk(dir: &Path, depth: u32, skip: &[String], out: &mut Vec<Entry>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut subdirs = Vec::new();
    for e in read.flatten() {
        if out.len() >= MAX_ENTRIES {
            return;
        }
        let Ok(meta) = e.metadata() else { continue };
        let name = e.file_name().to_string_lossy().to_string();
        // Liens symboliques / jonctions : listés, jamais suivis (ils peuvent sortir du dossier).
        let link = e.file_type().map(|t| t.is_symlink()).unwrap_or(true);
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
        out.push(Entry { path: plain(&e.path()), name: name.clone(), dir: meta.is_dir(), size: if meta.is_dir() { 0 } else { meta.len() }, modified });
        if meta.is_dir() && !link && !skip.contains(&name.to_lowercase()) {
            subdirs.push(e.path());
        }
    }
    if depth > 1 {
        for d in subdirs {
            walk(&d, depth - 1, skip, out);
        }
    }
}

/// Fichier texte (2 Mo au plus).
#[tauri::command]
pub fn ext2_files_read(app: AppHandle, id: String, path: String) -> Result<String, String> {
    require(&app, &id, "files")?;
    let p = within(&app, &id, &path)?;
    if std::fs::metadata(&p).map(|m| !m.is_file() || m.len() > 2 << 20).unwrap_or(true) {
        return Err("tooLarge".into());
    }
    std::fs::read_to_string(p).map_err(|e| e.to_string())
}

/// Extensions qui exécutent quelque chose quand on les « ouvre ».
const RUNNABLE: &[&str] = &["exe", "com", "bat", "cmd", "ps1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "msi", "msc", "scr", "lnk", "url", "pif", "cpl", "hta", "reg", "jar"];

/// Ouvre un fichier ou un dossier avec son application (.uproject → Unreal, .sln → Visual
/// Studio…). Un exécutable demande la permission `terminal`.
#[tauri::command]
pub fn ext2_files_open(app: AppHandle, id: String, path: String) -> Result<(), String> {
    require(&app, &id, "files")?;
    let p = within(&app, &id, &path)?;
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if p.is_file() && RUNNABLE.contains(&ext.as_str()) {
        require(&app, &id, "terminal")?;
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(plain(&p), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ext2_files_reveal(app: AppHandle, id: String, path: String) -> Result<(), String> {
    require(&app, &id, "files")?;
    let p = within(&app, &id, &path)?;
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(plain(&p)).map_err(|e| e.to_string())
}

// --- Terminal ------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RunSpec {
    cwd: String,
    /// Vide (`terminal.run`) : ouvre seulement le shell dans le dossier.
    #[serde(default)]
    command: String,
    title: Option<String>,
    /// `default` (profil par défaut de Windows Terminal), `cmd`, `powershell`, `pwsh`,
    /// `gitbash`, `wsl`.
    shell: Option<String>,
}

fn checked(app: &AppHandle, id: &str, spec: &RunSpec, allow_empty: bool) -> Result<PathBuf, String> {
    require(app, id, "terminal")?;
    if (!allow_empty && spec.command.trim().is_empty()) || spec.command.len() > 6000 || spec.command.contains(['\r', '\n']) {
        return Err("invalidCommand".into());
    }
    let cwd = within(app, id, &spec.cwd)?;
    if !cwd.is_dir() {
        return Err("notFound".into());
    }
    Ok(cwd)
}

/// Lance `command` dans un nouvel onglet de Windows Terminal, avec le shell choisi (profil par
/// défaut de Windows Terminal si `default`), dans `cwd` ; l'onglet reste ouvert à la fin.
#[tauri::command]
pub fn ext2_terminal_run(app: AppHandle, id: String, spec: RunSpec) -> Result<(), String> {
    let cwd = checked(&app, &id, &spec, true)?;
    let shown = if spec.command.trim().is_empty() { "Terminal" } else { &spec.command };
    let title: String = spec.title.as_deref().unwrap_or(shown).chars().filter(|c| !c.is_control() && !"\"&|<>^%;".contains(*c)).take(60).collect();
    run_visible(&cwd, spec.command.trim(), &title, spec.shell.as_deref().unwrap_or("default"))
}

/// Lance `command` sans fenêtre de console (ex. `code .`), dans `cwd`.
#[tauri::command]
pub fn ext2_terminal_launch(app: AppHandle, id: String, spec: RunSpec) -> Result<(), String> {
    let cwd = checked(&app, &id, &spec, false)?;
    let mut cmd = std::process::Command::new("cmd");
    fresh_env(&mut cmd);
    cmd.current_dir(&cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("/d /c {}", spec.command)).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    cmd.args(["-c", &spec.command]);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Debug, PartialEq)]
enum Shell {
    Cmd,
    PowerShell,
    Pwsh,
    /// Chemin de bash.exe (Git for Windows).
    Bash(String),
    /// Distribution WSL (None = celle par défaut).
    Wsl(Option<String>),
}

impl Shell {
    /// Programme et arguments (ligne brute) : `command`, puis le shell reste ouvert.
    fn line(&self, command: &str) -> (String, String) {
        let bash = |c: &str| c.replace('\\', "\\\\").replace('"', "\\\"");
        let empty = command.is_empty();
        match self {
            Shell::Cmd => ("cmd.exe".into(), if empty { String::new() } else { format!("/k {command}") }),
            Shell::PowerShell => ("powershell.exe".into(), if empty { "-NoLogo".into() } else { format!("-NoLogo -NoExit -Command {command}") }),
            Shell::Pwsh => ("pwsh.exe".into(), if empty { "-NoLogo".into() } else { format!("-NoLogo -NoExit -Command {command}") }),
            Shell::Bash(path) => (path.clone(), if empty { "-l -i".into() } else { format!("-l -i -c \"{}; exec bash -l -i\"", bash(command)) }),
            Shell::Wsl(distro) => {
                let d = distro.as_ref().map(|d| format!("-d \"{d}\" ")).unwrap_or_default();
                ("wsl.exe".into(), if empty { d.trim_end().to_string() } else { format!("{d}-- bash -lic \"{}; exec bash\"", bash(command)) })
            }
        }
    }

    fn named(name: &str) -> Option<Shell> {
        Some(match name {
            "cmd" => Shell::Cmd,
            "powershell" => Shell::PowerShell,
            "pwsh" => Shell::Pwsh,
            "gitbash" => Shell::Bash(git_bash()?),
            "wsl" => Shell::Wsl(None),
            _ => return None,
        })
    }
}

/// bash.exe de Git for Windows.
fn git_bash() -> Option<String> {
    let env = |k: &str| std::env::var(k).ok();
    [env("ProgramFiles"), env("ProgramW6432"), env("ProgramFiles(x86)"), env("LOCALAPPDATA").map(|l| format!(r"{l}\Programs"))]
        .into_iter()
        .flatten()
        .map(|base| format!(r"{base}\Git\bin\bash.exe"))
        .find(|p| Path::new(p).is_file())
}

/// Retire les commentaires `//` et `/* */` (les réglages de Windows Terminal sont du JSONC).
fn strip_jsonc(src: &str) -> String {
    let (mut out, mut chars) = (String::with_capacity(src.len()), src.chars().peekable());
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                out.extend(chars.next());
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            while chars.peek().is_some_and(|&n| n != '\n') {
                chars.next();
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            for n in chars.by_ref() {
                if prev == '*' && n == '/' {
                    break;
                }
                prev = n;
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Profils d'un fichier de réglages ou d'un fragment (`profiles` : tableau, ou `{ list }`).
fn profiles_of(v: &serde_json::Value) -> Vec<serde_json::Value> {
    let p = &v["profiles"];
    p.as_array().or_else(|| p["list"].as_array()).cloned().unwrap_or_default()
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&strip_jsonc(&std::fs::read_to_string(path).ok()?)).ok()
}

/// Profil par défaut de Windows Terminal : son nom (pour `-p`) et son shell.
fn wt_default() -> Option<(String, Shell)> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let settings = [
        format!(r"{local}\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"),
        format!(r"{local}\Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\settings.json"),
        format!(r"{local}\Microsoft\Windows Terminal\settings.json"),
    ];
    let v = settings.iter().find_map(|p| read_json(Path::new(p)))?;
    let guid = v["defaultProfile"].as_str()?.to_lowercase();
    let same = |p: &serde_json::Value| p["guid"].as_str().is_some_and(|g| g.to_lowercase() == guid);
    let mut profile = profiles_of(&v).into_iter().find(same)?;
    // Profil venu d'un fragment (Git Bash…) : sa ligne de commande est dans le fragment.
    if profile["commandline"].as_str().is_none() {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
        for dir in [format!(r"{program_data}\Microsoft\Windows Terminal\Fragments"), format!(r"{local}\Microsoft\Windows Terminal\Fragments")] {
            let files = std::fs::read_dir(&dir).into_iter().flatten().flatten().flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten());
            let found = files.filter_map(|f| read_json(&f.path())).flat_map(|v| profiles_of(&v)).find(same);
            if let Some(cmd) = found.and_then(|p| p["commandline"].as_str().map(String::from)) {
                profile["commandline"] = cmd.into();
                break;
            }
        }
    }
    let name = profile["name"].as_str().unwrap_or("").to_string();
    let shell = classify(profile["commandline"].as_str().unwrap_or(""), profile["source"].as_str().unwrap_or(""), &name);
    Some((name, shell))
}

/// Shell d'après la ligne de commande (ou la source) d'un profil.
fn classify(commandline: &str, source: &str, name: &str) -> Shell {
    let low = commandline.to_lowercase();
    if let Some(i) = low.find("bash.exe") {
        return Shell::Bash(commandline[..i + "bash.exe".len()].trim_start_matches(['"', ' ']).to_string());
    }
    if source.to_lowercase().contains("wsl") {
        return Shell::Wsl(Some(name.to_string()).filter(|n| !n.is_empty()));
    }
    if low.contains("wsl.exe") {
        return Shell::Wsl(None);
    }
    if low.contains("pwsh") || source == "Windows.Terminal.PowershellCore" {
        return Shell::Pwsh;
    }
    if low.contains("powershell") {
        return Shell::PowerShell;
    }
    Shell::Cmd
}

#[cfg(windows)]
fn run_visible(cwd: &Path, command: &str, title: &str, shell: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let dir = plain(cwd);
    // Shell : celui demandé, sinon celui du profil par défaut de Windows Terminal (dont on
    // garde l'apparence avec `-p`).
    let (profile, chosen) = match Shell::named(shell) {
        Some(s) => (None, s),
        None if shell == "gitbash" => return Err("gitBashNotFound".into()),
        None => wt_default().map(|(n, s)| (Some(n), s)).unwrap_or((None, Shell::Cmd)),
    };
    let mut wt = std::process::Command::new("wt.exe");
    fresh_env(&mut wt);
    wt.args(["-w", "0", "new-tab", "--title", title, "-d", &dir]);
    if let Some(p) = profile.filter(|p| !p.is_empty()) {
        wt.args(["-p", &p]);
    }
    // Rien à lancer avec le profil par défaut : l'onglet tel quel.
    if !(command.is_empty() && profile_only(shell)) {
        let (program, args) = chosen.line(command);
        // Windows Terminal : `;` sépare ses propres commandes, on l'échappe.
        wt.raw_arg(format!("\"{program}\" {}", args.replace(';', "\\;")));
    }
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let (program, args) = chosen.line(command);
    let mut console = std::process::Command::new(program);
    fresh_env(&mut console);
    console
        .current_dir(cwd)
        .raw_arg(args)
        .creation_flags(0x0000_0010) // CREATE_NEW_CONSOLE
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Environnement d'un programme ouvert depuis l'Explorateur, relu dans le registre
/// (`CreateEnvironmentBlock`) : celui d'Island peut dater de son lancement (PATH modifié
/// depuis, installation de pnpm…) ou venir d'un parent qui n'avait pas le PATH utilisateur.
/// Windows Terminal donne l'environnement de l'appelant au nouvel onglet. À défaut, on
/// garde celui d'Island.
#[cfg(windows)]
fn fresh_env(cmd: &mut std::process::Command) {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{TOKEN_DUPLICATE, TOKEN_QUERY};
    use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let vars = unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut token).is_err() {
            return;
        }
        let mut block = std::ptr::null_mut();
        let made = CreateEnvironmentBlock(&mut block, Some(token), false);
        let _ = CloseHandle(token);
        if made.is_err() || block.is_null() {
            return;
        }
        // Bloc UTF-16 : `NOM=valeur\0` … `\0`.
        let mut vars = Vec::new();
        let mut p = block as *const u16;
        loop {
            let len = (0..).take_while(|&i| *p.add(i) != 0).count();
            if len == 0 {
                break;
            }
            let entry = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            // Les entrées `=C:=C:\…` (dossier courant par lecteur) commencent par `=`.
            if let Some((k, v)) = entry.get(1..).and_then(|rest| rest.split_once('=')).map(|(k, v)| (format!("{}{k}", &entry[..1]), v.to_string())) {
                vars.push((k, v));
            }
            p = p.add(len + 1);
        }
        let _ = DestroyEnvironmentBlock(block);
        vars
    };
    if vars.iter().any(|(k, _)| k.eq_ignore_ascii_case("path")) {
        cmd.env_clear().envs(vars);
    }
}

#[cfg(not(windows))]
fn fresh_env(_cmd: &mut std::process::Command) {}

/// Shell non nommé : le profil par défaut de Windows Terminal.
fn profile_only(shell: &str) -> bool {
    Shell::named(shell).is_none() && shell != "gitbash"
}

#[cfg(not(windows))]
fn run_visible(_cwd: &Path, _command: &str, _title: &str, _shell: &str) -> Result<(), String> {
    Err("unsupported".into())
}

// --- Docker Engine ------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct DockerRequest {
    method: String,
    path: String,
    body: Option<String>,
}

#[derive(Serialize)]
pub struct DockerResponse {
    status: u16,
    body: String,
}

const DOCKER_MAX: usize = 16 << 20;

/// Refuse les requêtes qui ne finissent jamais (flux) ou mal formées.
fn docker_path_ok(path: &str) -> bool {
    let low = path.to_ascii_lowercase();
    let (route, query) = low.split_once('?').unwrap_or((&low, ""));
    path.starts_with('/')
        && path.len() <= 2000
        && path.bytes().all(|b| b.is_ascii_graphic())
        && !route.ends_with("/attach")
        && !route.ends_with("/attach/ws")
        && !route.ends_with("/events")
        && !route.ends_with("/wait")
        && !query.split('&').any(|kv| kv == "follow=true" || kv == "follow=1")
        && (!route.ends_with("/stats") || query.split('&').any(|kv| kv == "stream=false" || kv == "stream=0"))
}

/// Requête vers le moteur Docker local (`/containers/json?all=1`, `/containers/<id>/restart`…).
/// `dockerUnavailable` si Docker n'est pas lancé.
#[tauri::command]
pub async fn ext2_docker(app: AppHandle, id: String, req: DockerRequest) -> Result<DockerResponse, String> {
    require(&app, &id, "docker")?;
    let method = req.method.to_ascii_uppercase();
    if !["GET", "POST", "DELETE", "PUT", "HEAD"].contains(&method.as_str()) || !docker_path_ok(&req.path) {
        return Err("invalidRequest".into());
    }
    let body = req.body.unwrap_or_default();
    if body.len() > 1 << 20 {
        return Err("tooLarge".into());
    }
    tauri::async_runtime::spawn_blocking(move || docker_call(&method, &req.path, &body)).await.map_err(|e| e.to_string())?
}

fn docker_call(method: &str, path: &str, body: &str) -> Result<DockerResponse, String> {
    let mut conn = docker_connect().ok_or("dockerUnavailable")?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: docker\r\nUser-Agent: Island\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    conn.write_all(head.as_bytes()).and_then(|_| conn.write_all(body.as_bytes())).map_err(|_| "dockerUnavailable".to_string())?;
    let mut raw = Vec::new();
    conn.take(DOCKER_MAX as u64 + 1).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    if raw.len() > DOCKER_MAX {
        return Err("tooLarge".into());
    }
    parse_http(&raw)
}

#[cfg(windows)]
fn docker_connect() -> Option<std::fs::File> {
    // Le moteur sert une instance du tube à la fois : on réessaie un peu s'il est occupé.
    for _ in 0..10 {
        match std::fs::OpenOptions::new().read(true).write(true).open(r"\\.\pipe\docker_engine") {
            Ok(f) => return Some(f),
            Err(e) if e.raw_os_error() == Some(231) => std::thread::sleep(std::time::Duration::from_millis(50)), // ERROR_PIPE_BUSY
            Err(_) => return None,
        }
    }
    None
}

#[cfg(unix)]
fn docker_connect() -> Option<std::os::unix::net::UnixStream> {
    let s = std::os::unix::net::UnixStream::connect("/var/run/docker.sock").ok()?;
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    Some(s)
}

/// Réponse HTTP/1.1 brute → statut + corps (dé-découpé si `chunked`).
fn parse_http(raw: &[u8]) -> Result<DockerResponse, String> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("badResponse")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut rest = &raw[split + 4..];
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("badResponse")?;
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        let mut out = Vec::new();
        loop {
            let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") else { break };
            let size = usize::from_str_radix(String::from_utf8_lossy(&rest[..eol]).split(';').next().unwrap_or("").trim(), 16).map_err(|_| "badResponse")?;
            rest = &rest[eol + 2..];
            if size == 0 || rest.len() < size {
                out.extend_from_slice(&rest[..size.min(rest.len())]);
                break;
            }
            out.extend_from_slice(&rest[..size]);
            rest = rest.get(size + 2..).unwrap_or(&[]);
        }
        out
    } else {
        rest.to_vec()
    };
    Ok(DockerResponse { status, body: String::from_utf8_lossy(&body).into_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_plain_and_chunked() {
        let r = parse_http(b"HTTP/1.1 204 No Content\r\nServer: Docker\r\n\r\n").unwrap();
        assert_eq!((r.status, r.body.as_str()), (204, ""));
        let r = parse_http(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n[{\"a\r\n4\r\n\":1}\r\n1\r\n]\r\n0\r\n\r\n").unwrap();
        assert_eq!((r.status, r.body.as_str()), (200, "[{\"a\":1}]"));
    }

    #[test]
    fn shells_from_terminal_profiles() {
        let git = classify(r#""C:/Program Files/Git/bin/bash.exe" -i -l"#, "Git", "Git Bash");
        assert_eq!(git, Shell::Bash("C:/Program Files/Git/bin/bash.exe".into()));
        assert_eq!(classify(r"%SystemRoot%\System32\cmd.exe", "", "Invite de commandes"), Shell::Cmd);
        assert_eq!(classify("", "Microsoft.WSL", "Ubuntu"), Shell::Wsl(Some("Ubuntu".into())));
        assert_eq!(classify("", "Windows.Terminal.PowershellCore", "PowerShell"), Shell::Pwsh);
        assert_eq!(git.line("pnpm run dev").1, r#"-l -i -c "pnpm run dev; exec bash -l -i""#);
        let v: serde_json::Value = serde_json::from_str(&strip_jsonc("{ // x\n \"a\": \"http://b\" /* c */ }")).unwrap();
        assert_eq!(v["a"], "http://b");
    }

    /// Environnement frais : PATH utilisateur présent même si ce processus ne l'a pas.
    #[cfg(windows)]
    #[test]
    fn fresh_env_has_user_path() {
        let mut cmd = std::process::Command::new("cmd");
        cmd.env("PATH", r"C:\Windows\System32");
        fresh_env(&mut cmd);
        let path = cmd.get_envs().find(|(k, _)| k.eq_ignore_ascii_case("path")).and_then(|(_, v)| v).unwrap().to_string_lossy().to_lowercase();
        let user = std::env::var("USERPROFILE").unwrap().to_lowercase();
        assert!(path.contains(&user), "{path}");
    }

    /// Profil par défaut réel de ce poste : `cargo test wt_default -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn wt_default_on_this_machine() {
        println!("{:?}", wt_default());
    }

    #[test]
    fn streaming_docker_paths_are_refused() {
        assert!(docker_path_ok("/containers/json?all=1"));
        assert!(docker_path_ok("/containers/abc/restart"));
        assert!(docker_path_ok("/containers/abc/stats?stream=false"));
        assert!(!docker_path_ok("/containers/abc/stats"));
        assert!(!docker_path_ok("/containers/abc/logs?follow=true&stdout=1"));
        assert!(!docker_path_ok("/containers/abc/attach"));
        assert!(!docker_path_ok("/events"));
        assert!(!docker_path_ok("containers/json"));
        assert!(!docker_path_ok("/containers/json HTTP/1.1\r\nX: y"));
    }
}
