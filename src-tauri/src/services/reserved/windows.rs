// Hook clavier bas niveau (WH_KEYBOARD_LL) pour la touche Windows seule.
//
// 1. Le shell ne voit JAMAIS une frappe Win seule :
//   • Win↓ réel (appui frais, sans Ctrl/Maj/Alt déjà tenus) → RETENU (pas transmis) ;
//   • Win↑ réel alors que Win est retenu → retenu aussi, action Island : pour Windows,
//     rien ne s'est passé → pas de menu Démarrer, aucune touche coincée ;
//   • autre touche ↓ pendant que Win est retenu → combo : on la retient et on injecte
//     «Win↓, cette touche↓» DANS L'ORDRE (Win+E, Win+Maj+S… marchent), puis tout passe
//     normalement jusqu'au Win↑ réel, transmis tel quel (il relâche le Win injecté) ;
//   • Win+L : le verrouillage ne s'injecte pas → `LockWorkStation` ;
//   • Ctrl/Maj/Alt déjà tenus à l'appui de Win → tout passe nativement.
//
// 2. Mode launcher : après un Win seul, le launcher s'ouvre SANS que la fenêtre d'Island
//    devienne active, et ce hook CAPTE la frappe pour lui (événements `launcher://key`).
//    Constaté sur Windows 11 (tests réels) : si une fenêtre devient active par programme
//    juste après une frappe Win seule — même retenue par le hook —, le shell ouvre sa
//    recherche ~0,4 s plus tard (jamais sans activation, jamais avec Ctrl+Espace, jamais
//    après un clic de l'utilisateur). Ni masquage par touche injectée (Alt, vkE8), ni
//    raccourci relais, ni réinstallation du hook n'y changent rien.
//    Sorties de capture garanties côté Rust (le clavier ne peut pas rester bloqué) :
//    Échap, nouvelle frappe Win, Alt+Tab, combinaison Win ; un clic dans le launcher (le
//    front coupe la capture et prend le focus normalement — un clic ne déclenche rien).
//
// Limite : un process non élevé ne peut pas injecter vers une fenêtre administrateur au
// premier plan ; dans ce cas une combinaison Win+touche peut ne pas arriver.
//
// Robustesse : callback ultra-court (app.emit déporté sur un thread émetteur), thread hook
// en `THREAD_PRIORITY_TIME_CRITICAL`.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, MapVirtualKeyW, SendInput, ToUnicodeEx, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, SetWindowsHookExW, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

// Signature de NOS injections → le hook les ignore (pas de ré-entrance ni de faux combo).
const SIG: usize = 0x1515_0001;
const VK_LWIN_U: u32 = 0x5B;
const VK_RWIN_U: u32 = 0x5C;

// Actif seulement si quelqu'un a demandé la touche Win (sinon le hook laisse tout passer
// immédiatement → aucun impact sur la frappe système).
static WIN_ENABLED: AtomicBool = AtomicBool::new(false);
/// État de la touche Win : IDLE, HELD (retenue, le système ne sait rien), COMBO (Win↓
/// injecté, combinaison en cours), NATIVE (tout passe tel quel jusqu'au Win↑), SWALLOW
/// (Win+L traité : le Win↑ réel est absorbé).
static WIN_STATE: AtomicU8 = AtomicU8::new(IDLE);
/// Touche Win retenue (gauche ou droite).
static WIN_VK: AtomicU32 = AtomicU32::new(0);
const IDLE: u8 = 0;
const HELD: u8 = 1;
const COMBO: u8 = 2;
const NATIVE: u8 = 3;
const SWALLOW: u8 = 4;

/// Mode launcher (cf. en-tête §2) ; sinon un Win seul émet `reserved://key` (extensions).
static WIN_LAUNCHER: AtomicBool = AtomicBool::new(false);
/// Le launcher est ouvert (tenu à jour par le front) : un Win seul le referme.
static LAUNCHER_OPEN: AtomicBool = AtomicBool::new(false);
/// La frappe est captée pour le launcher.
static CAPTURE: AtomicBool = AtomicBool::new(false);
/// Modificateurs pendant la capture (ils sont retenus, donc invisibles pour le système).
static MODS: AtomicU8 = AtomicU8::new(0);
const M_SHIFT: u8 = 1;
const M_CTRL: u8 = 2;
const M_ALT: u8 = 4;
const M_ALTGR: u8 = 8;
const M_CAPS: u8 = 16;
/// Dernier Win seul traité (ms) : anti-rebond.
static LAST_WIN: AtomicU64 = AtomicU64::new(0);

static APP: OnceLock<AppHandle> = OnceLock::new();
static HOOK_STARTED: AtomicBool = AtomicBool::new(false);

/// Ce que le callback confie au thread émetteur (il ne fait jamais d'`emit` lui-même).
enum Ev {
    /// Win seul, mode extensions.
    Super,
    /// Win seul, mode launcher : ouvrir (capture démarrée) ou fermer.
    LauncherWin(bool),
    /// Frappe captée pour le launcher.
    Key(KeyEv),
}

#[derive(Clone, serde::Serialize)]
pub struct KeyEv {
    vk: u32,
    /// Caractère(s) produit(s) selon la disposition du clavier (vide pour les touches de
    /// contrôle et les raccourcis Ctrl / Alt).
    text: String,
    ctrl: bool,
    shift: bool,
    alt: bool,
}

static TX: OnceLock<std::sync::mpsc::Sender<Ev>> = OnceLock::new();

fn send(ev: Ev) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(ev);
    }
}

pub fn set_key(app: &AppHandle, key: &str, enabled: bool) -> Result<(), String> {
    if !key.eq_ignore_ascii_case("super") && !key.eq_ignore_ascii_case("win") {
        return Err(format!("touche réservée inconnue : {key}"));
    }
    let _ = APP.set(app.clone());
    WIN_ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        WIN_STATE.store(IDLE, Ordering::SeqCst);
        CAPTURE.store(false, Ordering::SeqCst);
    }
    ensure_hook_thread();
    Ok(())
}

/// Mode launcher de la touche Windows (cf. en-tête §2).
pub fn set_win_launcher(on: bool) {
    WIN_LAUNCHER.store(on, Ordering::SeqCst);
    if !on {
        CAPTURE.store(false, Ordering::SeqCst);
    }
}

/// État du launcher vu du front : ouvert ou non, frappe captée ou non.
pub fn set_launcher_state(open: bool, capture: bool) {
    LAUNCHER_OPEN.store(open, Ordering::SeqCst);
    CAPTURE.store(open && capture, Ordering::SeqCst);
}

// Le hook n'est installé qu'À LA PREMIÈRE activation, sur son propre thread avec une
// message-pump (indispensable pour qu'un hook LL reçoive les événements).
fn ensure_hook_thread() {
    if HOOK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(app) = APP.get().cloned() {
        let (tx, rx) = std::sync::mpsc::channel::<Ev>();
        let _ = TX.set(tx);
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                let _ = match ev {
                    Ev::Super => app.emit("reserved://key", serde_json::json!({ "key": "Super" })),
                    Ev::LauncherWin(open) => app.emit("launcher://win", open),
                    Ev::Key(k) => app.emit("launcher://key", k),
                };
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

/// Frappe clavier injectée (signée SIG : le hook la laisse passer).
fn key_input(vk: u32, scan: u32, extended: bool, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk as u16),
                wScan: scan as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIG,
            },
        },
    }
}

fn inject(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Combo : rejoue «Win↓, touche↓» dans l'ordre (la touche réelle a été retenue).
fn replay_combo(win_vk: u32, kbd: &KBDLLHOOKSTRUCT) {
    let win_scan = unsafe { MapVirtualKeyW(win_vk, MAPVK_VK_TO_VSC) };
    inject(&[
        key_input(win_vk, win_scan, true, false),
        key_input(kbd.vkCode, kbd.scanCode, kbd.flags.0 & LLKHF_EXTENDED.0 != 0, false),
    ]);
}

fn modifier_held() -> bool {
    unsafe { [VK_SHIFT, VK_CONTROL, VK_MENU].iter().any(|k| GetAsyncKeyState(k.0 as i32) as u16 & 0x8000 != 0) }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Win seul relâché.
fn on_lone_win() {
    // Anti-rebond : un spam de Win ne déclenche pas une rafale d'ouvertures/fermetures.
    let now = now_ms();
    if now.saturating_sub(LAST_WIN.swap(now, Ordering::Relaxed)) < 150 {
        return;
    }
    if !WIN_LAUNCHER.load(Ordering::Relaxed) {
        return send(Ev::Super);
    }
    let open = !LAUNCHER_OPEN.load(Ordering::Relaxed);
    LAUNCHER_OPEN.store(open, Ordering::Relaxed);
    if open {
        // Capture démarrée tout de suite : une frappe rapide après Win n'est pas perdue.
        let caps = unsafe { GetKeyState(0x14) } & 1 != 0;
        MODS.store(if caps { M_CAPS } else { 0 }, Ordering::Relaxed);
    }
    CAPTURE.store(open, Ordering::Relaxed);
    send(Ev::LauncherWin(open));
}

/// Texte produit par une touche, selon la disposition du clavier de la fenêtre active et
/// les modificateurs captés (accents, AltGr, Maj, verr. maj, touches mortes).
fn key_text(kbd: &KBDLLHOOKSTRUCT, mods: u8) -> String {
    let mut state = [0u8; 256];
    if mods & M_SHIFT != 0 {
        state[0x10] = 0x80;
    }
    if mods & (M_CTRL | M_ALTGR) != 0 {
        state[0x11] = 0x80;
    }
    if mods & (M_ALT | M_ALTGR) != 0 {
        state[0x12] = 0x80;
    }
    if mods & M_CAPS != 0 {
        state[0x14] = 0x01;
    }
    let mut buf = [0u16; 8];
    let n = unsafe {
        let hkl = GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), None));
        ToUnicodeEx(kbd.vkCode, kbd.scanCode, &state, &mut buf, 0, Some(hkl))
    };
    if n <= 0 {
        return String::new(); // touche morte (combinée à la suivante) ou rien
    }
    String::from_utf16_lossy(&buf[..n as usize]).chars().filter(|c| !c.is_control()).collect()
}

/// Frappe pendant la capture : tout est retenu, sauf verr. maj ; Échap et Alt+Tab coupent
/// la capture ici même (le clavier ne peut pas rester bloqué si le front ne répond pas).
fn on_capture(vk: u32, is_down: bool, kbd: &KBDLLHOOKSTRUCT, code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let bit = match vk {
        0x10 | 0xA0 | 0xA1 => M_SHIFT,
        0x11 | 0xA2 | 0xA3 => M_CTRL,
        0x12 | 0xA4 => M_ALT,
        0xA5 => M_ALTGR,
        _ => 0,
    };
    if bit != 0 {
        if is_down {
            MODS.fetch_or(bit, Ordering::Relaxed);
        } else {
            MODS.fetch_and(!bit, Ordering::Relaxed);
        }
        return LRESULT(1);
    }
    if vk == 0x14 {
        if is_down {
            MODS.fetch_xor(M_CAPS, Ordering::Relaxed);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    if !is_down {
        return LRESULT(1);
    }
    let mods = MODS.load(Ordering::Relaxed);
    let altgr = mods & M_ALTGR != 0;
    let ctrl = mods & M_CTRL != 0 && !altgr;
    let alt = mods & M_ALT != 0;
    // Alt+Tab : on rend la main au système (Alt avait été retenu → on le rejoue).
    if vk == 0x09 && alt {
        CAPTURE.store(false, Ordering::Relaxed);
        LAUNCHER_OPEN.store(false, Ordering::Relaxed);
        send(Ev::LauncherWin(false));
        inject(&[key_input(0xA4, 0, false, false), key_input(0x09, kbd.scanCode, false, false)]);
        return LRESULT(1);
    }
    if vk == 0x1B {
        CAPTURE.store(false, Ordering::Relaxed);
        LAUNCHER_OPEN.store(false, Ordering::Relaxed);
    }
    let text = if ctrl || alt { String::new() } else { key_text(kbd, mods) };
    send(Ev::Key(KeyEv { vk, text, ctrl, shift: mods & M_SHIFT != 0, alt }));
    LRESULT(1)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || !WIN_ENABLED.load(Ordering::Relaxed) {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let kbd = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // Ignore nos propres injections → ni boucle, ni faux combo.
    if kbd.dwExtraInfo == SIG {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let vk = kbd.vkCode;
    let m = wparam.0 as u32;
    let is_down = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
    let is_up = m == WM_KEYUP || m == WM_SYSKEYUP;
    let is_win = vk == VK_LWIN_U || vk == VK_RWIN_U;
    let state = WIN_STATE.load(Ordering::Relaxed);

    if is_win {
        if is_down {
            match state {
                IDLE if modifier_held() && !CAPTURE.load(Ordering::Relaxed) => WIN_STATE.store(NATIVE, Ordering::Relaxed),
                IDLE => {
                    WIN_STATE.store(HELD, Ordering::Relaxed);
                    WIN_VK.store(vk, Ordering::Relaxed);
                    return LRESULT(1); // retenu : le système ne voit rien
                }
                HELD | SWALLOW => return LRESULT(1), // répétition automatique
                _ => {}
            }
            return CallNextHookEx(None, code, wparam, lparam);
        }
        if is_up {
            WIN_STATE.store(IDLE, Ordering::Relaxed);
            if state == HELD {
                // Win seul : jamais vu par le système → pas de menu Démarrer.
                on_lone_win();
                return LRESULT(1);
            }
            if state == SWALLOW {
                return LRESULT(1);
            }
            // COMBO (relâche le Win injecté) ou NATIVE : passe tel quel.
            return CallNextHookEx(None, code, wparam, lparam);
        }
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // Autre touche pendant que Win est retenu → combo : rejouée derrière un Win↓ injecté.
    if is_down && state == HELD {
        // Win+L : le verrouillage ne s'injecte pas (séquence sécurisée) → appel direct.
        if vk == 0x4C && !modifier_held() {
            WIN_STATE.store(SWALLOW, Ordering::Relaxed);
            let _ = unsafe { windows::Win32::System::Shutdown::LockWorkStation() };
            return LRESULT(1);
        }
        // Une combinaison Win ferme le launcher capté (la fenêtre ciblée va changer).
        if CAPTURE.swap(false, Ordering::Relaxed) {
            LAUNCHER_OPEN.store(false, Ordering::Relaxed);
            send(Ev::LauncherWin(false));
        }
        WIN_STATE.store(COMBO, Ordering::Relaxed);
        replay_combo(WIN_VK.load(Ordering::Relaxed), kbd);
        return LRESULT(1);
    }

    if (is_down || is_up) && CAPTURE.load(Ordering::Relaxed) {
        return on_capture(vk, is_down, kbd, code, wparam, lparam);
    }
    CallNextHookEx(None, code, wparam, lparam)
}
