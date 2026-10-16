// Catalogue des launchers : dossiers d'installation → jeu (source la plus sûre). Construit
// au démarrage (fil à part) et sur `activity_refresh_catalog`. Parseurs VDF/ACF (Steam) et
// manifestes Epic (JSON) purs et testés ; le reste (registre, disques) est Windows.

use super::Source;

/// Un jeu installé : dossier (normalisé, cf. `norm_dir`) → identité.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub dir: String,
    pub name: String,
    pub source: Source,
    pub app_id: Option<String>,
    pub art: Option<String>,
}

#[derive(Default, Debug)]
pub struct Catalog {
    pub entries: Vec<Entry>,
}

impl Catalog {
    /// Entrée dont le dossier contient `path_lower` (chemin normalisé) ; la plus profonde
    /// gagne (bibliothèque imbriquée).
    pub fn lookup(&self, path_lower: &str) -> Option<&Entry> {
        self.entries.iter().filter(|e| path_lower.starts_with(&e.dir)).max_by_key(|e| e.dir.len())
    }

    /// Ajoute un jeu si son dossier est plausible (pas une racine de disque ni un dossier
    /// système commun, qui avaleraient toutes les apps).
    pub fn push(&mut self, dir: &str, name: &str, source: Source, app_id: Option<String>, art: Option<String>) {
        let name = name.trim();
        let Some(dir) = norm_dir(dir) else { return };
        if name.is_empty() || too_broad(&dir) || self.entries.iter().any(|e| e.dir == dir) {
            return;
        }
        self.entries.push(Entry { dir, name: name.to_string(), source, app_id, art });
    }
}

/// Chemin normalisé pour comparaison : minuscules, `\`, sans `\` doublé.
pub fn norm_path(p: &str) -> String {
    let mut s = p.trim().replace('/', "\\").to_lowercase();
    while s.contains("\\\\") {
        s = s.replace("\\\\", "\\");
    }
    s
}

/// Dossier normalisé terminé par `\` (préfixe exact pour `starts_with`).
pub fn norm_dir(p: &str) -> Option<String> {
    let s = norm_path(p);
    let s = s.trim_end_matches('\\');
    if s.len() < 3 {
        return None;
    }
    Some(format!("{s}\\"))
}

fn too_broad(dir: &str) -> bool {
    let segs = dir.trim_end_matches('\\').split('\\').count();
    if segs < 2 {
        return true; // racine de disque
    }
    const ROOTS: [&str; 7] =
        ["program files", "program files (x86)", "programdata", "windows", "users", "xboxgames", "steamlibrary"];
    segs == 2 && ROOTS.iter().any(|r| dir.trim_end_matches('\\').rsplit('\\').next() == Some(r))
}

// --- Exclusions (jamais des jeux) ----------------------------------------------------------

/// Launchers / clients de jeux eux-mêmes.
pub const LAUNCHERS: &[&str] = &[
    "steam.exe", "steamwebhelper.exe", "steamservice.exe", "gameoverlayui.exe", "epicgameslauncher.exe",
    "epicwebhelper.exe", "galaxyclient.exe", "galaxyclient helper.exe", "goggalaxy.exe", "eadesktop.exe",
    "eabackgroundservice.exe", "ealauncher.exe", "origin.exe", "originwebhelperservice.exe", "ubisoftconnect.exe",
    "upc.exe", "uplaywebcore.exe", "ubisoftgamelauncher.exe", "battle.net.exe", "agent.exe", "blizzard error.exe",
    "xboxpcapp.exe", "xboxapp.exe", "gamingservices.exe", "playnite.desktopapp.exe", "playnite.fullscreenapp.exe",
    "riotclientservices.exe", "riotclientux.exe", "riotclientuxrender.exe", "heroic.exe", "itch.exe",
    "amazon games.exe", "amazon games ui.exe", "launcher.exe", "rockstarservice.exe", "socialclubhelper.exe",
    "discord.exe", "discordptb.exe", "discordcanary.exe", "overwolf.exe",
];

/// Navigateurs (et leurs webviews).
pub const BROWSERS: &[&str] = &[
    "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "opera_gx.exe", "vivaldi.exe",
    "iexplore.exe", "arc.exe", "zen.exe", "librewolf.exe", "waterfox.exe", "chromium.exe", "thorium.exe",
    "msedgewebview2.exe", "floorp.exe", "yandex.exe",
];

/// Utilitaires livrés DANS les dossiers de jeux (rapport de plantage, anti-triche,
/// redistribuables, installeurs) : pas le jeu lui-même.
pub fn is_helper(exe: &str) -> bool {
    const EXACT: &[&str] = &[
        "unitycrashhandler64.exe", "unitycrashhandler32.exe", "crashreportclient.exe", "crashpad_handler.exe",
        "dxsetup.exe", "dotnetfx.exe", "steamerrorreporter.exe", "steamerrorreporter64.exe", "cefprocess.exe",
        "start_protected_game.exe", "easyanticheat.exe", "easyanticheat_setup.exe", "easyanticheat_eos_setup.exe",
    ];
    EXACT.contains(&exe)
        || exe.starts_with("vc_redist")
        || exe.starts_with("vcredist")
        || exe.starts_with("unins")
        || exe.starts_with("beservice")
        || exe.starts_with("crashreport")
        || exe.contains("setup")
        || exe.contains("installer")
}

// --- VDF / ACF (Steam KeyValues texte) -----------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// Enfant par clé (insensible à la casse, comme Steam).
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(kv) => kv.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }
    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(kv) => kv,
            Vdf::Str(_) => &[],
        }
    }
}

/// Parseur KeyValues tolérant : chaînes entre guillemets (échappements `\\ \" \n \t`),
/// jetons nus, `{ }`, commentaires `//`, conditions `[$WIN32]` ignorées. Une erreur
/// arrête le parsing en gardant ce qui précède.
pub fn parse_vdf(src: &str) -> Vdf {
    let toks = tokenize(src);
    let mut i = 0;
    Vdf::Obj(parse_obj(&toks, &mut i, 0))
}

#[derive(Debug, PartialEq)]
enum Tok {
    S(String),
    Open,
    Close,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '"' => break,
                        '\\' => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(o) => s.push(o),
                            None => break,
                        },
                        o => s.push(o),
                    }
                }
                out.push(Tok::S(s));
            }
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '[' => {
                // Condition de plateforme : ignorée.
                for c in it.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => {
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    it.next();
                }
                out.push(Tok::S(s));
            }
        }
    }
    out
}

fn parse_obj(toks: &[Tok], i: &mut usize, depth: usize) -> Vec<(String, Vdf)> {
    let mut kv = Vec::new();
    while *i < toks.len() {
        match &toks[*i] {
            Tok::Close => {
                *i += 1;
                return kv;
            }
            Tok::Open => {
                // Bloc sans clé : toléré, aplati.
                *i += 1;
                if depth < 64 {
                    kv.extend(parse_obj(toks, i, depth + 1));
                }
            }
            Tok::S(k) => {
                *i += 1;
                match toks.get(*i) {
                    Some(Tok::S(v)) => {
                        kv.push((k.clone(), Vdf::Str(v.clone())));
                        *i += 1;
                    }
                    Some(Tok::Open) => {
                        *i += 1;
                        let child = if depth < 64 { parse_obj(toks, i, depth + 1) } else { Vec::new() };
                        kv.push((k.clone(), Vdf::Obj(child)));
                    }
                    _ => return kv,
                }
            }
        }
    }
    kv
}

/// Racines de bibliothèques listées dans `libraryfolders.vdf` (format récent : objets
/// `"0" { "path" "…" }` ; ancien : `"1" "D:\\Steam"`).
pub fn steam_libraries(vdf: &str) -> Vec<String> {
    let root = parse_vdf(vdf);
    let Some(lf) = root.get("libraryfolders").or_else(|| root.get("LibraryFolders")) else { return Vec::new() };
    let mut out = Vec::new();
    for (k, v) in lf.entries() {
        if !k.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let p = match v {
            Vdf::Str(s) => Some(s.as_str()),
            Vdf::Obj(_) => v.get("path").and_then(Vdf::str),
        };
        if let Some(p) = p.filter(|p| !p.is_empty()) {
            out.push(p.to_string());
        }
    }
    out
}

/// `appmanifest_*.acf` → (appid, nom, installdir).
pub fn parse_acf(src: &str) -> Option<(String, String, String)> {
    let root = parse_vdf(src);
    let st = root.get("AppState")?;
    let id = st.get("appid")?.str()?.trim().to_string();
    let name = st.get("name")?.str()?.trim().to_string();
    let dir = st.get("installdir")?.str()?.trim().to_string();
    (!id.is_empty() && !name.is_empty() && !dir.is_empty()).then_some((id, name, dir))
}

/// Outils Steam qui ne sont pas des jeux (redistribuables, Proton, SDK).
fn steam_tool(appid: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    appid == "228980" || n.contains("redistributable") || n.starts_with("proton") || n.contains("steam linux runtime")
}

pub fn steam_art(appid: &str) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg")
}

/// Ajoute les jeux d'une bibliothèque Steam (`<lib>/steamapps/appmanifest_*.acf`).
pub fn add_steam_library(cat: &mut Catalog, lib: &std::path::Path) {
    let apps = lib.join("steamapps");
    let Ok(rd) = std::fs::read_dir(&apps) else { return };
    for e in rd.flatten() {
        let fname = e.file_name().to_string_lossy().to_lowercase();
        if !(fname.starts_with("appmanifest_") && fname.ends_with(".acf")) {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(e.path()) else { continue };
        let Some((id, name, dir)) = parse_acf(&txt) else { continue };
        if steam_tool(&id, &name) {
            continue;
        }
        let full = apps.join("common").join(&dir);
        cat.push(&full.to_string_lossy(), &name, Source::Steam, Some(id.clone()), Some(steam_art(&id)));
    }
}

// --- Epic (manifestes `.item`, JSON) -------------------------------------------------------

/// Manifeste Epic → (nom, dossier, appName). Les installs non-jeu (moteur Unreal…) sont
/// écartées quand `AppCategories` est présent sans `games`.
pub fn parse_epic_item(src: &str) -> Option<(String, String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(src).ok()?;
    if let Some(cats) = v.get("AppCategories").and_then(|c| c.as_array()) {
        if !cats.is_empty() && !cats.iter().any(|c| c.as_str().is_some_and(|s| s.eq_ignore_ascii_case("games"))) {
            return None;
        }
    }
    if v.get("bIsIncompleteInstall").and_then(|b| b.as_bool()) == Some(true) {
        return None;
    }
    let name = v.get("DisplayName")?.as_str()?.trim().to_string();
    let dir = v.get("InstallLocation")?.as_str()?.trim().to_string();
    let id = v.get("AppName").and_then(|s| s.as_str()).map(str::to_string);
    (!name.is_empty() && !dir.is_empty()).then_some((name, dir, id))
}

// --- Xbox / Game Pass (`MicrosoftGame.config`, XML) ----------------------------------------

/// `MicrosoftGame.config` → (nom affiché, identité). Extraction d'attributs à la main
/// (pas de parseur XML pour deux valeurs).
pub fn parse_ms_game_config(src: &str) -> (Option<String>, Option<String>) {
    fn attr(src: &str, tag: &str, attr: &str) -> Option<String> {
        let start = src.find(&format!("<{tag}"))?;
        let rest = &src[start..];
        let end = rest.find('>')?;
        let el = &rest[..end];
        let needle = format!("{attr}=\"");
        let a = el.find(&needle)? + needle.len();
        let v = &el[a..];
        let v = &v[..v.find('"')?];
        let v = v.trim();
        (!v.is_empty() && !v.starts_with("ms-resource:")).then(|| v.to_string())
    }
    (attr(src, "ShellVisuals", "DefaultDisplayName"), attr(src, "Identity", "Name"))
}

/// Remonte au plus 4 niveaux depuis l'exe à la recherche d'un `MicrosoftGame.config`
/// (jeux Game Pass, y compris sous `WindowsApps`). Simple test d'existence de fichier.
pub fn xbox_probe(path: &str) -> Option<(String, Option<String>)> {
    let mut dir = std::path::Path::new(path).parent();
    for _ in 0..4 {
        let d = dir?;
        let cfg = d.join("MicrosoftGame.config");
        if cfg.is_file() {
            let txt = std::fs::read_to_string(&cfg).unwrap_or_default();
            let (name, id) = parse_ms_game_config(&txt);
            let name = name.unwrap_or_else(|| {
                d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            });
            return Some((name, id));
        }
        dir = d.parent();
    }
    None
}

// --- Construction (Windows) ----------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn build() -> Catalog {
    let mut cat = Catalog::default();
    steam(&mut cat);
    epic(&mut cat);
    gog(&mut cat);
    ubisoft(&mut cat);
    uninstall(&mut cat);
    xbox(&mut cat);
    cat
}

#[cfg(not(target_os = "windows"))]
pub fn build() -> Catalog {
    Catalog::default()
}

#[cfg(target_os = "windows")]
fn steam(cat: &mut Catalog) {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let root: Option<String> = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Valve\\Steam")
        .and_then(|k| k.get_value("SteamPath"))
        .or_else(|_| {
            RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
                .and_then(|k| k.get_value("InstallPath"))
        })
        .ok();
    let Some(root) = root else { return };
    let root = std::path::PathBuf::from(root.replace('/', "\\"));
    let mut libs = vec![root.clone()];
    if let Ok(txt) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        libs.extend(steam_libraries(&txt).into_iter().map(std::path::PathBuf::from));
    }
    let mut seen = std::collections::HashSet::new();
    for lib in libs {
        if seen.insert(norm_path(&lib.to_string_lossy())) {
            add_steam_library(cat, &lib);
        }
    }
}

#[cfg(target_os = "windows")]
fn epic(cat: &mut Catalog) {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
    let dir = std::path::Path::new(&pd).join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests");
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if !e.file_name().to_string_lossy().to_lowercase().ends_with(".item") {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(e.path()) else { continue };
        if let Some((name, dir, id)) = parse_epic_item(&txt) {
            cat.push(&dir, &name, Source::Epic, id, None);
        }
    }
}

#[cfg(target_os = "windows")]
fn subkeys(root: &winreg::RegKey, path: &str) -> Vec<(String, winreg::RegKey)> {
    let Ok(k) = root.open_subkey(path) else { return Vec::new() };
    k.enum_keys().flatten().filter_map(|n| k.open_subkey(&n).ok().map(|s| (n, s))).collect()
}

#[cfg(target_os = "windows")]
fn gog(cat: &mut Catalog) {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    for (id, k) in subkeys(&hklm, "SOFTWARE\\WOW6432Node\\GOG.com\\Games") {
        let name: String = k.get_value("gameName").unwrap_or_default();
        let path: String = k.get_value("path").unwrap_or_default();
        let gid: String = k.get_value("gameID").unwrap_or(id);
        cat.push(&path, &name, Source::Gog, Some(gid), None);
    }
}

#[cfg(target_os = "windows")]
fn ubisoft(cat: &mut Catalog) {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    for (id, k) in subkeys(&hklm, "SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs") {
        let dir: String = k.get_value("InstallDir").unwrap_or_default();
        let name = dir.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or("").to_string();
        cat.push(&dir, &name, Source::Ubisoft, Some(id), None);
    }
}

/// EA app / Battle.net (au mieux) : entrées de désinstallation par éditeur, + `EA Games`.
#[cfg(target_os = "windows")]
fn uninstall(cat: &mut Catalog) {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    const PATHS: [&str; 2] = [
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
    ];
    const NOT_GAMES: [&str; 6] = ["ea app", "ea", "origin", "battle.net", "ea desktop", "blizzard battle.net"];
    for root in [&hklm, &hkcu] {
        for p in PATHS {
            for (_, k) in subkeys(root, p) {
                let publisher: String = k.get_value("Publisher").unwrap_or_default();
                let source = if publisher.contains("Electronic Arts") {
                    Source::Ea
                } else if publisher.contains("Blizzard") {
                    Source::Battlenet
                } else {
                    continue;
                };
                let name: String = k.get_value("DisplayName").unwrap_or_default();
                if NOT_GAMES.contains(&name.trim().to_lowercase().as_str()) {
                    continue;
                }
                let dir: String = k.get_value("InstallLocation").unwrap_or_default();
                cat.push(dir.trim_matches('"'), &name, source, None, None);
            }
        }
    }
    for (name, k) in subkeys(&hklm, "SOFTWARE\\WOW6432Node\\EA Games") {
        let dir: String = k.get_value("Install Dir").unwrap_or_default();
        cat.push(&dir, &name, Source::Ea, None, None);
    }
}

/// Game Pass : `<lecteur>:\XboxGames\<jeu>\Content\MicrosoftGame.config`.
#[cfg(target_os = "windows")]
fn xbox(cat: &mut Catalog) {
    for d in b'C'..=b'Z' {
        let root = std::path::PathBuf::from(format!("{}:\\XboxGames", d as char));
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let cfg = e.path().join("Content").join("MicrosoftGame.config");
            let Ok(txt) = std::fs::read_to_string(&cfg) else { continue };
            let (name, id) = parse_ms_game_config(&txt);
            let name = name.unwrap_or_else(|| e.file_name().to_string_lossy().to_string());
            cat.push(&e.path().to_string_lossy(), &name, Source::Xbox, id, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"228980"		"0"
		}
	}
	// commentaire
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;

    #[test]
    fn vdf_libraries() {
        assert_eq!(steam_libraries(LIBS), vec!["C:\\Program Files (x86)\\Steam", "D:\\SteamLibrary"]);
        // Ancien format.
        let old = "\"LibraryFolders\" { \"TimeNextStatsReport\" \"1\" \"1\" \"E:\\\\Games\\\\Steam\" }";
        assert_eq!(steam_libraries(old), vec!["E:\\Games\\Steam"]);
    }

    #[test]
    fn vdf_tolerant() {
        // Accolade manquante, jeton nu, condition : ne panique pas, garde le début.
        let v = parse_vdf("\"a\" { \"b\" \"1\" [$WIN32] c 2 \"d\" { \"e\" \"x\"");
        let a = v.get("a").unwrap();
        assert_eq!(a.get("b").and_then(Vdf::str), Some("1"));
        assert_eq!(a.get("C").and_then(Vdf::str), Some("2"));
        assert_eq!(a.get("d").and_then(|d| d.get("e")).and_then(Vdf::str), Some("x"));
        assert_eq!(parse_vdf(""), Vdf::Obj(vec![]));
        assert_eq!(parse_vdf("}}}{"), Vdf::Obj(vec![]));
    }

    #[test]
    fn acf() {
        let src = r#""AppState"
{
	"appid"		"1245620"
	"Universe"		"1"
	"name"		"ELDEN RING"
	"StateFlags"		"4"
	"installdir"		"ELDEN RING"
	"UserConfig" { "language" "french" }
}"#;
        assert_eq!(parse_acf(src), Some(("1245620".into(), "ELDEN RING".into(), "ELDEN RING".into())));
        assert_eq!(parse_acf("\"AppState\" { \"appid\" \"1\" }"), None);
        assert!(steam_tool("228980", "Steamworks Common Redistributables"));
        assert!(!steam_tool("1245620", "ELDEN RING"));
    }

    #[test]
    fn epic_item() {
        let src = r#"{ "FormatVersion": 0, "DisplayName": "Fortnite", "InstallLocation": "C:\\Program Files\\Epic Games\\Fortnite",
            "LaunchExecutable": "FortniteGame/Binaries/Win64/FortniteLauncher.exe", "AppName": "Fortnite",
            "AppCategories": ["public", "games", "applications"] }"#;
        assert_eq!(
            parse_epic_item(src),
            Some(("Fortnite".into(), "C:\\Program Files\\Epic Games\\Fortnite".into(), Some("Fortnite".into())))
        );
        let engine = r#"{ "DisplayName": "Unreal Engine", "InstallLocation": "C:\\UE_5.4", "AppCategories": ["public", "engines"] }"#;
        assert_eq!(parse_epic_item(engine), None);
        let incomplete = r#"{ "DisplayName": "X", "InstallLocation": "C:\\X", "bIsIncompleteInstall": true }"#;
        assert_eq!(parse_epic_item(incomplete), None);
        assert_eq!(parse_epic_item("pas du json"), None);
    }

    #[test]
    fn ms_config() {
        let src = r#"<Game configVersion="1"><Identity Name="Microsoft.624F8B84B80" Publisher="CN=x" Version="1.0"/>
            <ShellVisuals DefaultDisplayName="Forza Horizon 5" PublisherDisplayName="Xbox"/></Game>"#;
        assert_eq!(parse_ms_game_config(src), (Some("Forza Horizon 5".into()), Some("Microsoft.624F8B84B80".into())));
        let res = r#"<ShellVisuals DefaultDisplayName="ms-resource:Name"/>"#;
        assert_eq!(parse_ms_game_config(res), (None, None));
    }

    #[test]
    fn lookup_and_guards() {
        let mut c = Catalog::default();
        c.push("D:/SteamLibrary/steamapps/common/ELDEN RING", "ELDEN RING", Source::Steam, Some("1".into()), None);
        c.push("D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\Sub", "Nested", Source::Steam, None, None);
        c.push("C:\\", "Root", Source::Gog, None, None);
        c.push("C:\\Program Files", "PF", Source::Ea, None, None);
        c.push("D:\\Witcher3", "The Witcher 3", Source::Gog, None, None);
        assert_eq!(c.entries.len(), 3);
        let p = norm_path("D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe");
        assert_eq!(c.lookup(&p).map(|e| e.name.as_str()), Some("ELDEN RING"));
        let p = norm_path("d:\\steamlibrary\\steamapps\\common\\elden ring\\game\\sub\\x.exe");
        assert_eq!(c.lookup(&p).map(|e| e.name.as_str()), Some("Nested"));
        // Préfixe de nom sans séparateur : pas de faux positif.
        assert!(c.lookup(&norm_path("D:\\Witcher3Mods\\x.exe")).is_none());
        assert_eq!(c.lookup(&norm_path("d:\\witcher3\\bin\\x64\\witcher3.exe")).map(|e| e.name.as_str()), Some("The Witcher 3"));
    }
}
