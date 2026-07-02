// Service `reserved` : capture des touches que l'OS RÉSERVE et que le plugin
// global-shortcut (RegisterHotKey) ne peut donc PAS enregistrer — la touche
// Windows SEULE en premier. Un hook clavier bas niveau (WH_KEYBOARD_LL, comme
// PowerToys / AutoHotkey — API légitime, PAS d'injection DLL) intercepte Win :
//   • frappe sèche (Win pressé puis relâché sans autre touche) → action Island ;
//   • Win + autre touche (Win+Maj+S, Win+D, Win+L…) → laissé INTACT au système.
// Gated par la permission `shortcuts` (déjà déclarée par les ext qui posent des
// raccourcis). Émet `reserved://key` { key } au déclenchement.

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;

/// Active/désactive la capture d'une touche réservée pour une extension.
/// `key` = identifiant logique ("Super"/"Win" = touche Windows).
#[tauri::command]
pub fn reserved_key_set(
    app: AppHandle,
    ext_id: String,
    key: String,
    enabled: bool,
) -> Result<(), String> {
    crate::ext::require_perm!(&app, &ext_id, "shortcuts");
    #[cfg(target_os = "windows")]
    {
        windows::set_key(&app, &key, enabled)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, &key, enabled);
        Err("touches réservées : Windows uniquement".into())
    }
}
