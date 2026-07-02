// Infrastructure de l'hôte : fenêtres (réglages), shell (révéler/ouvrir URL),
// zone de notification (tray), et détection d'app en plein écran (auto-hide).
// Tauri = cross-platform ; la détection plein écran est Windows (gated, no-op ailleurs).

use tauri::{AppHandle, Manager};

/// Ouvre (et met au premier plan) la fenêtre de réglages.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// Icône de la zone de notification (system tray) : clic gauche → Réglages,
/// menu contextuel → Réglages / Quitter. Seul moyen de fermer Island proprement
/// (les fenêtres se cachent, jamais ne se ferment).
pub(crate) fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let settings_i = MenuItem::with_id(app, "tray-settings", "Réglages…", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "tray-quit", "Quitter Island", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_i, &PredefinedMenuItem::separator(app)?, &quit_i])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("icône par défaut").clone())
        .tooltip("Island")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-settings" => {
                let _ = open_settings(app.clone());
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
                let _ = open_settings(tray.app_handle().clone());
            }
        })
        .build(app)?;

    Ok(())
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
            let mut last = false;
            loop {
                let fs = unsafe { foreground_is_fullscreen(overlay_hwnd) };
                if fs != last {
                    last = fs;
                    let _ = app.emit("fullscreen://changed", serde_json::json!({ "active": fs }));
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}
