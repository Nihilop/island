// Infrastructure de l'hôte : fenêtres (réglages), shell (révéler/ouvrir URL),
// zone de notification (tray), et détection d'app en plein écran (auto-hide).
// Tauri = cross-platform ; la détection plein écran est Windows (gated, no-op ailleurs).

use tauri::{AppHandle, Manager};

/// GARDE-FOU MÉMOIRE : Island surveille sa PROPRE conso (process principal + process
/// enfants du webview) et se récupère si ça dérape — pour ne JAMAIS dégrader la machine
/// du client, même si une extension fuit. Toutes les 20 s : somme la mémoire de l'arbre
/// de process ; au seuil HAUT, émet `island://reclaim` → le front recharge l'overlay AU
/// REPOS (récupère la mémoire JS accumulée ; le storage persiste). Cross-OS (sysinfo).
pub(crate) fn start_memory_watchdog(app: AppHandle) {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    use tauri::Emitter;

    // En build DEBUG (pnpm tauri dev), le webview embarque Vite/HMR/source-maps → le
    // footprint normal est 2-3× la prod → on monte les seuils pour ne pas crier au loup.
    let (warn_mb, hard_mb): (u64, u64) = if cfg!(debug_assertions) { (1400, 2400) } else { (500, 900) };
    let me = std::process::id();

    std::thread::spawn(move || {
        let mut sys = System::new();
        let mut warned = false;
        let mut last_reclaim_mb: u64 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(20));
            sys.refresh_processes(ProcessesToUpdate::All, true);

            // Ensemble transitif : notre process + tous ses descendants (webview & co).
            let mut tree = std::collections::HashSet::new();
            tree.insert(Pid::from_u32(me));
            loop {
                let before = tree.len();
                for (pid, p) in sys.processes() {
                    if let Some(parent) = p.parent() {
                        if tree.contains(&parent) {
                            tree.insert(*pid);
                        }
                    }
                }
                if tree.len() == before {
                    break;
                }
            }
            let bytes: u64 = tree.iter().filter_map(|pid| sys.process(*pid)).map(|p| p.memory()).sum();
            let mb = bytes / (1024 * 1024);

            if mb >= hard_mb && mb > last_reclaim_mb + 100 {
                last_reclaim_mb = mb;
                eprintln!("[island/watchdog] mémoire {mb} Mo ≥ {hard_mb} → récupération (reload overlay au repos)");
                let _ = app.emit("island://reclaim", mb);
            } else if mb >= warn_mb && !warned {
                warned = true;
                eprintln!("[island/watchdog] ⚠ mémoire élevée : {mb} Mo (une extension fuit peut-être)");
            } else if mb < warn_mb {
                warned = false;
            }
        }
    });
}


/// Révèle un fichier dans l'explorateur (clic sur une notification de capture).
#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Ouvre une URL http(s) dans le navigateur par défaut (primitive générique).
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("open_url: URL http(s) uniquement".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

// --- Zone de notification (tray) ------------------------------------------------------
// Clic gauche = ouvrir le notch ; menu = Ouvrir, Réglages…, Mode focus ☐, Démarrer avec
// Windows ☐, Quitter. Le front est la source de vérité du mode focus et des libellés
// (i18n) : il les renvoie via `tray_sync`.

pub(crate) struct TrayItems {
    open: tauri::menu::MenuItem<tauri::Wry>,
    settings: tauri::menu::MenuItem<tauri::Wry>,
    focus: tauri::menu::CheckMenuItem<tauri::Wry>,
    autostart: tauri::menu::CheckMenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
}

/// Force le menu contextuel en thème sombre (uxtheme, exports par ordinal, Windows 10
/// 1903+). Échec silencieux → menu clair.
#[cfg(target_os = "windows")]
fn force_dark_menus() {
    use windows::core::{w, PCSTR};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    unsafe {
        let Ok(lib) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        if let Some(set_mode) = GetProcAddress(lib, PCSTR(135 as *const u8)) {
            let f: extern "system" fn(i32) -> i32 = std::mem::transmute(set_mode);
            f(2); // ForceDark
        }
        if let Some(flush) = GetProcAddress(lib, PCSTR(136 as *const u8)) {
            let f: extern "system" fn() = std::mem::transmute(flush);
            f();
        }
    }
}

pub(crate) fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Emitter;
    use tauri_plugin_autostart::ManagerExt;

    #[cfg(target_os = "windows")]
    force_dark_menus();

    let auto_on = app.autolaunch().is_enabled().unwrap_or(false);
    let items = TrayItems {
        open: MenuItem::with_id(app, "tray-open", "Ouvrir Island\tCtrl+Alt+Espace", true, None::<&str>)?,
        settings: MenuItem::with_id(app, "tray-settings", "Réglages…", true, None::<&str>)?,
        focus: CheckMenuItem::with_id(app, "tray-focus", "Mode focus", true, false, None::<&str>)?,
        autostart: CheckMenuItem::with_id(app, "tray-autostart", "Démarrer avec Windows", true, auto_on, None::<&str>)?,
        quit: MenuItem::with_id(app, "tray-quit", "Quitter", true, None::<&str>)?,
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.open,
            &items.settings,
            &PredefinedMenuItem::separator(app)?,
            &items.focus,
            &items.autostart,
            &PredefinedMenuItem::separator(app)?,
            &items.quit,
        ],
    )?;
    app.manage(items);

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("icône par défaut").clone())
        .tooltip("Island")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-open" => {
                let _ = app.emit("notch://open", None::<String>);
            }
            "tray-settings" => {
                let _ = app.emit("notch://open", Some("settings"));
            }
            "tray-focus" => {
                let _ = app.emit("notch://focus-toggle", ());
            }
            "tray-autostart" => {
                let al = app.autolaunch();
                let on = al.is_enabled().unwrap_or(false);
                let _ = if on { al.disable() } else { al.enable() };
                if let Some(items) = app.try_state::<TrayItems>() {
                    let _ = items.autostart.set_checked(!on);
                }
                let _ = app.emit("autostart://changed", !on);
            }
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = tray.app_handle().emit("notch://open", None::<String>);
            }
        })
        .build(app)?;

    Ok(())
}

#[derive(serde::Deserialize)]
pub struct TrayLabels {
    open: String,
    settings: String,
    focus: String,
    autostart: String,
    quit: String,
}

/// Le front renvoie les libellés traduits et l'état réel des cases à cocher.
#[tauri::command]
pub fn tray_sync(app: AppHandle, labels: Option<TrayLabels>, focus_mode: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let Some(items) = app.try_state::<TrayItems>() else { return Ok(()) };
    if let Some(l) = labels {
        let _ = items.open.set_text(l.open);
        let _ = items.settings.set_text(l.settings);
        let _ = items.focus.set_text(l.focus);
        let _ = items.autostart.set_text(l.autostart);
        let _ = items.quit.set_text(l.quit);
    }
    let _ = items.focus.set_checked(focus_mode);
    let _ = items.autostart.set_checked(app.autolaunch().is_enabled().unwrap_or(false));
    Ok(())
}

// --- Raccourci global d'ouverture du notch (réglable ; défaut Ctrl + Alt + Espace) ----

static OPEN_SHORTCUT: std::sync::Mutex<Option<tauri_plugin_global_shortcut::Shortcut>> =
    std::sync::Mutex::new(None);

/// Vrai si `shortcut` est le raccourci d'ouverture courant.
pub(crate) fn is_open_shortcut(shortcut: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    OPEN_SHORTCUT.lock().unwrap_or_else(|p| p.into_inner()).as_ref() == Some(shortcut)
}

/// Remplace le raccourci d'ouverture (`None` = aucun). Renvoie `false` si la combinaison
/// est invalide ou déjà possédée par un autre programme ; l'app tourne alors sans.
#[tauri::command]
pub fn notch_set_shortcut(app: AppHandle, accel: Option<String>) -> bool {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let gs = app.global_shortcut();
    let mut cur = OPEN_SHORTCUT.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(old) = cur.take() {
        let _ = gs.unregister(old);
    }
    let Some(accel) = accel.filter(|a| !a.is_empty()) else { return true };
    let Ok(sc) = Shortcut::from_str(&accel) else { return false };
    if gs.register(sc).is_err() {
        return false;
    }
    *cur = Some(sc);
    true
}

// --- Service foreground : détection d'une app en plein écran ----------------
// (Sert l'auto-hide de l'île ; pourra plus tard exposer l'énumération des
// fenêtres à l'API capture pour la capture de fenêtre.)

/// Vrai si la fenêtre au premier plan couvre tout son moniteur (jeu/vidéo
/// plein écran, borderless inclus), hors bureau/shell et hors notre overlay.
#[cfg(target_os = "windows")]
unsafe fn foreground_is_fullscreen(overlay_hwnd: isize) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };

    // Signal OFFICIEL de Windows — celui que le système utilise lui-même pour décider de
    // masquer les notifications. Un jeu / une app PLEIN ÉCRAN (exclusif OU borderless) met
    // l'état à BUSY / D3D_FULL_SCREEN / PRESENTATION ; un bureau normal — Y COMPRIS une app
    // MAXIMISÉE (Spotify) barre des tâches auto-masquée — reste à ACCEPTS_NOTIFICATIONS.
    // C'est ce qui distingue de façon fiable « jeu borderless » de « fenêtre maximisée »,
    // là où la seule géométrie (couvre le moniteur) donnait un faux positif sur Spotify et,
    // après le filtre WS_MAXIMIZE, un faux négatif sur les jeux borderless (ex. Guild Wars 2).
    let state = match SHQueryUserNotificationState() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if !(state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE) {
        return false;
    }

    // L'état système confirme un plein écran ; on s'assure que c'est bien la fenêtre de
    // premier plan (pas notre overlay, pas le shell) et qu'elle couvre RÉELLEMENT son
    // moniteur (sinon : plein écran sur un AUTRE moniteur pendant qu'on bosse → on reste).
    let hwnd = GetForegroundWindow();
    if hwnd.0.is_null() || hwnd.0 as isize == overlay_hwnd {
        return false;
    }
    // Exclut TOUTE fenêtre de NOTRE processus (overlay compris) : quand la view d'une
    // extension s'ouvre, l'overlay passe plein écran et devient premier plan → sans ça il
    // était pris pour un « jeu plein écran » et le raccourci (ex. touche Win de Flow) était
    // coupé/rétabli en boucle. Le test hwnd seul ne suffit pas (hwnd de premier plan ≠ hwnd
    // capturé au démarrage selon le focus).
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() {
        return false;
    }
    let mut cls = [0u16; 256];
    let n = GetClassNameW(hwnd, &mut cls);
    let class = String::from_utf16_lossy(&cls[..n as usize]);
    if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
        return false;
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return false;
    }
    let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
        return false;
    }
    let m = mi.rcMonitor;
    // La fenêtre couvre tout le moniteur (≥ pour gérer un léger overscan).
    rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
}

/// Poll du premier plan ; émet `fullscreen://changed { active }` au changement.
/// No-op hors Windows.
pub(crate) fn start_fullscreen_monitor(app: AppHandle) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Emitter;
        let overlay_hwnd: isize = app
            .get_webview_window("overlay")
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize)
            .unwrap_or(0);

        std::thread::spawn(move || {
            use std::sync::atomic::Ordering;
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            let mut last = false;
            let mut hidden = false;
            let mut last_fg: isize = 0;
            loop {
                let fs = unsafe { foreground_is_fullscreen(overlay_hwnd) };
                if fs != last {
                    last = fs;
                    let _ = app.emit("fullscreen://changed", serde_json::json!({ "active": fs }));
                }
                let win = app.get_webview_window("overlay");
                // Visibilité « masquer en plein écran ».
                let hide = fs && crate::overlay::HIDE_IN_FULLSCREEN.load(Ordering::SeqCst);
                if let Some(w) = &win {
                    if hide != hidden {
                        hidden = hide;
                        if hide {
                            let _ = w.hide();
                        } else {
                            let _ = w.show();
                            let _ = w.set_always_on_top(true);
                            crate::overlay::refresh();
                        }
                    }
                }
                // Une autre app topmost a pu passer devant : on réaffirme à chaque
                // changement de premier plan.
                let fg = unsafe { GetForegroundWindow() }.0 as isize;
                if fg != last_fg {
                    last_fg = fg;
                    if let (Some(w), false) = (&win, hidden) {
                        let _ = w.set_always_on_top(true);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}
