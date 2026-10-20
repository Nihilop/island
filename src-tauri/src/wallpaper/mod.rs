// Fonds animés (outil core) : scènes enregistrées, affectation par écran, moteur natif.
//
//   scene.rs       : format de scène (calques, données bornées) + placement + uniformes + préréglages
//   layer.wgsl     : passe d'un calque ; composite.wgsl : passe finale (partagés avec l'aperçu)
//   engine.rs      : thread de rendu (fenêtres du calque du bureau, wgpu, pause, souris)
//   share.rs       : partage (`.islandscene` : export, import vérifié, double-clic)
//
// Rangement : `<app_data>/wallpapers/scenes/<id>/scene.json` (+ les images des calques
// copiées à côté). Les préréglages (`preset-*`) sont en lecture seule. Affectation et options dans
// le store `__app__` (clé `wallpaper`) : une scène pour « Tous les écrans », que chaque
// écran peut remplacer par la sienne. Restaurées au démarrage par le Rust lui-même —
// masquer l'outil du dock n'arrête pas un fond appliqué.
// Le fond STATIQUE n'est pas ici : c'est le service `desktop` (utilisé aussi par les
// automatisations et les extensions).

pub mod scene;
pub mod share;
#[cfg(target_os = "windows")]
mod engine;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use scene::Scene;

const STORE: &str = "__app__";
const KEY: &str = "wallpaper";

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Scène de « Tous les écrans » (None = fond Windows). `active` : ancien nom.
    #[serde(alias = "active")]
    pub all: Option<String>,
    /// Écrans réglés à part : id d'écran (`hardware::displays`) → scène ; "" = fond Windows
    /// sur cet écran.
    pub screens: BTreeMap<String, String>,
    /// Images par seconde (15–60).
    pub fps: u32,
    /// Pause pendant un jeu (service Activité).
    pub pause_in_game: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { all: None, screens: BTreeMap::new(), fps: 30, pause_in_game: true }
    }
}

fn prefs(app: &AppHandle) -> Prefs {
    crate::store::get(app, STORE, KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn save_prefs(app: &AppHandle, p: &Prefs) {
    crate::store::set(app, STORE, KEY, serde_json::to_value(p).ok());
    let _ = app.emit("wallpaper://changed", ());
}

/// Vignette d'une scène : un vrai rendu, fait par l'éditeur à chaque enregistrement
/// (convention `store/` du magasin, cf. `island_formats::media`).
pub const SNAPSHOT: &str = "store/thumbnail.png";

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_preset(id: &str) -> bool {
    id.starts_with("preset-")
}

fn scenes_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("wallpapers").join("scenes"))
}

fn scene_dir(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) || is_preset(id) {
        return Err("invalidScene".into());
    }
    Ok(scenes_dir(app)?.join(id))
}

/// Scène + dossier de ses images (None pour un préréglage).
fn load(app: &AppHandle, id: &str) -> Result<(Scene, Option<PathBuf>), String> {
    if is_preset(id) {
        let (_, s) = scene::presets().into_iter().find(|(p, _)| *p == id).ok_or("invalidScene")?;
        return Ok((s, None));
    }
    let dir = scene_dir(app, id)?;
    let raw = std::fs::read(dir.join("scene.json")).map_err(|_| "invalidScene")?;
    let s: Scene = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    s.check()?;
    Ok((s, Some(dir)))
}

// --- Commandes : scènes ----------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub(crate) id: String,
    preset: bool,
    /// Vignette disponible (`wallpaper_thumbnail` : rendu de l'éditeur, sinon calques composés).
    thumbnail: bool,
    scene: Scene,
    /// Modification (ms) ; 0 pour un préréglage.
    updated: u64,
}

#[tauri::command]
pub fn wallpaper_list(app: AppHandle) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = scene::presets().into_iter().map(|(id, scene)| Entry { id: id.into(), preset: true, thumbnail: true, scene, updated: 0 }).collect();
    let dir = scenes_dir(&app)?;
    let mut mine = Vec::new();
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        if !valid_id(&id) {
            continue;
        }
        let json = e.path().join("scene.json");
        match std::fs::metadata(&json) {
            Ok(m) => {
                if let Ok((scene, _)) = load(&app, &id) {
                    let updated = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0);
                    mine.push(Entry { id, preset: false, thumbnail: true, scene, updated });
                }
            }
            // Brouillon jamais enregistré (image importée puis éditeur fermé) : ménage après 1 jour.
            Err(_) => {
                let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() > 86_400);
                if old {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }
    mine.sort_by(|a, b| b.updated.cmp(&a.updated));
    out.extend(mine);
    Ok(out)
}

#[tauri::command]
pub fn wallpaper_get(app: AppHandle, id: String) -> Result<Scene, String> {
    load(&app, &id).map(|(s, _)| s)
}

/// Enregistre une scène (id choisi par l'éditeur). Si elle est affichée, le fond suit.
#[tauri::command]
pub fn wallpaper_save(app: AppHandle, id: String, scene: Scene) -> Result<(), String> {
    scene.check()?;
    let dir = scene_dir(&app, &id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut s = scene;
    s.version = scene::VERSION;
    // Images qui ne servent plus (calque supprimé, image remplacée).
    let keep: std::collections::HashSet<&String> = s.images().collect();
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n != "scene.json" && !keep.contains(&n) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let json = serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("scene.json"), json).map_err(|e| e.to_string())?;
    let _ = app.emit("wallpaper://changed", ());
    if prefs(&app).uses(&id) {
        push(&app);
    }
    Ok(())
}

#[tauri::command]
pub fn wallpaper_delete(app: AppHandle, id: String) -> Result<(), String> {
    let dir = scene_dir(&app, &id)?;
    let mut p = prefs(&app);
    if p.uses(&id) {
        if p.all.as_deref() == Some(&id) {
            p.all = None;
        }
        p.screens.retain(|_, v| v != &id);
        save_prefs(&app, &p);
        push(&app);
    }
    let _ = std::fs::remove_dir_all(dir);
    let _ = app.emit("wallpaper://changed", ());
    Ok(())
}

/// Copie une image choisie par l'utilisateur dans le dossier de la scène ; renvoie son nom
/// (à mettre dans `image` d'un calque). Horodaté : l'aperçu ne garde pas l'ancienne en cache.
#[tauri::command]
pub fn wallpaper_import_image(app: AppHandle, id: String, path: String) -> Result<String, String> {
    let dir = scene_dir(&app, &id)?;
    let src = PathBuf::from(&path);
    let ext = src.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let ext = if ext == "jpeg" { "jpg".to_string() } else { ext };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = format!("layer-{stamp}.{ext}");
    if !scene::valid_image_name(&name) || !src.is_file() {
        return Err("invalidImage".into());
    }
    image::image_dimensions(&src).map_err(|_| "invalidImage")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&src, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Enregistre un masque peint dans l'éditeur (PNG en base64) ; renvoie son nom. Validé
/// (vraie image, 4096 px max), normalisé en niveaux de gris. Un nouveau fichier à chaque
/// fois (l'aperçu ne garde pas l'ancien en cache) ; les anciens partent au prochain
/// enregistrement de la scène.
#[tauri::command]
pub fn wallpaper_save_mask(app: AppHandle, id: String, data: String) -> Result<String, String> {
    use base64::Engine;
    let dir = scene_dir(&app, &id)?;
    if data.len() > 24 * 1024 * 1024 {
        return Err("tooLarge".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "invalidImage")?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|_| "invalidImage")?;
    if img.width() > 4096 || img.height() > 4096 {
        return Err("tooLarge".into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = format!("mask-{stamp}.png");
    img.to_luma8().save(dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Enregistre le rendu de la scène fait par l'éditeur (PNG en base64), 1920 px au plus.
#[tauri::command]
pub fn wallpaper_save_thumbnail(app: AppHandle, id: String, data: String) -> Result<(), String> {
    use base64::Engine;
    let dir = scene_dir(&app, &id)?;
    if data.len() > 16 * 1024 * 1024 {
        return Err("tooLarge".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "invalidImage")?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|_| "invalidImage")?;
    if img.width() > 1920 || img.height() > 1920 {
        return Err("tooLarge".into());
    }
    let path = dir.join(SNAPSHOT);
    std::fs::create_dir_all(path.parent().expect("store/")).map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let _ = app.emit("wallpaper://changed", ());
    Ok(())
}

/// Vignette de la scène (cartes de l'outil, écrans) : le rendu enregistré par l'éditeur ;
/// à défaut (préréglage, scène ancienne), les calques composés (`islandscene::thumbnail`).
#[tauri::command]
pub async fn wallpaper_thumbnail(app: AppHandle, id: String) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_preset(&id) {
            if let Ok(bytes) = std::fs::read(scene_dir(&app, &id)?.join(SNAPSHOT)) {
                return Ok(tauri::ipc::Response::new(bytes));
            }
        }
        let (s, dir) = load(&app, &id)?;
        let thumb = island_formats::islandscene::thumbnail(&s, |f| dir.as_ref().and_then(|d| image::open(d.join(f)).ok()));
        Ok(tauri::ipc::Response::new(island_formats::islandscene::png(&thumb)?))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Octets d'une image ou d'un masque de la scène (aperçu, vignettes).
#[tauri::command]
pub fn wallpaper_image(app: AppHandle, id: String, name: String) -> Result<tauri::ipc::Response, String> {
    if !scene::valid_image_name(&name) {
        return Err("invalidImage".into());
    }
    let bytes = std::fs::read(scene_dir(&app, &id)?.join(name)).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

// --- Commandes : fond actif ---------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(flatten)]
    prefs: Prefs,
    running: bool,
    paused: bool,
    error: Option<String>,
}

impl Prefs {
    fn uses(&self, id: &str) -> bool {
        self.all.as_deref() == Some(id) || self.screens.values().any(|v| v == id)
    }
    fn any(&self) -> bool {
        self.all.is_some() || self.screens.values().any(|v| !v.is_empty())
    }
}

/// Envoie l'affectation au moteur (une scène illisible = fond Windows sur cet écran).
fn push(app: &AppHandle) {
    let p = prefs(app);
    #[cfg(target_os = "windows")]
    {
        let mut cache: BTreeMap<String, Option<engine::Loaded>> = BTreeMap::new();
        let mut get = |id: &str| -> Option<engine::Loaded> {
            cache
                .entry(id.to_string())
                .or_insert_with(|| match load(app, id) {
                    Ok((scene, dir)) => Some(engine::Loaded { id: id.to_string(), scene: Box::new(scene), dir }),
                    Err(e) => {
                        eprintln!("[wallpaper] scène {id} : {e}");
                        None
                    }
                })
                .clone()
        };
        if !p.any() {
            return engine::send(app, engine::Cmd::Stop);
        }
        let all = p.all.as_deref().and_then(&mut get);
        let screens = p.screens.iter().map(|(d, id)| (d.clone(), if id.is_empty() { None } else { get(id) })).collect();
        engine::send(app, engine::Cmd::Show { all, screens });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (app, p);
}

fn send_options(app: &AppHandle, p: &Prefs) {
    #[cfg(target_os = "windows")]
    engine::send(app, engine::Cmd::Options { fps: p.fps, pause_in_game: p.pause_in_game });
    #[cfg(not(target_os = "windows"))]
    let _ = (app, p);
}

#[tauri::command]
pub fn wallpaper_state(app: AppHandle) -> State {
    #[cfg(target_os = "windows")]
    let st = engine::status();
    #[cfg(not(target_os = "windows"))]
    let st = Default::default();
    State { prefs: prefs(&app), running: st.running, paused: st.paused, error: st.error }
}

/// Applique une scène (None = fond Windows). `display` None = tous les écrans (les réglages
/// par écran sont oubliés) ; sinon cet écran seulement.
#[tauri::command]
pub fn wallpaper_apply(app: AppHandle, id: Option<String>, display: Option<String>) -> Result<(), String> {
    if let Some(i) = &id {
        load(&app, i)?;
    }
    let mut p = prefs(&app);
    match display {
        None => {
            p.all = id;
            p.screens.clear();
        }
        Some(d) => {
            if !crate::services::hardware::displays().iter().any(|x| x.id == d) {
                return Err("invalidDisplay".into());
            }
            p.screens.insert(d, id.unwrap_or_default());
        }
    }
    save_prefs(&app, &p);
    push(&app);
    Ok(())
}

#[tauri::command]
pub fn wallpaper_set_options(app: AppHandle, fps: u32, pause_in_game: bool) {
    let mut p = prefs(&app);
    p.fps = fps.clamp(15, 60);
    p.pause_in_game = pause_in_game;
    save_prefs(&app, &p);
    send_options(&app, &p);
}

/// Démarrage : restaure l'affectation ; à la fermeture, rend le fond à Windows.
pub fn start(app: &AppHandle) {
    // Double-clic sur un `.islandscene` → Island (HKCU, réécrit seulement si besoin).
    #[cfg(target_os = "windows")]
    if let Err(e) = crate::host::register_file_assoc(app, ".islandscene", "Island.Scene", "Scène Island") {
        eprintln!("[wallpaper] association .islandscene : {e}");
    }
    let p = prefs(app);
    send_options(app, &p);
    if p.any() {
        push(app);
    }
    let plugin = tauri::plugin::Builder::<tauri::Wry>::new("island-wallpaper")
        .on_event(|app, ev| {
            // Le thread de rendu détruit ses fenêtres et redessine le fond statique ; on lui
            // laisse le temps avant que le processus ne disparaisse.
            if matches!(ev, tauri::RunEvent::Exit) && prefs(app).any() {
                #[cfg(target_os = "windows")]
                engine::send(app, engine::Cmd::Stop);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("[wallpaper] hook de fermeture : {e}");
    }
}
