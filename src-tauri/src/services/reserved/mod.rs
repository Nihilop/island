// Service `reserved` : capture des touches que l'OS RÉSERVE et que le plugin
// global-shortcut (RegisterHotKey) ne peut donc PAS enregistrer — la touche
// Windows SEULE en premier. Un hook clavier bas niveau (WH_KEYBOARD_LL, comme
// PowerToys / AutoHotkey — API légitime, PAS d'injection DLL) intercepte Win :
//   • frappe sèche (Win pressé puis relâché sans autre touche) → action Island ;
//   • Win + autre touche (Win+Maj+S, Win+D, Win+L…) → laissé INTACT au système.
// Utilisé par le launcher natif (ouverture par la touche Windows, cf. launcher/hotkey.rs).

#[cfg(target_os = "windows")]
pub(crate) mod windows;
