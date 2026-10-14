// Fichiers pour l'Étagère (et plus tard le Convertisseur) : infos sur des chemins,
// ouvrir avec l'app associée, copier une liste de fichiers dans le presse-papiers, et
// coller depuis le presse-papiers (fichiers, ou image enregistrée en PNG).

use std::path::Path;

#[derive(serde::Serialize)]
pub struct FileInfo {
    path: String,
    name: String,
    ext: String,
    size: u64,
    /// image | audio | video | document | sheet | archive | code | other
    category: &'static str,
    thumb: Option<String>,
}

fn category(ext: &str) -> &'static str {
    match ext {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico" | "tif" | "tiff" | "dds" | "tga" | "svg" | "heic" => "image",
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wma" => "audio",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "wmv" | "m4v" => "video",
        "pdf" | "txt" | "md" | "markdown" | "html" | "htm" | "doc" | "docx" | "rtf" | "odt" => "document",
        "xlsx" | "xlsm" | "xls" | "ods" | "csv" | "tsv" => "sheet",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        "rs" | "ts" | "js" | "vue" | "py" | "json" | "toml" | "yaml" | "yml" | "c" | "cpp" | "cs" | "java" | "go" => "code",
        _ => "other",
    }
}

/// Infos sur des fichiers (dossiers et chemins invalides ignorés).
#[tauri::command]
pub async fn files_probe(paths: Vec<String>) -> Vec<FileInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .filter_map(|path| {
                let md = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
                let p = Path::new(&path);
                let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                Some(FileInfo {
                    name: p.file_name()?.to_string_lossy().into_owned(),
                    category: category(&ext),
                    thumb: super::cliphist::file_thumb(&path, 240),
                    size: md.len(),
                    ext,
                    path,
                })
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Ouvre un fichier avec l'app associée.
#[tauri::command]
pub fn file_open(app: tauri::AppHandle, path: String) -> Result<(), String> {
    if !Path::new(&path).is_file() {
        return Err("missing".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// Copie une liste de fichiers (à coller dans l'Explorateur).
#[tauri::command]
pub fn files_copy(paths: Vec<String>) -> Result<(), String> {
    if paths.iter().any(|p| !Path::new(p).exists()) {
        return Err("missing".into());
    }
    arboard::Clipboard::new()
        .and_then(|mut c| c.set().file_list(&paths))
        .map_err(|e| e.to_string())
}

/// Ctrl+V : fichiers du presse-papiers, ou image enregistrée en PNG dans
/// `Images\Island\Collé AAAA-MM-JJ HH-MM-SS.png`. Vide si rien d'exploitable.
#[tauri::command]
pub async fn files_from_clipboard(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Manager;
    let pictures = app.path().picture_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        if let Ok(files) = cb.get().file_list() {
            if !files.is_empty() {
                return Ok(files.iter().map(|p| p.to_string_lossy().into_owned()).collect());
            }
        }
        let Ok(img) = cb.get_image() else { return Ok(vec![]) };
        let dir = pictures.join("Island");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = chrono_like_stamp();
        let path = dir.join(format!("Collé {stamp}.png"));
        let png = super::cliphist::encode_png(&img.bytes, img.width as u32, img.height as u32).ok_or("encode")?;
        std::fs::write(&path, png).map_err(|e| e.to_string())?;
        Ok(vec![path.to_string_lossy().into_owned()])
    })
    .await
    .map_err(|e| e.to_string())?
}

/// « AAAA-MM-JJ HH-MM-SS » en heure locale (sans dépendance de date).
fn chrono_like_stamp() -> String {
    #[cfg(target_os = "windows")]
    {
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
    }
    #[cfg(not(target_os = "windows"))]
    format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}
