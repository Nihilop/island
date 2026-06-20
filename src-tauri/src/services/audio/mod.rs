// Audio loopback système (capture du son du bureau pour l'enregistrement vidéo).
// Pas de commande exposée : utilisé en interne par le service `capture`.
// Contrat multi-OS : `start_capture(pcm_path) -> AudioCapture`, `AudioCapture::stop()
// -> Option<AudioFormat>`. Windows = WASAPI loopback ; macOS/Linux = à venir.

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::{start_capture, AudioCapture};
