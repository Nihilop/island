// Service `capture` : screenshot + enregistrement vidéo. La couche commande (ici)
// est cross-platform ; l'implémentation native (Windows Graphics Capture + ffmpeg)
// vit dans `windows.rs`. L'ENCODAGE est délégué à l'extension (binaire + args fournis,
// gated `native-encoder`, cantonné au dossier de l'extension) : l'hôte ne fait que
// capturer les frames et les piper.

use tauri::{AppHandle, Emitter, Manager};

use crate::ext::require_perm;

#[cfg(target_os = "windows")]
mod windows;

#[derive(serde::Serialize)]
pub struct DisplayInfo {
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Zone de capture (pixels physiques, relatif au moniteur).
#[derive(serde::Deserialize, Clone, Copy)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Encodeur fourni par l'extension : id (résolution du dossier) + binaire relatif + args.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSpec {
    ext_id: String,
    bin: String,
    args: Vec<String>,
    /// Codec audio pour le mux (ex. `["-c:a","aac","-b:a","160k"]`). Absent = défaut.
    audio_args: Option<Vec<String>>,
}

/// Liste les écrans disponibles (pour le picker « Écran 1 / Écran 2 »). Gated `capture`.
#[tauri::command]
pub fn capture_list_displays(app: AppHandle, ext_id: String) -> Vec<DisplayInfo> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "capture") {
        return Vec::new();
    }
    #[cfg(target_os = "windows")]
    {
        windows::list_displays()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// Screenshot d'un écran (1-based ; `None` = principal) → PNG dans Images/Island.
/// `exclude_island = true` (défaut côté SDK) cache l'overlay de la capture.
#[tauri::command]
pub fn capture_screenshot(
    app: AppHandle,
    ext_id: String,
    display: Option<usize>,
    exclude_island: bool,
    dir: Option<String>,
    region: Option<Region>,
) -> Result<String, String> {
    require_perm!(&app, &ext_id, "capture");
    let base = match dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => std::path::PathBuf::from(d),
        None => app.path().picture_dir().map_err(|e| e.to_string())?.join("Island"),
    };
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = base.join(format!("capture-{ts}.png"));
    let path_str = path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        if exclude_island {
            crate::overlay::set_overlay_excluded(&app, true);
            // Laisse DWM recomposer le bureau sans l'overlay avant de capturer.
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        let res = windows::screenshot(display, path_str.clone(), region);
        if exclude_island {
            crate::overlay::set_overlay_excluded(&app, false);
        }
        res?;
        Ok(path_str)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (display, exclude_island, path_str, region);
        Err("capture: Windows uniquement".to_string())
    }
}

/// Démarre l'enregistrement vidéo. L'ENCODAGE est délégué à l'extension (binaire +
/// args), gated par la permission `native-encoder` ET cantonné au dossier de
/// l'extension. L'hôte ne fait que CAPTURER les frames et piper.
#[tauri::command]
pub fn capture_start_recording(
    app: AppHandle,
    ext_id: String,
    display: Option<usize>,
    exclude_island: bool,
    dir: Option<String>,
    region: Option<Region>,
    fps: Option<u32>,
    audio: Option<bool>,
    encoder: EncoderSpec,
) -> Result<String, String> {
    // Gated `capture` (l'extension appelante) ET `native-encoder` (propriétaire du binaire).
    require_perm!(&app, &ext_id, "capture");
    if !crate::ext::ext_has_permission(&app, &encoder.ext_id, "native-encoder") {
        return Err("capture: l'extension n'a pas la permission « native-encoder »".into());
    }
    let bin = crate::ext::resolve_in_ext(&app, &encoder.ext_id, &encoder.bin)?;
    if !bin.exists() {
        return Err("capture: encodeur absent (à télécharger d'abord)".into());
    }

    let base = match dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => std::path::PathBuf::from(d),
        None => app.path().video_dir().map_err(|e| e.to_string())?.join("Island"),
    };
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path_str = base.join(format!("record-{ts}.mp4")).to_string_lossy().to_string();
    let fps = fps.unwrap_or(30).clamp(1, 240);
    let audio = audio.unwrap_or(false);

    #[cfg(target_os = "windows")]
    {
        let enc = windows::Encoder {
            bin: bin.to_string_lossy().to_string(),
            args: encoder.args,
            audio_args: encoder.audio_args.unwrap_or_default(),
        };
        if exclude_island {
            crate::overlay::set_overlay_excluded(&app, true);
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        match windows::start_recording(display, path_str.clone(), region, fps, audio, enc) {
            Ok(()) => Ok(path_str),
            Err(e) => {
                if exclude_island {
                    crate::overlay::set_overlay_excluded(&app, false);
                }
                Err(e)
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (display, exclude_island, path_str, region, fps, audio, encoder, bin);
        Err("capture: Windows uniquement".to_string())
    }
}

/// Télécharge un binaire DANS le dossier de l'extension (gated `native-encoder`).
/// `zip_entry` présent = l'URL pointe un ZIP → on extrait cette entrée (suffixe).
/// Émet `encoder://download { extId, percent, done }` pour la progression.
#[tauri::command]
pub async fn ext_fetch_binary(
    app: AppHandle,
    ext_id: String,
    url: String,
    dest: String,
    zip_entry: Option<String>,
) -> Result<(), String> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "native-encoder") {
        return Err("téléchargement: permission « native-encoder » requise".into());
    }
    let dest_path = crate::ext::resolve_in_ext(&app, &ext_id, &dest)?;
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let app2 = app.clone();
    let ext2 = ext_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        download_binary(&app2, &ext2, &url, &dest_path, zip_entry.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Téléchargement bloquant (ureq) + extraction zip optionnelle + events de progression.
fn download_binary(
    app: &AppHandle,
    ext_id: &str,
    url: &str,
    dest: &std::path::Path,
    zip_entry: Option<&str>,
) -> Result<(), String> {
    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let total: u64 = resp.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut reader = resp.into_reader();

    let tmp = dest.with_extension("part");
    {
        let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 65536];
        let mut done: u64 = 0;
        let mut last: u64 = 0;
        loop {
            let n = std::io::Read::read(&mut reader, &mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            // ~1 event par % (évite de spammer l'UI).
            if total > 0 && done - last >= total / 100 {
                last = done;
                let pct = (done as f64 / total as f64 * 100.0) as u32;
                let _ = app.emit("encoder://download", serde_json::json!({ "extId": ext_id, "percent": pct }));
            }
        }
    }

    if let Some(suffix) = zip_entry {
        let file = std::fs::File::open(&tmp).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut idx = None;
        for i in 0..zip.len() {
            let name = zip.by_index(i).map_err(|e| e.to_string())?.name().replace('\\', "/");
            if name.ends_with(suffix) {
                idx = Some(i);
                break;
            }
        }
        let i = idx.ok_or("téléchargement: entrée introuvable dans le zip")?;
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
    } else {
        std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    }

    let _ = app.emit("encoder://download", serde_json::json!({ "extId": ext_id, "percent": 100, "done": true }));
    Ok(())
}

/// Un enregistrement est-il en cours ? (resync de l'UI au (re)montage d'une vue). Gated `capture`.
#[tauri::command]
pub fn capture_is_recording(app: AppHandle, ext_id: String) -> bool {
    if !crate::ext::ext_has_permission(&app, &ext_id, "capture") {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        windows::is_recording()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Arrête l'enregistrement, finalise le MP4, restaure l'overlay, renvoie le chemin.
#[tauri::command]
pub fn capture_stop_recording(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let res = windows::stop_recording();
        crate::overlay::set_overlay_excluded(&app, false);
        res
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("capture: Windows uniquement".to_string())
    }
}
