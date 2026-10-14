// Fenêtre du notch : taille FIXE (760 × 560 DIP), collée en haut-centre du moniteur
// choisi, jamais redimensionnée — toute l'animation se fait dans la WebView. Une fenêtre
// interactive plein écran met les apps en dessous en pause (occlusion Chromium) ; une
// boîte fixe qui ne couvre que le haut-centre n'a pas ce problème et évite tout resize
// natif pendant les springs.
//
// Click-through : le front publie le(s) rectangle(s) de hit (forme + marge) ; un thread
// Rust lit le curseur et bascule `set_ignore_cursor_events` uniquement au changement.
// Comme une WebView click-through ne reçoit plus d'events souris, ce thread émet aussi
// le survol (`overlay://hover`) et le clic hors zone (`overlay://dismiss`).

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};
#[cfg(target_os = "windows")]
use tauri::Emitter;

/// Taille logique de la fenêtre (zone utile 740 × 540 + 10 px de marge pour l'ombre).
pub const WIN_W: f64 = 760.0;
pub const WIN_H: f64 = 560.0;

/// Rectangle interactif (px CSS, relatif à la fenêtre).
#[derive(serde::Deserialize)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

static HIT_REGIONS: Mutex<Vec<Rect>> = Mutex::new(Vec::new());
/// Garder la fenêtre cliquable même hors zone (holds : saisie, drag, dialogue système…).
static PINNED: AtomicBool = AtomicBool::new(false);
/// Mode focus : toujours click-through, seuls les triple-clics sur le notch sont comptés.
static PASSTHROUGH: AtomicBool = AtomicBool::new(false);
/// Force la réapplication de l'état click-through au prochain tick (après un `show`).
static REFRESH: AtomicBool = AtomicBool::new(false);
/// Masquer la fenêtre quand une app passe en plein écran.
pub(crate) static HIDE_IN_FULLSCREEN: AtomicBool = AtomicBool::new(false);
/// Fenêtre au premier plan avant que le notch prenne le focus clavier.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

// --- Placement (moniteur + décalage), persisté dans le store de l'app (`__app__.json`,
// clé `display`) pour placer la fenêtre sans saut au démarrage, avant que le front charge.

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct DisplayPrefs {
    /// Nom de périphérique (`\\.\DISPLAYn`) ; `None` = principal.
    monitor: Option<String>,
    /// Décalage horizontal depuis le centre, en DIP.
    offset: f64,
}

static DISPLAY: Mutex<Option<DisplayPrefs>> = Mutex::new(None);

const STORE: &str = "__app__";
const KEY: &str = "display";

/// Builds alpha : le placement vivait dans `display.json` → repris une fois puis supprimé.
fn migrate_legacy(app: &AppHandle) -> Option<DisplayPrefs> {
    let path = app.path().app_config_dir().ok()?.join("display.json");
    let prefs: DisplayPrefs = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    crate::store::set(app, STORE, KEY, serde_json::to_value(&prefs).ok());
    let _ = std::fs::remove_file(path);
    Some(prefs)
}

fn display(app: &AppHandle) -> DisplayPrefs {
    let mut d = DISPLAY.lock().unwrap_or_else(|p| p.into_inner());
    d.get_or_insert_with(|| {
        crate::store::get(app, STORE, KEY)
            .and_then(|v| serde_json::from_value(v).ok())
            .or_else(|| migrate_legacy(app))
            .unwrap_or_default()
    })
    .clone()
}

fn find_monitor(win: &WebviewWindow, name: &Option<String>) -> tauri::Result<Option<Monitor>> {
    if let Some(n) = name {
        if let Some(m) = win.available_monitors()?.into_iter().find(|m| m.name() == Some(n)) {
            return Ok(Some(m));
        }
    }
    win.primary_monitor()
}

/// Place la fenêtre en haut-centre du moniteur choisi, décalée de l'offset persisté.
/// Appliqué deux fois : en changeant de moniteur, le nouveau DPI n'est pris en compte
/// qu'après le premier déplacement.
pub(crate) fn place(win: &WebviewWindow) -> tauri::Result<()> {
    let prefs = display(win.app_handle());
    for _ in 0..2 {
        let Some(m) = find_monitor(win, &prefs.monitor)? else { return Ok(()) };
        let scale = m.scale_factor();
        let (mw, w, h) = (m.size().width as f64, WIN_W * scale, WIN_H * scale);
        let max = ((mw - w) / 2.0).max(0.0);
        let dx = (prefs.offset * scale).clamp(-max, max);
        win.set_size(PhysicalSize::new(w.round() as u32, h.round() as u32))?;
        win.set_position(PhysicalPosition::new(
            m.position().x + ((mw - w) / 2.0 + dx).round() as i32,
            m.position().y,
        ))?;
    }
    REFRESH.store(true, Ordering::SeqCst);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct MonitorInfo {
    name: String,
    primary: bool,
    width: f64,
    height: f64,
}

/// Moniteurs disponibles (taille en DIP) pour le sélecteur des réglages.
#[tauri::command]
pub fn notch_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let win = app.get_webview_window("overlay").ok_or("overlay introuvable")?;
    let primary = win.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    let list = win.available_monitors().map_err(|e| e.to_string())?;
    Ok(list
        .iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            let s = m.scale_factor();
            Some(MonitorInfo {
                primary: Some(&name) == primary.as_ref(),
                width: m.size().width as f64 / s,
                height: m.size().height as f64 / s,
                name,
            })
        })
        .collect())
}

#[tauri::command]
pub fn notch_get_display(app: AppHandle) -> DisplayPrefs {
    display(&app)
}

/// Déplace le notch. Pendant un drag on l'appelle à chaque frame avec `persist = false`,
/// puis une fois au relâchement avec `persist = true`.
#[tauri::command]
pub fn notch_set_display(
    app: AppHandle,
    monitor: Option<String>,
    offset: f64,
    persist: bool,
) -> Result<(), String> {
    let prefs = DisplayPrefs { monitor, offset };
    if persist {
        crate::store::set(&app, STORE, KEY, serde_json::to_value(&prefs).ok());
    }
    *DISPLAY.lock().unwrap_or_else(|p| p.into_inner()) = Some(prefs);
    let win = app.get_webview_window("overlay").ok_or("overlay introuvable")?;
    place(&win).map_err(|e| e.to_string())
}

// --- Click-through, holds, mode focus ------------------------------------------------

/// Le front publie ses zones interactives courantes.
#[tauri::command]
pub fn set_hit_regions(regions: Vec<Rect>) {
    *HIT_REGIONS.lock().unwrap_or_else(|p| p.into_inner()) = regions;
}

/// Garde la fenêtre cliquable hors de la zone de hit (cf. « holds » côté front).
#[tauri::command]
pub fn notch_set_pinned(pinned: bool) {
    PINNED.store(pinned, Ordering::SeqCst);
    REFRESH.store(true, Ordering::SeqCst);
}

/// Mode focus : la fenêtre laisse tout passer ; trois clics rapides sur le notch émettent
/// `notch://focus-exit`.
#[tauri::command]
pub fn notch_set_passthrough(on: bool) {
    PASSTHROUGH.store(on, Ordering::SeqCst);
    REFRESH.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn notch_set_hide_in_fullscreen(on: bool) {
    HIDE_IN_FULLSCREEN.store(on, Ordering::SeqCst);
}

/// Réapplique l'état click-through au prochain tick (après un `show`).
pub(crate) fn refresh() {
    REFRESH.store(true, Ordering::SeqCst);
}

// --- Focus clavier -------------------------------------------------------------------

/// Met le notch au premier plan SANS simuler de touche. `set_focus` de Tauri, quand Windows
/// refuse le premier plan (Island est en fond : la touche Windows a été interceptée, le
/// raccourci global…), simule un appui sur Alt ; le shell en profitait pour donner le
/// focus à sa fenêtre de recherche, qui s'affichait ensuite (le « menu Démarrer » qui
/// s'ouvrait à la fermeture du launcher). Méthode PowerToys Run : on attache un instant
/// notre file d'entrée à celle de la fenêtre au premier plan, ce qui autorise
/// `SetForegroundWindow`, puis on détache.
#[cfg(target_os = "windows")]
pub(crate) fn force_foreground(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow};
    unsafe {
        let h = HWND(hwnd as *mut _);
        let fg = GetForegroundWindow();
        if fg == h {
            return;
        }
        if SetForegroundWindow(h).as_bool() {
            return;
        }
        let me = GetCurrentThreadId();
        let other = GetWindowThreadProcessId(fg, None);
        let attached = other != 0 && other != me && AttachThreadInput(me, other, true).as_bool();
        let _ = BringWindowToTop(h);
        // Pas de SetFocus sur la fenêtre : la WebView reprend le focus à l'activation ; le
        // forcer sur la fenêtre principale faisait perdre le focus à la WebView → le notch
        // se refermait aussitôt.
        let _ = SetForegroundWindow(h);
        if attached {
            let _ = AttachThreadInput(me, other, false);
        }
    }
}

/// Fenêtres du shell (recherche / Démarrer, barre des tâches, bureau) : on ne leur rend
/// jamais le focus — réactiver le bureau faisait passer la recherche Windows au premier
/// plan (le « menu Démarrer » qui s'ouvrait à la fermeture du launcher).
#[cfg(target_os = "windows")]
fn is_shell_window(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut cls = [0u16; 64];
    let n = unsafe { GetClassNameW(HWND(hwnd as *mut _), &mut cls) } as usize;
    let c = String::from_utf16_lossy(&cls[..n]);
    matches!(
        c.as_str(),
        "Windows.UI.Core.CoreWindow" | "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "XamlExplorerHostIslandWindow" | "TopLevelWindowForOverflowXamlIsland"
    )
}

/// `take = true` : mémorise la fenêtre au premier plan puis donne le focus au notch (Échap,
/// saisie). `take = false` : si le notch a toujours le focus, le rend à la fenêtre
/// précédente — sinon la frappe irait dans le vide.
#[tauri::command]
pub fn notch_focus(app: AppHandle, take: bool) -> Result<(), String> {
    let win = app.get_webview_window("overlay").ok_or("overlay introuvable")?;
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};
        let ours = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        let fg = unsafe { GetForegroundWindow() }.0 as isize;
        if take {
            if fg != ours {
                PREV_FOREGROUND.store(if is_shell_window(fg) { 0 } else { fg }, Ordering::SeqCst);
            }
            force_foreground(ours);
        } else {
            let prev = PREV_FOREGROUND.swap(0, Ordering::SeqCst);
            if fg == ours && prev != 0 && !is_shell_window(prev) {
                unsafe {
                    let _ = SetForegroundWindow(HWND(prev as *mut _));
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    if take {
        win.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Donne le focus clavier à l'overlay (utilisé par les extensions).
#[tauri::command]
pub fn overlay_focus(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("overlay") {
        #[cfg(target_os = "windows")]
        force_foreground(w.hwnd().map(|h| h.0 as isize).unwrap_or(0));
        #[cfg(not(target_os = "windows"))]
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// --- Mémoire WebView2 ----------------------------------------------------------------

/// Notch fermé depuis un moment → cible mémoire WebView2 « Low » (~300 → ~80 Mo) ;
/// retour à « Normal » au premier survol. Ignoré si le runtime est trop ancien.
#[tauri::command]
pub fn notch_memory_low(app: AppHandle, low: bool) -> Result<(), String> {
    let win = app.get_webview_window("overlay").ok_or("overlay introuvable")?;
    #[cfg(target_os = "windows")]
    win.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows::core::Interface;
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        if let Ok(core) = wv.controller().CoreWebView2() {
            if let Ok(c19) = core.cast::<ICoreWebView2_19>() {
                let _ = c19.SetMemoryUsageTargetLevel(level);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "windows"))]
    let _ = (win, low);
    Ok(())
}

// --- Thread de hit-test --------------------------------------------------------------

/// Distance (px CSS) du point au rectangle ; 0 dedans.
fn distance(r: &Rect, x: f64, y: f64) -> f64 {
    let dx = (r.x - x).max(0.0).max(x - (r.x + r.w));
    let dy = (r.y - y).max(0.0).max(y - (r.y + r.h));
    (dx * dx + dy * dy).sqrt()
}

/// Cadence adaptative : ~60 Hz quand le curseur est à moins de 160 px de la zone (ou
/// bouton enfoncé), 20 Hz sinon.
#[cfg(target_os = "windows")]
pub(crate) fn start_click_through(win: WebviewWindow) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    const NEAR: f64 = 160.0;
    /// Zone de dépôt agrandie pendant un vrai drag venu d'ailleurs (Explorateur…).
    const DROP_GROW: f64 = 48.0;
    const DRAG_START: f64 = 6.0;
    const TRIPLE_GAP: std::time::Duration = std::time::Duration::from_millis(450);

    let _ = win.set_ignore_cursor_events(true);
    let app = win.app_handle().clone();
    std::thread::spawn(move || {
        let mut last_inside = false;
        let mut last_ignore = true;
        let mut last_lbtn = false;
        let mut clicks: (u8, std::time::Instant) = (0, std::time::Instant::now());
        // Appui commencé HORS de la zone agrandie (position écran), et drag confirmé.
        let mut press_outside: Option<(i32, i32)> = None;
        let mut drop_zone = false;
        let mut scale = win.scale_factor().unwrap_or(1.0);
        let mut i: u32 = 0;
        loop {
            if i % 40 == 0 {
                scale = win.scale_factor().unwrap_or(scale);
            }
            i = i.wrapping_add(1);
            let origin = win.outer_position().map(|p| (p.x, p.y)).unwrap_or((0, 0));

            let mut pt = POINT::default();
            let lbtn = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0;
            let pressed = lbtn && !last_lbtn;
            last_lbtn = lbtn;
            let cursor = unsafe { GetCursorPos(&mut pt) }.is_ok();
            let (cx, cy) = ((pt.x - origin.0) as f64 / scale, (pt.y - origin.1) as f64 / scale);
            // Distance à la zone de hit, agrandie de `grow` à gauche, à droite et en bas.
            let dist = |grow: f64| {
                HIT_REGIONS
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .iter()
                    .map(|r| distance(&Rect { x: r.x - grow, y: r.y, w: r.w + 2.0 * grow, h: r.h + grow }, cx, cy))
                    .fold(f64::INFINITY, f64::min)
            };

            // Dépôt rapide : un appui qui commence en dehors de la zone agrandie puis bouge de
            // plus de 6 px est un vrai drag (fichiers depuis l'Explorateur) → la zone grandit
            // pour faciliter le dépôt sur le petit notch. Un simple clic juste sous le notch
            // ne l'agrandit jamais.
            if pressed {
                press_outside = (cursor && dist(DROP_GROW) > 0.0).then_some((pt.x, pt.y));
            }
            if !lbtn {
                press_outside = None;
                drop_zone = false;
            } else if let Some((x0, y0)) = press_outside {
                let moved = (((pt.x - x0).pow(2) + (pt.y - y0).pow(2)) as f64).sqrt() / scale;
                if moved > DRAG_START {
                    drop_zone = true;
                }
            }

            let (over, near) = if cursor {
                let d = dist(if drop_zone { DROP_GROW } else { 0.0 });
                (d == 0.0, d < NEAR)
            } else {
                (false, false)
            };

            let passthrough = PASSTHROUGH.load(Ordering::SeqCst);
            let inside = over && !passthrough;

            let ignore = passthrough || !(inside || PINNED.load(Ordering::SeqCst));
            if ignore != last_ignore || REFRESH.swap(false, Ordering::SeqCst) {
                let _ = win.set_ignore_cursor_events(ignore);
                last_ignore = ignore;
            }
            if inside != last_inside {
                let _ = app.emit("overlay://hover", inside);
                last_inside = inside;
            }

            if pressed {
                if passthrough && over {
                    let now = std::time::Instant::now();
                    clicks = if now - clicks.1 <= TRIPLE_GAP { (clicks.0 + 1, now) } else { (1, now) };
                    if clicks.0 >= 3 {
                        clicks.0 = 0;
                        let _ = app.emit("notch://focus-exit", ());
                    }
                } else if !inside && !drop_zone {
                    let _ = app.emit("overlay://dismiss", ());
                }
            }

            let ms = if near || lbtn { 16 } else { 50 };
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    });
}

// --- Pont de dépôt de fichiers ---------------------------------------------------------
// Le drag-drop natif de Tauri est désactivé sur l'overlay (selon la version de WebView2, la
// fenêtre sous le curseur appartient à un autre process → curseur « interdit », pas de
// dépôt). À la place : drop HTML5 normal dans la page, qui renvoie les objets `File` via
// `chrome.webview.postMessageWithAdditionalObjects("island-drop", files)` ; ici on lit
// leurs chemins réels (`ICoreWebView2File::Path`) et on émet `drop://files`.

#[cfg(target_os = "windows")]
pub(crate) fn install_drop_bridge(win: &WebviewWindow) {
    let app = win.app_handle().clone();
    let _ = win.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs2,
        };
        use webview2_com::{take_pwstr, WebMessageReceivedEventHandler};
        use windows::core::{Interface, PWSTR};

        let Ok(core) = wv.controller().CoreWebView2() else { return };
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut msg = PWSTR::null();
            if args.TryGetWebMessageAsString(&mut msg).is_err() || take_pwstr(msg) != "island-drop" {
                return Ok(());
            }
            let mut paths = Vec::new();
            if let Ok(objs) = args.cast::<ICoreWebView2WebMessageReceivedEventArgs2>().and_then(|a| a.AdditionalObjects()) {
                let mut n = 0u32;
                let _ = objs.Count(&mut n);
                for i in 0..n {
                    let Ok(file) = objs.GetValueAtIndex(i).and_then(|o| o.cast::<ICoreWebView2File>()) else { continue };
                    let mut p = PWSTR::null();
                    if file.Path(&mut p).is_ok() {
                        paths.push(take_pwstr(p));
                    }
                }
            }
            let _ = app.emit("drop://files", paths);
            Ok(())
        }));
        let mut token = 0i64;
        let _ = core.add_WebMessageReceived(&handler, &mut token);
    });
}
