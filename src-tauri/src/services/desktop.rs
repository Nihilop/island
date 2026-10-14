// Petites commandes « bureau » pour les outils intégrés (non gardées, pas d'extension) :
//   - volume principal (lecture rapide, sans énumérer les sessions comme le mixer) ;
//   - mode Focus de Windows (`FocusSessionManager`, Windows 11 22H2+). Absent → la tuile
//     de la vue d'ensemble est désactivée.

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

/// `None` = mode Focus non pris en charge sur ce Windows.
#[tauri::command]
pub async fn focus_state() -> Option<bool> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(|| {
        use windows::UI::Shell::FocusSessionManager;
        if !FocusSessionManager::IsSupported().ok()? {
            return None;
        }
        FocusSessionManager::GetDefault().ok()?.IsFocusActive().ok()
    })
    .await
    .ok()
    .flatten();
    #[allow(unreachable_code)]
    None
}

#[tauri::command]
pub async fn focus_set(on: bool) -> Result<(), String> {
    let _ = on;
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(move || {
        use windows::UI::Shell::FocusSessionManager;
        let m = FocusSessionManager::GetDefault().map_err(|e| e.to_string())?;
        if on {
            m.TryStartFocusSession().map_err(|e| e.to_string())?;
        } else {
            m.DeactivateFocus().map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    Err("non supporté".into())
}
