// Petites commandes « bureau » pour les outils intégrés : volume principal (lecture
// rapide, sans énumérer les sessions comme le mixer).
// Le mode Focus de Windows (`FocusSessionManager`) n'est pas utilisable : réservé par
// Microsoft aux apps packagées (« Feature … focussessionmanager … is not available »).
// La tuile Focus de la vue d'ensemble pilote donc le mode focus du notch.

#[derive(serde::Serialize)]
pub struct Volume {
    level: f32,
    muted: bool,
}

#[tauri::command]
pub async fn master_volume() -> Option<Volume> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(|| crate::services::system::master_volume())
        .await
        .ok()
        .flatten()
        .map(|(level, muted)| Volume { level, muted });
    #[allow(unreachable_code)]
    None
}
