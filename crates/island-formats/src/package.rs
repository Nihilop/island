//! Paquets d'extension Island (`.island`, zip) : identifiants, chemins, manifeste, empreinte
//! et signature ed25519 (`ISLAND-SIGNATURE.json` : clé publique + signature de l'empreinte).
//! Partagé par l'app (installation) et le serveur du magasin (publication) : un paquet est
//! vérifié par les MÊMES règles des deux côtés. Les clés de confiance sont passées par
//! l'appelant (clé officielle dans l'app, clé du magasin côté serveur).

use std::io::{Read, Seek};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const SIGNATURE_FILE: &str = "ISLAND-SIGNATURE.json";
pub const MAX_ENTRIES: usize = 2000;
pub const MAX_TOTAL: u64 = 64 << 20;

/// Identifiant valide : nom de domaine inversé, `[a-z0-9-]` par segment, au moins deux
/// segments, pas de segment vide (donc ni `..` ni `.`).
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 100
        && parts.len() >= 2
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// Motif de domaine d'une permission (`network:`, `images:`) : `exemple.com` ou
/// `*.exemple.com`, caractères `[a-z0-9.-]` seulement (il finit dans une CSP).
pub fn valid_host_pattern(d: &str) -> bool {
    let host = d.strip_prefix("*.").unwrap_or(d);
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|l| !l.is_empty() && !l.starts_with('-') && l.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// Chemin relatif sûr (pas d'absolu, pas de `..`, pas de lecteur).
pub fn valid_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && !path.contains(':')
        && path.split(['/', '\\']).all(|s| !s.is_empty() && s != "." && s != "..")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDecl {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub view: String,
    #[serde(default = "default_size")]
    pub size: [u32; 2],
}

fn default_size() -> [u32; 2] {
    [520, 300]
}

/// Fenêtre d'extension (permission `windows`) : une page ouverte dans sa propre fenêtre.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowDecl {
    pub id: String,
    pub view: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_window_size")]
    pub size: [u32; 2],
    #[serde(default)]
    pub min_size: Option<[u32; 2]>,
}

fn default_window_size() -> [u32; 2] {
    [960, 600]
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributes {
    #[serde(default)]
    pub windows: Vec<WindowDecl>,
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
    #[serde(default)]
    pub live: Vec<Value>,
    #[serde(default)]
    pub launcher: Vec<Value>,
    #[serde(default)]
    pub commands: Vec<Value>,
    #[serde(default)]
    pub settings: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub main: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub locales: std::collections::BTreeMap<String, String>,
}

impl Manifest {
    /// Validation stricte : identifiant, chemins relatifs sûrs, tailles bornées.
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!("identifiant invalide : {}", self.id));
        }
        if self.name.trim().is_empty() || self.version.trim().is_empty() {
            return Err("nom ou version manquant".into());
        }
        let mut paths = vec![self.main.as_str()];
        paths.extend(self.icon.as_deref());
        paths.extend(self.contributes.tools.iter().map(|t| t.view.as_str()));
        paths.extend(self.contributes.tools.iter().filter_map(|t| t.icon.as_deref()));
        paths.extend(self.contributes.windows.iter().map(|w| w.view.as_str()));
        paths.extend(self.locales.values().map(String::as_str));
        if let Some(bad) = paths.into_iter().find(|p| !valid_rel(p)) {
            return Err(format!("chemin invalide : {bad}"));
        }
        for p in &self.permissions {
            let ok = match p.split_once(':') {
                Some(("network" | "images", d)) => valid_host_pattern(d),
                Some(_) => false,
                None => !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()),
            };
            if !ok {
                return Err(format!("permission invalide : {p}"));
            }
        }
        for w in &self.contributes.windows {
            if !valid_id(&format!("x.{}", w.id)) {
                return Err(format!("fenêtre invalide : {}", w.id));
            }
        }
        for t in &self.contributes.tools {
            if !valid_id(&format!("x.{}", t.id)) {
                return Err(format!("outil invalide : {}", t.id));
            }
        }
        Ok(())
    }
}

// --- Lecture et vérification d'un paquet ------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Signature {
    /// Signé par une clé de confiance.
    Official,
    /// Signature valide, mais clé inconnue.
    Untrusted,
    Unsigned,
    /// Signature présente mais fausse (paquet modifié) : jamais installable.
    Invalid,
}

/// Paquet lu et vérifié.
pub struct Package {
    pub manifest: Manifest,
    pub files: Vec<(String, Vec<u8>)>,
    pub signature: Signature,
    /// Clé publique du signataire (base64), si signé.
    pub key: Option<String>,
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

/// Empreinte courte d'une clé publique (8 caractères hex), affichée comme « éditeur ».
pub fn fingerprint(key: &str) -> String {
    hex(&Sha256::digest(key.as_bytes()))[..8].to_string()
}

/// État de signature d'un paquet ; `trusted` = clés publiques (base64) de confiance.
pub fn verify(files: &[(String, Vec<u8>)], trusted: &[&str]) -> (Signature, Option<String>) {
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
        Some(true) if key.as_deref().is_some_and(|k| trusted.contains(&k)) => (Signature::Official, key),
        Some(true) => (Signature::Untrusted, key),
        _ => (Signature::Invalid, key),
    }
}

/// Lit et vérifie un paquet `.island` (zip) : chemins sûrs, tailles bornées, manifeste
/// valide, fichiers référencés présents, signature (`trusted` = clés de confiance).
pub fn read<R: Read + Seek>(reader: R, trusted: &[&str]) -> Result<Package, String> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| "corrupt")?;
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
    let (signature, key) = verify(&files, trusted);
    Ok(Package { manifest, files, signature, key })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn ids() {
        assert!(valid_id("dev.island.github"));
        assert!(valid_id("com.example.my-ext"));
        assert!(!valid_id(".."));
        assert!(!valid_id("a..b"));
        assert!(!valid_id("single"));
        assert!(!valid_id("Dev.Island"));
        assert!(!valid_id("__app__.x"));
    }

    #[test]
    fn host_patterns() {
        assert!(valid_host_pattern("s4.anilist.co"));
        assert!(valid_host_pattern("*.example.org"));
        assert!(!valid_host_pattern("example"));
        assert!(!valid_host_pattern("a.com; script-src *"));
        assert!(!valid_host_pattern("A.com"));
        assert!(!valid_host_pattern("*.*.com"));
        assert!(!valid_host_pattern("x.com/path"));
    }

    #[test]
    fn rel_paths() {
        assert!(valid_rel("dist/main.js"));
        assert!(!valid_rel("../x"));
        assert!(!valid_rel("/etc/passwd"));
        assert!(!valid_rel("C:\\x"));
        assert!(!valid_rel("dist//x"));
        assert!(!valid_rel("dist/./x"));
    }

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

    #[test]
    fn signature_states() {
        assert!(verify(&files(), &[]).0 == Signature::Unsigned);
        let key = SigningKey::from_bytes(&[7; 32]);
        let ok = signed(&key, &files());
        assert!(verify(&ok, &[]).0 == Signature::Untrusted); // clé valide mais non officielle
        let mut tampered = ok.clone();
        tampered[1].1 = b"y".to_vec();
        assert!(verify(&tampered, &[]).0 == Signature::Invalid);
    }
}
