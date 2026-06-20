// Launcher : actions système intégrées (les extensions s'ajoutent côté front).
// Cross-platform.

#[derive(serde::Serialize)]
pub struct LauncherAction {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    kind: &'static str,
    toggle: bool,
}

#[tauri::command]
pub fn list_launcher() -> Vec<LauncherAction> {
    vec![
        LauncherAction { id: "settings", label: "Réglages", icon: "settings", kind: "settings", toggle: false },
        LauncherAction { id: "dnd", label: "Ne pas déranger", icon: "moon", kind: "dnd", toggle: true },
    ]
}
