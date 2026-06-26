// Capture du son SYSTÈME via WASAPI loopback (ce qui sort des haut-parleurs).
// On écrit le PCM brut dans un fichier ; à l'arrêt de l'enregistrement, l'hôte
// muxe ce PCM avec la vidéo via ffmpeg (l'extension fournit le codec audio).
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};

const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;

/// Format réel du flux capturé (pour dire à ffmpeg comment lire le PCM).
#[derive(Clone)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub pcm: &'static str, // "f32le" | "s16le"
}

pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    format: Arc<Mutex<Option<AudioFormat>>>,
}

impl AudioCapture {
    /// Arrête la capture, attend la fin d'écriture, renvoie le format réel.
    pub fn stop(mut self) -> Option<AudioFormat> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.format.lock().ok().and_then(|g| g.clone())
    }
}

/// Démarre la capture loopback (son système) vers `pcm_path`. Ne bloque pas.
pub fn start_capture(pcm_path: String) -> AudioCapture {
    start_capture_source(pcm_path, false)
}

/// Démarre la capture vers `pcm_path` : `mic=true` → entrée micro (`eCapture`),
/// `mic=false` → son système (loopback `eRender`). Ne bloque pas (thread dédié).
pub fn start_capture_source(pcm_path: String, mic: bool) -> AudioCapture {
    let stop = Arc::new(AtomicBool::new(false));
    let format = Arc::new(Mutex::new(None));
    let stop_t = stop.clone();
    let fmt_t = format.clone();
    let handle = std::thread::spawn(move || {
        if let Err(e) = run(&pcm_path, &stop_t, &fmt_t, mic) {
            eprintln!("audio capture ({}): {e}", if mic { "mic" } else { "loopback" });
        }
    });
    AudioCapture { stop, handle: Some(handle), format }
}

fn run(pcm_path: &str, stop: &AtomicBool, fmt_out: &Mutex<Option<AudioFormat>>, mic: bool) -> Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        // mic → eCapture (entrée micro, pas de flag loopback) ;
        // sinon → eRender + loopback (on capte ce qui est JOUÉ = son système).
        let (dataflow, stream_flags) = if mic {
            (eCapture, 0)
        } else {
            (eRender, AUDCLNT_STREAMFLAGS_LOOPBACK)
        };
        let device = enumerator.GetDefaultAudioEndpoint(dataflow, eConsole).map_err(|e| e.to_string())?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;

        let pwfx = client.GetMixFormat().map_err(|e| e.to_string())?;
        let wf = *pwfx;
        let channels = wf.nChannels;
        let sample_rate = wf.nSamplesPerSec;
        let block_align = wf.nBlockAlign as usize;
        // Le mix WASAPI est quasi toujours float 32 ; 16-bit possible.
        let pcm = if wf.wBitsPerSample == 16 { "s16le" } else { "f32le" };
        if let Ok(mut g) = fmt_out.lock() {
            *g = Some(AudioFormat { sample_rate, channels, pcm });
        }

        client
            .Initialize(AUDCLNT_SHAREMODE_SHARED, stream_flags, 0, 0, pwfx, None)
            .map_err(|e| e.to_string())?;
        let capture: IAudioCaptureClient = client.GetService().map_err(|e| e.to_string())?;
        client.Start().map_err(|e| e.to_string())?;

        let mut file = File::create(pcm_path).map_err(|e| e.to_string())?;

        while !stop.load(Ordering::Relaxed) {
            let mut packet = capture.GetNextPacketSize().map_err(|e| e.to_string())?;
            while packet != 0 {
                let mut pdata: *mut u8 = std::ptr::null_mut();
                let mut frames: u32 = 0;
                let mut flags: u32 = 0;
                capture
                    .GetBuffer(&mut pdata, &mut frames, &mut flags, None, None)
                    .map_err(|e| e.to_string())?;
                let bytes = frames as usize * block_align;
                if flags & AUDCLNT_BUFFERFLAGS_SILENT == 0 && !pdata.is_null() {
                    let slice = std::slice::from_raw_parts(pdata, bytes);
                    let _ = file.write_all(slice);
                } else {
                    // Silence loopback : on écrit des zéros pour garder la synchro A/V.
                    let _ = file.write_all(&vec![0u8; bytes]);
                }
                capture.ReleaseBuffer(frames).map_err(|e| e.to_string())?;
                packet = capture.GetNextPacketSize().map_err(|e| e.to_string())?;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let _ = client.Stop();
        CoTaskMemFree(Some(pwfx as *const _));
        CoUninitialize();
    }
    Ok(())
}
