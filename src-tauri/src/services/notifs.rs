// Notifications : raccourci natif vers le centre de notifications Windows. Island ne
// garde rien : la liste EST celle de Windows (`UserNotificationListener`), et fermer une
// notification ici la ferme dans Windows (et inversement).
//
// L'événement `NotificationChanged` n'existe que pour les apps packagées (MSIX) : on
// compare donc la liste toutes les 800 ms (ids seulement ; le détail n'est relu que si
// elle a changé). Événements émis :
// - `notifs://changed` : la liste a changé (le front relit `notifs_list`) ;
// - `notifs://new` (Notif) : notification arrivée depuis le dernier passage.
//
// Erreurs = codes courts : denied, unsupported, partial:<n> (n notifications restées après
// « Tout effacer »), ou le message HRESULT brut si Windows refuse la suppression.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notif {
    id: u32,
    app: String,
    /// AppUserModelId de l'app source (pour l'ouvrir).
    aumid: String,
    /// Logo de l'app (data URL), s'il est lisible.
    icon: Option<String>,
    title: String,
    body: String,
    /// Arrivée (ms depuis l'époque Unix).
    time: i64,
}

/// Service actif (outil Notifications activé dans les réglages).
static ENABLED: AtomicBool = AtomicBool::new(false);
static LIST: Mutex<Vec<Notif>> = Mutex::new(Vec::new());

fn list() -> std::sync::MutexGuard<'static, Vec<Notif>> {
    LIST.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut seen: Option<HashSet<u32>> = None;
        let mut icons: HashMap<String, Option<String>> = HashMap::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(800));
            if !ENABLED.load(Ordering::Relaxed) || !imp::allowed() {
                seen = None;
                continue;
            }
            let Some(ids) = imp::ids() else { continue };
            let ids_set: HashSet<u32> = ids.iter().copied().collect();
            if seen.as_ref() == Some(&ids_set) {
                continue;
            }
            let fresh: Vec<Notif> = imp::read(&mut icons);
            let first = seen.is_none();
            let new: Vec<Notif> = match &seen {
                Some(prev) => fresh.iter().filter(|n| !prev.contains(&n.id)).cloned().collect(),
                None => Vec::new(),
            };
            *list() = fresh;
            seen = Some(ids_set);
            let _ = app.emit("notifs://changed", ());
            if !first {
                // Plusieurs d'un coup : seule la plus récente s'affiche.
                if let Some(n) = new.into_iter().max_by_key(|n| n.time) {
                    let _ = app.emit("notifs://new", n);
                }
            }
        }
    });
}

#[derive(serde::Serialize)]
pub struct Status {
    /// allowed | denied | unspecified | unsupported
    access: &'static str,
}

#[tauri::command]
pub fn notifs_status() -> Status {
    Status { access: imp::access() }
}

/// Demande l'accès (boîte de dialogue Windows la première fois).
#[tauri::command]
pub async fn notifs_request_access() -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(|| Status { access: imp::request() }).await.map_err(|e| e.to_string())
}

/// Active / coupe la surveillance (outil activé ou non).
#[tauri::command]
pub fn notifs_set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        list().clear();
    }
}

/// Notifications actuelles du centre Windows, plus récentes d'abord.
#[tauri::command]
pub fn notifs_list() -> Vec<Notif> {
    let mut l = list().clone();
    l.sort_by(|a, b| b.time.cmp(&a.time));
    l
}

/// Ferme une notification (aussi dans le centre Windows). Hors du thread principal : les
/// appels WinRT bloquants (`.get()`) n'ont rien à faire sur le thread UI.
#[tauri::command]
pub async fn notifs_dismiss(id: u32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        imp::remove(id)?;
        list().retain(|n| n.id != id);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ferme toutes les notifications (comme « Tout effacer » de Windows). La liste en mémoire
/// n'est vidée que de ce qui a réellement disparu côté Windows.
#[tauri::command]
pub async fn notifs_clear() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let left = imp::clear()?;
        list().retain(|n| left.contains(&n.id));
        if left.is_empty() {
            Ok(())
        } else {
            Err(format!("partial:{}", left.len()))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ouvre l'app source.
#[tauri::command]
pub fn notifs_open(aumid: String) -> Result<(), String> {
    if aumid.is_empty() || aumid.contains(['"', '\0']) {
        return Err("notFound".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{aumid}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Ouvre la page Windows « Accès aux notifications » (accès refusé).
#[tauri::command]
pub fn notifs_open_settings() -> Result<(), String> {
    std::process::Command::new("explorer.exe").arg("ms-settings:privacy-notifications").spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
mod imp {
    use std::collections::HashMap;

    use base64::Engine as _;
    use windows::Foundation::Size;
    use windows::Storage::Streams::DataReader;
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

    use super::Notif;

    fn listener() -> Option<UserNotificationListener> {
        UserNotificationListener::Current().ok()
    }

    fn status_str(s: UserNotificationListenerAccessStatus) -> &'static str {
        match s {
            UserNotificationListenerAccessStatus::Allowed => "allowed",
            UserNotificationListenerAccessStatus::Denied => "denied",
            _ => "unspecified",
        }
    }

    pub fn access() -> &'static str {
        listener().and_then(|l| l.GetAccessStatus().ok()).map(status_str).unwrap_or("unsupported")
    }

    pub fn allowed() -> bool {
        access() == "allowed"
    }

    pub fn request() -> &'static str {
        listener()
            .and_then(|l| l.RequestAccessAsync().and_then(|op| op.get()).ok())
            .map(status_str)
            .unwrap_or("unsupported")
    }

    fn all() -> Option<Vec<UserNotification>> {
        let l = listener()?;
        let v = l.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.get()).ok()?;
        Some(v.into_iter().collect())
    }

    pub fn ids() -> Option<Vec<u32>> {
        Some(all()?.iter().filter_map(|n| n.Id().ok()).collect())
    }

    /// Logo de l'app (64 px) en data URL.
    fn logo(n: &UserNotification) -> Option<String> {
        let info = n.AppInfo().ok()?.DisplayInfo().ok()?;
        let stream = info.GetLogo(Size { Width: 64.0, Height: 64.0 }).ok()?.OpenReadAsync().ok()?.get().ok()?;
        let kind = stream.ContentType().map(|s| s.to_string()).unwrap_or_default();
        let len = stream.Size().ok()? as u32;
        if len == 0 || len > 512 * 1024 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream).ok()?;
        reader.LoadAsync(len).ok()?.get().ok()?;
        let mut buf = vec![0u8; len as usize];
        reader.ReadBytes(&mut buf).ok()?;
        let mime = if kind.starts_with("image/") { kind } else { "image/png".into() };
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
    }

    pub fn read(icons: &mut HashMap<String, Option<String>>) -> Vec<Notif> {
        let Some(list) = all() else { return Vec::new() };
        let binding = KnownNotificationBindings::ToastGeneric().ok();
        list.iter()
            .filter_map(|n| {
                let id = n.Id().ok()?;
                let info = n.AppInfo().ok();
                let aumid = info.as_ref().and_then(|a| a.AppUserModelId().ok()).map(|s| s.to_string()).unwrap_or_default();
                let app = info
                    .as_ref()
                    .and_then(|a| a.DisplayInfo().ok())
                    .and_then(|d| d.DisplayName().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let texts: Vec<String> = binding
                    .as_ref()
                    .and_then(|b| n.Notification().ok()?.Visual().ok()?.GetBinding(b).ok())
                    .and_then(|b| b.GetTextElements().ok())
                    .map(|els| {
                        els.into_iter()
                            .filter_map(|t| t.Text().ok().map(|s| s.to_string().trim().to_string()))
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                // 1601 → 1970, unités de 100 ns → ms.
                let time = n.CreationTime().map(|d| (d.UniversalTime - 116_444_736_000_000_000) / 10_000).unwrap_or(0);
                let icon = icons.entry(aumid.clone()).or_insert_with(|| logo(n)).clone();
                let mut it = texts.into_iter();
                Some(Notif { id, app, aumid, icon, title: it.next().unwrap_or_default(), body: it.collect::<Vec<_>>().join("\n"), time })
            })
            .collect()
    }

    pub fn remove(id: u32) -> Result<(), String> {
        listener().ok_or("unsupported")?.RemoveNotification(id).map_err(|e| e.to_string())
    }

    /// Supprime une à une (`RemoveNotification`, vérifié fiable hors paquet) plutôt que
    /// `ClearNotifications`, puis relit la liste. Renvoie les ids encore présents.
    pub fn clear() -> Result<Vec<u32>, String> {
        let l = listener().ok_or("unsupported")?;
        let before = ids().ok_or("unsupported")?;
        let mut last_err = None;
        let mut ok = 0usize;
        for id in &before {
            match l.RemoveNotification(*id) {
                Ok(()) => ok += 1,
                Err(e) => last_err = Some(e.to_string()),
            }
        }
        // Aucune suppression acceptée : on remonte l'erreur Windows telle quelle.
        if let (0, Some(e)) = (ok, last_err) {
            return Err(e);
        }
        let now = ids().ok_or("unsupported")?;
        Ok(before.into_iter().filter(|id| now.contains(id)).collect())
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::collections::HashMap;

    use super::Notif;

    pub fn access() -> &'static str {
        "unsupported"
    }
    pub fn allowed() -> bool {
        false
    }
    pub fn request() -> &'static str {
        "unsupported"
    }
    pub fn ids() -> Option<Vec<u32>> {
        None
    }
    pub fn read(_: &mut HashMap<String, Option<String>>) -> Vec<Notif> {
        Vec::new()
    }
    pub fn remove(_: u32) -> Result<(), String> {
        Err("unsupported".into())
    }
    pub fn clear() -> Result<Vec<u32>, String> {
        Err("unsupported".into())
    }
}
