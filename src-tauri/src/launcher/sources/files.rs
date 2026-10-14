// Fichiers et dossiers : parcours des dossiers personnels usuels, borné en profondeur et
// en nombre, sans les dossiers bruyants (node_modules, .git…). Un seul `file_type` par
// entrée. Everything, s'il tourne, complète au moment de la recherche (tout le disque).

use std::path::Path;

use crate::launcher::model::{Item, Kind};

const MAX_DEPTH: u8 = 4;
const MAX_ITEMS: usize = 12_000;

/// Dossiers ignorés (par nom, insensible à la casse).
const SKIP: &[&str] = &[
    "node_modules", "target", "dist", "build", "out", ".git", ".svn", ".hg", "__pycache__", ".venv", "venv",
    ".idea", ".vscode", ".cache", "cache", "temp", "tmp", "intermediate", "deriveddatacache", "binaries", "saved",
    "obj", "bin", "packages", ".gradle", ".next", ".nuxt", "vendor",
];

fn roots() -> Vec<std::path::PathBuf> {
    let Ok(home) = std::env::var("USERPROFILE") else { return Vec::new() };
    let home = std::path::PathBuf::from(home);
    ["Desktop", "Documents", "Downloads", "Pictures", "Videos", "Music"]
        .iter()
        .map(|s| home.join(s))
        .filter(|p| p.is_dir())
        .collect()
}

/// Chemin affiché : le dossier personnel devient `~`.
pub fn pretty(path: &str) -> String {
    match std::env::var("USERPROFILE") {
        Ok(home) if path.len() > home.len() && path[..home.len()].eq_ignore_ascii_case(&home) => format!("~{}", &path[home.len()..]),
        _ => path.to_string(),
    }
}

pub fn item(path: &Path, is_dir: bool) -> Option<Item> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let target = path.to_string_lossy().to_string();
    let parent = path.parent().map(|p| pretty(&p.to_string_lossy())).unwrap_or_default();
    Some(Item {
        id: format!("file:{}", target.to_lowercase()),
        kind: if is_dir { Kind::Folder } else { Kind::File },
        title: name,
        subtitle: parent,
        target,
        keywords: String::new(),
    })
}

fn walk(dir: &Path, depth: u8, out: &mut Vec<Item>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ITEMS {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() >= MAX_ITEMS {
            return;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name.starts_with('~') || name.eq_ignore_ascii_case("desktop.ini") || name.eq_ignore_ascii_case("thumbs.db") {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        // Les liens (jonctions…) ne sont pas suivis : pas de boucle, pas d'arbre géant.
        if ft.is_symlink() {
            continue;
        }
        let is_dir = ft.is_dir();
        if is_dir && SKIP.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        let path = e.path();
        if let Some(it) = item(&path, is_dir) {
            out.push(it);
        }
        if is_dir {
            walk(&path, depth + 1, out);
        }
    }
}

pub fn items() -> Vec<Item> {
    let mut out = Vec::new();
    for r in roots() {
        if let Some(it) = item(&r, true) {
            out.push(it);
        }
        walk(&r, 1, &mut out);
    }
    out
}
