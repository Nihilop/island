// Lecture des vidéos de calques pour le moteur des fonds animés (Windows, Media Foundation).
//
// Un lecteur par fichier, sur son propre fil : le Source Reader décode (décodeur matériel
// quand il existe), convertit en RGB32 et le fil cadence les images sur leur horodatage
// (× la vitesse du calque). L'image la plus récente attend dans `latest` ; le moteur la copie
// dans la texture du calque avant de dessiner. En fin de vidéo, on repart au début de la
// boucle (`playback.start`). Pas de son. Pause (jeu, plein écran) : le fil dort.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{GUID, HSTRING};
use windows::Win32::Media::MediaFoundation::{
    IMFSample, IMFSourceReader, MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video, MFStartup,
    MFVideoFormat_RGB32, MFSTARTUP_FULL, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_I8;

use super::scene::Playback;

/// Côté le plus grand accepté (au-delà, la copie de chaque image coûte trop cher).
const MAX_SIDE: u32 = 4096;

pub struct Player {
    pub width: u32,
    pub height: u32,
    /// Dernière image décodée (BGRA, `width × height × 4`), pas encore copiée.
    latest: Arc<Mutex<Option<Vec<u8>>>>,
    paused: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Player {
    /// Ouvre la vidéo et lance sa lecture en boucle. Erreur si elle est illisible.
    pub fn open(path: &Path, playback: &Playback) -> Result<Player, String> {
        let latest = Arc::new(Mutex::new(None));
        let paused = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (path, speed, start) = (path.to_path_buf(), playback.speed.clamp(0.25, 4.0) as f64, playback.start.max(0.0) as f64);
        let (l, p, s) = (latest.clone(), paused.clone(), stop.clone());
        std::thread::Builder::new()
            .name("wallpaper-video".into())
            .spawn(move || {
                let reader = match unsafe { open_reader(&path) } {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                let _ = tx.send(Ok((reader.width, reader.height)));
                reader.run(speed, start, &l, &p, &s);
            })
            .map_err(|e| e.to_string())?;
        let (width, height) = rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "videoTimeout".to_string())??;
        Ok(Player { width, height, latest, paused, stop })
    }

    /// L'image la plus récente, si une nouvelle est arrivée depuis le dernier appel.
    pub fn take(&self) -> Option<Vec<u8>> {
        self.latest.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Reader {
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    /// Octets par ligne ; négatif = image rangée de bas en haut.
    stride: i32,
}

const STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

unsafe fn open_reader(path: &Path) -> Result<Reader, String> {
    let err = |e: windows::core::Error| format!("vidéo : {e}");
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(err)?;
    let mut attrs = None;
    MFCreateAttributes(&mut attrs, 2).map_err(err)?;
    let attrs = attrs.ok_or("vidéo : attributs")?;
    attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).map_err(err)?;
    attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).map_err(err)?;
    let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attrs).map_err(err)?;
    reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false).map_err(err)?;
    reader.SetStreamSelection(STREAM, true).map_err(err)?;
    let wanted = MFCreateMediaType().map_err(err)?;
    wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(err)?;
    wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(err)?;
    reader.SetCurrentMediaType(STREAM, None, &wanted).map_err(err)?;
    let current = reader.GetCurrentMediaType(STREAM).map_err(err)?;
    let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(err)?;
    let (width, height) = ((size >> 32) as u32, size as u32);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err("videoTooLarge".into());
    }
    let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or(width as i32 * 4);
    Ok(Reader { reader, width, height, stride })
}

/// Position (unités de 100 ns) pour `SetCurrentPosition`.
fn position(seconds: f64) -> PROPVARIANT {
    let mut v = PROPVARIANT::default();
    unsafe {
        let inner = &mut *v.Anonymous.Anonymous;
        inner.vt = VT_I8;
        inner.Anonymous.hVal = (seconds * 10_000_000.0) as i64;
    }
    v
}

impl Reader {
    fn seek(&self, seconds: f64) -> bool {
        unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &position(seconds)).is_ok() }
    }

    /// Copie l'échantillon en BGRA dense, alpha forcé à 255 (RGB32 ne le garantit pas).
    fn frame(&self, sample: &IMFSample) -> Option<Vec<u8>> {
        unsafe {
            let buffer = sample.ConvertToContiguousBuffer().ok()?;
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            buffer.Lock(&mut ptr, None, Some(&mut len)).ok()?;
            let src = std::slice::from_raw_parts(ptr, len as usize);
            let row = self.width as usize * 4;
            let stride = self.stride.unsigned_abs() as usize;
            let mut out = vec![0u8; row * self.height as usize];
            if stride >= row && src.len() >= stride * (self.height as usize - 1) + row {
                for y in 0..self.height as usize {
                    let from = if self.stride >= 0 { y } else { self.height as usize - 1 - y } * stride;
                    let dst = &mut out[y * row..(y + 1) * row];
                    dst.copy_from_slice(&src[from..from + row]);
                    for px in dst.chunks_exact_mut(4) {
                        px[3] = 255;
                    }
                }
            }
            let _ = buffer.Unlock();
            Some(out)
        }
    }

    fn run(self, speed: f64, start: f64, latest: &Mutex<Option<Vec<u8>>>, paused: &AtomicBool, stop: &AtomicBool) {
        // Horloge de la boucle : `clock` correspond à l'horodatage `base` (100 ns).
        let restart = |base: &mut i64, clock: &mut Instant, at: f64| {
            *base = (at * 10_000_000.0) as i64;
            *clock = Instant::now();
        };
        let (mut base, mut clock) = (0i64, Instant::now());
        let mut loop_start = start;
        if loop_start > 0.0 && !self.seek(loop_start) {
            loop_start = 0.0;
        }
        restart(&mut base, &mut clock, loop_start);
        let mut frames_this_loop = 0u32;
        while !stop.load(Ordering::Relaxed) {
            if paused.load(Ordering::Relaxed) {
                let since = Instant::now();
                while paused.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                }
                clock += since.elapsed();
                continue;
            }
            let (mut flags, mut ts, mut sample) = (0u32, 0i64, None);
            if unsafe { self.reader.ReadSample(STREAM, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample)) }.is_err() {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                // Début de boucle au-delà de la fin de la vidéo : on repart de zéro.
                if frames_this_loop == 0 {
                    loop_start = 0.0;
                }
                frames_this_loop = 0;
                if !self.seek(loop_start) {
                    return;
                }
                restart(&mut base, &mut clock, loop_start);
                continue;
            }
            let Some(sample) = sample else { continue };
            frames_this_loop += 1;
            // Attente jusqu'à l'heure de cette image (par petits pas : arrêt et pause réactifs).
            let due = clock + Duration::from_secs_f64(((ts - base).max(0) as f64 / 10_000_000.0) / speed);
            while let Some(wait) = due.checked_duration_since(Instant::now()) {
                if stop.load(Ordering::Relaxed) || paused.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(wait.min(Duration::from_millis(50)));
            }
            if let Some(f) = self.frame(&sample) {
                *latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lecture réelle (Media Foundation) : `ISLAND_TEST_VIDEOS=a.mp4;b.webm cargo test video -- --ignored`.
    #[test]
    #[ignore]
    fn decodes_and_loops_real_videos() {
        let list = std::env::var("ISLAND_TEST_VIDEOS").expect("ISLAND_TEST_VIDEOS");
        for path in list.split(';') {
            let p = Player::open(Path::new(path), &Playback { speed: 2.0, start: 0.5 }).unwrap_or_else(|e| panic!("{path} : {e}"));
            assert_eq!((p.width, p.height), (640, 360), "{path}");
            // 2 s de vidéo, boucle dès 0,5 s, vitesse ×2 : un tour ≈ 0,75 s. Sur 2 s on doit
            // recevoir des images régulièrement (la boucle repart).
            let t0 = Instant::now();
            let mut frames = 0;
            while t0.elapsed() < Duration::from_secs(2) {
                if let Some(f) = p.take() {
                    assert_eq!(f.len(), 640 * 360 * 4);
                    assert!(f.chunks_exact(4).all(|px| px[3] == 255));
                    frames += 1;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            println!("{path} : {frames} images en 2 s");
            assert!(frames > 30, "{path} : seulement {frames} images");
            // Pause : plus aucune image.
            p.set_paused(true);
            std::thread::sleep(Duration::from_millis(300));
            let _ = p.take();
            std::thread::sleep(Duration::from_millis(500));
            assert!(p.take().is_none(), "{path} : des images pendant la pause");
        }
    }
}
