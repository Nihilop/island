// Pont vers le CLI officiel Bitwarden (`bw`). Le binaire est téléchargé à la demande depuis
// la release GitHub de Bitwarden (empreinte SHA-256 vérifiée), dans le dossier de données
// d'Island ; il tourne avec son propre dossier de config (isolé d'un éventuel `bw` déjà
// installé). Chaque opération = un process `bw` lancé depuis Rust (jamais `bw serve`, qui
// exposerait le coffre déverrouillé à tout process local). Le mot de passe maître passe
// par une variable d'environnement du process enfant, jamais par la ligne de commande.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

/// Sérialise les appels `bw` (ils partagent le même fichier de données).
static RUN: Mutex<()> = Mutex::new(());

pub fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("bitwarden")
}

pub fn exe(app: &AppHandle) -> PathBuf {
    data_dir(app).join(if cfg!(windows) { "bw.exe" } else { "bw" })
}

pub fn installed(app: &AppHandle) -> bool {
    exe(app).is_file()
}

pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Message d'erreur de `bw` (sur stderr ou stdout selon les versions).
    pub fn message(&self) -> String {
        let s = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        s.trim().to_string()
    }
}

/// Lance `bw <args>` en mode INTERACTIF (sans `--nointeraction`) : stdin/stdout/stderr
/// branchés, pour répondre aux invites (code 2FA, vérification de nouvel appareil) que
/// le CLI n'accepte pas en option.
pub fn spawn_interactive(app: &AppHandle, args: &[&str], envs: &[(&str, &str)]) -> Result<std::process::Child, String> {
    // Attend la fin d'un appel en cours (ex. `logout` lancé en arrière-plan).
    drop(RUN.lock().unwrap_or_else(|p| p.into_inner()));
    let mut cmd = Command::new(exe(app));
    cmd.args(args)
        .env("BITWARDENCLI_APPDATA_DIR", data_dir(app).join("data"))
        .env_remove("BW_NOINTERACTION")
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map_err(|e| format!("bw: {e}"))
}

/// Lance `bw <args>` avec `envs` (session, mot de passe) et `stdin` optionnel.
pub fn run(app: &AppHandle, args: &[&str], envs: &[(&str, &str)], stdin: Option<&str>) -> Result<Output, String> {
    let _guard = RUN.lock().unwrap_or_else(|p| p.into_inner());
    let dir = data_dir(app);
    let mut cmd = Command::new(exe(app));
    cmd.args(args)
        .arg("--nointeraction")
        .env("BITWARDENCLI_APPDATA_DIR", dir.join("data"))
        .env("BW_NOINTERACTION", "true")
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("bw: {e}"))?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        use std::io::Write;
        let _ = pipe.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| format!("bw: {e}"))?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

// --- Installation --------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    /// Empreinte calculée par GitHub (« sha256:… »), présente sur les releases récentes.
    digest: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().user_agent("Island (bitwarden-cli installer)").build()
}

/// Télécharge la dernière release CLI officielle, vérifie son SHA-256 et extrait `bw.exe`.
/// Progression : `vault://install { percent }`.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let body = agent()
        .get("https://api.github.com/repos/bitwarden/clients/releases?per_page=40")
        .call()
        .map_err(|_| "network")?
        .into_string()
        .map_err(|e| e.to_string())?;
    let releases: Vec<Release> = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let rel = releases
        .into_iter()
        .find(|r| r.tag_name.starts_with("cli-v") && !r.prerelease && !r.draft)
        .ok_or("bw:release du client introuvable")?;
    let zip = rel
        .assets
        .iter()
        .find(|a| a.name.starts_with(&format!("bw-{platform}-")) && a.name.ends_with(".zip"))
        .ok_or("bw:archive du client introuvable")?;
    // Empreinte attendue : celle que GitHub calcule pour l'archive, sinon le fichier
    // `bw-<os>-sha256-*.txt` des anciennes releases.
    let expected = match zip.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        Some(hex) => hex.to_lowercase(),
        None => {
            let sum = rel
                .assets
                .iter()
                .find(|a| a.name.starts_with(&format!("bw-{platform}-sha256-")))
                .ok_or("bw:empreinte du client introuvable")?;
            agent()
                .get(&sum.browser_download_url)
                .call()
                .map_err(|_| "network")?
                .into_string()
                .map_err(|e| e.to_string())?
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_lowercase()
        }
    };

    let resp = agent().get(&zip.browser_download_url).call().map_err(|_| "network")?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = [0u8; 64 * 1024];
    let mut last_pct = -1i64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if total > 0 {
            let pct = (bytes.len() as u64 * 100 / total) as i64;
            if pct != last_pct {
                last_pct = pct;
                let _ = app.emit("vault://install", pct);
            }
        }
    }

    let actual = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>();
    if actual != expected {
        return Err("bw:empreinte SHA-256 invalide".into());
    }

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let name = if cfg!(windows) { "bw.exe" } else { "bw" };
    let mut file = archive.by_name(name).map_err(|e| e.to_string())?;
    let dir = data_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{name}.part"));
    let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
    drop(out);
    std::fs::rename(&tmp, exe(app)).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(exe(app), std::fs::Permissions::from_mode(0o755));
    }
    let _ = app.emit("vault://install", 100);
    Ok(())
}
