// Service `input` : automatisation d'entrée clavier (tape du texte dans l'application
// active). ⚠ Palier de confiance ÉLEVÉ — peut écrire dans n'importe quelle app → gardé
// par la permission `input` (affichée en évidence à l'install). Windows = SendInput.

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;

/// Tape `text` dans l'application qui a le focus (saisie Unicode). Gated `input`.
#[tauri::command]
pub fn input_type_text(app: AppHandle, ext_id: String, text: String) -> Result<(), String> {
    crate::ext::require_perm!(&app, &ext_id, "input");
    #[cfg(target_os = "windows")]
    {
        windows::type_text(&text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        Err("input: Windows uniquement".into())
    }
}
