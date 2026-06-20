//! Contrôleur média natif (Windows SMTC) — AGNOSTIQUE (aucune app en dur).
//!
//! Suit la **session média active de l'OS** (`GetCurrentSession` : l'app que
//! Windows considère comme courante — Spotify, Deezer, un navigateur…) via
//! `GlobalSystemMediaTransportControlsSessionManager`, et expose les contrôles
//! transport (play/pause/suivant/préc./seek). Le volume cible la session audio
//! Windows (WASAPI / ISimpleAudioVolume) du **process derrière cette session
//! courante** — identifié dynamiquement depuis son AUMID.
//!
//! Tout le WinRT/COM vit sur un thread MTA dédié ; le front pilote via des
//! commandes Tauri et reçoit l'état par l'event `media://update`.

use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine as _;
use tauri::{AppHandle, Emitter};

use windows::core::Interface;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

enum Ctl {
    Toggle,
    Next,
    Prev,
    Seek(i64), // ms
}

static CONTROL: OnceLock<Sender<Ctl>> = OnceLock::new();

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaUpdate {
    active: bool,
    title: Option<String>,
    artist: Option<String>,
    is_playing: bool,
    position_ms: i64,
    duration_ms: i64,
    /// Data-URL de la pochette — fournie uniquement quand le titre change.
    art: Option<String>,
    /// App source du média (jeton normalisé de l'AUMID, ex. "spotify", "msedge") →
    /// permet à une extension dédiée (Spotify) de ne réagir QU'À son app.
    source: Option<String>,
}

impl MediaUpdate {
    fn inactive() -> Self {
        MediaUpdate {
            active: false,
            title: None,
            artist: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: 0,
            art: None,
            source: None,
        }
    }
}

/// Démarre le thread contrôleur (MTA) : poll de l'état + exécution des contrôles.
pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<Ctl>();
    let _ = CONTROL.set(tx);

    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut last_title = String::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(700)) {
                Ok(c) => {
                    run_ctl(c);
                    if let Some(u) = read_update(&mut last_title) {
                        let _ = app.emit("media://update", u);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(u) = read_update(&mut last_title) {
                        let _ = app.emit("media://update", u);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

// --- Commandes (appelées depuis lib.rs) -------------------------------------

pub fn toggle() {
    send(Ctl::Toggle);
}
pub fn next() {
    send(Ctl::Next);
}
pub fn prev() {
    send(Ctl::Prev);
}
pub fn seek(position_ms: i64) {
    send(Ctl::Seek(position_ms));
}

fn send(c: Ctl) {
    if let Some(tx) = CONTROL.get() {
        let _ = tx.send(c);
    }
}

/// Volume : opérations COM courtes, isolées sur leur propre thread MTA.
pub fn get_volume() -> f32 {
    std::thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            read_volume().unwrap_or(-1.0)
        }
    })
    .join()
    .unwrap_or(-1.0)
}

pub fn set_volume(level: f32) {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        write_volume(level.clamp(0.0, 1.0));
    });
}

// --- SMTC -------------------------------------------------------------------

/// Session média ACTIVE selon l'OS (peu importe l'app). C'est la notion système de
/// « l'app média courante » — on ne privilégie aucune application.
fn current_session() -> Option<Session> {
    let mgr = Manager::RequestAsync().ok()?.get().ok()?;
    mgr.GetCurrentSession().ok()
}

fn run_ctl(c: Ctl) {
    if let Some(s) = current_session() {
        let _ = match c {
            Ctl::Toggle => s.TryTogglePlayPauseAsync().and_then(|o| o.get()),
            Ctl::Next => s.TrySkipNextAsync().and_then(|o| o.get()),
            Ctl::Prev => s.TrySkipPreviousAsync().and_then(|o| o.get()),
            Ctl::Seek(ms) => s
                .TryChangePlaybackPositionAsync(ms * 10_000)
                .and_then(|o| o.get()),
        };
    }
}

fn read_update(last_title: &mut String) -> Option<MediaUpdate> {
    let mgr = Manager::RequestAsync().ok()?.get().ok()?;
    let session = match mgr.GetCurrentSession().ok() {
        Some(s) => s,
        None => {
            last_title.clear();
            return Some(MediaUpdate::inactive());
        }
    };

    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();

    let timeline = session.GetTimelineProperties().ok()?;
    let position_ms = timeline.Position().map(|t| t.Duration / 10_000).unwrap_or(0);
    let duration_ms = timeline.EndTime().map(|t| t.Duration / 10_000).unwrap_or(0);

    let is_playing = session
        .GetPlaybackInfo()
        .and_then(|p| p.PlaybackStatus())
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);

    let mut art = None;
    if title != *last_title {
        art = read_thumb(&session);
        *last_title = title.clone();
    }

    let source = session
        .SourceAppUserModelId()
        .ok()
        .map(|h| normalize_app_hint(&h.to_string()));

    Some(MediaUpdate {
        active: true,
        title: Some(title),
        artist: Some(artist),
        is_playing,
        position_ms,
        duration_ms,
        art,
        source,
    })
}

fn read_thumb(session: &Session) -> Option<String> {
    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
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
    let mime = stream
        .ContentType()
        .map(|h| h.to_string())
        .unwrap_or_else(|_| "image/jpeg".to_string());
    Some(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

// --- Volume (WASAPI, session audio de l'app média COURANTE) -----------------

/// Jeton normalisé identifiant l'app média courante, dérivé de l'AUMID de la
/// session SMTC active → sert à retrouver SA session audio. Ex. `"Spotify.exe"` →
/// `"spotify"`, un chemin `C:\…\Deezer.exe` → `"deezer"`, un AUMID UWP `"Pkg_…!App"`
/// → `"pkg_…"` (best-effort ; si rien ne matche, le volume est indisponible).
fn current_media_app_hint() -> Option<String> {
    let mgr = Manager::RequestAsync().ok()?.get().ok()?;
    let session = mgr.GetCurrentSession().ok()?;
    let aumid = session.SourceAppUserModelId().ok()?.to_string();
    let hint = normalize_app_hint(&aumid);
    (!hint.is_empty()).then_some(hint)
}

fn normalize_app_hint(aumid: &str) -> String {
    let lower = aumid.to_lowercase();
    // UWP : "Famille_hash!AppId" → on garde la partie avant '!'.
    let head = lower.split('!').next().unwrap_or(&lower);
    // Chemin/exe → nom de fichier sans extension.
    let base = head.rsplit(['\\', '/']).next().unwrap_or(head);
    base.strip_suffix(".exe").unwrap_or(base).trim().to_string()
}

/// Session audio (WASAPI) du process correspondant à l'app média courante.
unsafe fn current_app_volume() -> Option<ISimpleAudioVolume> {
    let hint = current_media_app_hint()?;
    let enumerator: IMMDeviceEnumerator =
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
    let mgr: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).ok()?;
    let sessions = mgr.GetSessionEnumerator().ok()?;
    let count = sessions.GetCount().ok()?;
    for i in 0..count {
        if let Ok(ctrl) = sessions.GetSession(i) {
            if let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() {
                if let Ok(pid) = ctrl2.GetProcessId() {
                    if process_matches(pid, &hint) {
                        return ctrl2.cast::<ISimpleAudioVolume>().ok();
                    }
                }
            }
        }
    }
    None
}

unsafe fn read_volume() -> Option<f32> {
    current_app_volume().and_then(|v| v.GetMasterVolume().ok())
}

unsafe fn write_volume(level: f32) {
    if let Some(v) = current_app_volume() {
        let _ = v.SetMasterVolume(level, std::ptr::null());
    }
}

/// Le process `pid` correspond-il à l'app média courante ? (comparaison sur le nom
/// d'exécutable sans extension, tolérante : égal / inclus dans un sens ou l'autre).
fn process_matches(pid: u32, hint: &str) -> bool {
    if hint.is_empty() {
        return false;
    }
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return false;
        }
        let full = String::from_utf16_lossy(&buf[..size as usize]).to_lowercase();
        let base = full.rsplit(['\\', '/']).next().unwrap_or(&full);
        let proc = base.strip_suffix(".exe").unwrap_or(base);
        proc == hint || proc.contains(hint) || hint.contains(proc)
    }
}
