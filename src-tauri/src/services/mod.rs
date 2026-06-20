// Services exposés aux extensions via le SDK. Chaque service spécifique à l'OS est
// scindé en DEUX couches : l'interface + les commandes + le check de permission
// (cross-platform, ici) et l'implémentation native isolée dans un sous-module
// `windows.rs` derrière `#[cfg(target_os = "windows")]`. Porter sur macOS/Linux =
// déposer un `macos.rs`/`linux.rs` remplissant le même contrat, sans toucher aux
// commandes ni aux permissions.

pub mod apps;
pub mod audio;
pub mod capture;
pub mod clipboard;
pub mod input;
pub mod media;
pub mod net;
pub mod secrets;
pub mod system;
pub mod tts;
pub mod windows;
