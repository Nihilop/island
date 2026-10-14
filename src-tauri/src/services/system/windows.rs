// Volume maître du périphérique de sortie (WASAPI IAudioEndpointVolume), lu sur un
// thread MTA dédié (COM), comme le contrôleur média.

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

unsafe fn endpoint() -> Option<IAudioEndpointVolume> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
    device.Activate(CLSCTX_ALL, None).ok()
}

/// (niveau 0–1, muet ?) du périphérique de sortie par défaut.
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
