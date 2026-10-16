// Service `capture` v2 (cf. docs/internal/capture-spec.md) : Island capture ET encode,
// l'extension ne fait que piloter. Aucun exécutable téléchargé : capture Windows Graphics
// Capture, encodage H.264 / H.265 + AAC par Media Foundation (encodeur matériel).
//
// Cette couche (commandes, permission, types JSON, calculs purs testables) est
// cross-platform ; l'implémentation native vit dans `windows.rs` (capture, sélection,
// enregistrement), `encoder.rs` (MediaTranscoder) et `audio.rs` (WASAPI + mixage).
//
// Permission `capture` (à haut risque : tout l'écran et le son) revérifiée ici à chaque
// commande à partir du manifeste installé. La fenêtre de sélection (`capture-select`)
// n'appelle que `capture_select_init` / `capture_select_done`, refusés à toute autre fenêtre.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, WebviewWindow};

mod audio;
#[cfg(target_os = "windows")]
mod encoder;
#[cfg(target_os = "windows")]
mod windows;

/// Libellé de la fenêtre de sélection (le front route sur ce libellé).
pub const SELECT_LABEL: &str = "capture-select";

fn require(app: &AppHandle, id: &str) -> Result<(), String> {
    crate::extensions::services::require(app, id, "capture")
}

// --- Types du contrat (camelCase côté JS) ------------------------------------------------

/// Rectangle en px physiques, relatif au moniteur. Flottants acceptés (le front calcule en
/// px CSS × devicePixelRatio) : arrondis au pixel à l'usage.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    /// Écran entier (index 1-based, ordre d'énumération = `Monitor::from_index`).
    Screen { display: usize },
    /// Zone d'un écran.
    Region { display: usize, rect: Rect },
    /// Fenêtre (HWND).
    Window { hwnd: i64 },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub index: usize,
    pub name: String,
    /// Position et taille physiques dans le bureau virtuel.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Facteur d'échelle (DPI effectif / 96).
    pub scale: f64,
    pub primary: bool,
}

/// Fenêtre candidate de la sélection (rect physique relatif au moniteur, ordre Z).
#[derive(Serialize, Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: i64,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize)]
pub struct SelectInit {
    pub mode: String,
    pub display: Display,
    pub windows: Vec<WindowInfo>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone, Copy)]
pub struct ShotOpts {
    #[serde(default = "yes")]
    pub clipboard: bool,
    #[serde(default = "yes")]
    pub save: bool,
}

#[derive(Serialize)]
pub struct ShotResult {
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    #[default]
    H264,
    Hevc,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Low,
    #[default]
    Medium,
    High,
}

fn default_fps() -> u32 {
    30
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct RecordOpts {
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default)]
    pub codec: Codec,
    #[serde(default)]
    pub quality: Quality,
    #[serde(default)]
    pub system_audio: bool,
    #[serde(default)]
    pub mic: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StopResult {
    pub path: String,
    pub duration_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordState {
    pub recording: bool,
    pub started_at: Option<u64>,
    pub target: Option<Target>,
    /// Extension propriétaire de l'enregistrement (l'activité « ● REC » de l'hôte
    /// arrête avec cet identifiant).
    pub id: Option<String>,
}

/// Charge utile de `capture://recording`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEvent {
    pub recording: bool,
    pub started_at: u64,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Arrêt sur erreur (fenêtre fermée, écran débranché, encodeur en échec…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub const RECORDING_EVENT: &str = "capture://recording";

// --- Commandes ------------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
const UNSUPPORTED: &str = "unsupported";

/// Écrans disponibles (index 1-based, géométrie physique du bureau virtuel).
#[tauri::command]
pub async fn capture_displays(app: AppHandle, id: String) -> Result<Vec<Display>, String> {
    require(&app, &id)?;
    #[cfg(target_os = "windows")]
    {
        Ok(windows::displays())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(UNSUPPORTED.into())
    }
}

/// Ouvre la sélection d'Island sur le moniteur sous la souris et attend le choix
/// (`null` = annulé). Une sélection déjà ouverte est annulée.
#[tauri::command]
pub async fn capture_select(app: AppHandle, id: String, mode: String) -> Result<Option<Target>, String> {
    require(&app, &id)?;
    if !matches!(mode.as_str(), "region" | "window" | "screen") {
        return Err("invalidMode".into());
    }
    #[cfg(target_os = "windows")]
    {
        windows::select(app, mode).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(UNSUPPORTED.into())
    }
}

/// Appelé par la page `capture-select` au montage : mode, écran, fenêtres candidates.
#[tauri::command]
pub async fn capture_select_init(window: WebviewWindow) -> Result<SelectInit, String> {
    if window.label() != SELECT_LABEL {
        return Err("forbidden".into());
    }
    #[cfg(target_os = "windows")]
    {
        windows::select_init()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(UNSUPPORTED.into())
    }
}

/// Réponse de la page `capture-select` : résout `capture_select` et ferme la fenêtre.
#[tauri::command]
pub async fn capture_select_done(window: WebviewWindow, result: Option<Target>) -> Result<(), String> {
    if window.label() != SELECT_LABEL {
        return Err("forbidden".into());
    }
    #[cfg(target_os = "windows")]
    {
        windows::select_done(&window, result);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = result;
        Err(UNSUPPORTED.into())
    }
}

/// Capture d'écran → PNG (`Images\Island`) et/ou presse-papiers.
#[tauri::command]
pub async fn capture_screenshot(app: AppHandle, id: String, target: Target, opts: ShotOpts) -> Result<ShotResult, String> {
    require(&app, &id)?;
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || windows::screenshot(&app, target, opts))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (target, opts);
        Err(UNSUPPORTED.into())
    }
}

/// Démarre l'enregistrement (un seul à la fois, toutes extensions confondues).
#[tauri::command]
pub async fn capture_record_start(app: AppHandle, id: String, target: Target, opts: RecordOpts) -> Result<(), String> {
    require(&app, &id)?;
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || windows::record_start(&app, id, target, opts))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (target, opts);
        Err(UNSUPPORTED.into())
    }
}

/// Arrête l'enregistrement de l'extension et finalise le MP4.
#[tauri::command]
pub async fn capture_record_stop(app: AppHandle, id: String) -> Result<StopResult, String> {
    require(&app, &id)?;
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || windows::record_stop(&app, &id))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(UNSUPPORTED.into())
    }
}

#[tauri::command]
pub async fn capture_record_state(app: AppHandle, id: String) -> Result<RecordState, String> {
    require(&app, &id)?;
    #[cfg(target_os = "windows")]
    {
        Ok(windows::record_state())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(RecordState { recording: false, started_at: None, target: None, id: None })
    }
}

// --- Calculs purs (testés) ------------------------------------------------------------------

/// Unités des horodatages Media Foundation : 100 ns.
pub(crate) const TICKS_PER_SEC: i64 = 10_000_000;

/// Valeur paire inférieure (H.264 / H.265 exigent des dimensions paires).
pub(crate) fn even_down(v: u32) -> u32 {
    v & !1
}

/// Débit vidéo (bit/s) : bits par pixel par image selon la qualité × pixels × fps,
/// borné à [1, 150] Mb/s (1080p30 medium ≈ 7,5 Mb/s).
pub(crate) fn bitrate(w: u32, h: u32, fps: u32, q: Quality) -> u32 {
    let bpp = match q {
        Quality::Low => 0.07,
        Quality::Medium => 0.12,
        Quality::High => 0.2,
    };
    let b = w as f64 * h as f64 * fps as f64 * bpp;
    b.clamp(1_000_000.0, 150_000_000.0) as u32
}

/// Zone de découpe `(x, y, w, h)` bornée au cadre réel `fw × fh` (une zone près d'un bord
/// déborde sinon → erreur de crop). `even` : dimensions arrondies à la valeur paire
/// inférieure (vidéo). `None` si la zone est vide (ou < 2 px en vidéo).
pub(crate) fn clamp_crop(r: &Rect, fw: u32, fh: u32, even: bool) -> Option<(u32, u32, u32, u32)> {
    if !(r.x.is_finite() && r.y.is_finite() && r.w.is_finite() && r.h.is_finite()) {
        return None;
    }
    let clamp = |v: f64, max: u32| v.round().clamp(0.0, max as f64) as u32;
    let (x0, x1) = (clamp(r.x, fw), clamp(r.x + r.w, fw));
    let (y0, y1) = (clamp(r.y, fh), clamp(r.y + r.h, fh));
    let (mut w, mut h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    if even {
        w = even_down(w);
        h = even_down(h);
    }
    let min = if even { 2 } else { 1 };
    (w >= min && h >= min).then_some((x0, y0, w, h))
}

/// Dimensions de sortie : l'entrée si elle tient dans les limites du codec (H.264 niveau
/// 5.1 : 4096 × 2304 ; HEVC : 8192 × 4320), sinon réduite en gardant le ratio (un écran
/// ultra-large 5120 × 1440 dépasse H.264) — MediaTranscoder met à l'échelle. Toujours paires.
pub(crate) fn fit_dims(w: u32, h: u32, codec: Codec) -> (u32, u32) {
    let (mw, mh) = match codec {
        Codec::H264 => (4096.0, 2304.0),
        Codec::Hevc => (8192.0, 4320.0),
    };
    let k = (mw / w.max(1) as f64).min(mh / h.max(1) as f64).min(1.0);
    let fit = |v: u32| even_down(((v as f64) * k).floor() as u32).max(2);
    (fit(w), fit(h))
}

/// Horodatage (100 ns) de l'image `n` à `fps` : calcul exact, sans dérive cumulée.
pub(crate) fn frame_ts(n: u64, fps: u32) -> i64 {
    (n as i128 * TICKS_PER_SEC as i128 / fps.max(1) as i128) as i64
}

/// Indice de la dernière image due à l'instant `elapsed` (100 ns depuis le départ).
pub(crate) fn frame_due(elapsed: i64, fps: u32) -> u64 {
    (elapsed.max(0) as i128 * fps.max(1) as i128 / TICKS_PER_SEC as i128) as u64
}

/// Horodatage (100 ns) de l'échantillon audio `pos` à `rate` Hz.
pub(crate) fn sample_ts(pos: u64, rate: u32) -> i64 {
    (pos as i128 * TICKS_PER_SEC as i128 / rate.max(1) as i128) as i64
}

/// Nom de fichier horodaté : `<prefix> AAAA-MM-JJ HH-MM-SS.<ext>`.
pub(crate) fn stamped_name(prefix: &str, ext: &str, t: (u16, u16, u16, u16, u16, u16)) -> String {
    let (y, mo, d, h, mi, s) = t;
    format!("{prefix} {y:04}-{mo:02}-{d:02} {h:02}-{mi:02}-{s:02}.{ext}")
}

/// Chemin libre dans `dir` : `name`, sinon `name (2)`, `name (3)`… (deux captures dans
/// la même seconde).
pub(crate) fn unique_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    (2..1000)
        .map(|i| dir.join(format!("{stem} ({i}).{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_rounding() {
        assert_eq!(even_down(1921), 1920);
        assert_eq!(even_down(1080), 1080);
        assert_eq!(even_down(1), 0);
    }

    #[test]
    fn bitrate_formula() {
        // 1080p30 medium ≈ 7,5 Mb/s (spec).
        let b = bitrate(1920, 1080, 30, Quality::Medium);
        assert!((7_400_000..7_500_000).contains(&b), "{b}");
        assert!(bitrate(1920, 1080, 30, Quality::Low) < b);
        assert!(bitrate(1920, 1080, 30, Quality::High) > b);
        assert_eq!(bitrate(16, 16, 30, Quality::Low), 1_000_000);
        assert_eq!(bitrate(8192, 4320, 60, Quality::High), 150_000_000);
    }

    #[test]
    fn crop_clamping() {
        let r = |x, y, w, h| Rect { x, y, w, h };
        assert_eq!(clamp_crop(&r(10.0, 20.0, 100.0, 50.0), 1920, 1080, false), Some((10, 20, 100, 50)));
        // Déborde à droite / en bas → borné au cadre.
        assert_eq!(clamp_crop(&r(1900.0, 1070.0, 100.0, 100.0), 1920, 1080, false), Some((1900, 1070, 20, 10)));
        // Origine négative → réduit la largeur au lieu de décaler.
        assert_eq!(clamp_crop(&r(-10.0, 0.0, 30.0, 10.0), 1920, 1080, false), Some((0, 0, 20, 10)));
        // Vidéo : dimensions paires.
        assert_eq!(clamp_crop(&r(0.0, 0.0, 101.0, 51.0), 1920, 1080, true), Some((0, 0, 100, 50)));
        // Vide / dégénéré / hors cadre.
        assert_eq!(clamp_crop(&r(0.0, 0.0, 0.0, 10.0), 1920, 1080, false), None);
        assert_eq!(clamp_crop(&r(0.0, 0.0, 1.0, 1.0), 1920, 1080, true), None);
        assert_eq!(clamp_crop(&r(5000.0, 0.0, 10.0, 10.0), 1920, 1080, false), None);
        assert_eq!(clamp_crop(&r(f64::NAN, 0.0, 10.0, 10.0), 1920, 1080, false), None);
        // Flottants (px CSS × DPR) arrondis.
        assert_eq!(clamp_crop(&r(10.4, 10.6, 99.8, 50.2), 1920, 1080, false), Some((10, 11, 100, 50)));
    }

    #[test]
    fn fit_dims_limits() {
        assert_eq!(fit_dims(1920, 1080, Codec::H264), (1920, 1080));
        assert_eq!(fit_dims(3840, 2160, Codec::H264), (3840, 2160));
        let (w, h) = fit_dims(5120, 1440, Codec::H264);
        assert!(w <= 4096 && h <= 2304 && w % 2 == 0 && h % 2 == 0, "{w}x{h}");
        assert_eq!(fit_dims(5120, 1440, Codec::Hevc), (5120, 1440));
    }

    #[test]
    fn timestamps() {
        assert_eq!(frame_ts(0, 30), 0);
        assert_eq!(frame_ts(30, 30), TICKS_PER_SEC);
        assert_eq!(frame_ts(1, 60), 166_666);
        // Pas de dérive : 1 h d'images à 60 fps tombe pile sur 3600 s.
        assert_eq!(frame_ts(60 * 3600, 60), 3600 * TICKS_PER_SEC);
        // Monotone.
        assert!((1..1000).all(|n| frame_ts(n, 60) > frame_ts(n - 1, 60)));
        assert_eq!(frame_due(0, 30), 0);
        assert_eq!(frame_due(TICKS_PER_SEC - 1, 30), 29);
        assert_eq!(frame_due(TICKS_PER_SEC, 30), 30);
        assert_eq!(frame_due(-5, 30), 0);
        assert_eq!(sample_ts(960, 48_000), 200_000); // 20 ms
        assert_eq!(sample_ts(48_000, 48_000), TICKS_PER_SEC);
    }

    #[test]
    fn names() {
        assert_eq!(stamped_name("Capture", "png", (2026, 3, 7, 9, 5, 2)), "Capture 2026-03-07 09-05-02.png");
        let dir = std::env::temp_dir().join(format!("island-capture-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let a = unique_path(&dir, "x.png");
        assert_eq!(a, dir.join("x.png"));
        std::fs::write(&a, b"").unwrap();
        assert_eq!(unique_path(&dir, "x.png"), dir.join("x (2).png"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn target_json() {
        let t: Target = serde_json::from_str(r#"{"kind":"region","display":2,"rect":{"x":1,"y":2,"w":3.5,"h":4}}"#).unwrap();
        assert_eq!(t, Target::Region { display: 2, rect: Rect { x: 1.0, y: 2.0, w: 3.5, h: 4.0 } });
        let t: Target = serde_json::from_str(r#"{"kind":"window","hwnd":12345}"#).unwrap();
        assert_eq!(t, Target::Window { hwnd: 12345 });
        let o: RecordOpts = serde_json::from_str(r#"{"fps":60,"codec":"hevc","quality":"high","systemAudio":true,"mic":false}"#).unwrap();
        assert_eq!((o.fps, o.codec, o.quality, o.system_audio, o.mic), (60, Codec::Hevc, Quality::High, true, false));
        let s: ShotOpts = serde_json::from_str("{}").unwrap();
        assert!(s.clipboard && s.save);
    }
}
