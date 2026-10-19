//! Analyse antivirus par ClamAV (`clamd`, protocole `INSTREAM`), si `CLAMAV_URL` est
//! configuré. Complément, pas la défense principale : la validation de structure
//! (`island-formats`) et, pour les extensions, la relecture humaine restent l'essentiel.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Résultat d'une analyse (colonne `versions.scan`).
#[derive(Debug, PartialEq)]
pub enum Scan {
    Clean,
    /// Pas d'analyseur configuré.
    Skipped,
    /// Menace détectée (nom ClamAV).
    Found(String),
}

impl Scan {
    pub fn label(&self) -> &'static str {
        match self {
            Scan::Clean => "clean",
            Scan::Skipped => "skipped",
            Scan::Found(_) => "found",
        }
    }
}

/// Envoie `bytes` à `clamd` (`addr` = `host:port`) par blocs de 64 Ko.
pub async fn scan(addr: Option<&str>, bytes: &[u8]) -> Result<Scan, String> {
    let Some(addr) = addr else { return Ok(Scan::Skipped) };
    let mut s = tokio::net::TcpStream::connect(addr).await.map_err(|e| format!("clamd : {e}"))?;
    s.write_all(b"zINSTREAM\0").await.map_err(|e| e.to_string())?;
    for chunk in bytes.chunks(64 * 1024) {
        s.write_all(&(chunk.len() as u32).to_be_bytes()).await.map_err(|e| e.to_string())?;
        s.write_all(chunk).await.map_err(|e| e.to_string())?;
    }
    s.write_all(&0u32.to_be_bytes()).await.map_err(|e| e.to_string())?;
    let mut reply = Vec::new();
    s.read_to_end(&mut reply).await.map_err(|e| e.to_string())?;
    parse(&String::from_utf8_lossy(&reply))
}

/// Réponse de `clamd` : `stream: OK`, `stream: <nom> FOUND`, ou une erreur.
fn parse(reply: &str) -> Result<Scan, String> {
    let r = reply.trim_end_matches('\0').trim();
    let body = r.strip_prefix("stream:").unwrap_or(r).trim();
    if body == "OK" {
        Ok(Scan::Clean)
    } else if let Some(name) = body.strip_suffix("FOUND") {
        Ok(Scan::Found(name.trim().to_string()))
    } else {
        Err(format!("clamd : {r}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clamd_replies() {
        assert_eq!(parse("stream: OK\0").unwrap(), Scan::Clean);
        assert_eq!(parse("stream: Win.Test.EICAR_HDB-1 FOUND\0").unwrap(), Scan::Found("Win.Test.EICAR_HDB-1".into()));
        assert!(parse("INSTREAM size limit exceeded. ERROR\0").is_err());
    }

    #[tokio::test]
    async fn no_scanner_means_skipped() {
        assert_eq!(scan(None, b"x").await.unwrap(), Scan::Skipped);
    }
}
