// Implémentation Windows du service `desktop` : `IDesktopWallpaper` (fond statique) et
// calque `WorkerW` (fenêtre derrière les icônes du bureau).

use windows::core::{w, BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION, DWPOS_CENTER, DWPOS_FILL, DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH,
    DWPOS_TILE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetAncestor, GetClassNameW, GetCursorPos, SendMessageTimeoutW, WindowFromPoint, GA_ROOT,
    SMTO_NORMAL,
};

use super::Wallpaper;
use crate::services::hardware::{displays, DisplayInfo};

// --- Fond statique (IDesktopWallpaper) ------------------------------------------------------

/// COM initialisé le temps d'un appel (threads bloquants de Tauri).
struct Com;
impl Com {
    fn init() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        Com
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// (chemin de périphérique `IDesktopWallpaper`, écran correspondant) — appariés par
/// rectangle, les deux API ne partageant pas d'identifiant.
fn monitors(dw: &IDesktopWallpaper) -> Vec<(String, DisplayInfo)> {
    let list = displays();
    let n = unsafe { dw.GetMonitorDevicePathCount() }.unwrap_or(0);
    (0..n)
        .filter_map(|i| {
            let path = take(unsafe { dw.GetMonitorDevicePathAt(i) }.ok()?);
            let r: RECT = unsafe { dw.GetMonitorRECT(&HSTRING::from(path.as_str())) }.ok()?;
            let d = list.iter().find(|d| d.x == r.left && d.y == r.top && d.width as i32 == r.right - r.left)?;
            Some((path, d.clone()))
        })
        .collect()
}

fn position(fit: &str) -> DESKTOP_WALLPAPER_POSITION {
    match fit {
        "fit" => DWPOS_FIT,
        "stretch" => DWPOS_STRETCH,
        "center" => DWPOS_CENTER,
        "tile" => DWPOS_TILE,
        "span" => DWPOS_SPAN,
        _ => DWPOS_FILL,
    }
}

fn instance() -> Result<IDesktopWallpaper, String> {
    unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }.map_err(|e| e.to_string())
}

pub fn wallpapers() -> Result<Vec<Wallpaper>, String> {
    let _com = Com::init();
    let dw = instance()?;
    Ok(monitors(&dw)
        .into_iter()
        .map(|(path, d)| Wallpaper { display: d.id, path: take(unsafe { dw.GetWallpaper(&HSTRING::from(path.as_str())) }.unwrap_or(PWSTR::null())) })
        .collect())
}

pub fn set_wallpaper(image: &str, display: Option<&str>, fit: Option<&str>) -> Result<(), String> {
    let _com = Com::init();
    let dw = instance()?;
    if let Some(f) = fit {
        unsafe { dw.SetPosition(position(f)) }.map_err(|e| e.to_string())?;
    }
    let img = HSTRING::from(image);
    match display {
        None => unsafe { dw.SetWallpaper(PCWSTR::null(), &img) }.map_err(|e| e.to_string()),
        Some(id) => {
            let (path, _) = monitors(&dw).into_iter().find(|(_, d)| d.id == id).ok_or("invalidDisplay")?;
            unsafe { dw.SetWallpaper(&HSTRING::from(path.as_str()), &img) }.map_err(|e| e.to_string())
        }
    }
}

// --- Calque du fond (WorkerW) ---------------------------------------------------------------

fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Fenêtre du calque de fond, derrière les icônes : on y attache les fonds animés.
///
/// Le message non documenté `0x052C` demande à `Progman` de créer ce calque (méthode de
/// Wallpaper Engine / Lively). Deux organisations selon la version de Windows :
///   - jusqu'à 23H2 : un `WorkerW` de premier niveau, juste après celui qui porte les icônes
///     (`SHELLDLL_DefView`) ;
///   - 24H2 et + : un `WorkerW` ENFANT de `Progman`, sous les icônes.
pub fn wallpaper_layer() -> Option<isize> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), PCWSTR::null()).ok()?;
        let mut res = 0usize;
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(0x1), SMTO_NORMAL, 1000, Some(&mut res));
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, Some(&mut res));

        unsafe extern "system" fn each(top: HWND, data: LPARAM) -> BOOL {
            if FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok() {
                if let Ok(worker) = FindWindowExW(None, Some(top), w!("WorkerW"), PCWSTR::null()) {
                    *(data.0 as *mut isize) = worker.0 as isize;
                    return false.into();
                }
            }
            true.into()
        }
        let mut found = 0isize;
        let _ = EnumWindows(Some(each), LPARAM(&mut found as *mut _ as isize));
        if found != 0 {
            return Some(found);
        }
        FindWindowExW(Some(progman), None, w!("WorkerW"), PCWSTR::null()).ok().map(|h| h.0 as isize)
    }
}

/// Le curseur est-il sur le bureau visible (icônes ou fond), et non sur une fenêtre ?
pub fn cursor_on_desktop() -> Option<POINT> {
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok()?;
        let h = WindowFromPoint(p);
        if h.0.is_null() {
            return None;
        }
        let root = GetAncestor(h, GA_ROOT);
        matches!(class_of(root).as_str(), "Progman" | "WorkerW").then_some(p)
    }
}

/// Position du curseur (écran virtuel, px physiques).
pub fn cursor() -> Option<POINT> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok().map(|_| p)
}

/// Redessine le fond statique après le retrait d'un fond animé (sinon le calque garde la
/// dernière image du fond animé jusqu'au prochain rafraîchissement du bureau).
pub fn repaint_wallpaper() {
    if let Ok(list) = wallpapers() {
        for w in list.iter().filter(|w| !w.path.is_empty()) {
            let _ = set_wallpaper(&w.path, Some(&w.display), None);
        }
    }
    if let Some(layer) = wallpaper_layer() {
        unsafe {
            let _ = RedrawWindow(Some(HWND(layer as *mut _)), None, None, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW);
        }
    }
}
