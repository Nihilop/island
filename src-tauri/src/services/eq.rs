// Égaliseur : niveaux réels du son qui sort des haut-parleurs (loopback WASAPI de la
// sortie par défaut), mixé en mono. Toutes les 25 ms : FFT 1024 points (fenêtre de Hann)
// → 4 bandes 40–160 / 160–600 / 600–2400 / 2400–9000 Hz, gain automatique par bande,
// seuil de bruit, courbe puissance 0,8, lissage attaque/relâchement → `eq://levels`.
//
// Ne tourne que si au moins un égaliseur visible le demande (compteur d'abonnés) ; le
// thread est parké sinon. Rien n'est enregistré.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;

static SUBSCRIBERS: AtomicI32 = AtomicI32::new(0);
static WAKER: OnceLock<std::thread::Thread> = OnceLock::new();

/// `on = true` : un égaliseur devient visible ; `false` : il disparaît.
#[tauri::command]
pub fn eq_subscribe(on: bool) {
    let n = if on {
        SUBSCRIBERS.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        (SUBSCRIBERS.fetch_sub(1, Ordering::SeqCst) - 1).max(0)
    };
    if n <= 0 {
        SUBSCRIBERS.store(0, Ordering::SeqCst);
    }
    if let Some(t) = WAKER.get() {
        t.unpark();
    }
}

pub fn start(app: AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let handle = std::thread::spawn(move || imp::run(app));
        let _ = WAKER.set(handle.thread().clone());
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

fn active() -> bool {
    SUBSCRIBERS.load(Ordering::SeqCst) > 0
}

#[cfg(target_os = "windows")]
mod imp {
    use std::time::{Duration, Instant};

    use rustfft::{num_complex::Complex, FftPlanner};
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

    const N: usize = 1024;
    const TICK: Duration = Duration::from_millis(25);
    const BANDS: [(f32, f32); 4] = [(40.0, 160.0), (160.0, 600.0), (600.0, 2400.0), (2400.0, 9000.0)];
    const NOISE_FLOOR: f32 = 2e-3;
    const PEAK_DECAY: f32 = 0.996;
    const ATTACK: f32 = 0.55;
    const RELEASE: f32 = 0.18;
    const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;

    pub fn run(app: AppHandle) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        loop {
            while !super::active() {
                std::thread::park();
            }
            // Retour d'erreur ou changement de périphérique → on rouvre.
            if let Err(e) = unsafe { capture(&app) } {
                eprintln!("[eq] {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }

    unsafe fn default_device_id(e: &IMMDeviceEnumerator) -> Option<String> {
        let d = e.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let id = d.GetId().ok()?;
        let s = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const _));
        s
    }

    unsafe fn capture(app: &AppHandle) -> Result<(), String> {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let device_id = default_device_id(&e);
        let device = e.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| e.to_string())?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
        let pwfx = client.GetMixFormat().map_err(|e| e.to_string())?;
        let wf = *pwfx;
        let channels = wf.nChannels.max(1) as usize;
        let rate = wf.nSamplesPerSec as f32;
        let is_i16 = wf.wBitsPerSample == 16;
        let block = wf.nBlockAlign as usize;
        client
            .Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 0, 0, pwfx, None)
            .map_err(|e| e.to_string())?;
        CoTaskMemFree(Some(pwfx as *const _));
        let cap: IAudioCaptureClient = client.GetService().map_err(|e| e.to_string())?;
        client.Start().map_err(|e| e.to_string())?;

        let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
        let hann: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
        let mut ring: Vec<f32> = Vec::with_capacity(N * 2);
        let mut peaks = [1e-3f32; 4];
        let mut out = [0f32; 4];
        let mut buf = vec![Complex::new(0.0, 0.0); N];
        let mut last_check = Instant::now();

        let result = loop {
            if !super::active() {
                break Ok(());
            }
            // Le périphérique par défaut a changé → reconnexion.
            if last_check.elapsed() >= Duration::from_secs(2) {
                last_check = Instant::now();
                if default_device_id(&e) != device_id {
                    break Ok(());
                }
            }
            let tick_start = Instant::now();

            // Vide la file de paquets dans le tampon circulaire (mono).
            loop {
                let packet = match cap.GetNextPacketSize() {
                    Ok(p) => p,
                    Err(_) => break,
                };
                if packet == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                if cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    break;
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT != 0 || data.is_null();
                for f in 0..frames as usize {
                    let mut sum = 0f32;
                    if !silent {
                        let frame = data.add(f * block);
                        for c in 0..channels {
                            sum += if is_i16 {
                                *(frame as *const i16).add(c) as f32 / 32768.0
                            } else {
                                *(frame as *const f32).add(c)
                            };
                        }
                    }
                    ring.push(sum / channels as f32);
                }
                let _ = cap.ReleaseBuffer(frames);
            }
            // On ne garde que les N derniers échantillons (purge des consommés).
            if ring.len() > N {
                ring.drain(..ring.len() - N);
            }

            // FFT (zéro-padding si le loopback n'a rien envoyé : il se tait dans le silence).
            let pad = N - ring.len();
            for i in 0..N {
                let s = if i < pad { 0.0 } else { ring[i - pad] };
                buf[i] = Complex::new(s * hann[i], 0.0);
            }
            fft.process(&mut buf);
            let bin_hz = rate / N as f32;
            let mut levels = [0f32; 4];
            for (b, (lo, hi)) in BANDS.iter().enumerate() {
                let (a, z) = ((lo / bin_hz) as usize, ((hi / bin_hz) as usize).min(N / 2));
                let energy: f32 = buf[a.max(1)..z.max(a + 1)].iter().map(|c| c.norm()).sum::<f32>() / N as f32;
                peaks[b] = (peaks[b] * PEAK_DECAY).max(energy).max(1e-3);
                let v = if energy < NOISE_FLOOR { 0.0 } else { (energy / peaks[b]).min(1.0).powf(0.8) };
                let k = if v > out[b] { ATTACK } else { RELEASE };
                out[b] += (v - out[b]) * k;
                levels[b] = out[b];
            }
            let _ = app.emit("eq://levels", levels);

            if let Some(rest) = TICK.checked_sub(tick_start.elapsed()) {
                std::thread::sleep(rest);
            }
        };
        let _ = client.Stop();
        result
    }
}
