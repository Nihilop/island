// Installation des extensions v2 : paquets `.island`, signature, permissions consenties,
// désinstallation, mode développeur.
//
// Paquet = zip : `manifest.json` + fichiers de l'extension + `ISLAND-SIGNATURE.json`
// (`{ key, sig }`, base64). La signature ed25519 porte sur l'EMPREINTE du contenu : les
// lignes `<sha256 hex>  <chemin>\n` de toutes les autres entrées, triées par chemin.
// Fabriqué et signé par `packages/sdk/bin/island.mjs pack`.
//
// Installation normale : signature valide ET clé officielle (`OFFICIAL_KEYS`). Mode
// développeur : un paquet non signé ou signé par une autre clé est accepté après un
// avertissement explicite, et des dossiers non empaquetés peuvent être chargés.
// Les permissions CONSENTIES sont enregistrées hors du dossier de l'extension
// (`extensions-v2-state/<id>.json`) : c'est elles qui font foi, pas le manifeste.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use super::{root, valid_id, valid_rel, Manifest};

/// Clés publiques ed25519 (base64) des paquets officiels. La clé privée correspondante
/// n'est PAS dans le dépôt (créée par `island keygen`, gardée par l'auteur d'Island).
const OFFICIAL_KEYS: &[&str] = &["IrXutmIS7EwKdJ61YF0Tw2ECeoFjXKLJE9pnCcDTxj4="];

pub const SIGNATURE_FILE: &str = "ISLAND-SIGNATURE.json";
const MAX_ENTRIES: usize = 2000;
const MAX_TOTAL: u64 = 64 << 20;

// --- État par extension (permissions consenties, secrets créés) -------------------------

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    /// Permissions acceptées à l'installation.
    #[serde(default)]
    pub granted: Vec<String>,
    /// Clés de secrets créées (pour les effacer à la désinstallation).
    #[serde(default)]
    pub secrets: BTreeSet<String>,
}

fn state_path(app: &AppHandle, id: &str) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("extensions-v2-state").join(format!("{id}.json"))
}

pub fn state(app: &AppHandle, id: &str) -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(state_path(app, id)).ok()?).ok()
}

pub fn save_state(app: &AppHandle, id: &str, s: &State) {
    let p = state_path(app, id);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(s).unwrap_or_default());
}

// --- Mode développeur ---------------------------------------------------------------------

struct Dev {
    enabled: bool,
    folders: Vec<PathBuf>,
}

static DEV: Mutex<Dev> = Mutex::new(Dev { enabled: false, folders: Vec::new() });

/// Dossier de développement qui porte l'extension `id` (mode développeur actif).
pub fn dev_dir(id: &str) -> Option<PathBuf> {
    let dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
    if !dev.enabled {
        return None;
    }
    dev.folders.iter().find(|f| read_manifest(f).is_some_and(|m| m.id == id)).cloned()
}

pub fn dev_folders() -> Vec<PathBuf> {
    let dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
    if dev.enabled { dev.folders.clone() } else { Vec::new() }
}

pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    let m: Manifest = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).ok()?).ok()?;
    m.validate().is_ok().then_some(m)
}

/// Réglage du mode développeur (poussé par le front).
#[tauri::command]
pub fn ext2_set_dev(enabled: bool, folders: Vec<String>) {
    let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
    dev.enabled = enabled;
    dev.folders = folders.into_iter().map(PathBuf::from).filter(|p| p.is_absolute()).collect();
}

/// Manifeste d'un dossier (pour l'ajouter en mode développeur).
#[tauri::command]
pub fn ext2_dev_inspect(folder: String) -> Result<Manifest, String> {
    let dir = PathBuf::from(&folder);
    let txt = std::fs::read_to_string(dir.join("manifest.json")).map_err(|_| "noManifest")?;
    let m: Manifest = serde_json::from_str(&txt).map_err(|e| format!("manifest:{e}"))?;
    m.validate()?;
    Ok(m)
}

// --- Lecture et vérification d'un paquet ------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Signature {
    /// Signé par une clé officielle.
    Official,
    /// Signature valide, mais clé inconnue.
    Untrusted,
    Unsigned,
    /// Signature présente mais fausse (paquet modifié) : jamais installable.
    Invalid,
}

struct Package {
    manifest: Manifest,
    files: Vec<(String, Vec<u8>)>,
    signature: Signature,
    key: Option<String>,
}

#[derive(Deserialize)]
struct SigFile {
    key: String,
    sig: String,
}

/// Empreinte signée : `<sha256 hex>  <chemin>\n`, triée par chemin.
pub fn digest(files: &[(String, Vec<u8>)]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .filter(|(p, _)| p != SIGNATURE_FILE)
        .map(|(p, b)| format!("{}  {p}\n", hex(&Sha256::digest(b))))
        .collect();
    lines.sort_by(|a, b| a[66..].cmp(&b[66..]));
    lines.concat()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn verify(files: &[(String, Vec<u8>)]) -> (Signature, Option<String>) {
    use ed25519_dalek::{Signature as Sig, Verifier, VerifyingKey};
    let Some((_, raw)) = files.iter().find(|(p, _)| p == SIGNATURE_FILE) else { return (Signature::Unsigned, None) };
    let b64 = base64::engine::general_purpose::STANDARD;
    let check = || -> Option<bool> {
        let s: SigFile = serde_json::from_slice(raw).ok()?;
        let key: [u8; 32] = b64.decode(&s.key).ok()?.try_into().ok()?;
        let sig: [u8; 64] = b64.decode(&s.sig).ok()?.try_into().ok()?;
        let vk = VerifyingKey::from_bytes(&key).ok()?;
        Some(vk.verify(digest(files).as_bytes(), &Sig::from_bytes(&sig)).is_ok())
    };
    let key = serde_json::from_slice::<SigFile>(raw).ok().map(|s| s.key);
    match check() {
        Some(true) if key.as_deref().is_some_and(|k| OFFICIAL_KEYS.contains(&k)) => (Signature::Official, key),
        Some(true) => (Signature::Untrusted, key),
        _ => (Signature::Invalid, key),
    }
}

fn read_package(path: &str) -> Result<Package, String> {
    if !path.to_lowercase().ends_with(".island") {
        return Err("notIsland".into());
    }
    let file = std::fs::File::open(path).map_err(|_| "notFound")?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "corrupt")?;
    if zip.len() > MAX_ENTRIES {
        return Err("tooLarge".into());
    }
    let mut files = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|_| "corrupt")?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if !valid_rel(&name) || entry.enclosed_name().is_none() {
            return Err(format!("badPath:{name}"));
        }
        total += entry.size();
        if total > MAX_TOTAL {
            return Err("tooLarge".into());
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.by_ref().take(MAX_TOTAL).read_to_end(&mut buf).map_err(|_| "corrupt")?;
        files.push((name, buf));
    }
    let (_, raw) = files.iter().find(|(p, _)| p == "manifest.json").ok_or("noManifest")?;
    let manifest: Manifest = serde_json::from_slice(raw).map_err(|e| format!("manifest:{e}"))?;
    manifest.validate()?;
    // Tout ce que le manifeste référence doit être dans le paquet.
    let has = |p: &str| files.iter().any(|(n, _)| n == p);
    let mut needed = vec![manifest.main.clone()];
    needed.extend(manifest.contributes.tools.iter().map(|t| t.view.clone()));
    if let Some(missing) = needed.iter().find(|p| !has(p)) {
        return Err(format!("missingFile:{missing}"));
    }
    let (signature, key) = verify(&files);
    Ok(Package { manifest, files, signature, key })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    from_version: String,
    /// Permissions demandées en plus de celles déjà accordées.
    new_permissions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspect {
    manifest: Manifest,
    signature: Signature,
    /// Empreinte courte de la clé (8 caractères) si signé.
    publisher: Option<String>,
    size: u64,
    update: Option<Update>,
}

/// Examine un paquet avant consentement.
#[tauri::command]
pub async fn ext2_inspect(app: AppHandle, path: String) -> Result<Inspect, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pkg = read_package(&path)?;
        let size = pkg.files.iter().map(|(_, b)| b.len() as u64).sum();
        let update = super::read_manifest_at(&root(&app).join(&pkg.manifest.id)).map(|old| {
            let granted = state(&app, &old.id).map(|s| s.granted).unwrap_or(old.permissions);
            Update {
                from_version: old.version,
                new_permissions: pkg.manifest.permissions.iter().filter(|p| !granted.contains(p)).cloned().collect(),
            }
        });
        Ok(Inspect {
            publisher: pkg.key.as_ref().map(|k| hex(&Sha256::digest(k.as_bytes()))[..8].to_string()),
            manifest: pkg.manifest,
            signature: pkg.signature,
            size,
            update,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Installe (ou met à jour) un paquet. `allow_untrusted` : consentement explicite à un
/// paquet non officiel, accepté seulement en mode développeur.
#[tauri::command]
pub async fn ext2_install(app: AppHandle, path: String, allow_untrusted: bool) -> Result<Manifest, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pkg = read_package(&path)?;
        let dev = DEV.lock().unwrap_or_else(|p| p.into_inner()).enabled;
        match pkg.signature {
            Signature::Official => {}
            Signature::Invalid => return Err("invalidSignature".into()),
            _ if allow_untrusted && dev => {}
            _ => return Err("unsigned".into()),
        }
        let id = pkg.manifest.id.clone();
        let base = root(&app);
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        // Extraction dans un dossier temporaire, puis remplacement d'un coup.
        let tmp = base.join(format!(".tmp-{id}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        for (name, bytes) in &pkg.files {
            let dest = tmp.join(name);
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
        }
        let target = base.join(&id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
        let mut st = state(&app, &id).unwrap_or_default();
        st.granted = pkg.manifest.permissions.clone();
        save_state(&app, &id, &st);
        #[cfg(target_os = "windows")]
        let _ = register_island_assoc(&app);
        Ok(pkg.manifest)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Désinstalle : dossier, permissions, stockage et secrets de l'extension.
#[tauri::command]
pub fn ext2_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("invalidId".into());
    }
    let dir = root(&app).join(&id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    if let Some(st) = state(&app, &id) {
        for key in st.secrets {
            let _ = super::services::secret_delete(&id, &key);
        }
    }
    super::services::forget_cookies(&id);
    let _ = std::fs::remove_file(state_path(&app, &id));
    if let Ok(cfg) = app.path().app_config_dir() {
        let _ = std::fs::remove_file(cfg.join("storage").join(format!("ext2.{id}.json")));
    }
    Ok(())
}

// --- Ouverture d'un .island (double-clic, glisser sur le notch) -------------------------

static PENDING: Mutex<Option<String>> = Mutex::new(None);

/// Repère un `.island` dans l'argv : mémorisé (démarrage) et signalé au front.
pub fn handle_argv(app: &AppHandle, argv: &[String]) {
    let Some(path) = argv.iter().find(|a| a.to_lowercase().ends_with(".island")) else { return };
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some(path.clone());
    let _ = app.emit("ext2://install", path.clone());
}

/// Paquet ouvert au démarrage (le front le récupère une fois prêt).
#[tauri::command]
pub fn ext2_take_pending() -> Option<String> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner()).take()
}

// --- Association de fichier .island (Windows, HKCU, sans admin) --------------------

/// Écrit l'association `.island → Island` dans HKCU (per-utilisateur, sans admin).
#[cfg(target_os = "windows")]
pub(crate) fn register_island_assoc(app: &AppHandle) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    // Icône dédiée aux .island si elle est bundlée (resource), sinon celle de l'exe.
    let icon = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("icons").join("island-file.ico"))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{exe},0"));

    let classes = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Classes", KEY_READ | KEY_WRITE)
        .map_err(|e| e.to_string())?;

    classes
        .create_subkey(".island")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &"Island.Extension")
        .map_err(|e| e.to_string())?;

    let prog = classes.create_subkey("Island.Extension").map_err(|e| e.to_string())?.0;
    prog.set_value("", &"Extension Island").map_err(|e| e.to_string())?;
    prog.create_subkey("DefaultIcon")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &icon)
        .map_err(|e| e.to_string())?;
    prog.create_subkey("shell\\open\\command")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &format!("\"{exe}\" \"%1\""))
        .map_err(|e| e.to_string())?;

    // Notifie le shell que les associations ont changé (prise en compte immédiate).
    unsafe {
        use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn files() -> Vec<(String, Vec<u8>)> {
        vec![("manifest.json".into(), b"{}".to_vec()), ("dist/main.js".into(), b"x".to_vec())]
    }

    fn signed(key: &SigningKey, files: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
        let b64 = base64::engine::general_purpose::STANDARD;
        let sig = key.sign(digest(files).as_bytes());
        let json = format!(r#"{{"key":"{}","sig":"{}"}}"#, b64.encode(key.verifying_key().to_bytes()), b64.encode(sig.to_bytes()));
        let mut out = files.to_vec();
        out.push((SIGNATURE_FILE.into(), json.into_bytes()));
        out
    }

    #[test]
    fn digest_is_sorted_and_skips_signature() {
        let d = digest(&files());
        assert!(d.find("dist/main.js").unwrap() < d.find("manifest.json").unwrap());
        assert_eq!(digest(&signed(&SigningKey::from_bytes(&[7; 32]), &files())), d);
    }

    /// Paquet réel : `ISLAND_PKG=<chemin> cargo test real_package -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_package() {
        let path = std::env::var("ISLAND_PKG").expect("ISLAND_PKG");
        let pkg = read_package(&path).expect("paquet illisible");
        println!("{} {} : {} fichiers, signature {:?}", pkg.manifest.id, pkg.manifest.version, pkg.files.len(), pkg.signature as u8);
        assert!(pkg.signature == Signature::Official);
    }

    #[test]
    fn signature_states() {
        assert!(verify(&files()).0 == Signature::Unsigned);
        let key = SigningKey::from_bytes(&[7; 32]);
        let ok = signed(&key, &files());
        assert!(verify(&ok).0 == Signature::Untrusted); // clé valide mais non officielle
        let mut tampered = ok.clone();
        tampered[1].1 = b"y".to_vec();
        assert!(verify(&tampered).0 == Signature::Invalid);
    }
}
