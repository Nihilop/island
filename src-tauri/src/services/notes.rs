// Notes : fichiers Markdown, rangés « façon S3 ». Chaque note est un fichier `<id>.md`
// dans `<app_data>/notes/` ; son CHEMIN (`projets/island/Idées`) n'est qu'une donnée de
// l'index (`index.json`). Les dossiers se déduisent des préfixes des chemins ; un dossier
// vide est gardé par un marqueur (`folders`). Avantages sous Windows : tout nom est permis
// (accents, deux-points…), pas de limite de longueur de chemin, et renommer / déplacer un
// dossier ne touche que l'index (une écriture, atomique).
//
// Corbeille : une note supprimée est marquée (`trashed`) et purgée après 30 jours.
// Chaque écriture émet `notes://changed { id, source }` (`source` = libellé de la fenêtre
// qui a écrit) : la liste du notch et les fenêtres de notes se resynchronisent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

const TRASH_DAYS: i64 = 30;
const MAX_PATH: usize = 300;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    id: String,
    /// Chemin complet, nom compris (`dossier/sous-dossier/Nom`).
    path: String,
    created: i64,
    updated: i64,
    #[serde(default)]
    pinned: bool,
    /// Mise à la corbeille (ms), sinon None.
    #[serde(default)]
    trashed: Option<i64>,
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    notes: BTreeMap<String, NoteMeta>,
    /// Dossiers créés explicitement (marqueurs, pour garder un dossier vide).
    #[serde(default)]
    folders: BTreeSet<String>,
}

static INDEX: Mutex<Option<Index>> = Mutex::new(None);

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("notes")
}

fn file(app: &AppHandle, id: &str) -> PathBuf {
    dir(app).join(format!("{id}.md"))
}

/// Écriture atomique (fichier temporaire puis renommage).
fn write_atomic(path: &PathBuf, content: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Chemin normalisé : segments sans espaces superflus, ni vides, ni « . » / « .. ».
pub fn normalize(path: &str) -> Option<String> {
    let segs: Vec<&str> = path.split(['/', '\\']).map(str::trim).filter(|s| !s.is_empty() && *s != "." && *s != "..").collect();
    let p = segs.join("/");
    (!p.is_empty() && p.len() <= MAX_PATH).then_some(p)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Accès à l'index (chargé une fois ; migration de l'ancienne note unique au premier
/// lancement ; purge de la corbeille).
fn with_index<T>(app: &AppHandle, f: impl FnOnce(&mut Index) -> T) -> T {
    let mut g = INDEX.lock().unwrap_or_else(|p| p.into_inner());
    if g.is_none() {
        let path = dir(app).join("index.json");
        let mut idx: Index = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if !path.exists() {
            migrate_single_note(app, &mut idx);
        }
        let limit = now() - TRASH_DAYS * 86_400_000;
        let expired: Vec<String> = idx.notes.values().filter(|n| n.trashed.is_some_and(|t| t < limit)).map(|n| n.id.clone()).collect();
        for id in expired {
            idx.notes.remove(&id);
            let _ = std::fs::remove_file(file(app, &id));
        }
        *g = Some(idx);
        save(app, g.as_ref().unwrap());
    }
    f(g.as_mut().unwrap())
}

fn save(app: &AppHandle, idx: &Index) {
    let _ = write_atomic(&dir(app).join("index.json"), &serde_json::to_string_pretty(idx).unwrap_or_default());
}

/// Ancienne note unique (store `__app__`, clé `note`) → première note de la liste.
fn migrate_single_note(app: &AppHandle, idx: &mut Index) {
    let text = crate::store::get(app, "__app__", "note").and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string)).unwrap_or_default();
    if text.trim().is_empty() {
        return;
    }
    let id = uuid();
    if write_atomic(&file(app, &id), &text).is_ok() {
        let t = now();
        idx.notes.insert(id.clone(), NoteMeta { id, path: "Note".into(), created: t, updated: t, pinned: false, trashed: None });
    }
}

fn uuid() -> String {
    let b: [u8; 16] = rand_bytes();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn rand_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("getrandom");
    b
}

/// Chemin libre : « Nom », sinon « Nom 2 », « Nom 3 »… (hors corbeille).
fn unique(idx: &Index, path: &str, except: Option<&str>) -> String {
    let taken = |p: &str| idx.notes.values().any(|n| n.trashed.is_none() && n.path.eq_ignore_ascii_case(p) && Some(n.id.as_str()) != except);
    if !taken(path) {
        return path.to_string();
    }
    (2..).map(|i| format!("{path} {i}")).find(|p| !taken(p)).unwrap()
}

fn changed(app: &AppHandle, id: Option<&str>, source: &str) {
    let _ = app.emit("notes://changed", serde_json::json!({ "id": id, "source": source }));
}

// --- Commandes ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteItem {
    #[serde(flatten)]
    meta: NoteMeta,
    /// Début du texte, sans syntaxe Markdown (aperçu de la liste).
    preview: String,
}

#[derive(Serialize)]
pub struct Listing {
    notes: Vec<NoteItem>,
    folders: Vec<String>,
}

/// Texte d'aperçu : les premières lignes non vides, sans marqueurs Markdown.
fn preview(md: &str) -> String {
    let mut out = String::new();
    for line in md.lines() {
        let l = line.trim().trim_start_matches(['#', '>', '-', '*', '+', ' ']).trim_start_matches("[ ]").trim_start_matches("[x]").trim();
        if l.is_empty() || l.starts_with("```") {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&l.replace(['*', '_', '`'], ""));
        if out.chars().count() > 160 {
            break;
        }
    }
    out.chars().take(160).collect()
}

#[tauri::command]
pub async fn notes_list(app: AppHandle) -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (metas, folders) = with_index(&app, |idx| (idx.notes.values().cloned().collect::<Vec<_>>(), idx.folders.iter().cloned().collect()));
        let notes = metas
            .into_iter()
            .map(|meta| {
                let text = std::fs::read_to_string(file(&app, &meta.id)).unwrap_or_default();
                NoteItem { preview: preview(&text), meta }
            })
            .collect();
        Ok(Listing { notes, folders })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn notes_read(app: AppHandle, id: String) -> Result<String, String> {
    if !valid_id(&id) || !with_index(&app, |idx| idx.notes.contains_key(&id)) {
        return Err("notFound".into());
    }
    Ok(std::fs::read_to_string(file(&app, &id)).unwrap_or_default())
}

/// Meta d'une note (titre de fenêtre, dossier…).
#[tauri::command]
pub fn notes_meta(app: AppHandle, id: String) -> Result<NoteMeta, String> {
    with_index(&app, |idx| idx.notes.get(&id).cloned()).ok_or_else(|| "notFound".into())
}

#[tauri::command]
pub fn notes_write(window: WebviewWindow, app: AppHandle, id: String, content: String) -> Result<NoteMeta, String> {
    if !valid_id(&id) {
        return Err("notFound".into());
    }
    let meta = with_index(&app, |idx| {
        let n = idx.notes.get_mut(&id)?;
        n.updated = now();
        let m = n.clone();
        save(&app, idx);
        Some(m)
    })
    .ok_or("notFound")?;
    write_atomic(&file(&app, &id), &content)?;
    changed(&app, Some(&id), window.label());
    Ok(meta)
}

#[tauri::command]
pub fn notes_create(window: WebviewWindow, app: AppHandle, path: String, content: Option<String>) -> Result<NoteMeta, String> {
    let path = normalize(&path).ok_or("invalidPath")?;
    let id = uuid();
    write_atomic(&file(&app, &id), content.as_deref().unwrap_or(""))?;
    let meta = with_index(&app, |idx| {
        let t = now();
        let m = NoteMeta { id: id.clone(), path: unique(idx, &path, None), created: t, updated: t, pinned: false, trashed: None };
        idx.notes.insert(id.clone(), m.clone());
        save(&app, idx);
        m
    });
    changed(&app, Some(&id), window.label());
    Ok(meta)
}

/// Renommer ou déplacer une note (nouveau chemin complet).
#[tauri::command]
pub fn notes_move(window: WebviewWindow, app: AppHandle, id: String, path: String) -> Result<NoteMeta, String> {
    let path = normalize(&path).ok_or("invalidPath")?;
    let meta = with_index(&app, |idx| {
        let free = unique(idx, &path, Some(&id));
        let n = idx.notes.get_mut(&id)?;
        n.path = free;
        n.updated = now();
        let m = n.clone();
        save(&app, idx);
        Some(m)
    })
    .ok_or("notFound")?;
    changed(&app, Some(&id), window.label());
    Ok(meta)
}

#[tauri::command]
pub fn notes_pin(window: WebviewWindow, app: AppHandle, id: String, pinned: bool) -> Result<(), String> {
    with_index(&app, |idx| {
        let n = idx.notes.get_mut(&id)?;
        n.pinned = pinned;
        save(&app, idx);
        Some(())
    })
    .ok_or("notFound")?;
    changed(&app, Some(&id), window.label());
    Ok(())
}

/// Corbeille (`trash = true`), restauration (`false`).
#[tauri::command]
pub fn notes_trash(window: WebviewWindow, app: AppHandle, id: String, trash: bool) -> Result<(), String> {
    with_index(&app, |idx| {
        let restored_path = {
            let n = idx.notes.get(&id)?;
            unique(idx, &n.path, Some(&id))
        };
        let n = idx.notes.get_mut(&id)?;
        n.trashed = trash.then(now);
        if !trash {
            n.path = restored_path;
        }
        save(&app, idx);
        Some(())
    })
    .ok_or("notFound")?;
    changed(&app, Some(&id), window.label());
    Ok(())
}

/// Suppression définitive (depuis la corbeille).
#[tauri::command]
pub fn notes_purge(window: WebviewWindow, app: AppHandle, id: String) -> Result<(), String> {
    with_index(&app, |idx| {
        idx.notes.remove(&id)?;
        save(&app, idx);
        Some(())
    })
    .ok_or("notFound")?;
    let _ = std::fs::remove_file(file(&app, &id));
    changed(&app, Some(&id), window.label());
    Ok(())
}

#[tauri::command]
pub fn notes_folder_create(window: WebviewWindow, app: AppHandle, path: String) -> Result<String, String> {
    let path = normalize(&path).ok_or("invalidPath")?;
    with_index(&app, |idx| {
        idx.folders.insert(path.clone());
        save(&app, idx);
    });
    changed(&app, None, window.label());
    Ok(path)
}

/// Renomme / déplace un dossier : réécrit le préfixe des notes et des sous-dossiers.
#[tauri::command]
pub fn notes_folder_move(window: WebviewWindow, app: AppHandle, from: String, to: String) -> Result<(), String> {
    let (from, to) = (normalize(&from).ok_or("invalidPath")?, normalize(&to).ok_or("invalidPath")?);
    if to == from || to.starts_with(&format!("{from}/")) {
        return Err("invalidPath".into());
    }
    let re = |p: &str| -> Option<String> {
        if p == from {
            Some(to.clone())
        } else {
            p.strip_prefix(&format!("{from}/")).map(|rest| format!("{to}/{rest}"))
        }
    };
    with_index(&app, |idx| {
        for n in idx.notes.values_mut() {
            if let Some(p) = n.path.rsplit_once('/').and_then(|(folder, name)| re(folder).map(|f| format!("{f}/{name}"))) {
                n.path = p;
            }
        }
        idx.folders = idx.folders.iter().map(|f| re(f).unwrap_or_else(|| f.clone())).collect();
        save(&app, idx);
    });
    changed(&app, None, window.label());
    Ok(())
}

/// Supprime un dossier : ses notes (sous-dossiers compris) vont à la corbeille.
#[tauri::command]
pub fn notes_folder_delete(window: WebviewWindow, app: AppHandle, path: String) -> Result<(), String> {
    let path = normalize(&path).ok_or("invalidPath")?;
    let inside = |p: &str| p == path || p.starts_with(&format!("{path}/"));
    with_index(&app, |idx| {
        let t = now();
        for n in idx.notes.values_mut() {
            if n.trashed.is_none() && n.path.rsplit_once('/').is_some_and(|(folder, _)| inside(folder)) {
                n.trashed = Some(t);
            }
        }
        idx.folders.retain(|f| !inside(f));
        save(&app, idx);
    });
    changed(&app, None, window.label());
    Ok(())
}

/// Recherche plein texte (chemin + contenu), insensible à la casse. Renvoie des ids.
#[tauri::command]
pub async fn notes_search(app: AppHandle, query: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let metas = with_index(&app, |idx| idx.notes.values().filter(|n| n.trashed.is_none()).cloned().collect::<Vec<_>>());
        Ok(metas
            .into_iter()
            .filter(|n| n.path.to_lowercase().contains(&q) || std::fs::read_to_string(file(&app, &n.id)).is_ok_and(|t| t.to_lowercase().contains(&q)))
            .map(|n| n.id)
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize(" projets / island // Idées ").as_deref(), Some("projets/island/Idées"));
        assert_eq!(normalize("a\\b\\..\\c").as_deref(), Some("a/b/c"));
        assert_eq!(normalize("Réunion : bilan").as_deref(), Some("Réunion : bilan"));
        assert_eq!(normalize("  /  "), None);
    }

    #[test]
    fn preview_strips_markdown() {
        assert_eq!(preview("# Titre\n\n- [ ] **faire** `ça`\n```rs\ncode\n```"), "Titre faire ça code");
    }

    #[test]
    fn unique_paths() {
        let mut idx = Index::default();
        idx.notes.insert("a".into(), NoteMeta { id: "a".into(), path: "Note".into(), created: 0, updated: 0, pinned: false, trashed: None });
        assert_eq!(unique(&idx, "Note", None), "Note 2");
        assert_eq!(unique(&idx, "note", Some("a")), "note");
    }
}
