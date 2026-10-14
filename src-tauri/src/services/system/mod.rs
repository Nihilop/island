// Service `system` : capteurs de l'OS pour les outils intégrés (volume principal).
// L'implémentation native vit dans `windows.rs`.

#[cfg(target_os = "windows")]
mod windows;

/// Volume principal (niveau, muet), pour les outils intégrés.
#[cfg(target_os = "windows")]
pub(crate) fn master_volume() -> Option<(f32, bool)> {
    windows::volume()
}
