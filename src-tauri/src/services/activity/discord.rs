// Base des jeux détectables de Discord (`/api/applications/detectable`) : exécutables
// reconnus comme jeux, comparés comme le fait Discord (suffixe de chemin « dossier/exe »).
// Téléchargée au plus une fois par semaine ; on n'en garde qu'une version compacte
// (`<app_data>/activity/detectable.json` : win32, hors launchers, hors exe à arguments).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const URL: &str = "https://discord.com/api/applications/detectable";
/// Fraîcheur du cache.
pub const MAX_AGE_SECS: u64 = 7 * 24 * 3600;
/// Pas de nouvelle tentative (échec réseau compris) avant ce délai.
pub const RETRY_SECS: u64 = 6 * 3600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CacheApp {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Noms Discord en minuscules (`dossier/exe`, `exe`, ou `>chemin/exact`).
    pub exes: Vec<String>,
}

#[derive(Deserialize)]
struct RawApp {
    id: String,
    name: String,
    #[serde(default)]
    icon_hash: Option<String>,
    #[serde(default)]
    executables: Option<Vec<RawExe>>,
}

#[derive(Deserialize)]
struct RawExe {
    name: String,
    #[serde(default)]
    os: Option<String>,
    #[serde(default)]
    is_launcher: bool,
    #[serde(default)]
    arguments: Option<String>,
}

/// Réponse brute de l'API → liste compacte. Exclus : autres OS, launchers, et exe qui
/// exigent des arguments (lire la ligne de commande d'un autre process = lire sa mémoire).
pub fn from_raw(bytes: &[u8]) -> Result<Vec<CacheApp>, String> {
    let raw: Vec<RawApp> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    Ok(raw
        .into_iter()
        .filter_map(|a| {
            let exes: Vec<String> = a
                .executables
                .unwrap_or_default()
                .into_iter()
                .filter(|x| {
                    x.os.as_deref() == Some("win32") && !x.is_launcher && x.arguments.as_deref().unwrap_or("").is_empty()
                })
                .map(|x| x.name.to_lowercase().replace('\\', "/"))
                .filter(|n| n.trim_start_matches('>').len() > 4)
                .collect();
            (!exes.is_empty()).then_some(CacheApp { id: a.id, name: a.name, icon: a.icon_hash, exes })
        })
        .collect())
}

pub struct DApp {
    pub id: String,
    pub name: String,
    pub art: Option<String>,
}

struct Cand {
    name: String,
    exact: bool,
    app: u32,
}

/// Base indexée par nom de fichier exe (dernier segment) → candidats.
pub struct DiscordDb {
    apps: Vec<DApp>,
    by_exe: HashMap<String, Vec<Cand>>,
}

impl DiscordDb {
    pub fn build(list: Vec<CacheApp>) -> Self {
        let mut apps = Vec::with_capacity(list.len());
        let mut by_exe: HashMap<String, Vec<Cand>> = HashMap::new();
        for a in list {
            let idx = apps.len() as u32;
            for e in a.exes {
                let exact = e.starts_with('>');
                let name = e.trim_start_matches('>').to_string();
                let file = name.rsplit('/').next().unwrap_or(&name).to_string();
                by_exe.entry(file).or_default().push(Cand { name, exact, app: idx });
            }
            let art = a.icon.map(|h| format!("https://cdn.discordapp.com/app-icons/{}/{h}.png?size=256", a.id));
            apps.push(DApp { id: a.id, name: a.name, art });
        }
        Self { apps, by_exe }
    }

    /// Jeu correspondant à un chemin d'exe (n'importe quelle casse / séparateur). Le
    /// candidat au suffixe le plus long gagne (le plus spécifique).
    pub fn find(&self, path: &str) -> Option<&DApp> {
        let p = path.to_lowercase().replace('\\', "/");
        let segs: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        // Sans lettre de lecteur (comme Discord).
        let segs: &[&str] = if segs.first().is_some_and(|s| s.len() == 2 && s.ends_with(':')) { &segs[1..] } else { &segs };
        let file = segs.last()?;
        let full = segs.join("/");
        self.by_exe
            .get(*file)?
            .iter()
            .filter(|c| {
                if c.exact {
                    c.name == full
                } else {
                    // Suffixe aligné sur une frontière de dossier.
                    full == c.name || full.ends_with(&format!("/{}", c.name))
                }
            })
            .max_by_key(|c| c.name.len())
            .map(|c| &self.apps[c.app as usize])
    }
}

pub fn load_cache(path: &Path) -> Option<Vec<CacheApp>> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Télécharge la base (20 s max, 64 Mo max) et la réduit.
pub fn download() -> Result<Vec<CacheApp>, String> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(20)).build();
    let resp = agent.get(URL).set("User-Agent", "Island").call().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    resp.into_reader().take(64 << 20).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let list = from_raw(&buf)?;
    if list.is_empty() {
        return Err("empty".into());
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = r#"[
      {"id": "1", "name": "Elden Ring", "icon_hash": "abc", "executables": [
        {"is_launcher": false, "name": "elden ring/game/eldenring.exe", "os": "win32"},
        {"is_launcher": false, "name": "eldenring", "os": "darwin"}]},
      {"id": "2", "name": "Some Launcher", "executables": [{"is_launcher": true, "name": "launcherx.exe", "os": "win32"}]},
      {"id": "3", "name": "Minecraft", "executables": [{"is_launcher": false, "name": "javaw.exe", "os": "win32", "arguments": "net.minecraft"}]},
      {"id": "4", "name": "Generic", "executables": [{"is_launcher": false, "name": "game.exe", "os": "win32"}]},
      {"id": "5", "name": "Specific", "executables": [{"is_launcher": false, "name": "studio/game.exe", "os": "win32"}]},
      {"id": "6", "name": "Exact", "executables": [{"is_launcher": false, "name": ">tools/exact.exe", "os": "win32"}]},
      {"id": "7", "name": "Null", "executables": null}
    ]"#;

    fn db() -> DiscordDb {
        DiscordDb::build(from_raw(RAW.as_bytes()).unwrap())
    }

    #[test]
    fn raw_filter() {
        let l = from_raw(RAW.as_bytes()).unwrap();
        let ids: Vec<&str> = l.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "4", "5", "6"]);
        assert_eq!(l[0].exes, vec!["elden ring/game/eldenring.exe"]);
    }

    #[test]
    fn suffix_matching() {
        let d = db();
        let er = d.find("D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe").unwrap();
        assert_eq!(er.name, "Elden Ring");
        assert_eq!(er.art.as_deref(), Some("https://cdn.discordapp.com/app-icons/1/abc.png?size=256"));
        // Mauvais dossier : pas de correspondance.
        assert!(d.find("C:\\Other\\eldenring.exe").is_none());
        // Suffixe le plus spécifique gagne.
        assert_eq!(d.find("C:\\Games\\Studio\\game.exe").unwrap().name, "Specific");
        assert_eq!(d.find("C:\\Games\\Other\\game.exe").unwrap().name, "Generic");
        // Frontière de dossier : « mystudio/game.exe » ≠ « studio/game.exe ».
        assert_eq!(d.find("C:\\mystudio\\game.exe").unwrap().name, "Generic");
        // Exact (préfixe `>`) : chemin complet sans lecteur uniquement.
        assert_eq!(d.find("E:\\Tools\\exact.exe").unwrap().name, "Exact");
        assert!(d.find("E:\\x\\Tools\\exact.exe").is_none());
        // Launcher et exe à arguments exclus.
        assert!(d.find("C:\\x\\launcherx.exe").is_none());
        assert!(d.find("C:\\Java\\bin\\javaw.exe").is_none());
    }
}
