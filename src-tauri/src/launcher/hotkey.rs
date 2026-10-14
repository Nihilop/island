// Ouverture du launcher : raccourci global (Ctrl+Espace par défaut) OU touche Windows
// seule (service `reserved`, qui empêche le menu Démarrer). Jamais les deux : la touche
// Windows remplace le raccourci.
//
// Suspension : quand une app de la liste d'exclusion tourne (ex. UnrealEditor.exe, qui
// utilise Ctrl+Espace) ou qu'une app est en plein écran, le raccourci est réellement
// DÉSENREGISTRÉ (l'app reçoit la touche) et la touche Windows redevient native. Un thread
// vérifie toutes les 1,5 s (instantané des process : ~1 ms).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Accélérateur Tauri (« Control+Space »), vide = aucun.
    shortcut: String,
    /// Touche Windows seule (remplace le raccourci).
    win_key: bool,
    /// Noms d'exécutables qui suspendent le launcher (insensible à la casse).
    exclude: Vec<String>,
    /// Suspendu quand une app est en plein écran.
    fullscreen_off: bool,
}

static CONFIG: Mutex<Option<Config>> = Mutex::new(None);
/// Raccourci actuellement enregistré.
static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);
static SUSPENDED: AtomicBool = AtomicBool::new(false);
static MONITOR: AtomicBool = AtomicBool::new(false);

/// Vrai si `sc` est le raccourci du launcher (gestionnaire global de `lib.rs`).
pub fn is_launcher_shortcut(sc: &Shortcut) -> bool {
    REGISTERED.lock().unwrap_or_else(|p| p.into_inner()).as_ref() == Some(sc)
}

/// Applique la config + l'état de suspension. Renvoie false si le raccourci n'a pas pu
/// être enregistré (invalide ou déjà pris).
fn apply(app: &AppHandle) -> bool {
    let cfg = CONFIG.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or_default();
    let suspended = SUSPENDED.load(Ordering::Relaxed);
    let win = cfg.win_key && !suspended;
    #[cfg(target_os = "windows")]
    {
        // Touche Windows : le hook ouvre le launcher et capte la frappe (cf. en-tête de
        // `services::reserved::windows`) ; aucun raccourci n'est enregistré.
        crate::services::reserved::windows::set_win_launcher(win);
        let _ = crate::services::reserved::windows::set_key(app, "Super", win);
    }

    let want = (!cfg.win_key && !suspended && !cfg.shortcut.is_empty())
        .then(|| cfg.shortcut.parse::<Shortcut>().ok())
        .flatten();
    let gs = app.global_shortcut();
    let mut cur = REGISTERED.lock().unwrap_or_else(|p| p.into_inner());
    if *cur == want {
        return true;
    }
    if let Some(old) = cur.take() {
        let _ = gs.unregister(old);
    }
    match want {
        Some(sc) if gs.register(sc).is_ok() => {
            *cur = Some(sc);
            true
        }
        Some(_) => false,
        None => cfg.win_key || suspended || cfg.shortcut.is_empty(),
    }
}

/// Le launcher doit-il être suspendu maintenant ?
fn should_suspend(cfg: &Config) -> bool {
    (cfg.fullscreen_off && imp::fullscreen()) || (!cfg.exclude.is_empty() && imp::any_running(&cfg.exclude))
}

fn start_monitor(app: &AppHandle) {
    if MONITOR.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1500));
        let Some(cfg) = CONFIG.lock().unwrap_or_else(|p| p.into_inner()).clone() else { continue };
        let s = should_suspend(&cfg);
        if s != SUSPENDED.swap(s, Ordering::Relaxed) {
            apply(&app);
        }
    });
}

/// Réglages du launcher (poussés par le front). Renvoie `shortcutOk`.
#[tauri::command]
pub fn launcher_config(app: AppHandle, config: Config) -> bool {
    let mut cfg = config;
    cfg.exclude = cfg.exclude.iter().map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty()).collect();
    SUSPENDED.store(should_suspend(&cfg), Ordering::Relaxed);
    *CONFIG.lock().unwrap_or_else(|p| p.into_inner()) = Some(cfg);
    start_monitor(&app);
    apply(&app)
}

/// État du launcher (front → hook) : ouvert, et frappe captée par le hook (ouverture par
/// la touche Windows, tant que l'utilisateur n'a pas cliqué dedans).
#[tauri::command]
pub fn launcher_capture(open: bool, capture: bool) {
    #[cfg(target_os = "windows")]
    crate::services::reserved::windows::set_launcher_state(open, capture);
    #[cfg(not(target_os = "windows"))]
    let _ = (open, capture);
}

/// Texte du presse-papiers (Ctrl+V pendant la capture : la WebView n'a pas le focus).
#[tauri::command]
pub fn launcher_paste_text() -> Option<String> {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok()
}


#[cfg(target_os = "windows")]
mod imp {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};

    pub fn fullscreen() -> bool {
        matches!(unsafe { SHQueryUserNotificationState() }, Ok(s) if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE)
    }

    /// Un des exécutables (minuscules) tourne-t-il ?
    pub fn any_running(names: &[String]) -> bool {
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut found = false;
            let mut ok = Process32FirstW(snap, &mut e).is_ok();
            while ok {
                let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
                let exe = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
                if names.iter().any(|n| *n == exe) {
                    found = true;
                    break;
                }
                ok = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
            found
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn fullscreen() -> bool {
        false
    }
    pub fn any_running(_: &[String]) -> bool {
        false
    }
}
