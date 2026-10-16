// Encodeur MP4 (H.264 / H.265 + AAC) par Media Foundation : MediaStreamSource (images
// BGRA brutes + PCM 16 bits) → MediaTranscoder (encodeur matériel, conversion de couleur
// et mise à l'échelle) → fichier.
//
// Adapté de `windows_capture::encoder::VideoEncoder`, qu'on n'utilise pas tel quel :
// - chaque envoi y ATTEND que le transcodeur ait consommé l'échantillon ; avec la vidéo et
//   l'audio envoyés depuis le même objet, un transcodeur qui réclame l'autre flux bloque
//   tout (interblocage) ;
// - son descripteur d'entrée audio déclare de l'AAC alors qu'il reçoit du PCM ;
// - pas de vérification `CanTranscode` (HEVC absent → échec muet).
// Ici les producteurs ne bloquent jamais : file vidéo bornée (image jetée si pleine, le
// transcodeur est en retard) et file audio non bornée (quelques Ko par bloc) ; le
// gestionnaire `SampleRequested` lit simplement la file du flux demandé.

use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::core::{Interface, HSTRING};
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Media::Core::{
    AudioStreamDescriptor, MediaStreamSample, MediaStreamSource, MediaStreamSourceSampleRequestedEventArgs,
    MediaStreamSourceStartingEventArgs, VideoStreamDescriptor,
};
use windows::Media::MediaProperties::{
    AudioEncodingProperties, ContainerEncodingProperties, MediaEncodingProfile, MediaEncodingSubtypes,
    VideoEncodingProperties,
};
use windows::Media::Transcoding::{MediaTranscoder, TranscodeFailureReason};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Storage::Streams::IRandomAccessStream;
use windows::Storage::{FileAccessMode, StorageFile};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use super::{Codec, TICKS_PER_SEC};

/// Échantillon prêt pour le transcodeur (horodatage et durée en 100 ns).
pub(crate) struct Sample {
    pub bytes: Arc<Vec<u8>>,
    pub ts: i64,
    pub dur: i64,
}

type Queue = Mutex<Receiver<Option<Sample>>>;

pub(crate) struct Config {
    /// Taille des images envoyées (BGRA, lignes de bas en haut).
    pub in_w: u32,
    pub in_h: u32,
    /// Taille encodée (≠ entrée : le transcodeur met à l'échelle).
    pub out_w: u32,
    pub out_h: u32,
    pub fps: u32,
    pub codec: Codec,
    pub bitrate: u32,
    /// Piste audio AAC 48 kHz stéréo (entrée PCM 16 bits).
    pub audio: bool,
}

pub(crate) struct Encoder {
    video: SyncSender<Option<Sample>>,
    audio: Option<Sender<Option<Sample>>>,
    transcode: Option<JoinHandle<Result<(), String>>>,
    /// Échec du transcodeur (préparation refusée : `codecUnavailable`…), lu par la pompe.
    failure: std::sync::Arc<Mutex<Option<String>>>,
    source: MediaStreamSource,
    stream: IRandomAccessStream,
    starting: i64,
    requested: i64,
}

/// Objets WinRT « agiles » (utilisables depuis n'importe quel fil) que `windows-rs` ne
/// marque pas `Send` : profil d'encodage, source et flux, confiés au fil du transcodeur.
struct Agile<T>(T);
unsafe impl<T> Send for Agile<T> {}
impl<T> Agile<T> {
    /// Déballe par une méthode : la closure capture alors `Agile` entier (`Send`), pas son
    /// contenu (capture fine de Rust 2021).
    fn into_inner(self) -> T {
        self.0
    }
}

fn werr(e: windows::core::Error) -> String {
    e.message().to_string()
}

/// Chemin Win32 classique (StorageFile refuse le préfixe `\\?\` de `canonicalize`).
fn plain_path(p: &std::path::Path) -> String {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

impl Encoder {
    pub(crate) fn new(path: &std::path::Path, c: &Config) -> Result<Self, String> {
        Self::build(path, c).map_err(|e| match e {
            BuildError::Win(e) => werr(e),
            BuildError::Msg(m) => m,
        })
    }

    fn build(path: &std::path::Path, c: &Config) -> Result<Self, BuildError> {
        // Profil de sortie.
        let profile = MediaEncodingProfile::new()?;
        let v = VideoEncodingProperties::new()?;
        v.SetSubtype(&HSTRING::from(match c.codec {
            Codec::H264 => "H264",
            Codec::Hevc => "HEVC",
        }))?;
        v.SetBitrate(c.bitrate)?;
        v.SetWidth(c.out_w)?;
        v.SetHeight(c.out_h)?;
        v.FrameRate()?.SetNumerator(c.fps)?;
        v.FrameRate()?.SetDenominator(1)?;
        v.PixelAspectRatio()?.SetNumerator(1)?;
        v.PixelAspectRatio()?.SetDenominator(1)?;
        profile.SetVideo(&v)?;
        if c.audio {
            profile.SetAudio(&AudioEncodingProperties::CreateAac(super::audio::RATE, 2, 192_000)?)?;
        }
        let container = ContainerEncodingProperties::new()?;
        container.SetSubtype(&HSTRING::from("MPEG4"))?;
        profile.SetContainer(&container)?;

        // Flux d'entrée : BGRA non compressé + PCM 16 bits.
        let vin = VideoEncodingProperties::CreateUncompressed(&MediaEncodingSubtypes::Bgra8()?, c.in_w, c.in_h)?;
        vin.FrameRate()?.SetNumerator(c.fps)?;
        vin.FrameRate()?.SetDenominator(1)?;
        let vdesc = VideoStreamDescriptor::Create(&vin)?;
        let source = if c.audio {
            let ain = AudioEncodingProperties::CreatePcm(super::audio::RATE, 2, 16)?;
            let adesc = AudioStreamDescriptor::Create(&ain)?;
            MediaStreamSource::CreateFromDescriptors(&vdesc, &adesc)?
        } else {
            MediaStreamSource::CreateFromDescriptor(&vdesc)?
        };
        source.SetBufferTime(TimeSpan::default())?;
        source.SetCanSeek(false)?;

        let starting = source.Starting(&TypedEventHandler::<MediaStreamSource, MediaStreamSourceStartingEventArgs>::new(
            |_, args| {
                if let Some(args) = args.as_ref() {
                    args.Request()?.SetActualStartPosition(TimeSpan { Duration: 0 })?;
                }
                Ok(())
            },
        ))?;

        // File vidéo bornée (~0,5 s, ≤ 256 Mo d'images) : au-delà, le transcodeur est en
        // retard et on jette des images plutôt que d'accumuler de la mémoire.
        let frame_bytes = (c.in_w as usize * c.in_h as usize * 4).max(1);
        let bound = ((c.fps / 2) as usize).min((256 << 20) / frame_bytes).max(2);
        let (video_tx, video_rx) = mpsc::sync_channel::<Option<Sample>>(bound);
        let (audio_tx, audio_rx) = if c.audio {
            let (t, r) = mpsc::channel::<Option<Sample>>();
            (Some(t), Some(r))
        } else {
            (None, None)
        };
        let video_q: Queue = Mutex::new(video_rx);
        let audio_q: Option<Queue> = audio_rx.map(Mutex::new);

        let requested = source.SampleRequested(&TypedEventHandler::<
            MediaStreamSource,
            MediaStreamSourceSampleRequestedEventArgs,
        >::new(move |_, args| {
            let Some(args) = args.as_ref() else { return Ok(()) };
            let req = args.Request()?;
            let is_audio = req.StreamDescriptor()?.cast::<AudioStreamDescriptor>().is_ok();
            let q = if is_audio { audio_q.as_ref() } else { Some(&video_q) };
            // Expéditeur disparu = fin de flux, comme un `None` explicite.
            let next = q.and_then(|q| q.lock().unwrap_or_else(|p| p.into_inner()).recv().ok().flatten());
            match next {
                Some(s) => {
                    let buf = CryptographicBuffer::CreateFromByteArray(&s.bytes)?;
                    let sample = MediaStreamSample::CreateFromBuffer(&buf, TimeSpan { Duration: s.ts })?;
                    sample.SetDuration(TimeSpan { Duration: s.dur })?;
                    req.SetSample(&sample)?;
                }
                None => req.SetSample(None)?,
            }
            Ok(())
        }))?;

        // Fichier de sortie.
        std::fs::File::create(path).map_err(|e| BuildError::Msg(e.to_string()))?;
        let full = std::fs::canonicalize(path).map_err(|e| BuildError::Msg(e.to_string()))?;
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(plain_path(&full)))?.get()?;
        let stream = file.OpenAsync(FileAccessMode::ReadWrite)?.get()?;

        let transcoder = MediaTranscoder::new()?;
        transcoder.SetHardwareAccelerationEnabled(true)?;
        // Préparation ET transcodage dans un fil à part : la préparation réclame déjà des
        // échantillons (`SampleRequested`), que seule la pompe — démarrée au retour de
        // `new` — peut fournir. Attendre la préparation ici bloquait tout pour toujours.
        let failure = std::sync::Arc::new(Mutex::new(None::<String>));
        let objs = Agile((transcoder, source.clone(), stream.clone(), profile));
        let fail = failure.clone();
        let transcode = std::thread::spawn(move || -> Result<(), String> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            let (transcoder, src, out, profile) = objs.into_inner();
            let run = || -> Result<(), String> {
                let prepared = transcoder.PrepareMediaStreamSourceTranscodeAsync(&src, &out, &profile).map_err(werr)?.get().map_err(werr)?;
                if !prepared.CanTranscode().map_err(werr)? {
                    let reason = prepared.FailureReason().map_err(werr)?;
                    return Err(if reason == TranscodeFailureReason::CodecNotFound { "codecUnavailable" } else { "encoderUnavailable" }.into());
                }
                prepared.TranscodeAsync().map_err(werr)?.get().map_err(werr)?;
                Ok(())
            };
            let r = run();
            if let Err(e) = &r {
                *fail.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.clone());
            }
            drop(transcoder);
            r
        });

        Ok(Self { video: video_tx, audio: audio_tx, transcode: Some(transcode), failure, source, stream, starting, requested })
    }

    /// Envoie une image ; `false` si elle a été jetée (transcodeur en retard).
    pub(crate) fn push_video(&self, s: Sample) -> bool {
        self.video.try_send(Some(s)).is_ok()
    }

    pub(crate) fn push_audio(&self, s: Sample) {
        if let Some(a) = &self.audio {
            let _ = a.send(Some(s));
        }
    }

    /// Le transcodeur s'est arrêté avant la fin (format refusé, erreur matérielle, disque
    /// plein…).
    pub(crate) fn failed(&self) -> bool {
        self.transcode.as_ref().is_some_and(|t| t.is_finished())
    }

    /// Raison de l'échec du transcodeur, si connue (`codecUnavailable`…).
    pub(crate) fn failure(&self) -> Option<String> {
        self.failure.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Fin des flux → le transcodeur écrit l'index (moov) et termine ; attend au plus
    /// `timeout` (le fichier serait illisible sans cette étape).
    pub(crate) fn finish(mut self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        // Fin de flux vidéo : la file peut être pleine le temps que le transcodeur la vide.
        loop {
            match self.video.try_send(None) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                Err(TrySendError::Full(_)) => {
                    if self.failed() || Instant::now() > deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        if let Some(a) = &self.audio {
            let _ = a.send(None);
        }
        let result = loop {
            match &self.transcode {
                Some(t) if t.is_finished() => {
                    break self.transcode.take().map(|t| t.join().unwrap_or_else(|_| Err("panic".into()))).unwrap_or(Ok(()));
                }
                Some(_) if Instant::now() > deadline => break Err("encoderTimeout".into()),
                Some(_) => std::thread::sleep(Duration::from_millis(10)),
                None => break Ok(()),
            }
        };
        let _ = self.source.RemoveStarting(self.starting);
        let _ = self.source.RemoveSampleRequested(self.requested);
        if let Ok(op) = self.stream.FlushAsync() {
            let _ = op.get();
        }
        let _ = self.stream.Close();
        result
    }
}

enum BuildError {
    Win(windows::core::Error),
    Msg(String),
}

impl From<windows::core::Error> for BuildError {
    fn from(e: windows::core::Error) -> Self {
        BuildError::Win(e)
    }
}

/// Durée d'une image (100 ns), pour `SetDuration`.
pub(crate) fn frame_duration(fps: u32) -> i64 {
    TICKS_PER_SEC / fps.max(1) as i64
}
