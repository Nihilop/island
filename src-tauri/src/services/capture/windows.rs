// Implémentation Windows de la capture : écrans, sélection (fenêtre `capture-select`),
// capture d'écran et enregistrement via Windows Graphics Capture (anti-cheat safe, pas
// d'injection), encodage Media Foundation (`encoder.rs`), son WASAPI (`audio.rs`).
//
// Horloge unique : QPC en unités de 100 ns (`qpc_100ns`), partagée par la pompe vidéo et
// les paquets audio (WASAPI donne la QPC de chaque paquet) → audio et vidéo alignés.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, SetWindowDisplayAffinity, GWL_EXSTYLE, MONITORINFOF_PRIMARY, WDA_EXCLUDEFROMCAPTURE,
    WDA_NONE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings, TryIntoCaptureItemWithType,
};
use windows_capture::window::Window;

use super::audio::{self, Mixer, Source};
use super::encoder::{self, Encoder, Sample};
use super::{
    bitrate, clamp_crop, even_down, fit_dims, frame_due, frame_ts, sample_ts, stamped_name, unique_path, Display, RecordOpts,
    RecordState, RecordingEvent, Rect, SelectInit, ShotOpts, ShotResult, StopResult, Target, WindowInfo, RECORDING_EVENT,
    SELECT_LABEL, TICKS_PER_SEC,
};

// --- Utilitaires ---------------------------------------------------------------------------

/// Horloge QPC en 100 ns (même base que `u64QPCPosition` des paquets WASAPI).
pub(crate) fn qpc_100ns() -> i64 {
    let (mut c, mut f) = (0i64, 0i64);
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
        let _ = QueryPerformanceFrequency(&mut f);
    }
    (c as i128 * TICKS_PER_SEC as i128 / f.max(1) as i128) as i64
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Date et heure locales (nom des fichiers).
fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

fn hwnd(v: i64) -> HWND {
    HWND(v as isize as *mut core::ffi::c_void)
}

/// Exclut (ou réintègre) une fenêtre de NOTRE processus des captures d'écran.
fn set_excluded(h: HWND, on: bool) {
    unsafe {
        let _ = SetWindowDisplayAffinity(h, if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
    }
}

// --- Île exclue des captures ----------------------------------------------------------------

/// Nombre de captures en cours (capture d'écran pendant un enregistrement…) : l'île n'est
/// réintégrée qu'à la fin de la dernière.
static OVERLAY_HIDDEN: Mutex<usize> = Mutex::new(0);

struct OverlayHidden {
    app: AppHandle,
}

fn overlay_hwnd(app: &AppHandle) -> Option<HWND> {
    app.get_webview_window("overlay").and_then(|w| w.hwnd().ok()).map(|h| HWND(h.0))
}

/// Exclut l'île des captures ; `true` si elle vient de l'être (laisser DWM recomposer).
fn hide_overlay(app: &AppHandle) -> (OverlayHidden, bool) {
    let mut n = OVERLAY_HIDDEN.lock().unwrap_or_else(|p| p.into_inner());
    *n += 1;
    let first = *n == 1;
    if first {
        if let Some(h) = overlay_hwnd(app) {
            set_excluded(h, true);
        }
    }
    (OverlayHidden { app: app.clone() }, first)
}

impl Drop for OverlayHidden {
    fn drop(&mut self) {
        let mut n = OVERLAY_HIDDEN.lock().unwrap_or_else(|p| p.into_inner());
        *n = n.saturating_sub(1);
        if *n == 0 {
            if let Some(h) = overlay_hwnd(&self.app) {
                set_excluded(h, false);
            }
        }
    }
}

/// Délai pour que DWM recompose le bureau sans l'île avant la 1ʳᵉ image.
const RECOMPOSE: Duration = Duration::from_millis(90);

// --- Écrans ----------------------------------------------------------------------------------

fn display_of(index: usize, m: &Monitor) -> Display {
    let hmon = HMONITOR(m.as_raw_hmonitor());
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = unsafe { GetMonitorInfoW(hmon, &mut info) };
    let r = info.rcMonitor;
    let (mut dx, mut dy) = (96u32, 96u32);
    let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
    Display {
        index,
        name: m.name().ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Écran {index}")),
        x: r.left,
        y: r.top,
        width: (r.right - r.left).max(0) as u32,
        height: (r.bottom - r.top).max(0) as u32,
        scale: dx.max(1) as f64 / 96.0,
        primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
    }
}

/// Écrans dans l'ordre d'énumération : l'index 1-based correspond à `Monitor::from_index`
/// (et non au numéro `\\.\DISPLAYn`, qui peut différer).
pub fn displays() -> Vec<Display> {
    Monitor::enumerate().unwrap_or_default().iter().enumerate().map(|(i, m)| display_of(i + 1, m)).collect()
}

fn monitor_at(index: usize) -> Result<(Monitor, Display), String> {
    let list = Monitor::enumerate().map_err(|e| e.to_string())?;
    let m = *list.get(index.wrapping_sub(1)).ok_or("invalidDisplay")?;
    Ok((m, display_of(index, &m)))
}

fn cursor_display() -> Result<Display, String> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.map_err(|e| e.to_string())?;
    let hmon = unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) };
    let list = Monitor::enumerate().map_err(|e| e.to_string())?;
    let i = list.iter().position(|m| m.as_raw_hmonitor() == hmon.0).unwrap_or(0);
    list.get(i).map(|m| display_of(i + 1, m)).ok_or_else(|| "invalidDisplay".into())
}

// --- Fenêtres --------------------------------------------------------------------------------

fn own_window(h: HWND) -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid == unsafe { GetCurrentProcessId() }
}

/// Fenêtre capturable : existante, visible, hors Island (ses propres fenêtres ne sont
/// jamais une cible).
fn valid_window(v: i64) -> bool {
    let h = hwnd(v);
    v != 0 && unsafe { IsWindow(Some(h)).as_bool() && IsWindowVisible(h).as_bool() } && !own_window(h)
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

fn title(h: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Bords visibles (sans l'ombre DWM), px physiques du bureau virtuel.
fn frame_rect(h: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    let ok = unsafe {
        DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32)
    }
    .is_ok();
    if ok || unsafe { GetWindowRect(h, &mut r) }.is_ok() {
        Some(r)
    } else {
        None
    }
}

unsafe extern "system" fn collect(h: HWND, lp: LPARAM) -> windows::core::BOOL {
    let list = &mut *(lp.0 as *mut Vec<isize>);
    list.push(h.0 as isize);
    true.into()
}

/// Fenêtres visibles qui recoupent l'écran, ordre Z (dessus d'abord), rect relatif à
/// l'écran et borné à celui-ci. Exclues : invisibles, réduites, masquées (cloaked : autres
/// bureaux virtuels, apps UWP suspendues), fenêtres outils, calques click-through, shell
/// (bureau, barre des tâches), sans titre, et toutes celles d'Island.
fn list_windows(d: &Display) -> Vec<WindowInfo> {
    let mut all: Vec<isize> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut all as *mut Vec<isize> as isize));
    }
    let (mx0, my0) = (d.x, d.y);
    let (mx1, my1) = (d.x + d.width as i32, d.y + d.height as i32);
    let mut out = Vec::new();
    for raw in all {
        let h = HWND(raw as *mut _);
        unsafe {
            if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() || own_window(h) {
                continue;
            }
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
            if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) != 0 {
                continue;
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4);
            if cloaked != 0 {
                continue;
            }
        }
        if matches!(class_name(h).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            continue;
        }
        let t = title(h);
        if t.trim().is_empty() {
            continue;
        }
        let Some(r) = frame_rect(h) else { continue };
        let (x0, y0) = (r.left.max(mx0), r.top.max(my0));
        let (x1, y1) = (r.right.min(mx1), r.bottom.min(my1));
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        out.push(WindowInfo { hwnd: raw as i64, title: t, x: x0 - mx0, y: y0 - my0, w: (x1 - x0) as u32, h: (y1 - y0) as u32 });
    }
    out
}

// --- Sélection (fenêtre `capture-select`) -----------------------------------------------------

struct Pending {
    gen: u64,
    mode: String,
    display: Display,
    tx: mpsc::Sender<Option<Target>>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static GEN: AtomicU64 = AtomicU64::new(0);

/// Annule la sélection en attente (toutes, ou seulement la génération `gen`).
fn cancel_pending(gen: Option<u64>) {
    let mut p = PENDING.lock().unwrap_or_else(|p| p.into_inner());
    if p.as_ref().is_some_and(|p| gen.is_none_or(|g| g == p.gen)) {
        if let Some(p) = p.take() {
            let _ = p.tx.send(None);
        }
    }
}

pub async fn select(app: AppHandle, mode: String) -> Result<Option<Target>, String> {
    // Une seule sélection à la fois : la précédente répond `null`.
    cancel_pending(None);
    if let Some(w) = app.get_webview_window(SELECT_LABEL) {
        let _ = w.destroy();
        let a = app.clone();
        // Le libellé reste pris tant que la fenêtre n'est pas détruite.
        let _ = tauri::async_runtime::spawn_blocking(move || {
            for _ in 0..50 {
                if a.get_webview_window(SELECT_LABEL).is_none() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })
        .await;
    }
    let display = cursor_display()?;
    let (tx, rx) = mpsc::channel();
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some(Pending { gen, mode, display: display.clone(), tx });
    if let Err(e) = open_select_window(&app, &display, gen) {
        cancel_pending(Some(gen));
        return Err(e);
    }
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())
}

/// Fenêtre transparente, sans bordure, toujours devant, couvrant exactement l'écran sous
/// la souris (px physiques), exclue des captures.
fn open_select_window(app: &AppHandle, d: &Display, gen: u64) -> Result<(), String> {
    let s = d.scale.max(0.5);
    let win = WebviewWindowBuilder::new(app, SELECT_LABEL, WebviewUrl::App("index.html#/capture-select".into()))
        .title("Island — capture")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(true)
        .visible(false)
        .disable_drag_drop_handler()
        .position(d.x as f64 / s, d.y as f64 / s)
        .inner_size(d.width as f64 / s, d.height as f64 / s)
        .build()
        .map_err(|e| e.to_string())?;
    // Deux passes : en changeant d'écran, le nouveau DPI n'est appliqué qu'au 1er
    // déplacement (cf. overlay::place).
    for _ in 0..2 {
        let _ = win.set_position(PhysicalPosition::new(d.x, d.y));
        let _ = win.set_size(PhysicalSize::new(d.width, d.height));
    }
    let h = win.hwnd().ok().map(|h| HWND(h.0));
    if let Some(h) = h {
        set_excluded(h, true);
    }
    // Fermée sans réponse (Alt+F4, rechargement…) → `null`.
    win.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            cancel_pending(Some(gen));
        }
    });
    let _ = win.show();
    match h {
        Some(h) => crate::overlay::force_foreground(h.0 as isize),
        None => {
            let _ = win.set_focus();
        }
    }
    Ok(())
}

pub fn select_init() -> Result<SelectInit, String> {
    let (mode, display) = {
        let p = PENDING.lock().unwrap_or_else(|p| p.into_inner());
        let p = p.as_ref().ok_or("noSelection")?;
        (p.mode.clone(), p.display.clone())
    };
    let windows = if mode == "window" { list_windows(&display) } else { Vec::new() };
    Ok(SelectInit { mode, display, windows })
}

/// Valide la réponse de la page (écran imposé = celui de la sélection, zone bornée).
fn sanitize(t: Target, d: &Display) -> Option<Target> {
    match t {
        Target::Screen { .. } => Some(Target::Screen { display: d.index }),
        Target::Region { rect, .. } => clamp_crop(&rect, d.width, d.height, false).map(|(x, y, w, h)| Target::Region {
            display: d.index,
            rect: Rect { x: x as f64, y: y as f64, w: w as f64, h: h as f64 },
        }),
        Target::Window { hwnd } => valid_window(hwnd).then_some(Target::Window { hwnd }),
    }
}

pub fn select_done(window: &WebviewWindow, result: Option<Target>) {
    let p = PENDING.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some(p) = p {
        let _ = p.tx.send(result.and_then(|t| sanitize(t, &p.display)));
    }
    let _ = window.destroy();
}

// --- Démarrage d'une capture WGC ---------------------------------------------------------------

#[derive(Clone, Copy)]
enum Item {
    Monitor(Monitor),
    Window(Window),
}

struct Resolved {
    item: Item,
    crop: Option<Rect>,
    display: Option<Display>,
}

fn resolve(t: &Target) -> Result<Resolved, String> {
    match *t {
        Target::Screen { display } => {
            let (m, d) = monitor_at(display)?;
            Ok(Resolved { item: Item::Monitor(m), crop: None, display: Some(d) })
        }
        Target::Region { display, rect } => {
            let (m, d) = monitor_at(display)?;
            clamp_crop(&rect, d.width, d.height, false).ok_or("invalidRect")?;
            Ok(Resolved { item: Item::Monitor(m), crop: Some(rect), display: Some(d) })
        }
        Target::Window { hwnd: h } => {
            if !valid_window(h) {
                return Err("invalidTarget".into());
            }
            if unsafe { IsIconic(hwnd(h)).as_bool() } {
                return Err("windowMinimized".into());
            }
            Ok(Resolved { item: Item::Window(Window::from_raw_hwnd(hwnd(h).0)), crop: None, display: None })
        }
    }
}

type Control<H> = CaptureControl<H, <H as GraphicsCaptureApiHandler>::Error>;

/// Démarre WGC sans bordure jaune ni curseur demandé ; si Windows ne permet pas de les
/// régler (Windows 10 ancien), réessaie avec les réglages par défaut.
fn start_item<H, T>(item: T, cursor: CursorCaptureSettings, color: ColorFormat, flags: H::Flags) -> Result<Control<H>, String>
where
    H: GraphicsCaptureApiHandler<Error = String> + Send + 'static,
    H::Flags: Send + Clone,
    T: TryIntoCaptureItemWithType + Send + Copy + 'static,
{
    let settings = |c, b, f| {
        Settings::new(
            item,
            c,
            b,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            color,
            f,
        )
    };
    match H::start_free_threaded(settings(cursor, DrawBorderSettings::WithoutBorder, flags.clone())) {
        Ok(c) => Ok(c),
        Err(_) => H::start_free_threaded(settings(CursorCaptureSettings::Default, DrawBorderSettings::Default, flags))
            .map_err(|e| e.to_string()),
    }
}

fn start_capture<H>(item: Item, cursor: CursorCaptureSettings, color: ColorFormat, flags: H::Flags) -> Result<Control<H>, String>
where
    H: GraphicsCaptureApiHandler<Error = String> + Send + 'static,
    H::Flags: Send + Clone,
{
    match item {
        Item::Monitor(m) => start_item::<H, _>(m, cursor, color, flags),
        Item::Window(w) => start_item::<H, _>(w, cursor, color, flags),
    }
}

// --- Capture d'écran -------------------------------------------------------------------------

type ShotTx = mpsc::Sender<Result<(Vec<u8>, u32, u32), String>>;

/// Gestionnaire « une image » : renvoie la 1ʳᵉ image (RGBA, découpée) puis s'arrête.
struct Shot {
    crop: Option<Rect>,
    tx: ShotTx,
}

impl GraphicsCaptureApiHandler for Shot {
    type Flags = (Option<Rect>, ShotTx);
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { crop: ctx.flags.0, tx: ctx.flags.1 })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, ctrl: InternalCaptureControl) -> Result<(), Self::Error> {
        let (fw, fh) = (frame.width(), frame.height());
        let res = (|| {
            let (x, y, w, h) = match &self.crop {
                // Bornée au cadre RÉEL de l'image (≠ écran si le DPI trompe).
                Some(r) => clamp_crop(r, fw, fh, false).ok_or("invalidRect")?,
                None => (0, 0, fw, fh),
            };
            let mut fb =
                if self.crop.is_some() { frame.buffer_crop(x, y, x + w, y + h) } else { frame.buffer() }.map_err(|e| e.to_string())?;
            let data = fb.as_nopadding_buffer().map_err(|e| e.to_string())?.to_vec();
            Ok::<_, String>((data, w, h))
        })();
        let _ = self.tx.send(res);
        ctrl.stop(); // une image suffit
        Ok(())
    }
}

/// Alpha prémultiplié (WGC) → alpha droit (PNG). Écran : opaque.
fn fix_alpha(rgba: &mut [u8], window: bool) {
    for px in rgba.chunks_exact_mut(4) {
        if !window {
            px[3] = 255;
        } else if px[3] != 0 && px[3] != 255 {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

fn grab(r: &Resolved) -> Result<(Vec<u8>, u32, u32), String> {
    let (tx, rx) = mpsc::channel();
    let control = start_capture::<Shot>(r.item, CursorCaptureSettings::WithoutCursor, ColorFormat::Rgba8, (r.crop, tx))?;
    let res = rx.recv_timeout(Duration::from_secs(3));
    let _ = control.stop();
    let (mut rgba, w, h) = res.map_err(|_| "noFrame".to_string())??;
    fix_alpha(&mut rgba, matches!(r.item, Item::Window(_)));
    Ok((rgba, w, h))
}

pub fn screenshot(app: &AppHandle, target: Target, opts: ShotOpts) -> Result<ShotResult, String> {
    let r = resolve(&target)?;
    let (rgba, w, h) = {
        let (_hidden, first) = hide_overlay(app);
        if first {
            std::thread::sleep(RECOMPOSE);
        }
        grab(&r)?
    };
    let mut path = None;
    if opts.save {
        let dir = app.path().picture_dir().map_err(|e| e.to_string())?.join("Island");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let p = unique_path(&dir, &stamped_name("Capture", "png", local_now()));
        image::save_buffer_with_format(&p, &rgba, w, h, image::ColorType::Rgba8, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        path = Some(p.to_string_lossy().to_string());
    }
    if opts.clipboard {
        // PNG + bitmap (arboard) : collable tel quel dans un chat, un document, etc. Le
        // presse-papiers peut être tenu un instant par une autre app (ou par l'historique
        // du presse-papiers d'Island qui réagit à la capture précédente) : on réessaie.
        let mut copied = Err(arboard::Error::ClipboardOccupied);
        for attempt in 0..10 {
            let img = arboard::ImageData { width: w as usize, height: h as usize, bytes: std::borrow::Cow::Borrowed(&rgba) };
            copied = arboard::Clipboard::new().and_then(|mut c| c.set_image(img));
            if copied.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(40 + attempt * 30));
        }
        if let Err(e) = copied {
            eprintln!("[capture] presse-papiers : {e}");
            // Sans gravité si le fichier est enregistré.
            if path.is_none() {
                return Err(format!("clipboard: {e}"));
            }
        }
    }
    Ok(ShotResult { path, width: w, height: h })
}

// --- Enregistrement ----------------------------------------------------------------------------

/// Dernière image (BGRA, lignes de bas en haut, taille fixe du canevas) partagée entre
/// le gestionnaire WGC et la pompe à cadence fixe.
struct Shared {
    crop: Option<Rect>,
    /// Taille du canevas, fixée à la 1ʳᵉ image (paire) : une fenêtre redimensionnée est
    /// ensuite recadrée / complétée de noir, l'encodeur ne change jamais de taille.
    canvas: Mutex<Option<(u32, u32)>>,
    latest: Mutex<Option<Arc<Vec<u8>>>>,
    /// Cible disparue (fenêtre fermée, écran débranché).
    closed: AtomicBool,
}

impl Shared {
    fn latest(&self) -> Option<Arc<Vec<u8>>> {
        self.latest.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

struct Recorder {
    shared: Arc<Shared>,
}

impl GraphicsCaptureApiHandler for Recorder {
    type Flags = Arc<Shared>;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { shared: ctx.flags })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _ctrl: InternalCaptureControl) -> Result<(), Self::Error> {
        let (fw, fh) = (frame.width(), frame.height());
        // Zone bornée au cadre réel, dimensions paires (H.264 / H.265).
        let Some((x, y, w, h)) = (match &self.shared.crop {
            Some(r) => clamp_crop(r, fw, fh, true),
            None => {
                let (w, h) = (even_down(fw), even_down(fh));
                (w >= 2 && h >= 2).then_some((0, 0, w, h))
            }
        }) else {
            return Ok(()); // image dégénérée : on la saute
        };
        let (cw, ch) = *self.shared.canvas.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert((w, h));
        let mut fb = if (x, y, w, h) == (0, 0, fw, fh) { frame.buffer() } else { frame.buffer_crop(x, y, x + w, y + h) }
            .map_err(|e| e.to_string())?;
        let (bw, bh, pitch) = (fb.width() as usize, fb.height() as usize, fb.row_pitch() as usize);
        let raw = fb.as_raw_buffer();
        let (cw, ch) = (cw as usize, ch as usize);

        // Réutilise le tampon précédent s'il n'est plus en file (évite ~8 Mo d'allocation
        // par image en 1080p).
        let prev = self.shared.latest.lock().unwrap_or_else(|p| p.into_inner()).take();
        let mut buf = match prev.map(Arc::try_unwrap) {
            Some(Ok(v)) if v.len() == cw * ch * 4 => v,
            _ => vec![0u8; cw * ch * 4],
        };
        if bw < cw || bh < ch {
            buf.fill(0);
        }
        // Media Foundation attend du RGB32 de bas en haut : on retourne les lignes en copiant.
        let row = bw.min(cw) * 4;
        for r in 0..bh.min(ch) {
            let dst = (ch - 1 - r) * cw * 4;
            buf[dst..dst + row].copy_from_slice(&raw[r * pitch..r * pitch + row]);
        }
        *self.shared.latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(buf));
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.shared.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Bilan de la pompe.
struct Outcome {
    duration_ms: u64,
    /// Cause d'un arrêt anormal.
    error: Option<String>,
    /// Le fichier est inutilisable (encodeur en échec).
    fatal: bool,
}

struct Recording {
    id: String,
    target: Target,
    started_at: u64,
    path: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Outcome>>,
    control: Option<Control<Recorder>>,
    audio_stop: Arc<AtomicBool>,
    audio_threads: Vec<JoinHandle<()>>,
    outline: Option<outline::Outline>,
    _overlay: OverlayHidden,
}

static RECORDING: Mutex<Option<Recording>> = Mutex::new(None);
static STARTING: AtomicBool = AtomicBool::new(false);
/// Résultat d'un arrêt automatique, rendu au prochain `stop` de l'extension propriétaire.
static LAST: Mutex<Option<(String, Result<StopResult, String>)>> = Mutex::new(None);

/// Bloc audio : 20 ms à 48 kHz.
const CHUNK: usize = 960;
/// Retard de lecture de l'audio sur l'horloge (le temps que les paquets WASAPI arrivent) ;
/// ce qui n'est pas arrivé à temps devient du silence.
const AUDIO_LAG: i64 = 800_000; // 80 ms

struct ResetStarting;
impl Drop for ResetStarting {
    fn drop(&mut self) {
        STARTING.store(false, Ordering::SeqCst);
    }
}

/// Arrêt propre à la fermeture d'Island (sinon MP4 sans index = illisible). Le hook est un
/// mini-plugin enregistré à la volée : `lib.rs` n'a pas de boucle d'événements à nous.
fn ensure_exit_hook(app: &AppHandle) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let plugin = tauri::plugin::Builder::<tauri::Wry>::new("island-capture")
        .on_event(|app, ev| {
            if matches!(ev, tauri::RunEvent::Exit) {
                cancel_pending(None);
                let _ = stop_internal(app, None);
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("[capture] hook de fermeture : {e}");
    }
}

pub fn record_start(app: &AppHandle, id: String, target: Target, opts: RecordOpts) -> Result<(), String> {
    if STARTING.swap(true, Ordering::SeqCst) {
        return Err("alreadyRecording".into());
    }
    let _reset = ResetStarting;
    if RECORDING.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return Err("alreadyRecording".into());
    }
    ensure_exit_hook(app);
    let fps = opts.fps.clamp(1, 60);
    let r = resolve(&target)?;

    let (overlay, first) = hide_overlay(app);
    if first {
        std::thread::sleep(RECOMPOSE);
    }
    let shared = Arc::new(Shared {
        crop: r.crop,
        canvas: Mutex::new(None),
        latest: Mutex::new(None),
        closed: AtomicBool::new(false),
    });
    // Curseur visible en vidéo (comportement par défaut de WGC).
    let control = start_capture::<Recorder>(r.item, CursorCaptureSettings::Default, ColorFormat::Bgra8, shared.clone())?;
    let fail = |control: Control<Recorder>, e: &str| -> Result<(), String> {
        let _ = control.stop();
        Err(e.to_string())
    };

    // 1ʳᵉ image (≤ 3 s) → taille réelle du flux (≠ taille annoncée en DPI ou fenêtre).
    let mut waited = 0;
    while shared.latest().is_none() {
        if waited >= 3000 || shared.closed.load(Ordering::SeqCst) {
            return fail(control, "noFrame");
        }
        std::thread::sleep(Duration::from_millis(10));
        waited += 10;
    }
    let Some((in_w, in_h)) = *shared.canvas.lock().unwrap_or_else(|p| p.into_inner()) else {
        return fail(control, "noFrame");
    };
    if in_w < 16 || in_h < 16 {
        return fail(control, "targetTooSmall");
    }
    let (out_w, out_h) = fit_dims(in_w, in_h, opts.codec);
    let audio = opts.system_audio || opts.mic;
    let cfg = encoder::Config {
        in_w,
        in_h,
        out_w,
        out_h,
        fps,
        codec: opts.codec,
        bitrate: bitrate(out_w, out_h, fps, opts.quality),
        audio,
    };

    let dir = match app.path().video_dir() {
        Ok(d) => d.join("Island"),
        Err(e) => return fail(control, &e.to_string()),
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(control, &e.to_string());
    }
    let path = unique_path(&dir, &stamped_name("Enregistrement", "mp4", local_now()));

    let mut sources = Vec::new();
    if opts.system_audio {
        sources.push((audio::Kind::Loopback, Arc::new(Source::default())));
    }
    if opts.mic {
        sources.push((audio::Kind::Mic, Arc::new(Source::default())));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel::<Result<i64, String>>();
    let worker = {
        let (app, path, shared, stop) = (app.clone(), path.clone(), shared.clone(), stop.clone());
        let mixer = audio.then(|| Mixer::new(sources.iter().map(|(_, s)| s.clone()).collect()));
        std::thread::spawn(move || pump(app, cfg, path, shared, mixer, stop, ready_tx))
    };
    let t0 = match ready_rx.recv() {
        Ok(Ok(t0)) => t0,
        Ok(Err(e)) => {
            let _ = worker.join();
            let _ = std::fs::remove_file(&path);
            return fail(control, &e);
        }
        Err(_) => {
            let _ = worker.join();
            let _ = std::fs::remove_file(&path);
            return fail(control, "encoderFailed");
        }
    };
    let audio_stop = Arc::new(AtomicBool::new(false));
    let audio_threads = sources.into_iter().map(|(k, s)| audio::start(k, s, t0, audio_stop.clone())).collect();

    // Contour de la zone (exclu de la vidéo, ne capte pas la souris).
    let outline = match (&r.display, r.crop) {
        (Some(d), Some(rect)) => clamp_crop(&rect, d.width, d.height, true)
            .and_then(|(x, y, w, h)| outline::show(d.x + x as i32, d.y + y as i32, w as i32, h as i32, d.scale)),
        _ => None,
    };

    let started_at = now_ms();
    *RECORDING.lock().unwrap_or_else(|p| p.into_inner()) = Some(Recording {
        id: id.clone(),
        target,
        started_at,
        path,
        stop,
        worker: Some(worker),
        control: Some(control),
        audio_stop,
        audio_threads,
        outline,
        _overlay: overlay,
    });
    *LAST.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let _ = app.emit(
        RECORDING_EVENT,
        RecordingEvent { recording: true, started_at, id, path: None, duration_ms: None, error: None },
    );
    Ok(())
}

/// Pompe à cadence fixe : pousse la dernière image toutes les 1/fps (répétée si l'écran ne
/// bouge pas — WGC n'envoie rien sans changement), horodatée `n / fps` sur l'horloge QPC ;
/// en retard, saute des images plutôt que d'envoyer une rafale. L'audio est mélangé par
/// blocs de 20 ms jusqu'à `maintenant - AUDIO_LAG`, horodaté sur la même horloge.
fn pump(
    app: AppHandle,
    cfg: encoder::Config,
    path: PathBuf,
    shared: Arc<Shared>,
    mut mixer: Option<Mixer>,
    stop: Arc<AtomicBool>,
    ready: mpsc::Sender<Result<i64, String>>,
) -> Outcome {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let enc = match Encoder::new(&path, &cfg) {
        Ok(e) => e,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Outcome { duration_ms: 0, error: Some(e), fatal: true };
        }
    };
    let t0 = qpc_100ns();
    let _ = ready.send(Ok(t0));
    let fps = cfg.fps;
    let vdur = encoder::frame_duration(fps);
    let mut n: u64 = 0;
    let mut error: Option<String> = None;
    let mut fatal = false;

    let push_audio = |m: &mut Mixer, frames: usize| {
        let (at, pcm) = m.next(frames);
        enc.push_audio(Sample { bytes: Arc::new(pcm), ts: sample_ts(at, audio::RATE), dur: sample_ts(frames as u64, audio::RATE) });
    };

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if shared.closed.load(Ordering::SeqCst) {
            error = Some("targetClosed".into());
            break;
        }
        if enc.failed() {
            error = Some(enc.failure().filter(|e| e == "codecUnavailable" || e == "encoderUnavailable").unwrap_or_else(|| "encoderFailed".into()));
            fatal = true;
            break;
        }
        let now = qpc_100ns() - t0;
        let due = frame_due(now, fps);
        if due >= n {
            if due > n + 1 {
                n = due;
            }
            if let Some(f) = shared.latest() {
                enc.push_video(Sample { bytes: f, ts: frame_ts(n, fps), dur: vdur });
            }
            n += 1;
        }
        if let Some(m) = mixer.as_mut() {
            let until = ((now - AUDIO_LAG).max(0) as i128 * audio::RATE as i128 / TICKS_PER_SEC as i128) as u64;
            while m.pos() + CHUNK as u64 <= until {
                push_audio(m, CHUNK);
            }
        }
        let wait = frame_ts(n, fps) - (qpc_100ns() - t0);
        std::thread::sleep(Duration::from_micros((wait / 10).clamp(1_000, 10_000) as u64));
    }

    // Audio jusqu'à la fin exacte de la vidéo (`n` images) : on laisse arriver les derniers
    // paquets puis on complète ; l'audio ne dépasse jamais la vidéo.
    let end = frame_ts(n, fps);
    if let Some(m) = mixer.as_mut() {
        if !fatal {
            std::thread::sleep(Duration::from_micros((AUDIO_LAG / 10) as u64));
        }
        let end_pos = (end as i128 * audio::RATE as i128 / TICKS_PER_SEC as i128) as u64;
        while m.pos() < end_pos {
            let k = (end_pos - m.pos()).min(CHUNK as u64) as usize;
            push_audio(m, k);
        }
    }
    if let Err(e) = enc.finish(Duration::from_secs(20)) {
        error.get_or_insert(e);
        fatal = true;
    }
    // Arrêt de lui-même (cible fermée, encodeur en échec) : libère tout et prévient le front.
    if error.is_some() && !stop.load(Ordering::SeqCst) {
        std::thread::spawn(move || {
            let _ = stop_internal(&app, None);
        });
    }
    Outcome { duration_ms: (end / 10_000) as u64, error, fatal }
}

/// Arrête l'enregistrement en cours. `owner` : extension demandeuse (doit être la
/// propriétaire) ; `None` = arrêt interne (erreur, fermeture d'Island).
fn stop_internal(app: &AppHandle, owner: Option<&str>) -> Result<StopResult, String> {
    let mut rec = {
        let mut g = RECORDING.lock().unwrap_or_else(|p| p.into_inner());
        match g.as_ref() {
            None => return Err("notRecording".into()),
            Some(r) if owner.is_some_and(|o| o != r.id) => return Err("notOwner".into()),
            _ => g.take().ok_or("notRecording")?,
        }
    };
    rec.stop.store(true, Ordering::SeqCst);
    let outcome = rec
        .worker
        .take()
        .and_then(|w| w.join().ok())
        .unwrap_or(Outcome { duration_ms: 0, error: Some("panic".into()), fatal: true });
    if let Some(c) = rec.control.take() {
        let _ = c.stop();
    }
    rec.audio_stop.store(true, Ordering::SeqCst);
    for t in rec.audio_threads.drain(..) {
        let _ = t.join();
    }
    drop(rec.outline.take());
    let path = rec.path.to_string_lossy().to_string();
    let result = if outcome.fatal {
        let _ = std::fs::remove_file(&rec.path);
        Err(outcome.error.clone().unwrap_or_else(|| "encoderFailed".into()))
    } else {
        Ok(StopResult { path: path.clone(), duration_ms: outcome.duration_ms })
    };
    let _ = app.emit(
        RECORDING_EVENT,
        RecordingEvent {
            recording: false,
            started_at: rec.started_at,
            id: rec.id.clone(),
            path: result.as_ref().ok().map(|r| r.path.clone()),
            duration_ms: result.as_ref().ok().map(|r| r.duration_ms),
            error: outcome.error.clone(),
        },
    );
    if owner.is_none() {
        *LAST.lock().unwrap_or_else(|p| p.into_inner()) = Some((rec.id.clone(), result.clone()));
    }
    // `rec` (et la garde de l'île) libéré ici : l'île réapparaît dans les captures.
    result
}

pub fn record_stop(app: &AppHandle, id: &str) -> Result<StopResult, String> {
    match stop_internal(app, Some(id)) {
        Err(e) if e == "notRecording" => {
            // Arrêté tout seul entre-temps : on rend son résultat une fois.
            let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
            match last.take() {
                Some((owner, res)) if owner == id => res,
                other => {
                    *last = other;
                    Err(e)
                }
            }
        }
        r => r,
    }
}

pub fn record_state() -> RecordState {
    let g = RECORDING.lock().unwrap_or_else(|p| p.into_inner());
    match g.as_ref() {
        Some(r) => RecordState { recording: true, started_at: Some(r.started_at), target: Some(r.target), id: Some(r.id.clone()) },
        None => RecordState { recording: false, started_at: None, target: None, id: None },
    }
}

// --- Contour de la zone enregistrée ----------------------------------------------------------

/// Cadre Win32 natif autour de la zone : fenêtre en couches, transparente aux clics
/// (WS_EX_TRANSPARENT + HTTRANSPARENT), sans activation, toujours devant, exclue des
/// captures, découpée en anneau (SetWindowRgn) et tracée À L'EXTÉRIEUR de la zone.
mod outline {
    use std::sync::mpsc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateSolidBrush, DeleteObject, SetWindowRgn, RGN_DIFF};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage, RegisterClassW,
        SetLayeredWindowAttributes, SetWindowDisplayAffinity, ShowWindow, TranslateMessage, HTTRANSPARENT, LWA_ALPHA, MSG,
        SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WM_CLOSE, WM_DESTROY, WM_NCHITTEST, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    pub struct Outline {
        hwnd: isize,
        thread: Option<JoinHandle<()>>,
    }

    /// `x, y, w, h` : zone en px physiques du bureau virtuel.
    pub fn show(x: i32, y: i32, w: i32, h: i32, scale: f64) -> Option<Outline> {
        let b = (2.0 * scale).round().max(2.0) as i32;
        let (tx, rx) = mpsc::channel::<Option<isize>>();
        let thread = std::thread::spawn(move || unsafe { run(x - b, y - b, w + 2 * b, h + 2 * b, b, tx) });
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Some(hwnd)) => Some(Outline { hwnd, thread: Some(thread) }),
            _ => None,
        }
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(h, msg, wp, lp),
        }
    }

    unsafe fn run(x: i32, y: i32, w: i32, h: i32, b: i32, tx: mpsc::Sender<Option<isize>>) {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let class = w!("IslandCaptureOutline");
        static REGISTER: std::sync::Once = std::sync::Once::new();
        REGISTER.call_once(|| {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: hinst.into(),
                lpszClassName: class,
                // Rouge « REC » (COLORREF = 0x00BBGGRR).
                hbrBackground: CreateSolidBrush(COLORREF(0x0030_3BFF)),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });
        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            w!("Island — zone enregistrée"),
            WS_POPUP,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                let _ = tx.send(None);
                return;
            }
        };
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        // Anneau : rectangle extérieur moins la zone (le centre n'existe pas pour la souris).
        let outer = CreateRectRgn(0, 0, w, h);
        let inner = CreateRectRgn(b, b, w - b, h - b);
        CombineRgn(Some(outer), Some(outer), Some(inner), RGN_DIFF);
        let _ = DeleteObject(inner.into());
        SetWindowRgn(hwnd, Some(outer), false); // la région appartient désormais au système
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = tx.send(Some(hwnd.0 as isize));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    impl Drop for Outline {
        fn drop(&mut self) {
            unsafe {
                // WM_CLOSE → DefWindowProc → DestroyWindow (thread propriétaire) → WM_QUIT.
                let _ = PostMessageW(Some(HWND(self.hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }
}
