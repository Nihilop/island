// Son des enregistrements : son du PC (loopback WASAPI de la sortie par défaut) et/ou
// micro (entrée par défaut), ramenés en 48 kHz stéréo, mélangés (somme écrêtée) en PCM
// 16 bits pour l'encodeur AAC.
//
// Synchronisation : chaque paquet WASAPI porte l'horloge QPC de sa première trame (unités
// de 100 ns, même base que l'horloge vidéo) → on le place sur la ligne de temps de
// l'enregistrement. Un trou (le loopback n'émet RIEN quand rien ne joue) est comblé par du
// silence ; des données en retard sont jetées. Le mixeur lit la ligne de temps à pas fixe
// avec un léger retard (le temps que les paquets arrivent) : ce qui manque est du silence.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::collections::VecDeque;
use std::sync::Mutex;

/// Fréquence de sortie (AAC).
pub(crate) const RATE: u32 = 48_000;
/// Trou maximal comblé par du silence ; au-delà, on repart de la position du paquet.
const MAX_GAP: u64 = RATE as u64 * 5;
/// Avance tolérée d'une source sur son horloge (gigue) avant de jeter l'excédent.
const TOLERANCE: u64 = RATE as u64 / 10;

/// Rééchantillonneur linéaire en flux (stéréo f32). Suffisant pour de la voix et du son
/// système en capture d'écran ; l'état (position fractionnaire, dernière trame) traverse
/// les paquets pour éviter les clics aux jointures.
pub(crate) struct Resampler {
    /// Pas en trames d'entrée par trame de sortie.
    step: f64,
    /// Position de la prochaine sortie, relative au début du paquet courant
    /// (-1 ≤ pos : l'indice -1 désigne la dernière trame du paquet précédent).
    pos: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub(crate) fn new(in_rate: u32, out_rate: u32) -> Self {
        Self { step: in_rate.max(1) as f64 / out_rate.max(1) as f64, pos: 0.0, prev: [0.0; 2] }
    }

    pub(crate) fn process(&mut self, input: &[[f32; 2]], out: &mut Vec<[f32; 2]>) {
        let len = input.len();
        if len == 0 {
            return;
        }
        let at = |i: isize| if i < 0 { self.prev } else { input[i as usize] };
        loop {
            let i = self.pos.floor();
            if i + 1.0 > (len - 1) as f64 {
                break;
            }
            let frac = (self.pos - i) as f32;
            let (a, b) = (at(i as isize), at(i as isize + 1));
            out.push([a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]);
            self.pos += self.step;
        }
        self.pos -= len as f64;
        self.prev = input[len - 1];
    }
}

/// Ligne de temps d'une source (48 kHz stéréo), indexée en trames depuis le départ.
#[derive(Default)]
pub(crate) struct Timeline {
    /// Indice absolu de la première trame du tampon.
    base: u64,
    buf: VecDeque<[f32; 2]>,
}

impl Timeline {
    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    /// Écrit `data` à la position absolue `pos` (négative = avant le départ).
    pub(crate) fn write(&mut self, pos: i64, data: &[[f32; 2]]) {
        let mut data = data;
        // Avant le départ, ou déjà lu par le mixeur (qui y a mis du silence) : inutile.
        // Le jeter (plutôt que l'ajouter en bout) évite un décalage permanent après un à-coup.
        let mut pos = pos;
        if pos < self.base as i64 {
            let skip = (self.base as i64 - pos) as usize;
            if skip >= data.len() {
                return;
            }
            data = &data[skip..];
            pos = self.base as i64;
        }
        let pos = pos as u64;
        let end = self.end();
        if pos > end {
            let gap = pos - end;
            if gap <= MAX_GAP {
                self.buf.extend(std::iter::repeat_n([0.0; 2], gap as usize));
            } else {
                self.buf.clear();
                self.base = pos;
            }
        } else if end - pos > TOLERANCE {
            // Données en retard (déjà lues ou source trop rapide) : on jette le recouvrement.
            let over = (end - pos) as usize;
            if over >= data.len() {
                return;
            }
            data = &data[over..];
        }
        self.buf.extend(data.iter().copied());
    }

    /// Lit `n` trames à partir de `pos` (lecture séquentielle) ; ce qui manque = silence.
    pub(crate) fn read(&mut self, pos: u64, n: usize, out: &mut [[f32; 2]]) {
        if self.base < pos {
            let drop = ((pos - self.base) as usize).min(self.buf.len());
            self.buf.drain(..drop);
            self.base = pos;
        }
        let lead = (self.base.saturating_sub(pos) as usize).min(n);
        for (i, o) in out.iter_mut().take(n).enumerate() {
            *o = if i < lead { [0.0; 2] } else { self.buf.pop_front().unwrap_or([0.0; 2]) };
        }
        self.base = self.base.max(pos + n as u64);
        // Si le tampon était plus court que la lecture, `base` a dépassé la fin : rien à garder.
        if self.buf.is_empty() {
            self.base = pos + n as u64;
        }
    }
}

/// Source partagée entre son thread de capture et le mixeur.
#[derive(Default)]
pub(crate) struct Source {
    pub(crate) line: Mutex<Timeline>,
}

/// Mélange (somme écrêtée) de `n` trames de chaque source → PCM 16 bits stéréo LE.
pub(crate) fn mix(sources: &[&[[f32; 2]]], n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        for c in 0..2 {
            let s: f32 = sources.iter().map(|src| src.get(i).map_or(0.0, |f| f[c])).sum();
            let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Mixeur : lit toutes les sources à la même position et produit des blocs PCM.
pub(crate) struct Mixer {
    sources: Vec<std::sync::Arc<Source>>,
    pos: u64,
    scratch: Vec<Vec<[f32; 2]>>,
}

impl Mixer {
    pub(crate) fn new(sources: Vec<std::sync::Arc<Source>>) -> Self {
        let scratch = sources.iter().map(|_| Vec::new()).collect();
        Self { sources, pos: 0, scratch }
    }

    /// Position (trames) du prochain bloc.
    pub(crate) fn pos(&self) -> u64 {
        self.pos
    }

    /// Bloc suivant de `n` trames : (position de départ, PCM).
    pub(crate) fn next(&mut self, n: usize) -> (u64, Vec<u8>) {
        for (src, buf) in self.sources.iter().zip(self.scratch.iter_mut()) {
            buf.resize(n, [0.0; 2]);
            src.line.lock().unwrap_or_else(|p| p.into_inner()).read(self.pos, n, buf);
        }
        let views: Vec<&[[f32; 2]]> = self.scratch.iter().map(|b| b.as_slice()).collect();
        let pcm = mix(&views, n);
        let at = self.pos;
        self.pos += n as u64;
        (at, pcm)
    }
}

// --- Capture WASAPI ------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub(crate) use imp::{start, Kind};

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

    use super::{Resampler, Source, RATE};

    const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;
    const WAVE_FORMAT_PCM: u16 = 1;
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    #[derive(Clone, Copy, Debug)]
    pub(crate) enum Kind {
        /// Son du PC (ce qui sort des haut-parleurs).
        Loopback,
        /// Micro (entrée par défaut).
        Mic,
    }

    #[derive(Clone, Copy)]
    enum Sample {
        F32,
        I16,
        I32,
    }

    /// Lance la capture d'une source vers `src` ; `t0` = départ de l'enregistrement (QPC,
    /// 100 ns). S'arrête quand `stop` passe à vrai. Un périphérique absent ou en erreur
    /// termine le thread : la source reste muette (silence), l'enregistrement continue.
    pub(crate) fn start(kind: Kind, src: Arc<Source>, t0: i64, stop: Arc<AtomicBool>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            if let Err(e) = unsafe { run(kind, &src, t0, &stop) } {
                eprintln!("[capture] audio {kind:?} : {e}");
            }
        })
    }

    unsafe fn format(pwfx: *const WAVEFORMATEX) -> Result<(usize, u32, usize, Sample), String> {
        let wf = std::ptr::read_unaligned(pwfx);
        let tag = wf.wFormatTag;
        // WAVEFORMATEXTENSIBLE : SubFormat.Data1 (offset 24) = 1 (PCM entier) / 3 (flottant).
        let sub = if tag == WAVE_FORMAT_EXTENSIBLE && wf.cbSize >= 22 {
            std::ptr::read_unaligned((pwfx as *const u8).add(24) as *const u32) as u16
        } else {
            tag
        };
        let bits = wf.wBitsPerSample;
        let sample = match (sub, bits) {
            (WAVE_FORMAT_IEEE_FLOAT, 32) => Sample::F32,
            (WAVE_FORMAT_PCM, 16) => Sample::I16,
            (WAVE_FORMAT_PCM, 32) => Sample::I32,
            _ => return Err(format!("format audio non géré ({sub}, {bits} bits)")),
        };
        Ok((wf.nChannels.max(1) as usize, wf.nSamplesPerSec, wf.nBlockAlign as usize, sample))
    }

    unsafe fn run(kind: Kind, src: &Source, t0: i64, stop: &AtomicBool) -> Result<(), String> {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let flow = match kind {
            Kind::Loopback => eRender,
            Kind::Mic => eCapture,
        };
        let device = e.GetDefaultAudioEndpoint(flow, eConsole).map_err(|e| e.to_string())?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
        let pwfx = client.GetMixFormat().map_err(|e| e.to_string())?;
        let fmt = format(pwfx);
        let flags = match kind {
            Kind::Loopback => AUDCLNT_STREAMFLAGS_LOOPBACK,
            Kind::Mic => 0,
        };
        // Tampon de 200 ms : large marge pour une lecture toutes les 10 ms.
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, pwfx, None);
        CoTaskMemFree(Some(pwfx as *const _));
        let (channels, rate, block, sample) = fmt?;
        init.map_err(|e| e.to_string())?;
        let cap: IAudioCaptureClient = client.GetService().map_err(|e| e.to_string())?;
        client.Start().map_err(|e| e.to_string())?;

        let mut rs = Resampler::new(rate, RATE);
        let mut frames_in: Vec<[f32; 2]> = Vec::new();
        let mut frames_out: Vec<[f32; 2]> = Vec::new();
        let read = |p: *const u8, i: usize| -> f32 {
            match sample {
                Sample::F32 => *(p as *const f32).add(i),
                Sample::I16 => *(p as *const i16).add(i) as f32 / 32768.0,
                Sample::I32 => *(p as *const i32).add(i) as f32 / 2_147_483_648.0,
            }
        };

        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            loop {
                let packet = match cap.GetNextPacketSize() {
                    Ok(p) => p,
                    Err(e) => return Err(e.to_string()),
                };
                if packet == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let (mut n, mut fl, mut qpc) = (0u32, 0u32, 0u64);
                cap.GetBuffer(&mut data, &mut n, &mut fl, None, Some(&mut qpc)).map_err(|e| e.to_string())?;
                let silent = fl & AUDCLNT_BUFFERFLAGS_SILENT != 0 || data.is_null();
                frames_in.clear();
                for f in 0..n as usize {
                    if silent {
                        frames_in.push([0.0; 2]);
                        continue;
                    }
                    let p = data.add(f * block) as *const u8;
                    // Mono → dupliqué ; multicanal → avant gauche / avant droit.
                    let l = read(p, 0);
                    let r = if channels > 1 { read(p, 1) } else { l };
                    frames_in.push([l, r]);
                }
                let _ = cap.ReleaseBuffer(n);
                frames_out.clear();
                rs.process(&frames_in, &mut frames_out);
                // Position de la 1ʳᵉ trame sur la ligne de temps (QPC en 100 ns).
                let pos = (qpc as i64 - t0) as i128 * RATE as i128 / super::super::TICKS_PER_SEC as i128;
                src.line.lock().unwrap_or_else(|p| p.into_inner()).write(pos as i64, &frames_out);
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let _ = client.Stop();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_counts() {
        // Même fréquence : identité (une trame de retard à la jointure).
        let mut r = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        let input: Vec<[f32; 2]> = (0..480).map(|i| [i as f32, -(i as f32)]).collect();
        r.process(&input, &mut out);
        r.process(&input, &mut out);
        assert_eq!(out.len(), 959);
        assert_eq!(out[10], [10.0, -10.0]);
        // 44,1 → 48 kHz : ~48 000 trames pour 1 s, rampe interpolée exactement.
        let mut r = Resampler::new(44_100, 48_000);
        let mut out = Vec::new();
        let ramp: Vec<[f32; 2]> = (0..44_100).map(|i| [i as f32, 0.0]).collect();
        for chunk in ramp.chunks(441) {
            r.process(chunk, &mut out);
        }
        assert!((47_990..=48_000).contains(&out.len()), "{}", out.len());
        let step = 44_100.0 / 48_000.0;
        for (k, f) in out.iter().enumerate().step_by(997) {
            assert!((f[0] - k as f32 * step as f32).abs() < 0.05, "{k}: {}", f[0]);
        }
        // 96 → 48 kHz : moitié.
        let mut r = Resampler::new(96_000, 48_000);
        let mut out = Vec::new();
        r.process(&vec![[0.5; 2]; 9600], &mut out);
        assert!((4799..=4800).contains(&out.len()));
        assert!(out.iter().all(|f| f[0] == 0.5));
    }

    #[test]
    fn mixer_clips() {
        let a = [[0.75f32, -0.75], [0.25, 0.0]];
        let b = [[0.75f32, -0.75]];
        let pcm = mix(&[&a, &b], 2);
        let s: Vec<i16> = pcm.chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        // Somme écrêtée, source courte complétée par du silence.
        assert_eq!(s, vec![32767, -32767, 8192, 0]);
        assert_eq!(mix(&[], 3), vec![0u8; 12]);
    }

    #[test]
    fn timeline_gap_and_late() {
        let mut t = Timeline::default();
        let one = [[1.0f32; 2]; 10];
        t.write(0, &one);
        // Trou de 10 trames (loopback muet) → silence inséré.
        t.write(20, &one);
        let mut out = vec![[9.0f32; 2]; 30];
        t.read(0, 30, &mut out);
        assert!(out[..10].iter().all(|f| f[0] == 1.0));
        assert!(out[10..20].iter().all(|f| f[0] == 0.0));
        assert!(out[20..].iter().all(|f| f[0] == 1.0));
        // Lecture au-delà des données : silence, puis des données en retard sont jetées.
        let mut out = vec![[9.0f32; 2]; 10];
        t.read(30, 10, &mut out);
        assert!(out.iter().all(|f| f[0] == 0.0));
        t.write(35 - 5000, &one); // très en retard → ignoré
        t.write(-3, &one); // avant le départ / déjà lu → ignoré
        let mut out = vec![[9.0f32; 2]; 10];
        t.read(40, 10, &mut out);
        assert!(out.iter().all(|f| f[0] == 0.0), "{out:?}");
        // Données à l'heure après la lecture : bien placées.
        t.write(52, &one);
        let mut out = vec![[9.0f32; 2]; 12];
        t.read(50, 12, &mut out);
        assert!(out[..2].iter().all(|f| f[0] == 0.0));
        assert!(out[2..].iter().all(|f| f[0] == 1.0));
    }

    #[test]
    fn mixer_sequential_blocks() {
        let a = std::sync::Arc::new(Source::default());
        a.line.lock().unwrap().write(0, &[[0.5; 2]; 1000]);
        let b = std::sync::Arc::new(Source::default()); // muette (loopback silencieux)
        let mut m = Mixer::new(vec![a, b]);
        let (p0, pcm) = m.next(960);
        assert_eq!((p0, pcm.len()), (0, 960 * 4));
        let (p1, pcm) = m.next(960);
        assert_eq!(p1, 960);
        let first = i16::from_le_bytes([pcm[0], pcm[1]]);
        let last = i16::from_le_bytes([pcm[pcm.len() - 2], pcm[pcm.len() - 1]]);
        assert_eq!((first, last), (16384, 0));
        assert_eq!(m.pos(), 1920);
    }
}
