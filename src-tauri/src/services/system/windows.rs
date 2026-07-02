// Capteurs système Windows : batterie (GetSystemPowerStatus), connectivité réseau
// (InternetGetConnectedState), inactivité (GetLastInputInfo), volume maître
// (IAudioEndpointVolume / WASAPI). Les opérations COM (volume) sont isolées sur un
// thread MTA dédié, comme le contrôleur média.

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::Networking::WinInet::{InternetGetConnectedState, INTERNET_CONNECTION};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

const BATTERY_FLAG_NO_BATTERY: u8 = 128;

/// (pourcentage 0–100, en charge ?) ou None si pas de batterie / inconnu.
pub fn battery() -> Option<(u8, bool)> {
    unsafe {
        let mut s = SYSTEM_POWER_STATUS::default();
        GetSystemPowerStatus(&mut s).ok()?;
        if s.BatteryFlag & BATTERY_FLAG_NO_BATTERY != 0 || s.BatteryLifePercent > 100 {
            return None; // pas de batterie système, ou niveau inconnu (255)
        }
        Some((s.BatteryLifePercent, s.ACLineStatus == 1))
    }
}

/// Connecté à un réseau ?
pub fn online() -> bool {
    unsafe {
        let mut flags = INTERNET_CONNECTION::default();
        InternetGetConnectedState(&mut flags, None).is_ok()
    }
}

/// Millisecondes depuis la dernière entrée clavier/souris.
pub fn idle_ms() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii).as_bool() {
            GetTickCount().wrapping_sub(lii.dwTime) as u64
        } else {
            0
        }
    }
}

// --- Volume maître (WASAPI IAudioEndpointVolule, COM sur thread dédié) -------

unsafe fn endpoint() -> Option<IAudioEndpointVolume> {
    let enumerator: IMMDeviceEnumerator =
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
    device.Activate(CLSCTX_ALL, None).ok()
}

pub fn volume() -> Option<(f32, bool)> {
    std::thread::spawn(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ep = endpoint()?;
        let level = ep.GetMasterVolumeLevelScalar().ok()?;
        let muted = ep.GetMute().ok()?.as_bool();
        Some((level, muted))
    })
    .join()
    .unwrap_or(None)
}

pub fn set_volume(level: f32) -> Result<(), String> {
    let level = level.clamp(0.0, 1.0);
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ep = endpoint().ok_or_else(|| "system: périphérique audio indisponible".to_string())?;
        ep.SetMasterVolumeLevelScalar(level, std::ptr::null())
            .map_err(|e| e.to_string())
    })
    .join()
    .unwrap_or_else(|_| Err("system: thread volume".into()))
}

pub fn set_muted(muted: bool) -> Result<(), String> {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ep = endpoint().ok_or_else(|| "system: périphérique audio indisponible".to_string())?;
        ep.SetMute(muted, std::ptr::null()).map_err(|e| e.to_string())
    })
    .join()
    .unwrap_or_else(|_| Err("system: thread mute".into()))
}

/// Action d'alimentation via les outils système Windows. `CREATE_NO_WINDOW` évite le flash
/// d'une console. (sleep : SetSuspendState — met en veille, ou hiberne si l'hibernation est
/// activée sur la machine.)
pub fn power(action: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let (prog, args): (&str, &[&str]) = match action {
        "shutdown" => ("shutdown", &["/s", "/t", "0"]),
        "restart" => ("shutdown", &["/r", "/t", "0"]),
        "logoff" => ("shutdown", &["/l"]),
        "hibernate" => ("shutdown", &["/h"]),
        "lock" => ("rundll32.exe", &["user32.dll,LockWorkStation"]),
        "sleep" => ("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        other => return Err(format!("action d'alimentation inconnue : {other}")),
    };
    Command::new(prog)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
