// Hook clavier bas niveau (WH_KEYBOARD_LL) pour la touche Windows seule.
//
// Méthode reprise de **ChefKeys** (la lib que Flow Launcher utilise nativement, prouvée
// sur Windows 11) — cf. https://github.com/jjw24/ChefKeys :
//   • on LAISSE PASSER le keydown de Win → les combos (Win+Maj+S, Win+D, Win+L…) restent
//     100 % natifs, aucun rejeu/réordonnancement ;
//   • si une AUTRE touche suit pendant que Win est tenu → c'est un combo, on ne fait rien ;
//   • au keyup, si Win était SEUL → on BLOQUE le vrai keyup (return 1) et on injecte la
//     séquence «Alt↓, Win↑(synthétique), Alt↑» : le Alt intercalé fait croire à Windows que
//     Win a servi en combinaison → PAS de menu Démarrer ; le Win↑ synthétique relâche
//     proprement le Win qu'on avait laissé passer (pas de touche coincée). Puis action Island.
//
// Robustesse : callback ultra-court (app.emit déporté sur un thread émetteur), thread hook
// en `THREAD_PRIORITY_TIME_CRITICAL`, anti-flood sur l'émetteur.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_LMENU, VK_LWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

// Signature de NOS injections → le hook les ignore (pas de ré-entrance ni de faux combo).
const SIG: usize = 0x1515_0001;
const VK_LWIN_U: u32 = 0x5B;
const VK_RWIN_U: u32 = 0x5C;

// Actif seulement si une extension a demandé la touche Win (sinon le hook laisse tout
// passer immédiatement → aucun impact sur la frappe système).
static WIN_ENABLED: AtomicBool = AtomicBool::new(false);
// État de la machine (muté uniquement depuis le thread du hook).
static WIN_DOWN: AtomicBool = AtomicBool::new(false); // Win physiquement tenu
static WIN_COMBO: AtomicBool = AtomicBool::new(false); // une autre touche a suivi → combo

static APP: OnceLock<AppHandle> = OnceLock::new();
static HOOK_STARTED: AtomicBool = AtomicBool::new(false);
// Canal vers le thread émetteur : le callback y pousse (send non bloquant), le thread
// relaie vers l'événement Tauri. Indispensable → le callback reste ultra-court.
static EMIT_TX: OnceLock<std::sync::mpsc::Sender<()>> = OnceLock::new();

pub fn set_key(app: &AppHandle, key: &str, enabled: bool) -> Result<(), String> {
    if !key.eq_ignore_ascii_case("super") && !key.eq_ignore_ascii_case("win") {
        return Err(format!("touche réservée inconnue : {key}"));
    }
    let _ = APP.set(app.clone());
    WIN_ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        WIN_DOWN.store(false, Ordering::SeqCst);
        WIN_COMBO.store(false, Ordering::SeqCst);
    }
    ensure_hook_thread();
    Ok(())
}

// Le hook n'est installé qu'À LA PREMIÈRE activation, sur son propre thread avec une
// message-pump (indispensable pour qu'un hook LL reçoive les événements).
fn ensure_hook_thread() {
    if HOOK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // Thread ÉMETTEUR : fait le `app.emit` HORS du callback + anti-flood (une action, puis
    // ~150 ms où on absorbe les rebonds → un spam de Win ne déclenche pas une rafale de
    // toggles qui saturerait le thread principal et affamerait le hook).
    if let Some(app) = APP.get().cloned() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let _ = EMIT_TX.set(tx);
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                let _ = app.emit("reserved://key", serde_json::json!({ "key": "Super" }));
                std::thread::sleep(Duration::from_millis(150));
                while rx.try_recv().is_ok() {}
            }
        });
    }
    std::thread::spawn(|| unsafe {
        if SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0).is_err() {
            HOOK_STARTED.store(false, Ordering::SeqCst);
            return;
        }
        // Priorité maximale : le callback doit répondre avant le timeout LL (~300 ms) sinon
        // Windows ignore le blocage → menu Démarrer. Le callback reste minuscule.
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        let mut msg = MSG::default();
        // Boucle bloquante : garde le hook vivant tant que l'app tourne.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
    });
}

fn kbd_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIG,
            },
        },
    }
}

// Neutralise le menu Démarrer pour une frappe Win sèche (cf. ChefKeys `BlockWindowsStartMenu`) :
// Alt↓ + Win↑ synthétique + Alt↑. Le Alt intercalé = « Win a servi en combo » → pas de menu ;
// le Win↑ relâche le Win laissé passer au keydown.
fn block_start_menu() {
    let inputs = [
        kbd_input(VK_LMENU, false),
        kbd_input(VK_LWIN, true),
        kbd_input(VK_LMENU, true),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn emit_super() {
    // Non bloquant : le relais réel (app.emit) est fait par le thread émetteur.
    if let Some(tx) = EMIT_TX.get() {
        let _ = tx.send(());
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || !WIN_ENABLED.load(Ordering::Relaxed) {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let kbd = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // Ignore nos propres injections (Alt/Win synthétiques) → ni boucle, ni faux combo.
    if kbd.dwExtraInfo == SIG {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let vk = kbd.vkCode;
    let m = wparam.0 as u32;
    let is_down = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
    let is_up = m == WM_KEYUP || m == WM_SYSKEYUP;
    let is_win = vk == VK_LWIN_U || vk == VK_RWIN_U;

    if is_win {
        if is_down {
            // Fresh press → on réinitialise le combo (les auto-repeats ne le touchent pas).
            if !WIN_DOWN.swap(true, Ordering::Relaxed) {
                WIN_COMBO.store(false, Ordering::Relaxed);
            }
            // On LAISSE PASSER Win↓ → combos 100 % natifs.
            return CallNextHookEx(None, code, wparam, lparam);
        }
        if is_up {
            let lone = WIN_DOWN.swap(false, Ordering::Relaxed) && !WIN_COMBO.load(Ordering::Relaxed);
            if lone {
                // Win SEUL → neutralise le menu Démarrer, déclenche l'action, et BLOQUE le vrai Win↑.
                block_start_menu();
                emit_super();
                return LRESULT(1);
            }
            WIN_COMBO.store(false, Ordering::Relaxed);
            return CallNextHookEx(None, code, wparam, lparam); // combo → laisse passer Win↑
        }
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // Autre touche pendant que Win est tenu → combo → annule l'action « Win seul ».
    if is_down && WIN_DOWN.load(Ordering::Relaxed) {
        WIN_COMBO.store(true, Ordering::Relaxed);
    }
    CallNextHookEx(None, code, wparam, lparam)
}
