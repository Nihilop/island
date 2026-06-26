// Audio : capture loopback (son système) + MICRO. Couche UNIQUE — `capture` la consomme
// (mux vidéo) et elle est exposée aux extensions (perm `audio`) pour enregistrer/transcrire.
// `media.rs` (SMTC + volume) est un AUTRE axe (piloter, pas capturer) → reste séparé.
// Contrat multi-OS : `start_capture(pcm) -> AudioCapture`. Windows = WASAPI ; mac/linux à venir.

use tauri::AppHandle;

use crate::ext::require_perm;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::{start_capture, AudioCapture};

/// Une piste audio enregistrée : PCM brut + format (l'extension décode/convertit/transcrit).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    path: String,
    sample_rate: u32,
    channels: u16,
    pcm: String,    // "f32le" | "s16le"
    source: String, // "mic" | "system"
}

// Registre des enregistrements actifs (id → captures en cours).
#[cfg(target_os = "windows")]
mod registry {
    use super::windows::AudioCapture;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;

    pub struct Rec {
        pub captures: Vec<(AudioCapture, String, &'static str)>, // (capture, pcm_path, source)
    }
    pub static RECS: Mutex<Option<HashMap<String, Rec>>> = Mutex::new(None);
    pub static NEXT: AtomicU64 = AtomicU64::new(1);
}

/// Démarre un enregistrement audio. `source` ∈ `mic` | `system` | `both`. Gated `audio`.
/// Renvoie un id d'enregistrement à passer à `audio_record_stop`.
#[tauri::command]
pub fn audio_record_start(app: AppHandle, ext_id: String, source: String) -> Result<String, String> {
    require_perm!(&app, &ext_id, "audio");
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        let want_mic = source == "mic" || source == "both";
        let want_sys = source == "system" || source == "both";
        if !want_mic && !want_sys {
            return Err("audio: source invalide (mic | system | both)".into());
        }
        let id = format!("rec{}", registry::NEXT.fetch_add(1, Ordering::Relaxed));
        let dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("island-audio")
            .join(&id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let mut captures = Vec::new();
        if want_sys {
            let p = dir.join("system.pcm").to_string_lossy().to_string();
            captures.push((windows::start_capture_source(p.clone(), false), p, "system"));
        }
        if want_mic {
            let p = dir.join("mic.pcm").to_string_lossy().to_string();
            captures.push((windows::start_capture_source(p.clone(), true), p, "mic"));
        }

        let mut g = registry::RECS.lock().unwrap_or_else(|e| e.into_inner());
        g.get_or_insert_with(Default::default).insert(id.clone(), registry::Rec { captures });
        Ok(id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = source;
        Err("audio: Windows uniquement".into())
    }
}

/// Arrête un enregistrement → renvoie ses pistes PCM (l'extension les convertit/transcrit).
/// Gated `audio`.
#[tauri::command]
pub fn audio_record_stop(
    app: AppHandle,
    ext_id: String,
    recording_id: String,
) -> Result<Vec<AudioTrack>, String> {
    require_perm!(&app, &ext_id, "audio");
    #[cfg(target_os = "windows")]
    {
        let rec = {
            let mut g = registry::RECS.lock().unwrap_or_else(|e| e.into_inner());
            g.as_mut().and_then(|m| m.remove(&recording_id))
        };
        let rec = rec.ok_or("audio: enregistrement inconnu")?;
        let mut tracks = Vec::new();
        for (cap, path, source) in rec.captures {
            let (sample_rate, channels, pcm) = match cap.stop() {
                Some(f) => (f.sample_rate, f.channels, f.pcm.to_string()),
                None => (0, 0, String::new()),
            };
            tracks.push(AudioTrack { path, sample_rate, channels, pcm, source: source.to_string() });
        }
        Ok(tracks)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = recording_id;
        Err("audio: Windows uniquement".into())
    }
}
