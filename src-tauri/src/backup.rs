// Sauvegarde : export / import des réglages et des données des outils dans UN fichier JSON
// lisible (`island-sauvegarde-AAAA-MM-JJ.json`). Hors périmètre, volontairement :
// - les extensions (code, état, stockage `ext2.*`) ;
// - les secrets (Windows Hello, session Bitwarden, clés du trousseau) ;
// - ce qui dépend de la machine (placement du notch, géométrie des fenêtres).
//
// Import : par sections choisies, en REMPLACEMENT. Avant d'écrire quoi que ce soit, une
// sauvegarde complète de l'état actuel est faite dans `<app_data>/backups/` (filet de
// sécurité, les 10 dernières gardées). L'app redémarre ensuite : l'état en mémoire du front
// (qui réécrit ses données) ne doit pas écraser ce qui vient d'être importé.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};

const FORMAT: &str = "island-backup";
const VERSION: u32 = 1;
const STORE: &str = "__app__";
/// Sauvegardes de sécurité (avant import) gardées.
const KEEP_SAFETY: usize = 10;

/// D'où viennent les données d'une section.
enum Source {
    /// Clés du store `__app__`.
    Keys(&'static [&'static str]),
    /// Dossier des notes (index + fichiers Markdown).
    Notes,
    /// Fichier JSON du dossier de données de l'app.
    File(&'static str),
}

/// Sections, dans l'ordre d'affichage.
const SECTIONS: &[(&str, Source)] = &[
    ("settings", Source::Keys(&["notch", "lang", "theme", "physicalPresence"])),
    ("notes", Source::Notes),
    ("todos", Source::Keys(&["todos"])),
    ("budget", Source::Keys(&["budget"])),
    ("markets", Source::Keys(&["markets"])),
    ("weather", Source::Keys(&["weather"])),
    ("timer", Source::Keys(&["timer"])),
    ("shelf", Source::Keys(&["shelf"])),
    ("launcher", Source::File("launcher-usage.json")),
];

#[derive(Serialize, Deserialize)]
struct Backup {
    format: String,
    version: u32,
    /// Version d'Island qui a écrit le fichier.
    app: String,
    /// Horodatage (ms).
    created: i64,
    sections: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize, Default)]
struct NotesData {
    index: Value,
    /// id → contenu Markdown.
    files: BTreeMap<String, String>,
}

/// Aperçu d'une sauvegarde (avant import) ou résultat d'un export.
#[derive(Serialize)]
pub struct Summary {
    app: String,
    created: i64,
    /// Section → nombre d'éléments (notes, tâches…), 1 pour un bloc de réglages.
    sections: BTreeMap<String, usize>,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default()
}

fn notes_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("notes")
}

fn valid_note_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

// --- Lecture de l'état actuel ---------------------------------------------------------------

fn read_section(app: &AppHandle, source: &Source) -> Option<Value> {
    match source {
        Source::Keys(keys) => {
            let store = crate::store::read_store(app, STORE);
            let m: Map<String, Value> = keys.iter().filter_map(|k| store.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
            (!m.is_empty()).then_some(Value::Object(m))
        }
        Source::Notes => {
            let dir = notes_dir(app);
            let index: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).ok()?).ok()?;
            let files = index
                .get("notes")
                .and_then(Value::as_object)
                .map(|notes| {
                    notes
                        .keys()
                        .filter(|id| valid_note_id(id))
                        .map(|id| (id.clone(), std::fs::read_to_string(dir.join(format!("{id}.md"))).unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default();
            serde_json::to_value(NotesData { index, files }).ok()
        }
        Source::File(name) => std::fs::read_to_string(data_dir(app).join(name)).ok().and_then(|t| serde_json::from_str(&t).ok()),
    }
}

/// Nombre d'éléments d'une section (aperçu) : notes et tâches comptées, 1 pour le reste.
fn count(id: &str, v: &Value) -> usize {
    let len = |v: Option<&Value>| v.and_then(|x| x.as_array().map(Vec::len).or(x.as_object().map(Map::len))).unwrap_or(0);
    match id {
        "notes" => len(v.get("index").and_then(|i| i.get("notes"))),
        "todos" => len(v.get("todos").and_then(|t| t.get("items"))),
        _ => 1,
    }
}

fn summary(b: &Backup) -> Summary {
    Summary { app: b.app.clone(), created: b.created, sections: b.sections.iter().map(|(k, v)| (k.clone(), count(k, v))).collect() }
}

fn snapshot(app: &AppHandle, wanted: Option<&[String]>) -> Backup {
    let sections = SECTIONS
        .iter()
        .filter(|(id, _)| wanted.is_none_or(|w| w.iter().any(|x| x == id)))
        .filter_map(|(id, src)| read_section(app, src).map(|v| (id.to_string(), v)))
        .collect();
    Backup { format: FORMAT.into(), version: VERSION, app: app.package_info().version.to_string(), created: now(), sections }
}

fn read_backup(path: &str) -> Result<Backup, String> {
    let text = std::fs::read_to_string(path).map_err(|_| "unreadable")?;
    let b: Backup = serde_json::from_str(&text).map_err(|_| "invalid")?;
    if b.format != FORMAT {
        return Err("invalid".into());
    }
    if b.version > VERSION {
        return Err("tooNew".into());
    }
    Ok(b)
}

// --- Écriture -------------------------------------------------------------------------------

fn write_section(app: &AppHandle, source: &Source, v: &Value) -> Result<(), String> {
    match source {
        Source::Keys(keys) => {
            // Remplacement : les clés absentes de la sauvegarde sont retirées.
            for k in *keys {
                crate::store::set(app, STORE, k, v.get(*k).cloned());
            }
            Ok(())
        }
        Source::Notes => {
            let data: NotesData = serde_json::from_value(v.clone()).map_err(|_| "invalid")?;
            let dir = notes_dir(app);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Remplacement : les notes actuelles partent (elles sont dans la sauvegarde de sécurité).
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if e.path().extension().is_some_and(|x| x == "md") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            for (id, content) in &data.files {
                if valid_note_id(id) {
                    std::fs::write(dir.join(format!("{id}.md")), content).map_err(|e| e.to_string())?;
                }
            }
            std::fs::write(dir.join("index.json"), serde_json::to_string_pretty(&data.index).unwrap_or_default()).map_err(|e| e.to_string())?;
            crate::services::notes::forget_index();
            Ok(())
        }
        Source::File(name) => std::fs::write(data_dir(app).join(name), serde_json::to_string(v).unwrap_or_default()).map_err(|e| e.to_string()),
    }
}

/// Sauvegarde de sécurité de tout l'état actuel, avant un import.
fn safety_copy(app: &AppHandle) -> Result<(), String> {
    let dir = data_dir(app).join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let b = snapshot(app, None);
    std::fs::write(dir.join(format!("avant-import-{}.json", b.created)), serde_json::to_string_pretty(&b).unwrap_or_default()).map_err(|e| e.to_string())?;
    // On ne garde que les plus récentes (le nom contient l'horodatage : ordre lexical = ordre).
    let mut old: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("avant-import-"))).collect())
        .unwrap_or_default();
    old.sort();
    let extra = old.len().saturating_sub(KEEP_SAFETY);
    for p in old.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

// --- Commandes ------------------------------------------------------------------------------

/// Sections connues, dans l'ordre d'affichage, avec ce qu'elles contiennent actuellement.
#[tauri::command]
pub fn backup_current(app: AppHandle) -> Summary {
    let b = snapshot(&app, None);
    let mut s = summary(&b);
    for (id, _) in SECTIONS {
        s.sections.entry(id.to_string()).or_insert(0);
    }
    s
}

/// Écrit la sauvegarde des sections choisies. Extension `.json` imposée.
#[tauri::command]
pub fn backup_export(app: AppHandle, path: String, sections: Vec<String>) -> Result<Summary, String> {
    if !path.to_lowercase().ends_with(".json") {
        return Err("notJson".into());
    }
    let b = snapshot(&app, Some(&sections));
    std::fs::write(&path, serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(summary(&b))
}

/// Aperçu d'un fichier de sauvegarde (version, date, contenu).
#[tauri::command]
pub fn backup_inspect(path: String) -> Result<Summary, String> {
    read_backup(&path).map(|b| summary(&b))
}

/// Importe les sections choisies (remplacement), après une sauvegarde de sécurité, puis
/// redémarre l'app.
#[tauri::command]
pub fn backup_import(app: AppHandle, path: String, sections: Vec<String>) -> Result<(), String> {
    let b = read_backup(&path)?;
    safety_copy(&app)?;
    for (id, src) in SECTIONS {
        if !sections.iter().any(|s| s == id) {
            continue;
        }
        if let Some(v) = b.sections.get(*id) {
            write_section(&app, src, v)?;
        }
    }
    app.restart();
}

/// Ouvre le dossier des sauvegardes de sécurité dans l'Explorateur.
#[tauri::command]
pub fn backup_open_folder(app: AppHandle) {
    let dir = data_dir(&app).join("backups");
    let _ = std::fs::create_dir_all(&dir);
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, content: &str) -> String {
        let p = std::env::temp_dir().join(format!("island-backup-test-{name}.json"));
        std::fs::write(&p, content).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn rejects_foreign_and_newer_files() {
        assert_eq!(read_backup(&tmp("foreign", r#"{"hello":1}"#)).err().as_deref(), Some("invalid"));
        let other = r#"{"format":"autre","version":1,"app":"2.0.0","created":0,"sections":{}}"#;
        assert_eq!(read_backup(&tmp("other", other)).err().as_deref(), Some("invalid"));
        let newer = r#"{"format":"island-backup","version":99,"app":"9.0.0","created":0,"sections":{}}"#;
        assert_eq!(read_backup(&tmp("newer", newer)).err().as_deref(), Some("tooNew"));
        assert_eq!(read_backup("Z:/nope/absent.json").err().as_deref(), Some("unreadable"));
    }

    #[test]
    fn summary_counts_notes_and_todos() {
        let ok = r#"{"format":"island-backup","version":1,"app":"2.0.1","created":5,"sections":{
            "notes":{"index":{"notes":{"a":{},"b":{}}},"files":{"a":"x","b":"y"}},
            "todos":{"todos":{"items":[1,2,3],"sent":[]}},
            "settings":{"notch":{}}}}"#;
        let s = summary(&read_backup(&tmp("ok", ok)).unwrap());
        assert_eq!(s.sections["notes"], 2);
        assert_eq!(s.sections["todos"], 3);
        assert_eq!(s.sections["settings"], 1);
    }

    #[test]
    fn note_ids_are_safe_file_names() {
        assert!(valid_note_id("51fbe8a7438892470a4505f11237bb17"));
        assert!(!valid_note_id("../../evil"));
        assert!(!valid_note_id("a/b"));
        assert!(!valid_note_id(""));
    }
}
