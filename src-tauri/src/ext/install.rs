// Cycle de vie des extensions : packaging (.island = zip), installation, listing,
// lecture de fichiers (loader PROD), association de fichier .island (Windows),
// ouverture au double-clic, et live-reload du `dist/`. Cross-platform sauf l'assoc.

use tauri::{AppHandle, Emitter, Manager};

use super::sanitize;
use super::storage::{read_store, write_store};

fn read_manifest_from_island(path: &str) -> Result<serde_json::Value, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut mf = zip.by_name("manifest.json").map_err(|_| "manifest.json introuvable dans le .island".to_string())?;
    let mut s = String::new();
    std::io::Read::read_to_string(&mut mf, &mut s).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct InstalledExt {
    id: String,
    dir: String,
    manifest: serde_json::Value,
    dev: bool, // a une source (package.json) → projet en dev, packageable
}

// Ajoute un fichier au zip.
fn zip_add(
    zip: &mut zip::ZipWriter<std::fs::File>,
    src: &std::path::Path,
    name: &str,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    use std::io::Write;
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    zip.start_file(name, opts).map_err(|e| e.to_string())?;
    zip.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}
// Ajoute récursivement un dossier au zip (préfixe = chemin dans l'archive).
fn zip_add_dir(
    zip: &mut zip::ZipWriter<std::fs::File>,
    dir: &std::path::Path,
    prefix: &str,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        if p.is_dir() {
            zip_add_dir(zip, &p, &name, opts)?;
        } else {
            zip_add(zip, &p, &name, opts)?;
        }
    }
    Ok(())
}

/// Empaquette une extension installée (manifest.json + dist/) en `.island` vers
/// `out_path`. L'app ne COMPILE pas : elle zippe le build déjà produit.
#[tauri::command]
pub fn pack_extension(app: AppHandle, id: String, out_path: String) -> Result<(), String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("extensions")
        .join(sanitize(&id));
    let manifest = base.join("manifest.json");
    let dist = base.join("dist");
    if !manifest.exists() || !dist.exists() {
        return Err("manifest.json ou dist/ manquant — build l'extension d'abord".to_string());
    }

    let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip_add(&mut zip, &manifest, "manifest.json", opts)?;
    zip_add_dir(&mut zip, &dist, "dist", opts)?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Lit un fichier texte d'une extension installée (loader PROD : le front l'importe
/// via une Blob URL). Cantonné au dossier de l'extension (même garde que `resolve_in_ext` :
/// pas de `..`, pas de chemin absolu / lettre de lecteur, jamais hors du dossier).
#[tauri::command]
pub fn read_ext_file(app: AppHandle, id: String, file: String) -> Result<String, String> {
    let path = super::resolve_in_ext(&app, &id, &file)?;
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Scanne `%APPDATA%/island/extensions/` et renvoie les extensions installées
/// (id + chemin absolu du dossier + manifeste). Le front charge ensuite l'entrée
/// soit via `/@fs/` (mode dev, HMR), soit via son dist (prod).
#[tauri::command]
pub fn list_installed(app: AppHandle) -> Vec<InstalledExt> {
    let mut out = Vec::new();
    let base = match app.path().app_config_dir() {
        Ok(d) => d.join("extensions"),
        Err(_) => return out,
    };
    let entries = match std::fs::read_dir(&base) {
        Ok(r) => r,
        Err(_) => return out, // dossier absent = aucune extension installée
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = match std::fs::read_to_string(dir.join("manifest.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        let id = manifest.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let dev = dir.join("package.json").exists();
        out.push(InstalledExt { id, dir: dir.to_string_lossy().to_string(), manifest, dev });
    }
    out
}

/// Ouvre la fenêtre d'installation avec les infos du paquet.
#[tauri::command]
pub fn open_install(app: AppHandle, path: String) -> Result<(), String> {
    let manifest = read_manifest_from_island(&path)?;
    if let Some(w) = app.get_webview_window("install") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        w.emit("install://open", serde_json::json!({ "manifest": manifest, "path": path }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Extrait le .island dans `%APPDATA%/island/extensions/<id>/` et l'active.
#[tauri::command]
pub fn install_island(app: AppHandle, path: String) -> Result<String, String> {
    let manifest = read_manifest_from_island(&path)?;
    let id = manifest.get("id").and_then(|v| v.as_str()).ok_or("manifeste sans 'id'")?.to_string();
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("extensions")
        .join(sanitize(&id));

    let _ = std::fs::remove_dir_all(&dir); // réinstall propre
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        // `enclosed_name` confine l'entrée au dossier : rejette absolu / `..` / lettre de
        // lecteur / UNC. (Le simple `contains("..")` laissait passer les chemins ABSOLUS,
        // qui via `PathBuf::join` écrivaient HORS du dossier d'extension → write arbitraire.)
        let out = match entry.enclosed_name() {
            Some(rel) => dir.join(rel),
            None => continue, // entrée hostile → ignorée
        };
        if !out.starts_with(&dir) {
            continue; // défense en profondeur
        }
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out);
        } else {
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).map_err(|e| e.to_string())?;
            std::fs::write(&out, buf).map_err(|e| e.to_string())?;
        }
    }

    // Active l'extension (si une liste explicite existe ; sinon "tout activé" l'inclut déjà).
    let mut m = read_store(&app, "__app__");
    if let Some(v) = m.get("enabled") {
        let mut enabled: Vec<String> = serde_json::from_value(v.clone()).unwrap_or_default();
        if !enabled.contains(&id) {
            enabled.push(id.clone());
            m.insert("enabled".into(), serde_json::json!(enabled));
            write_store(&app, "__app__", &m);
        }
    }

    // À la première install, on associe aussi les .island à Island (best-effort).
    #[cfg(target_os = "windows")]
    let _ = register_island_assoc(&app);

    Ok(id)
}

// --- Association de fichier .island + ouverture au double-clic ----------------

/// Écrit l'association `.island → Island` dans HKCU (per-utilisateur, sans admin).
#[cfg(target_os = "windows")]
fn register_island_assoc(app: &AppHandle) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    // Icône dédiée aux .island si elle est bundlée (resource), sinon celle de l'exe.
    let icon = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("icons").join("island-file.ico"))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{exe},0"));

    let classes = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Classes", KEY_READ | KEY_WRITE)
        .map_err(|e| e.to_string())?;

    classes
        .create_subkey(".island")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &"Island.Extension")
        .map_err(|e| e.to_string())?;

    let prog = classes.create_subkey("Island.Extension").map_err(|e| e.to_string())?.0;
    prog.set_value("", &"Extension Island").map_err(|e| e.to_string())?;
    prog.create_subkey("DefaultIcon")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &icon)
        .map_err(|e| e.to_string())?;
    prog.create_subkey("shell\\open\\command")
        .map_err(|e| e.to_string())?
        .0
        .set_value("", &format!("\"{exe}\" \"%1\""))
        .map_err(|e| e.to_string())?;

    // Notifie le shell que les associations ont changé (prise en compte immédiate).
    unsafe {
        use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
    Ok(())
}

/// Bouton « Associer les .island » (Réglages) / appel manuel.
#[tauri::command]
pub fn register_file_association(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        register_island_assoc(&app)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Association : Windows uniquement".to_string())
    }
}

// Chemin d'un .island passé en double-clic AU DÉMARRAGE (la webview install n'est
// pas encore prête → elle le récupère via `take_pending_install` à son montage).
static PENDING_INSTALL: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn take_pending_install() -> Result<Option<serde_json::Value>, String> {
    let path = match PENDING_INSTALL.lock().unwrap_or_else(|p| p.into_inner()).take() {
        Some(p) => p,
        None => return Ok(None),
    };
    let manifest = read_manifest_from_island(&path)?;
    Ok(Some(serde_json::json!({ "manifest": manifest, "path": path })))
}

/// Repère un `.island` dans l'argv et déclenche la modal d'install.
/// `running` = l'app tournait déjà (la webview install écoute) → on émet direct ;
/// sinon (démarrage) → on stocke en pending + on montre la fenêtre.
pub(crate) fn handle_island_argv(app: &AppHandle, argv: &[String], running: bool) {
    let Some(path) = argv.iter().find(|a| a.to_lowercase().ends_with(".island")) else {
        return;
    };
    if running {
        let _ = open_install(app.clone(), path.clone());
    } else {
        *PENDING_INSTALL.lock().unwrap_or_else(|p| p.into_inner()) = Some(path.clone());
        if let Some(w) = app.get_webview_window("install") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

/// Surveille `%APPDATA%/<identifier>/extensions/` et émet `ext://changed { dir }`
/// quand un fichier d'un dossier `dist/` change (= une extension a été rebuildée).
/// Le front recharge alors cette extension (live-reload du `pnpm dev`).
pub(crate) fn start_dist_watcher(app: AppHandle) {
    use notify::RecursiveMode;
    use notify_debouncer_mini::new_debouncer;
    use std::time::Duration;

    let base = match app.path().app_config_dir() {
        Ok(d) => d.join("extensions"),
        Err(_) => return,
    };
    let _ = std::fs::create_dir_all(&base); // le watch échoue si le dossier n'existe pas

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = match new_debouncer(Duration::from_millis(300), tx) {
            Ok(d) => d,
            Err(_) => return,
        };
        if debouncer.watcher().watch(&base, RecursiveMode::Recursive).is_err() {
            return;
        }
        // Boucle bloquante : garde le debouncer vivant tant que l'app tourne.
        for res in rx {
            let events = match res {
                Ok(e) => e,
                Err(_) => continue,
            };
            // Racines d'extension (enfant direct de base) dont un fichier `dist/` a bougé.
            let mut dirs: std::collections::HashSet<std::path::PathBuf> = Default::default();
            for ev in events {
                if !ev.path.components().any(|c| c.as_os_str() == "dist") {
                    continue; // on ne réagit qu'aux changements du build, pas de la source
                }
                if let Ok(rel) = ev.path.strip_prefix(&base) {
                    if let Some(first) = rel.components().next() {
                        dirs.insert(base.join(first));
                    }
                }
            }
            for dir in dirs {
                let _ = app.emit("ext://changed", serde_json::json!({ "dir": dir.to_string_lossy() }));
            }
        }
    });
}
