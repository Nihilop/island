// Applications : réutilise l'énumération du service `apps` (AppsFolder = Win32 + Store,
// repli sur les raccourcis du menu Démarrer, + jeux Steam).

use crate::launcher::model::{Item, Kind};

#[cfg(target_os = "windows")]
pub fn items() -> Vec<Item> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    let list = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let l = crate::services::apps::windows::list_apps();
        CoUninitialize();
        l
    };
    list.into_iter()
        .map(|a| {
            let steam = a.path.starts_with("steam://");
            Item {
                id: format!("app:{}", a.path.to_lowercase()),
                kind: Kind::App,
                title: a.name,
                subtitle: String::new(),
                keywords: if steam { "steam jeu game".into() } else { String::new() },
                target: a.path,
            }
        })
        .collect()
}

#[cfg(not(target_os = "windows"))]
pub fn items() -> Vec<Item> {
    Vec::new()
}
