// Launcher natif d'Island (dans le notch, hors dock). Domaine autonome :
// - `sources/` : ce qu'on indexe (apps, réglages Windows, fichiers) ;
// - `model` / `normalize` / `matcher` / `rank` : le moteur de recherche (Rust, < 10 ms) ;
// - `inline` : réponses directes (calculatrice, URL).
//
// Index en mémoire derrière un `RwLock<Arc<…>>` : une reconstruction (au démarrage, puis
// à l'ouverture du launcher si l'index a plus de 5 min) construit un nouvel index en fond
// et l'échange d'un coup ; la recherche ne bloque jamais. Instantané sur disque pour un
// démarrage à chaud, usage (fréquence/récence) persisté à chaque ouverture.
// Everything (s'il tourne) complète les fichiers au moment de la recherche.

pub mod hotkey;
mod inline;
mod matcher;
mod model;
mod normalize;
pub mod processes;
mod rank;
mod sources;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use model::{Item, Kind, Prepared};
use rank::{Scope, Usage, UsageMap};

struct Index {
    items: Vec<Prepared>,
    built: Instant,
}

static INDEX: RwLock<Option<Arc<Index>>> = RwLock::new(None);
static USAGE: Mutex<Option<UsageMap>> = Mutex::new(None);
static LANG: Mutex<String> = Mutex::new(String::new());
static BUILDING: AtomicBool = AtomicBool::new(false);
static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
/// Résultats Everything de la dernière recherche (hors index), pour les ouvrir par id.
static EXTRA: Mutex<Vec<Item>> = Mutex::new(Vec::new());

const STALE: Duration = Duration::from_secs(5 * 60);

fn snapshot_path(app: &AppHandle) -> PathBuf {
    app.path().app_cache_dir().unwrap_or_default().join("launcher-index.json")
}

fn usage_path(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("launcher-usage.json")
}

fn index() -> Option<Arc<Index>> {
    INDEX.read().unwrap_or_else(|p| p.into_inner()).clone()
}

fn swap(items: Vec<Item>) {
    let idx = Index { items: items.into_iter().map(Prepared::new).collect(), built: Instant::now() };
    *INDEX.write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(idx));
}

fn lang() -> String {
    let l = LANG.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if l.is_empty() { "fr".into() } else { l }
}

fn with_usage<T>(f: impl FnOnce(&mut UsageMap) -> T) -> T {
    let mut g = USAGE.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(UsageMap::new))
}

/// Démarrage : instantané disque (résultats immédiats) puis reconstruction en fond.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        if let Ok(txt) = std::fs::read_to_string(usage_path(&app)) {
            if let Ok(u) = serde_json::from_str::<UsageMap>(&txt) {
                *USAGE.lock().unwrap_or_else(|p| p.into_inner()) = Some(u);
            }
        }
        if let Ok(txt) = std::fs::read_to_string(snapshot_path(&app)) {
            if let Ok(items) = serde_json::from_str::<Vec<Item>>(&txt) {
                swap(items);
            }
        }
        rebuild(&app);
    });
}

/// Reconstruit l'index complet (apps et fichiers en parallèle), l'échange et l'enregistre.
fn rebuild(app: &AppHandle) {
    if BUILDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let files = std::thread::spawn(sources::files::items);
    let mut items = sources::apps::items();
    items.extend(sources::settings::items(&lang()));
    items.extend(files.join().unwrap_or_default());
    let mut seen = HashSet::new();
    items.retain(|i| seen.insert(i.id.clone()));
    if let Ok(json) = serde_json::to_string(&items) {
        let p = snapshot_path(app);
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, json);
    }
    swap(items);
    BUILDING.store(false, Ordering::SeqCst);
    let _ = app.emit("launcher://index", ());
}

fn rebuild_in_background(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || rebuild(&app));
}

// --- Commandes ----------------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    id: String,
    kind: Kind,
    title: String,
    subtitle: String,
    /// Chemin (fichiers, dossiers, apps Win32) : copier, révéler.
    path: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    seq: u64,
    items: Vec<Hit>,
    /// Résultat de calculatrice.
    calc: Option<String>,
    /// URL tapée, prête à ouvrir.
    url: Option<String>,
    /// Index pas encore prêt (premier démarrage).
    indexing: bool,
}

fn hit(i: &Item) -> Hit {
    let is_path = matches!(i.kind, Kind::File | Kind::Folder) || (i.kind == Kind::App && i.target.contains(":\\"));
    Hit {
        id: i.id.clone(),
        kind: i.kind,
        title: i.title.clone(),
        subtitle: i.subtitle.clone(),
        path: is_path.then(|| i.target.clone()),
    }
}

/// Préfixes de portée : `a:` apps, `f:` fichiers, `d:` dossiers, `s:` réglages.
fn parse_scope(query: &str) -> (Scope, &str) {
    let q = query.trim_start();
    for (p, k) in [("a:", Kind::App), ("f:", Kind::File), ("d:", Kind::Folder), ("s:", Kind::Setting)] {
        if let Some(rest) = q.strip_prefix(p) {
            return (Scope::Only(k), rest.trim());
        }
    }
    (Scope::All, q.trim())
}

#[tauri::command]
pub async fn launcher_search(query: String, limit: Option<usize>, seq: u64) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let limit = limit.unwrap_or(40).clamp(1, 100);
        let (scope, raw) = parse_scope(&query);
        let q = normalize::normalize(raw);
        let idx = index();
        let usage = USAGE.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or_default();
        let mut items: Vec<Hit> = match &idx {
            Some(idx) => rank::top(&idx.items, &q, scope, &usage, limit).into_iter().map(|(i, _)| hit(&idx.items[i].item)).collect(),
            None => Vec::new(),
        };
        // Everything : fichiers hors des dossiers indexés, à la suite.
        let wants_files = matches!(scope, Scope::All | Scope::Only(Kind::File) | Scope::Only(Kind::Folder));
        let mut extra = Vec::new();
        #[cfg(target_os = "windows")]
        if wants_files && raw.chars().count() >= 3 && items.len() < limit {
            if let Some(found) = crate::services::apps::files::everything_search(raw, 15) {
                let have: HashSet<String> = items.iter().map(|h| h.id.clone()).collect();
                for f in found {
                    let Some(it) = sources::files::item(std::path::Path::new(&f.path), f.is_dir) else { continue };
                    if scope != Scope::All && Scope::Only(it.kind) != scope {
                        continue;
                    }
                    if !have.contains(&it.id) && items.len() < limit {
                        items.push(hit(&it));
                        extra.push(it);
                    }
                }
            }
        }
        let _ = wants_files;
        *EXTRA.lock().unwrap_or_else(|p| p.into_inner()) = extra;
        let calc = if scope == Scope::All { inline::calc(raw) } else { None };
        let url = if scope == Scope::All && calc.is_none() { inline::url(raw) } else { None };
        Ok(Response { seq, items, calc, url, indexing: idx.is_none() })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn find(id: &str) -> Option<Item> {
    if let Some(idx) = index() {
        if let Some(p) = idx.items.iter().find(|p| p.item.id == id) {
            return Some(p.item.clone());
        }
    }
    EXTRA.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|i| i.id == id).cloned()
}

fn record(app: &AppHandle, id: &str) {
    let json = with_usage(|u| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let e = u.entry(id.to_string()).or_insert(Usage::default());
        e.count += 1;
        e.last = now;
        // Ménage : on garde les 2 000 éléments les plus récents.
        if u.len() > 2000 {
            let mut v: Vec<(String, i64)> = u.iter().map(|(k, x)| (k.clone(), x.last)).collect();
            v.sort_by_key(|x| std::cmp::Reverse(x.1));
            for (k, _) in v.into_iter().skip(2000) {
                u.remove(&k);
            }
        }
        serde_json::to_string(&*u).ok()
    });
    if let Some(json) = json {
        let p = usage_path(app);
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, json);
    }
}

/// Ouvre un résultat. `action` : open | admin | reveal.
#[tauri::command]
pub async fn launcher_open(app: AppHandle, id: String, action: Option<String>) -> Result<(), String> {
    let item = find(&id).ok_or("notFound")?;
    let action = action.unwrap_or_else(|| "open".into());
    if action == "reveal" {
        let path = hit(&item).path.ok_or("notFound")?;
        crate::host::reveal_path(app, path)?;
        return Ok(());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        open_item(&item, action == "admin")?;
        record(&app2, &item.id);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
fn open_item(item: &Item, admin: bool) -> Result<(), String> {
    use crate::services::apps::windows as w;
    if item.kind == Kind::Setting && !item.target.starts_with("ms-settings:") {
        // Outil classique : « programme arguments ».
        let (prog, args) = item.target.split_once(' ').map(|(p, a)| (p, Some(a))).unwrap_or((&item.target, None));
        return w::shell_exec(if admin { "runas" } else { "open" }, prog, args);
    }
    if admin && item.kind != Kind::Setting {
        w::launch_admin(&item.target)
    } else {
        w::launch(&item.target)
    }
}

#[cfg(not(target_os = "windows"))]
fn open_item(_: &Item, _: bool) -> Result<(), String> {
    Err("unsupported".into())
}

/// Ouvre une URL tapée ou une recherche web (navigateur par défaut).
#[tauri::command]
pub fn launcher_open_url(app: AppHandle, url: String) -> Result<(), String> {
    crate::host::open_url(app, url)
}

/// Icônes (data URL PNG 32 px) des résultats visibles, en cache mémoire.
#[tauri::command]
pub async fn launcher_icons(ids: Vec<String>) -> Result<Vec<Option<String>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        ids.iter()
            .map(|id| {
                if let Some(v) = ICONS.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashMap::new).get(id) {
                    return v.clone();
                }
                let icon = find(id).filter(|i| i.kind != Kind::Setting).and_then(|i| icon_of(&i.target));
                ICONS.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashMap::new).insert(id.clone(), icon.clone());
                icon
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn icon_of(target: &str) -> Option<String> {
    crate::services::apps::windows::icon_data_url(target, 32)
}

#[cfg(not(target_os = "windows"))]
fn icon_of(_: &str) -> Option<String> {
    None
}

/// Le launcher s'ouvre : index périmé → reconstruction en fond (résultats actuels servis
/// en attendant, puis `launcher://index`).
#[tauri::command]
pub fn launcher_opened(app: AppHandle) {
    if index().is_none_or(|i| i.built.elapsed() > STALE) {
        rebuild_in_background(&app);
    }
}

/// Langue des titres (réglages Windows) ; change l'index sans tout reparcourir.
#[tauri::command]
pub fn launcher_set_lang(lang: String) {
    {
        let mut l = LANG.lock().unwrap_or_else(|p| p.into_inner());
        if *l == lang {
            return;
        }
        *l = lang.clone();
    }
    if let Some(idx) = index() {
        let mut items: Vec<Item> = idx.items.iter().filter(|p| p.item.kind != Kind::Setting).map(|p| p.item.clone()).collect();
        items.extend(sources::settings::items(&lang));
        swap(items);
    }
}
