//! Stockage des fichiers publiés. Développement : un dossier local. Production : R2
//! (variante à venir, étape « Production »). Les clés sont produites par le serveur
//! (`items/<id>/<version>.islandscene`…) et revérifiées ici : jamais de `..` ni de chemin
//! absolu.

use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

pub enum Storage {
    Local(PathBuf),
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 200
        && !key.starts_with('/')
        && key.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
        && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
}

impl Storage {
    /// `local:<dossier>` (seule forme pour l'instant).
    pub fn from_config(s: &str) -> Result<Self, String> {
        match s.split_once(':') {
            Some(("local", dir)) if !dir.is_empty() => Ok(Self::Local(PathBuf::from(dir))),
            _ => Err(format!("STORAGE invalide : {s} (attendu : local:<dossier>)")),
        }
    }

    fn path(&self, key: &str) -> Result<PathBuf, String> {
        if !valid_key(key) {
            return Err(format!("clé de stockage invalide : {key}"));
        }
        match self {
            Self::Local(dir) => Ok(dir.join(key)),
        }
    }

    /// Écrit (ou remplace) un fichier, de façon atomique (fichier temporaire puis renommage).
    pub async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.path(key)?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("part");
        let mut f = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
        f.write_all(bytes).await.map_err(|e| e.to_string())?;
        f.flush().await.map_err(|e| e.to_string())?;
        drop(f);
        tokio::fs::rename(&tmp, &path).await.map_err(|e| e.to_string())
    }

    /// Ouvre un fichier pour le servir : (fichier, taille). None s'il n'existe pas.
    pub async fn open(&self, key: &str) -> Result<Option<(tokio::fs::File, u64)>, String> {
        let path = self.path(key)?;
        match tokio::fs::File::open(&path).await {
            Ok(f) => {
                let len = f.metadata().await.map_err(|e| e.to_string())?.len();
                Ok(Some((f, len)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(key)?;
        match tokio::fs::read(&path).await {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn keys_never_escape_the_storage() {
        assert!(super::valid_key("items/sc_abc/1.islandscene"));
        assert!(!super::valid_key("../secret"));
        assert!(!super::valid_key("items/../../x"));
        assert!(!super::valid_key("/etc/passwd"));
        assert!(!super::valid_key("items//x"));
        assert!(!super::valid_key("items/C:x"));
    }
}
