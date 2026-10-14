// Windows Hello : service partagé par les outils (Coffre, Note…) et, plus tard, les
// extensions. Configuré UNE fois ; chaque outil s'y branche par une « portée » (scope).
//
// Principe :
// - Windows crée une clé RSA propre à Island dans le TPM (`KeyCredentialManager`). Elle ne
//   sort jamais de la puce et ne signe qu'après confirmation Hello (visage, empreinte ou
//   PIN Windows).
// - Signer un défi fixe donne toujours la même signature (RSA PKCS#1 v1.5) : son SHA-256
//   est la clé Hello `K`.
// - À la configuration, on tire une paire X25519 ; la clé privée est chiffrée par `K`
//   (AES-256-GCM), la publique reste en clair. Sceller une donnée n'utilise que la clé
//   publique (aucune invite) ; l'ouvrir exige Hello (K → clé privée → déchiffrement).
// - Si Hello est réinitialisé (nouvelle clé TPM), `K` change : les scellés deviennent
//   illisibles (`changed`) et on retombe sur la méthode normale (mot de passe…).
//
// Fichiers : `<app_data>/hello/key.json` (publique + privée chiffrée) et `<scope>.seal`.
// Erreurs = codes courts : notSupported, notEnrolled, canceled, changed, failed, notFound.

use std::path::PathBuf;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use x25519_dalek::{PublicKey, StaticSecret};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("hello")
}

fn key_path(app: &AppHandle) -> PathBuf {
    dir(app).join("key.json")
}

/// Portée → nom de fichier sûr.
fn seal_path(app: &AppHandle, scope: &str) -> PathBuf {
    let safe: String = scope.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    dir(app).join(format!("{safe}.seal"))
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("getrandom");
    b
}

fn aes_seal(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let nonce = random::<12>();
    let ct = Aes256Gcm::new(key.into()).encrypt(Nonce::from_slice(&nonce), plain).expect("aes-gcm");
    [nonce.as_slice(), &ct].concat()
}

fn aes_open(key: &[u8; 32], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < 12 + 16 {
        return None;
    }
    Aes256Gcm::new(key.into()).decrypt(Nonce::from_slice(&blob[..12]), &blob[12..]).ok()
}

/// Clé de scellement d'une portée : SHA-256(étiquette ‖ portée ‖ éphémère ‖ publique ‖ secret partagé).
fn seal_key(scope: &str, epk: &[u8; 32], pk: &[u8; 32], shared: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"island-hello-seal\0");
    h.update(scope.as_bytes());
    h.update([0]);
    h.update(epk);
    h.update(pk);
    h.update(shared);
    h.finalize().into()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct KeyFile {
    /// Clé publique X25519 (base64).
    pk: String,
    /// Clé privée X25519 chiffrée par K (base64 : nonce ‖ texte chiffré).
    sk: String,
}

fn read_key(app: &AppHandle) -> Option<([u8; 32], Vec<u8>)> {
    let f: KeyFile = serde_json::from_str(&std::fs::read_to_string(key_path(app)).ok()?).ok()?;
    let pk: [u8; 32] = B64.decode(f.pk).ok()?.try_into().ok()?;
    Some((pk, B64.decode(f.sk).ok()?))
}

/// Clé privée X25519, après confirmation Hello.
fn private_key(app: &AppHandle) -> Result<StaticSecret, String> {
    let (_, wrapped) = read_key(app).ok_or("notEnrolled")?;
    let k = imp::sign_key()?;
    let sk: [u8; 32] = aes_open(&k, &wrapped).ok_or("changed")?.try_into().map_err(|_| "changed")?;
    Ok(StaticSecret::from(sk))
}

// --- API interne (outils côté Rust) -------------------------------------------------

/// Hello est configuré sur ce PC (clé locale présente).
pub fn enrolled(app: &AppHandle) -> bool {
    read_key(app).is_some()
}

/// Scelle `data` pour `scope`, sans invite (clé publique seulement).
pub fn seal(app: &AppHandle, scope: &str, data: &[u8]) -> Result<(), String> {
    let (pk, _) = read_key(app).ok_or("notEnrolled")?;
    let esk = StaticSecret::from(random::<32>());
    let epk = PublicKey::from(&esk).to_bytes();
    let shared = esk.diffie_hellman(&PublicKey::from(pk)).to_bytes();
    let blob = [epk.as_slice(), &aes_seal(&seal_key(scope, &epk, &pk, &shared), data)].concat();
    std::fs::write(seal_path(app, scope), blob).map_err(|e| e.to_string())
}

/// Un scellé existe pour `scope`.
pub fn has(app: &AppHandle, scope: &str) -> bool {
    enrolled(app) && seal_path(app, scope).exists()
}

/// Ouvre le scellé de `scope` (invite Hello). Scellé illisible → supprimé + `changed`.
pub fn open(app: &AppHandle, scope: &str) -> Result<Vec<u8>, String> {
    let blob = std::fs::read(seal_path(app, scope)).map_err(|_| "notFound")?;
    if blob.len() < 32 {
        forget(app, scope);
        return Err("changed".into());
    }
    let (pk, _) = read_key(app).ok_or("notEnrolled")?;
    let sk = private_key(app)?;
    let epk: [u8; 32] = blob[..32].try_into().unwrap();
    let shared = sk.diffie_hellman(&PublicKey::from(epk)).to_bytes();
    aes_open(&seal_key(scope, &epk, &pk, &shared), &blob[32..]).ok_or_else(|| {
        forget(app, scope);
        "changed".to_string()
    })
}

/// Supprime le scellé de `scope` (écrasé avant suppression).
pub fn forget(app: &AppHandle, scope: &str) {
    let p = seal_path(app, scope);
    if let Ok(len) = std::fs::metadata(&p).map(|m| m.len()) {
        let _ = std::fs::write(&p, vec![0u8; len as usize]);
        let _ = std::fs::remove_file(p);
    }
}

/// Vérifie la présence de l'utilisateur (invite Hello) ; contrôle aussi que la clé TPM
/// est toujours celle de la configuration.
pub fn verify(app: &AppHandle) -> Result<(), String> {
    private_key(app).map(|_| ())
}

// --- Commandes ------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct Status {
    supported: bool,
    enrolled: bool,
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())? }
}

#[tauri::command]
pub async fn hello_status(app: AppHandle) -> Result<Status, String> {
    blocking(move || Ok(Status { supported: imp::supported(), enrolled: enrolled(&app) })).await
}

/// Configuration : crée la clé TPM (invite Hello), puis la paire de scellement.
#[tauri::command]
pub async fn hello_enroll(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        if !imp::supported() {
            return Err("notSupported".into());
        }
        imp::create()?;
        let k = imp::sign_key()?;
        let sk = StaticSecret::from(random::<32>());
        let file = KeyFile { pk: B64.encode(PublicKey::from(&sk).to_bytes()), sk: B64.encode(aes_seal(&k, &sk.to_bytes())) };
        // Nouvelle clé : les anciens scellés sont illisibles.
        let _ = std::fs::remove_dir_all(dir(&app));
        std::fs::create_dir_all(dir(&app)).map_err(|e| e.to_string())?;
        std::fs::write(key_path(&app), serde_json::to_string(&file).unwrap()).map_err(|e| e.to_string())
    })
    .await
}

/// Désactivation : supprime la clé TPM, la paire et tous les scellés.
#[tauri::command]
pub async fn hello_disable(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        if let Ok(rd) = std::fs::read_dir(dir(&app)) {
            for e in rd.flatten() {
                if let Some(scope) = e.path().file_stem().and_then(|s| s.to_str()).filter(|_| e.path().extension().is_some_and(|x| x == "seal")) {
                    forget(&app, scope);
                }
            }
        }
        let _ = std::fs::remove_dir_all(dir(&app));
        imp::delete();
        Ok(())
    })
    .await
}

/// Vérification seule (verrou de vue : Note, extensions…).
#[tauri::command]
pub async fn hello_verify(app: AppHandle) -> Result<(), String> {
    blocking(move || verify(&app)).await
}

// --- Implémentation native ----------------------------------------------------------

#[cfg(target_os = "windows")]
mod imp {
    use sha2::{Digest, Sha256};
    use windows::core::{h, Array, HSTRING};
    use windows::Security::Credentials::{KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus};
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};

    const NAME: &HSTRING = h!("Island");
    /// Défi fixe : la signature (déterministe) en dérive K.
    const CHALLENGE: &[u8] = b"island-hello-key-v1";

    fn status_err(s: KeyCredentialStatus) -> String {
        match s {
            KeyCredentialStatus::UserCanceled | KeyCredentialStatus::UserPrefersPassword => "canceled",
            KeyCredentialStatus::NotFound => "notEnrolled",
            KeyCredentialStatus::SecurityDeviceLocked => "locked",
            _ => "failed",
        }
        .into()
    }

    /// Le dialogue Hello s'ouvre souvent DERRIÈRE les fenêtres (app de bureau) : on le
    /// ramène au premier plan dès qu'il apparaît (≈ 3 s max).
    fn focus_prompt() {
        std::thread::spawn(|| {
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Ok(hwnd) = unsafe { FindWindowW(h!("Credential Dialog Xaml Host"), None) } {
                    if !hwnd.is_invalid() {
                        let _ = unsafe { SetForegroundWindow(hwnd) };
                        return;
                    }
                }
            }
        });
    }

    pub fn supported() -> bool {
        KeyCredentialManager::IsSupportedAsync().and_then(|op| op.get()).unwrap_or(false)
    }

    pub fn create() -> Result<(), String> {
        focus_prompt();
        let r = KeyCredentialManager::RequestCreateAsync(NAME, KeyCredentialCreationOption::ReplaceExisting)
            .and_then(|op| op.get())
            .map_err(|e| e.to_string())?;
        match r.Status().map_err(|e| e.to_string())? {
            KeyCredentialStatus::Success => Ok(()),
            s => Err(status_err(s)),
        }
    }

    /// Signe le défi (invite Hello) → K = SHA-256(signature).
    pub fn sign_key() -> Result<[u8; 32], String> {
        let open = KeyCredentialManager::OpenAsync(NAME).and_then(|op| op.get()).map_err(|e| e.to_string())?;
        match open.Status().map_err(|e| e.to_string())? {
            KeyCredentialStatus::Success => {}
            s => return Err(status_err(s)),
        }
        let cred = open.Credential().map_err(|e| e.to_string())?;
        let buf = CryptographicBuffer::CreateFromByteArray(CHALLENGE).map_err(|e| e.to_string())?;
        focus_prompt();
        let r = cred.RequestSignAsync(&buf).and_then(|op| op.get()).map_err(|e| e.to_string())?;
        match r.Status().map_err(|e| e.to_string())? {
            KeyCredentialStatus::Success => {}
            s => return Err(status_err(s)),
        }
        let mut sig = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&r.Result().map_err(|e| e.to_string())?, &mut sig).map_err(|e| e.to_string())?;
        Ok(Sha256::digest(&sig[..]).into())
    }

    pub fn delete() {
        let _ = KeyCredentialManager::DeleteAsync(NAME).and_then(|op| op.get());
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn supported() -> bool {
        false
    }
    pub fn create() -> Result<(), String> {
        Err("notSupported".into())
    }
    pub fn sign_key() -> Result<[u8; 32], String> {
        Err("notSupported".into())
    }
    pub fn delete() {}
}
