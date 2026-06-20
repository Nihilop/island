// Overlay plein écran : click-through par régions (la fenêtre laisse passer les
// clics sauf sur les zones interactives), couverture du moniteur, exclusion de la
// capture (WDA), focus clavier. L'état des régions est cross-platform ; le poll du
// curseur et l'affinité de capture sont spécifiques Windows (gated).

use std::sync::Mutex;

use tauri::{AppHandle, Manager, WebviewWindow};
#[cfg(target_os = "windows")]
use tauri::Emitter;

/// Rectangle interactif (px logiques, relatif au viewport overlay).
#[derive(serde::Deserialize)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

static HIT_REGIONS: Mutex<Vec<Rect>> = Mutex::new(Vec::new());

/// Le front publie ses zones interactives courantes (île, modal…).
#[tauri::command]
pub fn set_hit_regions(regions: Vec<Rect>) {
    *HIT_REGIONS.lock().unwrap_or_else(|p| p.into_inner()) = regions;
}

/// Donne le focus clavier à l'overlay (pour qu'un champ de saisie reçoive la frappe).
#[tauri::command]
pub fn overlay_focus(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("overlay") {
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Couvre le moniteur principal AU DÉMARRAGE. La géométrie est ensuite pilotée côté
/// front (`Island.vue`) : une PETITE boîte haut-centre par défaut (l'overlay ne recouvre
/// alors plus les autres fenêtres → Windows ne les met pas en pause / "occlusion"), et
/// plein écran seulement quand une surface le réclame (modal, fenêtre flottante,
/// sélection de zone, contour d'enregistrement).
pub(crate) fn cover_monitor(win: &WebviewWindow) -> tauri::Result<()> {
    if let Some(m) = win.primary_monitor()? {
        win.set_position(*m.position())?;
        win.set_size(*m.size())?;
    }
    Ok(())
}

/// Exclut (ou non) la fenêtre overlay d'Island des captures d'écran, tout en la
/// gardant visible à l'œil. C'est le mécanisme « ignorer les éléments d'Island ».
#[cfg(target_os = "windows")]
pub(crate) fn set_overlay_excluded(app: &AppHandle, exclude: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };
    if let Some(w) = app.get_webview_window("overlay") {
        if let Ok(hwnd) = w.hwnd() {
            let affinity = if exclude { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
            unsafe {
                let _ = SetWindowDisplayAffinity(hwnd, affinity);
            }
        }
    }
}

/// Poll `GetCursorPos` à ~60 Hz : la fenêtre laisse passer les clics partout,
/// sauf quand le curseur est sur une région interactive. Émet `overlay://hover`
/// (repli auto) et `overlay://dismiss` (clic hors de l'île).
#[cfg(target_os = "windows")]
pub(crate) fn start_click_through(win: WebviewWindow) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let _ = win.set_ignore_cursor_events(true);
    let app = win.app_handle().clone();
    std::thread::spawn(move || {
        let mut last_inside = false;
        let mut last_lbtn = false;
        let mut scale = win.scale_factor().unwrap_or(1.0);
        let mut origin = win.outer_position().map(|p| (p.x, p.y)).unwrap_or((0, 0));
        let mut i: u32 = 0;
        loop {
            // L'origine est rafraîchie à CHAQUE frame : l'overlay est déplacé/redimensionné
            // (petite boîte ↔ plein écran selon la surface) → le mapping curseur doit suivre
            // immédiatement, sinon ~1 s de décalage = surfaces non cliquables (ex. modal).
            if let Ok(p) = win.outer_position() {
                origin = (p.x, p.y);
            }
            if i % 60 == 0 {
                scale = win.scale_factor().unwrap_or(scale);
            }
            i = i.wrapping_add(1);

            let mut pt = POINT::default();
            let inside = unsafe {
                if GetCursorPos(&mut pt).is_ok() {
                    let cx = (pt.x - origin.0) as f64 / scale;
                    let cy = (pt.y - origin.1) as f64 / scale;
                    HIT_REGIONS
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .iter()
                        .any(|r| cx >= r.x && cy >= r.y && cx <= r.x + r.w && cy <= r.y + r.h)
                } else {
                    false
                }
            };
            if inside != last_inside {
                let _ = win.set_ignore_cursor_events(!inside);
                let _ = app.emit("overlay://hover", inside);
                last_inside = inside;
            }

            let lbtn = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0;
            if lbtn && !last_lbtn && !inside {
                let _ = app.emit("overlay://dismiss", ());
            }
            last_lbtn = lbtn;

            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    });
}
