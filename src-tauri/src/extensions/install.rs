// Installation des extensions v2 : paquets `.island`, signature, permissions consenties,
// désinstallation, mode développeur.
//
// Paquet = zip : `manifest.json` + fichiers de l'extension + `ISLAND-SIGNATURE.json`
// (`{ key, sig }`, base64). La signature ed25519 porte sur l'EMPREINTE du contenu : les
// lignes `<sha256 hex>  <chemin>\n` de toutes les autres entrées, triées par chemin.
// Fabriqué et signé par `packages/sdk/bin/island.mjs pack`.
//
// Installation normale : signature valide ET clé officielle (`OFFICIAL_KEYS`), ou clé du
// magasin (`STORE_KEYS` : extension relue puis signée par sa modération). Mode
// développeur : un paquet non signé ou signé par une autre clé est accepté après un
// avertissement explicite, et des dossiers non empaquetés peuvent être chargés.
// Les permissions CONSENTIES sont enregistrées hors du dossier de l'extension
// (`extensions-v2-state/<id>.json`) : c'est elles qui font foi, pas le manifeste.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::{root, valid_id, Manifest};

/// Clés publiques ed25519 (base64) des paquets officiels. La clé privée correspondante
/// n'est PAS dans le dépôt (créée par `island keygen`, gardée par l'auteur d'Island).
const OFFICIAL_KEYS: &[&str] = &["IrXutmIS7EwKdJ61YF0Tw2ECeoFjXKLJE9pnCcDTxj4="];

/// Clés publiques du magasin officiel (`STORE_SIGNING_KEY` du serveur ; une liste pour la
/// rotation). La clé de production s'ajoute ici avant l'ouverture du magasin.
const STORE_KEYS: &[&str] = &[];

/// Clés du magasin reconnues : `STORE_KEYS`, celles données À LA COMPILATION
/// (`ISLAND_STORE_KEYS`, séparées par des virgules : magasin d'un fork), plus, en
/// DÉVELOPPEMENT seulement, la clé du serveur local (`crate::market::dev_store_key`).
fn store_keys() -> Vec<String> {
    let mut keys: Vec<String> = STORE_KEYS.iter().map(|k| k.to_string()).collect();
    keys.extend(option_env!("ISLAND_STORE_KEYS").unwrap_or("").split(',').map(str::trim).filter(|k| !k.is_empty()).map(String::from));
    keys.extend(crate::market::dev_store_key());
    keys
}


// --- État par extension (permissions consenties, secrets créés) -------------------------

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    /// Permissions acceptées à l'installation.
    #[serde(default)]
    pub granted: Vec<String>,
    /// Clés de secrets créées (pour les effacer à la désinstallation).
    #[serde(default)]
    pub secrets: BTreeSet<String>,
    /// Dossiers donnés par l'utilisateur (permission `files`).
    #[serde(default)]
    pub folders: Vec<String>,
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

// Lecture et vérification : `island_formats::package` (partagé avec le magasin) ; la clé
// officielle reste ici.
use island_formats::package::{Package, Signature};

fn read_package(path: &str) -> Result<Package, String> {
    if !path.to_lowercase().ends_with(".island") {
        return Err("notIsland".into());
    }
    let file = std::fs::File::open(path).map_err(|_| "notFound")?;
    let store = store_keys();
    let store: Vec<&str> = store.iter().map(String::as_str).collect();
    island_formats::package::read_with(file, OFFICIAL_KEYS, &store)
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
            publisher: pkg.key.as_deref().map(island_formats::package::fingerprint),
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
            Signature::Official | Signature::Store => {}
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
    crate::host::register_file_assoc(app, ".island", "Island.Extension", "Extension Island")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paquet réel : `ISLAND_PKG=<chemin> cargo test real_package -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_package() {
        let path = std::env::var("ISLAND_PKG").expect("ISLAND_PKG");
        let pkg = read_package(&path).expect("paquet illisible");
        println!("{} {} : {} fichiers, signature {:?}", pkg.manifest.id, pkg.manifest.version, pkg.files.len(), pkg.signature as u8);
        assert!(pkg.signature == Signature::Official);
    }
}
