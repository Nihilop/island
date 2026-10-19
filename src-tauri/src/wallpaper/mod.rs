// Fonds animés (outil core) : scènes enregistrées, scène active, moteur natif.
//
//   scene.rs   : format de scène v1 (données bornées) + uniformes + préréglages
//   scene.wgsl : shader unique, partagé avec l'aperçu de l'éditeur
//   engine.rs  : thread de rendu (fenêtres du calque du bureau, wgpu, pause, souris)
//
// Rangement : `<app_data>/wallpapers/scenes/<id>/scene.json` (+ l'image de fond copiée à
// côté). Les préréglages (`preset-*`) sont en lecture seule. Scène active et options dans
// le store `__app__` (clé `wallpaper`) : restaurées au démarrage par le Rust lui-même —
// masquer l'outil du dock n'arrête pas un fond appliqué.
// Le fond STATIQUE n'est pas ici : c'est le service `desktop` (utilisé aussi par les
// automatisations et les extensions).

pub mod scene;
#[cfg(target_os = "windows")]
mod engine;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use scene::Scene;

const STORE: &str = "__app__";
const KEY: &str = "wallpaper";

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Scène appliquée (None = fond Windows normal).
    pub active: Option<String>,
    /// Images par seconde (15–60).
    pub fps: u32,
    /// Pause pendant un jeu (service Activité).
    pub pause_in_game: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { active: None, fps: 30, pause_in_game: true }
    }
}

fn prefs(app: &AppHandle) -> Prefs {
    crate::store::get(app, STORE, KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn save_prefs(app: &AppHandle, p: &Prefs) {
    crate::store::set(app, STORE, KEY, serde_json::to_value(p).ok());
    let _ = app.emit("wallpaper://changed", ());
}

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

/// Scène + chemin absolu de son image (si elle en a une).
fn load(app: &AppHandle, id: &str) -> Result<(Scene, Option<PathBuf>), String> {
    if is_preset(id) {
        let (_, s) = scene::presets().into_iter().find(|(p, _)| *p == id).ok_or("invalidScene")?;
        return Ok((s, None));
    }
    let dir = scene_dir(app, id)?;
    let raw = std::fs::read(dir.join("scene.json")).map_err(|_| "invalidScene")?;
    let s: Scene = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    s.check()?;
    let img = s.background.image.as_ref().map(|f| dir.join(f)).filter(|p| p.is_file());
    Ok((s, img))
}

// --- Commandes : scènes ----------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: String,
    preset: bool,
    scene: Scene,
    /// Modification (ms) ; 0 pour un préréglage.
    updated: u64,
}

#[tauri::command]
pub fn wallpaper_list(app: AppHandle) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = scene::presets().into_iter().map(|(id, scene)| Entry { id: id.into(), preset: true, scene, updated: 0 }).collect();
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
                    mine.push(Entry { id, preset: false, scene, updated });
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

/// Enregistre une scène (id choisi par l'éditeur). Si c'est la scène active, le fond suit.
#[tauri::command]
pub fn wallpaper_save(app: AppHandle, id: String, scene: Scene) -> Result<(), String> {
    scene.check()?;
    let dir = scene_dir(&app, &id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut s = scene;
    s.version = scene::VERSION;
    // Images qui ne servent plus (remplacées dans l'éditeur).
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n != "scene.json" && Some(&n) != s.background.image.as_ref() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let json = serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("scene.json"), json).map_err(|e| e.to_string())?;
    let _ = app.emit("wallpaper://changed", ());
    if prefs(&app).active.as_deref() == Some(&id) {
        show(&app, &id)?;
    }
    Ok(())
}

#[tauri::command]
pub fn wallpaper_delete(app: AppHandle, id: String) -> Result<(), String> {
    let dir = scene_dir(&app, &id)?;
    let mut p = prefs(&app);
    if p.active.as_deref() == Some(&id) {
        p.active = None;
        save_prefs(&app, &p);
        stop(&app);
    }
    let _ = std::fs::remove_dir_all(dir);
    let _ = app.emit("wallpaper://changed", ());
    Ok(())
}

/// Copie une image choisie par l'utilisateur dans le dossier de la scène ; renvoie son nom
/// (à mettre dans `background.image`). Horodaté : l'aperçu ne garde pas l'ancienne en cache.
#[tauri::command]
pub fn wallpaper_import_image(app: AppHandle, id: String, path: String) -> Result<String, String> {
    let dir = scene_dir(&app, &id)?;
    let src = PathBuf::from(&path);
    let ext = src.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let ext = if ext == "jpeg" { "jpg".to_string() } else { ext };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = format!("background-{stamp}.{ext}");
    if !scene::valid_image_name(&name) || !src.is_file() {
        return Err("invalidImage".into());
    }
    image::image_dimensions(&src).map_err(|_| "invalidImage")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&src, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Octets de l'image de fond (aperçu, vignettes).
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

fn show(app: &AppHandle, id: &str) -> Result<(), String> {
    let (s, image) = load(app, id)?;
    #[cfg(target_os = "windows")]
    engine::send(app, engine::Cmd::Show { scene: Box::new(s), image });
    #[cfg(not(target_os = "windows"))]
    let _ = (s, image);
    Ok(())
}

fn stop(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    engine::send(app, engine::Cmd::Stop);
    #[cfg(not(target_os = "windows"))]
    let _ = app;
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

/// Applique une scène (None = retour au fond Windows).
#[tauri::command]
pub fn wallpaper_apply(app: AppHandle, id: Option<String>) -> Result<(), String> {
    let mut p = prefs(&app);
    match &id {
        Some(i) => show(&app, i)?,
        None => stop(&app),
    }
    p.active = id;
    save_prefs(&app, &p);
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

/// Démarrage : restaure la scène active ; à la fermeture, rend le fond à Windows.
pub fn start(app: &AppHandle) {
    let p = prefs(app);
    send_options(app, &p);
    if let Some(id) = &p.active {
        if let Err(e) = show(app, id) {
            eprintln!("[wallpaper] scène {id} : {e}");
        }
    }
    let plugin = tauri::plugin::Builder::<tauri::Wry>::new("island-wallpaper")
        .on_event(|app, ev| {
            // Le thread de rendu détruit ses fenêtres et redessine le fond statique ; on lui
            // laisse le temps avant que le processus ne disparaisse.
            if matches!(ev, tauri::RunEvent::Exit) && prefs(app).active.is_some() {
                stop(app);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("[wallpaper] hook de fermeture : {e}");
    }
}
