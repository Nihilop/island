// Service `tts` : synthèse vocale (lit un texte à voix haute). Non gardé (sortie
// audio, comme `notify`/`open_url`). Windows = SAPI ; autres OS = no-op pour l'instant.

#[cfg(target_os = "windows")]
mod windows;

#[tauri::command]
pub fn tts_speak(text: String) {
    #[cfg(target_os = "windows")]
    windows::speak(&text);
    #[cfg(not(target_os = "windows"))]
    let _ = text;
}
