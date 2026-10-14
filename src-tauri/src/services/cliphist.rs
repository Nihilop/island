// Historique du presse-papiers (outil intégré) — EN MÉMOIRE uniquement (vide au
// redémarrage) : textes, liens, images, fichiers.
//
// Thread 300 ms sur `GetClipboardSequenceNumber` ; lecture seulement si le numéro change ;
// le contenu présent au démarrage est ignoré. Les copies marquées par les gestionnaires
// de mots de passe (dont la copie privée de notre coffre) sont ignorées. Priorité :
// fichiers > (texte + image : si le texte est une URL seule l'image gagne — « copier
// l'image » des navigateurs — sinon le texte gagne — Office) > image seule.
//
// Limites : 40 entrées, 8 images, aperçu texte 400 caractères, vignette 240 px, aperçu
// grand format 1400 px. Images stockées en PNG compressé rapide.
//
// Modes pilotés par le front : `all` (outil actif), `off` (rien n'est lu). Changer de mode
// purge ce qui n'est plus conservé.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use tauri::{AppHandle, Emitter};

const MAX_ENTRIES: usize = 40;
const MAX_IMAGES: usize = 8;
const PREVIEW_CHARS: usize = 400;
const THUMB: u32 = 240;
const LARGE: u32 = 1400;
const MAX_IMAGE_FILE: u64 = 40 * 1024 * 1024;

const MODE_ALL: u8 = 0;
const MODE_OFF: u8 = 2;
static MODE: AtomicU8 = AtomicU8::new(MODE_ALL);
/// Dernier numéro de séquence traité (nos propres recopies le mettent à jour).
static SEQ: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Text,
    Link,
    Image,
    Files,
}

struct Entry {
    id: u64,
    kind: Kind,
    at: u64,
    text: Option<String>,
    files: Vec<String>,
    png: Option<Vec<u8>>,
    size: Option<(u32, u32)>,
    thumb: Option<String>,
    /// Empreinte pour la déduplication.
    key: u64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    id: u64,
    kind: Kind,
    at: u64,
    preview: Option<String>,
    files: Vec<String>,
    thumb: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

static HISTORY: Mutex<VecDeque<Entry>> = Mutex::new(VecDeque::new());
static NEXT_ID: Mutex<u64> = Mutex::new(1);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn hash<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn is_link(t: &str) -> bool {
    let t = t.trim();
    !t.contains(char::is_whitespace) && (t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www."))
}

pub fn encode_png(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        enc.write_header().ok()?.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// Vignette PNG (côté long `max`) en data URL.
pub fn thumb_data_url(img: &image::DynamicImage, max: u32) -> Option<String> {
    let t = if img.width().max(img.height()) > max { img.thumbnail(max, max) } else { img.clone() };
    let rgba = t.to_rgba8();
    let png = encode_png(rgba.as_raw(), rgba.width(), rgba.height())?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

const IMAGE_EXT: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"];

/// Vignette d'un fichier image (≤ 40 Mo).
pub fn file_thumb(path: &str, max: u32) -> Option<String> {
    let ext = std::path::Path::new(path).extension()?.to_string_lossy().to_lowercase();
    if !IMAGE_EXT.contains(&ext.as_str()) || std::fs::metadata(path).ok()?.len() > MAX_IMAGE_FILE {
        return None;
    }
    thumb_data_url(&image::open(path).ok()?, max)
}

fn meta(e: &Entry) -> Meta {
    Meta {
        id: e.id,
        kind: e.kind,
        at: e.at,
        preview: e.text.as_ref().map(|t| t.chars().take(PREVIEW_CHARS).collect()),
        files: e.files.clone(),
        thumb: e.thumb.clone(),
        width: e.size.map(|s| s.0),
        height: e.size.map(|s| s.1),
    }
}

fn emit(app: &AppHandle) {
    let list: Vec<Meta> = HISTORY.lock().unwrap_or_else(|p| p.into_inner()).iter().map(meta).collect();
    let _ = app.emit("clip://changed", list);
}

/// Ajoute en tête ; une entrée identique existante remonte (pas de doublon).
fn push(mut e: Entry) {
    let mut h = HISTORY.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(pos) = h.iter().position(|x| x.key == e.key && x.kind == e.kind) {
        let mut old = h.remove(pos).unwrap();
        old.at = e.at;
        h.push_front(old);
        return;
    }
    let mut id = NEXT_ID.lock().unwrap_or_else(|p| p.into_inner());
    e.id = *id;
    *id += 1;
    h.push_front(e);
    // 8 images max (les plus récentes), 40 entrées max.
    let mut images = 0;
    h.retain(|x| {
        if x.kind == Kind::Image {
            images += 1;
            images <= MAX_IMAGES
        } else {
            true
        }
    });
    h.truncate(MAX_ENTRIES);
}

// --- Lecture du presse-papiers -------------------------------------------------------

#[cfg(target_os = "windows")]
fn sequence() -> u32 {
    unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

/// Copie marquée par un gestionnaire de mots de passe → à ignorer.
#[cfg(target_os = "windows")]
fn excluded() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    unsafe {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
        if IsClipboardFormatAvailable(exclude).is_ok() || IsClipboardFormatAvailable(ignore).is_ok() {
            return true;
        }
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if IsClipboardFormatAvailable(history).is_err() || OpenClipboard(None).is_err() {
            return false;
        }
        let mut deny = false;
        if let Ok(h) = GetClipboardData(history) {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u32;
            if !p.is_null() {
                deny = *p == 0;
                let _ = GlobalUnlock(g);
            }
        }
        let _ = CloseClipboard();
        deny
    }
}

fn read_entry() -> Option<Entry> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let at = now_ms();
    if let Ok(files) = cb.get().file_list() {
        if !files.is_empty() {
            let files: Vec<String> = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            let thumb = files.iter().find_map(|f| file_thumb(f, THUMB));
            return Some(Entry { id: 0, kind: Kind::Files, at, text: None, key: hash(&files), files, png: None, size: None, thumb });
        }
    }
    let text = cb.get_text().ok().filter(|t| !t.trim().is_empty());
    let prefer_image = text.as_deref().map(is_link).unwrap_or(true);
    if prefer_image {
        if let Ok(img) = cb.get_image() {
            let (w, h) = (img.width as u32, img.height as u32);
            let key = hash(img.bytes.as_ref());
            let rgba = image::RgbaImage::from_raw(w, h, img.bytes.into_owned())?;
            let dynimg = image::DynamicImage::ImageRgba8(rgba);
            let thumb = thumb_data_url(&dynimg, THUMB);
            let png = encode_png(dynimg.as_bytes(), w, h);
            return Some(Entry { id: 0, kind: Kind::Image, at, text: None, files: vec![], png, size: Some((w, h)), thumb, key });
        }
    }
    let t = text?;
    let kind = if is_link(&t) { Kind::Link } else { Kind::Text };
    Some(Entry { id: 0, kind, at, key: hash(&t), text: Some(t), files: vec![], png: None, size: None, thumb: None })
}

pub fn start(app: AppHandle) {
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        // Le contenu présent au démarrage est ignoré.
        SEQ.store(sequence(), Ordering::SeqCst);
        loop {
            std::thread::sleep(Duration::from_millis(300));
            if MODE.load(Ordering::SeqCst) == MODE_OFF {
                SEQ.store(sequence(), Ordering::SeqCst);
                continue;
            }
            let seq = sequence();
            if seq == SEQ.load(Ordering::SeqCst) {
                continue;
            }
            if excluded() {
                SEQ.store(seq, Ordering::SeqCst);
                continue;
            }
            // Presse-papiers occupé : on réessaie au tick suivant.
            let Some(entry) = read_entry() else { continue };
            SEQ.store(seq, Ordering::SeqCst);
            push(entry);
            emit(&app);
        }
    });
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

// --- Commandes -----------------------------------------------------------------------

#[tauri::command]
pub fn clip_list() -> Vec<Meta> {
    HISTORY.lock().unwrap_or_else(|p| p.into_inner()).iter().map(meta).collect()
}

/// `all` | `off`. Passer à `off` vide l'historique (libère la mémoire).
#[tauri::command]
pub fn clip_set_mode(app: AppHandle, mode: String) {
    let m = if mode == "off" { MODE_OFF } else { MODE_ALL };
    if MODE.swap(m, Ordering::SeqCst) != m && m == MODE_OFF {
        HISTORY.lock().unwrap_or_else(|p| p.into_inner()).clear();
        emit(&app);
    }
}

/// Écrit une entrée dans le presse-papiers et la remonte en tête.
fn write_entry(id: u64) -> Result<(), String> {
    let (kind, text, files, png) = {
        let mut h = HISTORY.lock().unwrap_or_else(|p| p.into_inner());
        let pos = h.iter().position(|e| e.id == id).ok_or("notFound")?;
        let mut e = h.remove(pos).unwrap();
        e.at = now_ms();
        let data = (e.kind, e.text.clone(), e.files.clone(), e.png.clone());
        h.push_front(e);
        data
    };
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    match kind {
        Kind::Text | Kind::Link => {
            let text = text.unwrap_or_default();
            // Une autre app (ou notre relevé) peut tenir le presse-papiers : on relit et on
            // réessaie une fois si le contenu ne correspond pas.
            for attempt in 0..3 {
                cb.set_text(text.clone()).map_err(|e| e.to_string())?;
                if cb.get_text().map(|t| t == text).unwrap_or(false) {
                    break;
                }
                if attempt == 2 {
                    return Err("clipboardBusy".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
        }
        Kind::Files => cb.set().file_list(&files).map_err(|e| e.to_string())?,
        Kind::Image => {
            // Décompression à la demande (verrou relâché avant).
            let img = image::load_from_memory(&png.ok_or("notFound")?).map_err(|e| e.to_string())?.to_rgba8();
            cb.set_image(arboard::ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: img.into_raw().into(),
            })
            .map_err(|e| e.to_string())?;
        }
    }
    // Notre propre recopie ne doit pas créer de doublon.
    #[cfg(target_os = "windows")]
    SEQ.store(sequence(), Ordering::SeqCst);
    Ok(())
}

/// Recopie une entrée dans le presse-papiers ; elle remonte en tête.
#[tauri::command]
pub async fn clip_copy(app: AppHandle, id: u64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_entry(id)?;
        emit(&app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clip_remove(app: AppHandle, id: u64) {
    HISTORY.lock().unwrap_or_else(|p| p.into_inner()).retain(|e| e.id != id);
    emit(&app);
}

#[tauri::command]
pub fn clip_clear(app: AppHandle) {
    HISTORY.lock().unwrap_or_else(|p| p.into_inner()).clear();
    emit(&app);
}

/// Aperçu grand format (côté long 1400 px) d'une image.
#[tauri::command]
pub async fn clip_preview(id: u64) -> Result<String, String> {
    let png = HISTORY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.png.clone().or_else(|| e.files.first().map(|_| Vec::new())))
        .ok_or("notFound")?;
    let files = HISTORY.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|e| e.id == id).map(|e| e.files.clone()).unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let img = if png.is_empty() {
            let f = files.iter().find(|f| file_thumb(f, 1).is_some()).ok_or("notFound")?;
            image::open(f).map_err(|e| e.to_string())?
        } else {
            image::load_from_memory(&png).map_err(|e| e.to_string())?
        };
        thumb_data_url(&img, LARGE).ok_or_else(|| "encode".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_entry(t: &str) -> Entry {
        Entry { id: 0, kind: Kind::Text, at: now_ms(), key: hash(t), text: Some(t.into()), files: vec![], png: None, size: None, thumb: None }
    }

    #[test]
    fn recopy_text() {
        push(text_entry("island-test-old"));
        push(text_entry("island-test-new"));
        let old = HISTORY.lock().unwrap().iter().find(|e| e.text.as_deref() == Some("island-test-old")).unwrap().id;
        write_entry(old).unwrap();
        let got = arboard::Clipboard::new().unwrap().get_text().unwrap();
        assert_eq!(got, "island-test-old");
        assert_eq!(HISTORY.lock().unwrap().front().unwrap().id, old);
    }
}
