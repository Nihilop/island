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
    /// Résumé, étiquettes, liens, licence (fiche du magasin).
    #[serde(flatten)]
    pub store: crate::media::StoreMeta,
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
    /// Signé par la clé du magasin : extension relue et approuvée par sa modération.
    Store,
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
    /// Médias de la fiche du magasin (`store/…`, images vérifiées).
    pub media: crate::media::Media,
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
    verify_with(files, trusted, &[])
}

/// Comme `verify`, avec en plus les clés du magasin (état `Store`).
pub fn verify_with(files: &[(String, Vec<u8>)], official: &[&str], store: &[&str]) -> (Signature, Option<String>) {
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
        Some(true) if key.as_deref().is_some_and(|k| official.contains(&k)) => (Signature::Official, key),
        Some(true) if key.as_deref().is_some_and(|k| store.contains(&k)) => (Signature::Store, key),
        Some(true) => (Signature::Untrusted, key),
        _ => (Signature::Invalid, key),
    }
}

/// Lit et vérifie un paquet `.island` (zip) : chemins sûrs, tailles bornées, manifeste
/// valide, fichiers référencés présents, signature (`trusted` = clés de confiance).
pub fn read<R: Read + Seek>(reader: R, trusted: &[&str]) -> Result<Package, String> {
    read_with(reader, trusted, &[])
}

/// Comme `read`, en reconnaissant aussi les clés du magasin (état `Store`).
pub fn read_with<R: Read + Seek>(reader: R, official: &[&str], store: &[&str]) -> Result<Package, String> {
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
    let mut media = crate::media::Media::default();
    for (name, bytes) in files.iter().filter(|(n, _)| n.starts_with(crate::media::PREFIX)) {
        media.add(name, bytes.clone())?;
    }
    media.check()?;
    let (signature, key) = verify_with(&files, official, store);
    Ok(Package { manifest, files, signature, key, media })
}

// --- Écriture, signature ------------------------------------------------------------------

/// Écrit un paquet `.island` (zip, deflate), fichiers triés par chemin.
pub fn write<W: std::io::Write + Seek>(writer: W, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    use std::io::Write as _;
    let mut zip = zip::ZipWriter::new(writer);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, bytes) in sorted {
        if !valid_rel(name) {
            return Err(format!("badPath:{name}"));
        }
        zip.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Remplace la signature du paquet par celle de `key` (même empreinte : `digest`).
pub fn sign(files: &[(String, Vec<u8>)], key: &ed25519_dalek::SigningKey) -> Vec<(String, Vec<u8>)> {
    use ed25519_dalek::Signer;
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut out: Vec<(String, Vec<u8>)> = files.iter().filter(|(p, _)| p != SIGNATURE_FILE).cloned().collect();
    let sig = key.sign(digest(&out).as_bytes());
    let json = serde_json::json!({ "key": b64.encode(key.verifying_key().to_bytes()), "sig": b64.encode(sig.to_bytes()) });
    out.push((SIGNATURE_FILE.into(), json.to_string().into_bytes()));
    out
}

/// `a` est une version strictement plus récente que `b` (« 1.10.0 » > « 1.9.2 » ; parties
/// numériques comparées une à une, un suffixe `-beta…` compte comme antérieur).
pub fn newer(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> (Vec<u64>, bool) {
        let (core, pre) = match v.trim().split_once('-') {
            Some((c, _)) => (c, true),
            None => (v.trim(), false),
        };
        (core.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let ((mut x, xp), (mut y, yp)) = (parts(a), parts(b));
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    match x.cmp(&y) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        // Même numéro : la version finale passe devant la préversion.
        std::cmp::Ordering::Equal => yp && !xp,
    }
}

/// Fichiers admis dans un paquet du magasin : ressources web seulement (aucun exécutable ni
/// script système).
pub fn allowed_file(path: &str) -> bool {
    if path == SIGNATURE_FILE {
        return true;
    }
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    matches!(
        ext.as_str(),
        "html" | "htm" | "js" | "mjs" | "css" | "json" | "wasm" | "map" | "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "ico" | "avif"
            | "woff" | "woff2" | "ttf" | "otf" | "txt" | "md" | "mp3" | "ogg" | "wav"
    )
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
    fn versions_compare() {
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("2", "1.9"));
        assert!(newer("1.0.0", "1.0.0-beta.1"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("1.0.0-beta", "1.0.0"));
        assert!(!newer("0.9", "1.0"));
    }

    #[test]
    fn only_web_files_are_allowed() {
        assert!(allowed_file("dist/main.js"));
        assert!(allowed_file("icons/a.SVG"));
        assert!(!allowed_file("tools/run.exe"));
        assert!(!allowed_file("x.ps1"));
        assert!(!allowed_file("README"));
    }

    #[test]
    fn store_signature_replaces_the_author_one() {
        let author = SigningKey::from_bytes(&[7; 32]);
        let store = SigningKey::from_bytes(&[9; 32]);
        let b64 = base64::engine::general_purpose::STANDARD;
        let store_pub = b64.encode(store.verifying_key().to_bytes());
        let resigned = sign(&signed(&author, &files()), &store);
        assert_eq!(resigned.iter().filter(|(p, _)| p == SIGNATURE_FILE).count(), 1);
        assert!(verify_with(&resigned, &[], &[&store_pub]).0 == Signature::Store);
        // Écrit puis relu : même contenu, même état.
        let mut out = std::io::Cursor::new(Vec::new());
        write(&mut out, &resigned).unwrap();
        let bytes = out.into_inner();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(z.len(), 3);
        assert!(z.by_name("dist/main.js").is_ok());
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
