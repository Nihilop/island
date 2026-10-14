//! Lecture en cours (Windows GSMTC) — agnostique : couvre tout ce que Windows affiche dans
//! son flyout média (Spotify, navigateurs, Apple Music, VLC…).
//!
//! Poll toutes les 400 ms (plus robuste aux changements de session que les events WinRT).
//! Choix de la session affichée : une session EN LECTURE gagne (la « courante » de Windows,
//! puis l'app déjà affichée, puis n'importe laquelle) ; si rien ne joue, on GARDE l'app
//! affichée (une pause par touche média ne fait pas sauter vers une autre app). Les
//! commandes visent l'app affichée, pas la « courante ».
//!
//! Émission `np://update` seulement si un champ visible change ou si la position dérive de
//! plus de 1,5 s par rapport à l'extrapolation (= seek).

use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use tauri::{AppHandle, Emitter};

use windows::core::HSTRING;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

use super::NowPlaying;

enum Ctl {
    Toggle,
    Next,
    Prev,
    Seek(f64), // secondes
}

static CONTROL: OnceLock<Sender<Ctl>> = OnceLock::new();
/// Dernier état émis (lu par `snapshot` quand une vue se monte).
static LAST: Mutex<Option<NowPlaying>> = Mutex::new(None);
/// AUMID de l'app affichée.
static SHOWN: Mutex<Option<String>> = Mutex::new(None);

const POLL: Duration = Duration::from_millis(400);
const ART_WINDOW: Duration = Duration::from_secs(10);
const DRIFT: f64 = 1.5;

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<Ctl>();
    let _ = CONTROL.set(tx);

    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut mgr: Option<Manager> = None;
        let mut st = PollState::default();
        loop {
            if mgr.is_none() {
                mgr = Manager::RequestAsync().ok().and_then(|op| op.get().ok());
                if mgr.is_none() {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            }
            let m = mgr.as_ref().unwrap();
            match rx.recv_timeout(POLL) {
                Ok(c) => {
                    if let Some(s) = shown_session(m) {
                        run_ctl(&s, c);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            match poll(m, &mut st) {
                Ok(Some(np)) => {
                    *LAST.lock().unwrap_or_else(|p| p.into_inner()) = Some(np.clone());
                    let _ = app.emit("np://update", np);
                }
                Ok(None) => {}
                // Le manager ne répond plus (ex. redémarrage de l'Explorateur) → recréé.
                Err(()) => mgr = None,
            }
        }
    });
}

pub fn snapshot() -> Option<NowPlaying> {
    LAST.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

pub fn toggle() {
    send(Ctl::Toggle);
}
pub fn next() {
    send(Ctl::Next);
}
pub fn prev() {
    send(Ctl::Prev);
}
pub fn seek(seconds: f64) {
    send(Ctl::Seek(seconds));
}

fn send(c: Ctl) {
    if let Some(tx) = CONTROL.get() {
        let _ = tx.send(c);
    }
}

fn run_ctl(s: &Session, c: Ctl) {
    let _ = match c {
        Ctl::Toggle => s.TryTogglePlayPauseAsync().and_then(|o| o.get()),
        Ctl::Next => s.TrySkipNextAsync().and_then(|o| o.get()),
        Ctl::Prev => s.TrySkipPreviousAsync().and_then(|o| o.get()),
        Ctl::Seek(secs) => {
            let start = s.GetTimelineProperties().and_then(|t| t.StartTime()).map(|d| d.Duration).unwrap_or(0);
            s.TryChangePlaybackPositionAsync(start + (secs * 1e7) as i64).and_then(|o| o.get())
        }
    };
}

fn aumid(s: &Session) -> String {
    s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default()
}

fn is_playing(s: &Session) -> bool {
    s.GetPlaybackInfo()
        .and_then(|p| p.PlaybackStatus())
        .map(|st| st == PlaybackStatus::Playing)
        .unwrap_or(false)
}

/// Session de l'app affichée (pour les commandes).
fn shown_session(mgr: &Manager) -> Option<Session> {
    let want = SHOWN.lock().unwrap_or_else(|p| p.into_inner()).clone()?;
    mgr.GetSessions().ok()?.into_iter().find(|s| aumid(s) == want)
}

/// Choisit la session à afficher (cf. en-tête du module).
fn pick(mgr: &Manager) -> Result<Option<Session>, ()> {
    let sessions: Vec<Session> = mgr.GetSessions().map_err(|_| ())?.into_iter().collect();
    let current = mgr.GetCurrentSession().ok();
    let shown = SHOWN.lock().unwrap_or_else(|p| p.into_inner()).clone();

    if let Some(c) = current.as_ref().filter(|c| is_playing(c)) {
        return Ok(Some(c.clone()));
    }
    if let Some(s) = shown.as_ref().and_then(|w| sessions.iter().find(|s| &aumid(s) == w && is_playing(s))) {
        return Ok(Some(s.clone()));
    }
    if let Some(s) = sessions.iter().find(|s| is_playing(s)) {
        return Ok(Some(s.clone()));
    }
    if let Some(s) = shown.as_ref().and_then(|w| sessions.iter().find(|s| &aumid(s) == w)) {
        return Ok(Some(s.clone()));
    }
    Ok(current)
}

#[derive(Default)]
struct PollState {
    /// Clé de piste (app + titre + artiste) : relance la lecture de la pochette.
    track: String,
    track_since: Option<Instant>,
    art: Option<String>,
    last_art_read: Option<Instant>,
    /// Dernière émission + instant, pour l'extrapolation de la dérive.
    emitted: Option<(NowPlaying, Instant)>,
}

/// Horloge système en ticks de 100 ns depuis 1601 (même base que `LastUpdatedTime`).
fn now_ticks() -> i64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64 / 100)
        .unwrap_or(0);
    unix + 11_644_473_600 * 10_000_000
}

fn poll(mgr: &Manager, st: &mut PollState) -> Result<Option<NowPlaying>, ()> {
    let Some(s) = pick(mgr)? else {
        *SHOWN.lock().unwrap_or_else(|p| p.into_inner()) = None;
        st.track.clear();
        st.art = None;
        let empty = NowPlaying::default();
        let changed = st.emitted.as_ref().map(|(e, _)| e.active).unwrap_or(true);
        st.emitted = Some((empty.clone(), Instant::now()));
        return Ok(changed.then_some(empty));
    };
    let app_id = aumid(&s);
    *SHOWN.lock().unwrap_or_else(|p| p.into_inner()) = Some(app_id.clone());

    let props = s.TryGetMediaPropertiesAsync().and_then(|o| o.get()).ok();
    let title = props.as_ref().and_then(|p| p.Title().ok()).map(|h| h.to_string()).unwrap_or_default();
    let artist = props.as_ref().and_then(|p| p.Artist().ok()).map(|h| h.to_string()).unwrap_or_default();
    let playing = is_playing(&s);
    let can_seek = s
        .GetPlaybackInfo()
        .and_then(|p| p.Controls())
        .and_then(|c| c.IsPlaybackPositionEnabled())
        .unwrap_or(false);

    // Position extrapolée : GSMTC ne la met à jour que sporadiquement.
    let (mut position, duration) = match s.GetTimelineProperties() {
        Ok(t) => {
            let start = t.StartTime().map(|d| d.Duration).unwrap_or(0);
            let end = t.EndTime().map(|d| d.Duration).unwrap_or(0);
            let pos = t.Position().map(|d| d.Duration).unwrap_or(0);
            let mut p = (pos - start) as f64 / 1e7;
            if playing {
                if let Ok(updated) = t.LastUpdatedTime() {
                    let dt = (now_ticks() - updated.UniversalTime) as f64 / 1e7;
                    if (0.0..86_400.0).contains(&dt) {
                        p += dt;
                    }
                }
            }
            (p, ((end - start) as f64 / 1e7).max(0.0))
        }
        Err(_) => (0.0, 0.0),
    };
    position = if duration > 0.0 { position.clamp(0.0, duration) } else { position.max(0.0) };

    // Pochette : relue à chaque changement de piste et pendant 10 s ensuite (certaines apps
    // la fournissent après le titre ; Windows peut encore servir celle de l'ancienne piste).
    let key = format!("{app_id}\u{1}{title}\u{1}{artist}");
    if key != st.track {
        st.track = key;
        st.track_since = Some(Instant::now());
        st.last_art_read = None;
        st.art = None;
    }
    let in_window = st.track_since.map(|t| t.elapsed() < ART_WINDOW).unwrap_or(false);
    let due = st.last_art_read.map(|t| t.elapsed() >= Duration::from_millis(1200)).unwrap_or(true);
    if in_window && due {
        st.last_art_read = Some(Instant::now());
        if let Some(a) = props.as_ref().and_then(read_thumb) {
            st.art = Some(a);
        }
    }

    let np = NowPlaying {
        active: true,
        title,
        artist,
        art: st.art.clone(),
        playing,
        position,
        duration,
        can_seek,
        app_id,
    };

    let emit = match &st.emitted {
        None => true,
        Some((prev, at)) => {
            let expected = if prev.playing { prev.position + at.elapsed().as_secs_f64() } else { prev.position };
            prev.active != np.active
                || prev.title != np.title
                || prev.artist != np.artist
                || prev.playing != np.playing
                || prev.can_seek != np.can_seek
                || prev.app_id != np.app_id
                || (prev.duration - np.duration).abs() > 0.5
                || prev.art.as_ref().map(|a| a.len()) != np.art.as_ref().map(|a| a.len())
                || (expected - np.position).abs() > DRIFT
        }
    };
    if emit {
        st.emitted = Some((np.clone(), Instant::now()));
    }
    Ok(emit.then_some(np))
}

fn read_thumb(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<String> {
    let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    reader.LoadAsync(size as u32).ok()?.get().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    // Type MIME déduit des octets (le content-type fourni peut casser la data URL).
    let mime = match buf.as_slice() {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, ..] => "image/jpeg",
        [b'G', b'I', b'F', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'B', b'M', ..] => "image/bmp",
        _ => "image/jpeg",
    };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&buf)))
}

// --- Correspondance app média ↔ exécutable -----------------------------------------

/// Forme « écrasée » : alphanumérique minuscule, sans « .exe ».
pub fn crush(s: &str) -> String {
    let lower = s.to_lowercase();
    let head = lower.split('!').next().unwrap_or(&lower);
    let base = head.rsplit(['\\', '/']).next().unwrap_or(head);
    base.strip_suffix(".exe").unwrap_or(base).chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Firefox expose un AUMID hashé fixe.
const FIREFOX_AUMID: &str = "308046b0af4a39cb";

/// L'exe (chemin complet) correspond-il à l'app média `app_id` ?
pub fn exe_matches(exe_path: &str, app_id: &str) -> bool {
    let exe = crush(exe_path);
    let app = crush(app_id);
    if app.is_empty() || exe.is_empty() {
        return false;
    }
    if app == FIREFOX_AUMID {
        return exe == "firefox";
    }
    exe == app || (exe.len() >= 4 && app.contains(&exe)) || (app.len() >= 4 && exe.contains(&app))
}


// --- « Ouvrir l'app source » ---------------------------------------------------------

/// Amène l'app de la lecture au premier plan : fenêtre du programme (de préférence celle
/// dont le titre contient la piste), sinon activation de l'app packagée, sinon relance de
/// l'exe en cours (les apps mono-instance réaffichent leur fenêtre).
pub fn open_source() -> Result<(), String> {
    let np = snapshot().filter(|n| n.active).ok_or("aucune lecture")?;
    let exes = crate::services::mixer::running_exes()
        .into_iter()
        .filter(|(_, path)| exe_matches(path, &np.app_id))
        .collect::<Vec<_>>();

    if let Some(hwnd) = find_window(&exes.iter().map(|(pid, _)| *pid).collect::<Vec<_>>(), &np.title) {
        focus_window(hwnd);
        return Ok(());
    }
    if np.app_id.contains('!') {
        return activate_packaged(&np.app_id);
    }
    if let Some((_, path)) = exes.first() {
        std::process::Command::new(path).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("app introuvable".into())
}

fn find_window(pids: &[u32], track_title: &str) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, GW_OWNER,
    };
    if pids.is_empty() {
        return None;
    }
    struct Ctx<'a> {
        pids: &'a [u32],
        found: Vec<(HWND, String)>,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> windows::core::BOOL {
        let ctx = &mut *(lp.0 as *mut Ctx);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if ctx.pids.contains(&pid)
            && IsWindowVisible(hwnd).as_bool()
            && GetWindow(hwnd, GW_OWNER).map(|o| o.0.is_null()).unwrap_or(true)
        {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; len as usize + 1];
                let n = GetWindowTextW(hwnd, &mut buf);
                ctx.found.push((hwnd, String::from_utf16_lossy(&buf[..n as usize])));
            }
        }
        true.into()
    }
    let mut ctx = Ctx { pids, found: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    let t = track_title.to_lowercase();
    ctx.found
        .iter()
        .find(|(_, title)| !t.is_empty() && title.to_lowercase().contains(&t))
        .or_else(|| ctx.found.first())
        .map(|(h, _)| *h)
}

fn focus_window(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() {
            return;
        }
        // Windows refuse le premier plan : un appui Alt simulé débloque (technique classique).
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, dwFlags: flags, ..Default::default() } },
        };
        SendInput(&[key(Default::default()), key(KEYEVENTF_KEYUP)], std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn activate_packaged(aumid: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE};
    std::thread::scope(|s| {
        s.spawn(|| unsafe {
            let _ = CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
            let mgr: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            mgr.ActivateApplication(&HSTRING::from(aumid), &HSTRING::new(), AO_NONE)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .join()
        .unwrap_or_else(|_| Err("activation".into()))
    })
}
