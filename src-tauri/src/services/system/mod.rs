// Service `system` : stats système + capteurs (batterie, réseau, volume, inactivité).
// La couche commande + permission est cross-platform (`sysinfo` pour les stats) ; les
// capteurs spécifiques OS vivent dans `windows.rs`. Tout est gardé par la permission
// `system`.

use std::sync::Mutex;

use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysStats {
    cpu: f32,        // usage CPU global (%)
    cores: Vec<f32>, // usage par cœur (%)
    mem_used: u64,   // octets
    mem_total: u64,  // octets
}

/// État de la batterie (absent = pas de batterie / poste fixe).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    percent: u8,
    charging: bool,
}

/// Volume MAÎTRE du périphérique de sortie (distinct de `media` qui pilote l'app média).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    level: f32, // 0.0–1.0
    muted: bool,
}

// System persistant : l'usage CPU est un delta depuis le précédent refresh.
static SYS: Mutex<Option<sysinfo::System>> = Mutex::new(None);

/// Snapshot des stats système (l'extension Monitoring poll à son rythme). Gated `system`.
#[tauri::command]
pub fn system_stats(app: AppHandle, ext_id: String) -> SysStats {
    if !crate::ext::ext_has_permission(&app, &ext_id, "system") {
        return SysStats { cpu: 0.0, cores: Vec::new(), mem_used: 0, mem_total: 0 };
    }
    let mut guard = SYS.lock().unwrap_or_else(|p| p.into_inner());
    let sys = guard.get_or_insert_with(sysinfo::System::new);
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let cores: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu = if cores.is_empty() {
        0.0
    } else {
        cores.iter().sum::<f32>() / cores.len() as f32
    };
    SysStats {
        cpu,
        cores,
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
    }
}

/// État de la batterie, ou `None` (poste fixe / inconnu). Gated `system`.
#[tauri::command]
pub fn system_battery(app: AppHandle, ext_id: String) -> Option<Battery> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "system") {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        windows::battery().map(|(percent, charging)| Battery { percent, charging })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Connecté à un réseau ? Gated `system`.
#[tauri::command]
pub fn system_online(app: AppHandle, ext_id: String) -> bool {
    if !crate::ext::ext_has_permission(&app, &ext_id, "system") {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        windows::online()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Millisecondes depuis la dernière entrée clavier/souris (0 = activité à l'instant,
/// ou indisponible). Gated `system`.
#[tauri::command]
pub fn system_idle_ms(app: AppHandle, ext_id: String) -> u64 {
    if !crate::ext::ext_has_permission(&app, &ext_id, "system") {
        return 0;
    }
    #[cfg(target_os = "windows")]
    {
        windows::idle_ms()
    }
    #[cfg(not(target_os = "windows"))]
    {
        0
    }
}

/// Volume maître du périphérique de sortie, ou `None`. Gated `system`.
#[tauri::command]
pub fn system_volume(app: AppHandle, ext_id: String) -> Option<Volume> {
    if !crate::ext::ext_has_permission(&app, &ext_id, "system") {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        windows::volume().map(|(level, muted)| Volume { level, muted })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Règle le volume maître (0.0–1.0). Gated `system`.
#[tauri::command]
pub fn system_set_volume(app: AppHandle, ext_id: String, level: f32) -> Result<(), String> {
    crate::ext::require_perm!(&app, &ext_id, "system");
    #[cfg(target_os = "windows")]
    {
        windows::set_volume(level)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = level;
        Err("system: Windows uniquement".into())
    }
}

/// Coupe / rétablit le son maître. Gated `system`.
#[tauri::command]
pub fn system_set_muted(app: AppHandle, ext_id: String, muted: bool) -> Result<(), String> {
    crate::ext::require_perm!(&app, &ext_id, "system");
    #[cfg(target_os = "windows")]
    {
        windows::set_muted(muted)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = muted;
        Err("system: Windows uniquement".into())
    }
}

/// Action d'alimentation : `shutdown` | `restart` | `sleep` | `hibernate` | `lock` | `logoff`.
/// Gardé par la permission DÉDIÉE `power` (éteindre/redémarrer = fort impact → consentement
/// explicite, séparé de `system`).
#[tauri::command]
pub fn system_power(app: AppHandle, ext_id: String, action: String) -> Result<(), String> {
    crate::ext::require_perm!(&app, &ext_id, "power");
    #[cfg(target_os = "windows")]
    {
        windows::power(&action)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = action;
        Err("power: Windows uniquement".into())
    }
}
