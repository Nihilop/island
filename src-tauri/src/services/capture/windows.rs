// Capture d'écran via Windows Graphics Capture (anti-cheat safe, pas d'injection).
// Screenshot (PNG) via windows-capture ; enregistrement vidéo = frames WGC piped
// vers ffmpeg (encodage CRF, couleurs BT.709 correctes, codec/qualité au choix).
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::{Frame, ImageFormat};
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

/// Énumère les moniteurs (pour le picker « Écran 1 / Écran 2 » côté extension).
pub fn list_displays() -> Vec<super::DisplayInfo> {
    let primary_dev = Monitor::primary().ok().and_then(|m| m.device_name().ok());
    let mut out = Vec::new();
    if let Ok(monitors) = Monitor::enumerate() {
        for m in monitors {
            let index = m.index().unwrap_or(0);
            let dev = m.device_name().ok();
            out.push(super::DisplayInfo {
                index,
                name: m.name().unwrap_or_else(|_| format!("Écran {index}")),
                width: m.width().unwrap_or(0),
                height: m.height().unwrap_or(0),
                primary: dev.is_some() && dev == primary_dev,
            });
        }
    }
    out
}

/// Handler « one-shot » : sauve la première frame (éventuellement croppée) en PNG.
struct Shot {
    path: String,
    region: Option<super::Region>,
}
impl GraphicsCaptureApiHandler for Shot {
    type Flags = (String, Option<super::Region>);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { path: ctx.flags.0, region: ctx.flags.1 })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        ctrl: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        match self.region {
            // crop : (start_w, start_h, end_w, end_h) en pixels physiques
            Some(r) => frame
                .buffer_crop(r.x, r.y, r.x + r.w, r.y + r.h)?
                .save_as_image(&self.path, ImageFormat::Png)?,
            None => frame.save_as_image(&self.path, ImageFormat::Png)?,
        }
        ctrl.stop(); // une seule frame suffit pour un screenshot
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Capture un moniteur (1-based ; `None` = moniteur principal) vers `path` (PNG).
/// `region` = zone à découper (px physiques) ; `None` = plein écran.
pub fn screenshot(display: Option<usize>, path: String, region: Option<super::Region>) -> Result<(), String> {
    let monitor = match display {
        Some(i) => Monitor::from_index(i).map_err(|e| e.to_string())?,
        None => Monitor::primary().map_err(|e| e.to_string())?,
    };
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::Default,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (path, region),
    );
    // `start` prend le contrôle du thread (pompe de messages) jusqu'au `stop()`.
    // Comme on stoppe à la 1ʳᵉ frame, ça revient vite ; on l'isole sur un thread.
    match std::thread::spawn(move || Shot::start(settings)).join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("capture: thread panic".to_string()),
    }
}

// --- Enregistrement vidéo (frames WGC → ffmpeg) -----------------------------

const CREATE_NO_WINDOW: u32 = 0x0800_0000; // pas de fenêtre console au lancement de l'encodeur

/// Encodeur fourni PAR L'EXTENSION : binaire (chemin absolu, déjà résolu & vérifié
/// dans le dossier de l'extension côté lib.rs) + arguments (codec/filtres/couleurs).
/// L'hôte reste AGNOSTIQUE : il n'ajoute que l'entrée (frames brutes) et la sortie.
pub struct Encoder {
    pub bin: String,
    pub args: Vec<String>,
    /// Args du codec AUDIO pour le mux (ex. `-c:a aac -b:a 160k`). Vide si pas d'audio.
    pub audio_args: Vec<String>,
}

/// 2ᵉ passe (si audio) : mux du MP4 vidéo + du PCM système → fichier final.
struct MuxJob {
    bin: String,
    temp_video: String,
    temp_pcm: String,
    audio_args: Vec<String>,
}

/// Dernière frame capturée (BGRA packé) + dimensions RÉELLES du buffer, partagées
/// entre le handler WGC et la pompe. Les vraies dims (≠ monitor.width() en DPI) sont
/// lues sur la 1ʳᵉ frame pour configurer ffmpeg (`-s`) → sinon frames rejetées = MP4 vide.
struct Latest {
    buf: Mutex<Option<Vec<u8>>>,
    dims: Mutex<Option<(u32, u32)>>,
}

struct Recorder {
    latest: Arc<Latest>,
    region: Option<super::Region>,
}
impl GraphicsCaptureApiHandler for Recorder {
    type Flags = (Arc<Latest>, Option<super::Region>);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { latest: ctx.flags.0, region: ctx.flags.1 })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _ctrl: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // Copie la dernière frame (zone croppée ou plein écran) + ses dimensions
        // réelles ; la pompe la pousse à cadence constante. Le buffer WGC est déjà
        // dans le bon sens pour ffmpeg (-f rawvideo) → aucun flip (sinon à l'envers).
        let (bytes, w, h) = match self.region {
            Some(r) => {
                let mut b = frame.buffer_crop(r.x, r.y, r.x + r.w, r.y + r.h)?;
                let (w, h) = (b.width(), b.height());
                (b.as_nopadding_buffer()?.to_vec(), w, h)
            }
            None => {
                let mut b = frame.buffer()?;
                let (w, h) = (b.width(), b.height());
                (b.as_nopadding_buffer()?.to_vec(), w, h)
            }
        };
        if let Ok(mut d) = self.latest.dims.lock() {
            *d = Some((w, h));
        }
        if let Ok(mut g) = self.latest.buf.lock() {
            *g = Some(bytes);
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

type RecCtrl = CaptureControl<Recorder, <Recorder as GraphicsCaptureApiHandler>::Error>;

struct Recording {
    control: RecCtrl,
    ffmpeg: Child,
    pump: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    path: String, // chemin FINAL rendu à l'UI
    audio: Option<crate::services::audio::AudioCapture>,
    mux: Option<MuxJob>,
}

static RECORDING: Mutex<Option<Recording>> = Mutex::new(None);

/// Démarre l'enregistrement d'un moniteur (1-based ; `None` = principal).
/// `region` = zone (px physiques) ; `None` = plein écran. L'encodage est fait par
/// `encoder` (binaire + args fournis par l'extension) ; l'hôte ne fait que capturer.
pub fn start_recording(
    display: Option<usize>,
    path: String,
    region: Option<super::Region>,
    fps: u32,
    audio: bool,
    encoder: Encoder,
) -> Result<(), String> {
    let mut slot = RECORDING.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_some() {
        return Err("capture: un enregistrement est déjà en cours".to_string());
    }
    let monitor = match display {
        Some(i) => Monitor::from_index(i).map_err(|e| e.to_string())?,
        None => Monitor::primary().map_err(|e| e.to_string())?,
    };
    // H.264/H.265 veulent des dimensions PAIRES → on arrondit (zone ET plein écran).
    let region = region.map(|mut r| {
        r.w &= !1;
        r.h &= !1;
        r
    });
    // On démarre la capture WGC AVANT ffmpeg pour lire les dimensions RÉELLES du
    // buffer sur la 1ʳᵉ frame (monitor.width() peut différer en DPI scaling ; si on se
    // trompe sur `-s`, ffmpeg rejette toutes les frames → MP4 vide de ~250 octets).
    let latest = Arc::new(Latest { buf: Mutex::new(None), dims: Mutex::new(None) });
    let stop = Arc::new(AtomicBool::new(false));

    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::Default,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8, // octets BGRA bruts attendus par ffmpeg (-pixel_format bgra)
        (latest.clone(), region),
    );
    let control = Recorder::start_free_threaded(settings).map_err(|e| e.to_string())?;

    // Attendre la 1ʳᵉ frame (max 3 s) → vraies dimensions du flux.
    let mut waited = 0u32;
    let (width, height) = loop {
        if let Some(d) = latest.dims.lock().ok().and_then(|g| *g) {
            break d;
        }
        if waited >= 3000 {
            let _ = control.stop();
            return Err("capture: aucune frame reçue (capture impossible)".into());
        }
        std::thread::sleep(Duration::from_millis(20));
        waited += 20;
    };

    // Avec audio : la vidéo va dans un temporaire, le son système est capturé en
    // parallèle (PCM), et on muxe les deux à l'arrêt. Sinon : vidéo direct → final.
    let (video_out, mux) = if audio {
        let temp_video = format!("{path}.v.mp4");
        let temp_pcm = format!("{path}.a.pcm");
        let job = MuxJob {
            bin: encoder.bin.clone(),
            temp_video: temp_video.clone(),
            temp_pcm,
            audio_args: encoder.audio_args.clone(),
        };
        (temp_video, Some(job))
    } else {
        (path.clone(), None)
    };

    // Commande : l'HÔTE n'impose que l'ENTRÉE (frames brutes BGRA, dimensions RÉELLES)
    // et la SORTIE (dernier arg) ; l'extension fournit tout l'encodage → cœur agnostique.
    let mut cmd_args: Vec<String> = vec![
        "-y".into(),
        "-f".into(), "rawvideo".into(),
        "-pixel_format".into(), "bgra".into(),
        "-video_size".into(), format!("{width}x{height}"),
        "-framerate".into(), fps.to_string(),
        "-i".into(), "-".into(),
    ];
    cmd_args.extend(encoder.args.iter().cloned());
    cmd_args.push(video_out.clone());

    let mut child = match Command::new(&encoder.bin)
        .args(&cmd_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = control.stop();
            return Err(format!("capture: encodeur introuvable ou illisible ({e})"));
        }
    };
    let stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            let _ = control.stop();
            let _ = child.kill();
            return Err("capture: stdin encodeur indisponible".into());
        }
    };

    // Pompe à fps CONSTANT : pousse la dernière frame sur stdin (duplique si l'écran
    // est statique → CRF la compresse à presque rien). Découple WGC (event-driven).
    let interval = Duration::from_micros(1_000_000 / fps.max(1) as u64);
    let frame_size = (width as usize) * (height as usize) * 4;
    let pump_latest = latest.clone();
    let pump_stop = stop.clone();
    let pump = std::thread::spawn(move || {
        let mut stdin = stdin;
        // Attendre la 1ʳᵉ frame (sinon ffmpeg démarre dans le vide).
        while !pump_stop.load(Ordering::Relaxed) {
            if pump_latest.buf.lock().map(|g| g.is_some()).unwrap_or(false) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut next = Instant::now();
        while !pump_stop.load(Ordering::Relaxed) {
            let frame = pump_latest.buf.lock().ok().and_then(|g| g.clone());
            if let Some(f) = frame {
                // Taille incohérente (résolution changée) → on saute pour ne pas casser ffmpeg.
                if f.len() == frame_size && stdin.write_all(&f).is_err() {
                    break; // ffmpeg a fermé son entrée
                }
            }
            next += interval;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now;
            }
        }
        // stdin droppé ici → ffmpeg reçoit EOF et finalise le conteneur.
    });

    // Capture du son système en parallèle (si demandé) → fichier PCM temporaire.
    let audio_cap = mux.as_ref().map(|m| crate::services::audio::start_capture(m.temp_pcm.clone()));

    *slot = Some(Recording {
        control,
        ffmpeg: child,
        pump: Some(pump),
        stop,
        path,
        audio: audio_cap,
        mux,
    });
    Ok(())
}

/// Arrête l'enregistrement, laisse ffmpeg finaliser le MP4, renvoie son chemin.
pub fn stop_recording() -> Result<String, String> {
    let mut rec = RECORDING
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .ok_or("capture: aucun enregistrement en cours".to_string())?;

    rec.stop.store(true, Ordering::Relaxed);
    // Stoppe la session WGC (plus de frames) — best-effort.
    let _ = rec.control.stop();
    // Joint la pompe → stdin droppé → ffmpeg reçoit EOF.
    if let Some(p) = rec.pump.take() {
        let _ = p.join();
    }
    // Laisse ffmpeg écrire l'index/moov et se terminer (vidéo finalisée).
    let _ = rec.ffmpeg.wait();

    // Pas d'audio : la vidéo est déjà au chemin final.
    let (audio, mux) = (rec.audio.take(), rec.mux.take());
    if let (Some(audio), Some(mux)) = (audio, mux) {
        let fmt = audio.stop(); // attend la fin d'écriture + renvoie le format réel
        if let Some(f) = fmt {
            // 2ᵉ passe : mux vidéo (copy, pas de ré-encodage) + PCM système → final.
            let mut args: Vec<String> = vec![
                "-y".into(),
                "-i".into(), mux.temp_video.clone(),
                "-f".into(), f.pcm.into(),
                "-ar".into(), f.sample_rate.to_string(),
                "-ac".into(), f.channels.to_string(),
                "-i".into(), mux.temp_pcm.clone(),
                "-map".into(), "0:v:0".into(),
                "-map".into(), "1:a:0".into(),
                "-c:v".into(), "copy".into(),
            ];
            if mux.audio_args.is_empty() {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "160k".into()]);
            } else {
                args.extend(mux.audio_args.iter().cloned());
            }
            args.push("-shortest".into());
            args.push(rec.path.clone());

            let status = Command::new(&mux.bin)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();

            let ok = status.map(|s| s.success()).unwrap_or(false) && std::path::Path::new(&rec.path).exists();
            if ok {
                let _ = std::fs::remove_file(&mux.temp_video);
            } else {
                // Mux raté : on récupère AU MOINS la vidéo (sans son).
                let _ = std::fs::rename(&mux.temp_video, &rec.path);
            }
            let _ = std::fs::remove_file(&mux.temp_pcm);
        } else {
            // Capture audio KO : on garde la vidéo seule.
            let _ = std::fs::rename(&mux.temp_video, &rec.path);
            let _ = std::fs::remove_file(&mux.temp_pcm);
        }
    }
    Ok(rec.path)
}

/// Y a-t-il un enregistrement en cours ? (pour resynchroniser l'UI au besoin)
pub fn is_recording() -> bool {
    RECORDING.lock().map(|s| s.is_some()).unwrap_or(false)
}
