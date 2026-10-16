// Fenêtres détachées : de vraies fenêtres Tauri ouvertes à la demande (une note dans sa
// propre fenêtre…), sans décoration Windows — la page dessine sa barre (titre, actions,
// croix). Une fenêtre par élément : rouvrir le même élément la ramène au premier plan.
// Taille et position mémorisées par élément (store `__app__`, clé `windows`).
//
// Libellé = `<type>-<clé>` ; la page chargée est `index.html#/<type>/<clé>` et `main.ts`
// choisit le composant racine d'après le libellé. Types autorisés : `KINDS` (liste
// blanche). Type `ext` : fenêtre d'une extension (`ext-<id>__<fenêtre>`, points de l'id
// remplacés par `_`), qui affiche la page déclarée dans une iframe isolée (ExtWindow.vue).
//
// Arguments : `window_open` peut transmettre une valeur JSON à la fenêtre (épisode à lire…),
// lue au chargement (`window_args`) ; si la fenêtre est déjà ouverte, elle la reçoit en
// événement (`window://args`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

/// Types de fenêtres détachées connus de `main.ts`.
const KINDS: &[&str] = &["note", "ext"];

/// Fenêtres détachées actuellement en plein écran (handle Win32) : l'île les traite comme
/// une app plein écran (masquage « en plein écran », cf. host.rs), alors qu'elle ignore
/// sinon toutes les fenêtres d'Island.
static FULLSCREEN: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());

/// Cette fenêtre d'Island est-elle une fenêtre détachée en plein écran ?
pub fn is_own_fullscreen(hwnd: isize) -> bool {
    FULLSCREEN.lock().unwrap_or_else(|p| p.into_inner()).contains(&hwnd)
}

fn hwnd_of(w: &WebviewWindow) -> Option<isize> {
    #[cfg(windows)]
    {
        w.hwnd().ok().map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = w;
        None
    }
}

/// Plein écran d'une fenêtre détachée (page qui passe un élément en plein écran).
#[tauri::command]
pub fn window_fullscreen(window: WebviewWindow, on: bool) -> Result<(), String> {
    window.set_fullscreen(on).map_err(|e| e.to_string())?;
    if let Some(h) = hwnd_of(&window) {
        let mut list = FULLSCREEN.lock().unwrap_or_else(|p| p.into_inner());
        list.retain(|x| *x != h);
        if on {
            list.push(h);
        }
    }
    Ok(())
}

/// Arguments en attente, par libellé de fenêtre.
static ARGS: std::sync::Mutex<Option<HashMap<String, serde_json::Value>>> = std::sync::Mutex::new(None);
const STORE: &str = "__app__";
const KEY: &str = "windows";

#[derive(Clone, Copy, Serialize, Deserialize)]
struct Geometry {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

fn geometries(app: &AppHandle) -> HashMap<String, Geometry> {
    crate::store::get(app, STORE, KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn save_geometry(app: &AppHandle, label: &str, g: Geometry) {
    let mut all = geometries(app);
    all.insert(label.to_string(), g);
    // On ne garde que les 200 dernières fenêtres connues.
    if all.len() > 200 {
        let drop: Vec<String> = all.keys().take(all.len() - 200).cloned().collect();
        for k in drop {
            all.remove(&k);
        }
    }
    crate::store::set(app, STORE, KEY, serde_json::to_value(all).ok());
}

/// Clé utilisable dans un libellé Tauri (`a-zA-Z0-9-_`).
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 160 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// La position mémorisée tombe-t-elle encore sur un écran (moniteur débranché…) ?
fn on_screen(app: &AppHandle, g: &Geometry) -> bool {
    app.available_monitors().unwrap_or_default().iter().any(|m| {
        let (p, s) = (m.position(), m.size());
        g.x + 40 >= p.x && g.y >= p.y && g.x + 40 < p.x + s.width as i32 && g.y + 20 < p.y + s.height as i32
    })
}

/// Ouvre (ou ramène au premier plan) la fenêtre `<kind>-<key>`. Renvoie son libellé.
/// `size` / `min_size` : taille par défaut et minimale (DIP), bornées.
#[tauri::command]
pub async fn window_open(
    app: AppHandle,
    kind: String,
    key: String,
    title: String,
    size: Option<[f64; 2]>,
    min_size: Option<[f64; 2]>,
    args: Option<serde_json::Value>,
) -> Result<String, String> {
    if !KINDS.contains(&kind.as_str()) || !valid_key(&key) {
        return Err("invalidWindow".into());
    }
    let label = format!("{kind}-{key}");
    if let Some(a) = &args {
        ARGS.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashMap::new).insert(label.clone(), a.clone());
    }
    if let Some(w) = app.get_webview_window(&label) {
        if let Some(a) = args {
            let _ = w.emit_to(&label, "window://args", a);
        }
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(label);
    }
    let clamp = |v: [f64; 2]| [v[0].clamp(320.0, 3840.0), v[1].clamp(240.0, 2160.0)];
    let [w0, h0] = clamp(size.unwrap_or([760.0, 640.0]));
    let [mw, mh] = clamp(min_size.unwrap_or([420.0, 320.0]));
    let saved = geometries(&app).get(&label).copied();
    let mut b = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(format!("index.html#/{kind}/{key}").into()))
        .title(title.chars().take(120).collect::<String>())
        .decorations(false)
        // Sans ça, WebView2 avale le glisser-déposer HTML5 (poignée de bloc de l'éditeur).
        .disable_drag_drop_handler()
        .shadow(true)
        .resizable(true)
        .min_inner_size(mw.min(w0), mh.min(h0))
        .inner_size(w0, h0)
        .focused(true)
        .visible(false);
    if saved.is_none() {
        b = b.center();
    }
    let win = b.build().map_err(|e| e.to_string())?;
    if let Some(g) = saved.filter(|g| on_screen(&app, g)) {
        let _ = win.set_size(PhysicalSize::new(g.w, g.h));
        let _ = win.set_position(PhysicalPosition::new(g.x, g.y));
    }
    let _ = win.show();
    let _ = win.set_focus();

    // Mémorise taille et position (au déplacement / redimensionnement, pas en plein écran).
    let (app2, label2, w2) = (app.clone(), label.clone(), win.clone());
    let hwnd = hwnd_of(&win);
    win.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            if let Some(h) = hwnd {
                FULLSCREEN.lock().unwrap_or_else(|p| p.into_inner()).retain(|x| *x != h);
            }
        }
        if matches!(e, WindowEvent::Moved(_) | WindowEvent::Resized(_)) && !w2.is_maximized().unwrap_or(false) && !w2.is_minimized().unwrap_or(false) {
            if let (Ok(p), Ok(s)) = (w2.outer_position(), w2.inner_size()) {
                save_geometry(&app2, &label2, Geometry { x: p.x, y: p.y, w: s.width, h: s.height });
            }
        }
    });
    Ok(label)
}

/// Arguments transmis à la fenêtre appelante par `window_open` (null si aucun).
#[tauri::command]
pub fn window_args(window: WebviewWindow) -> Option<serde_json::Value> {
    ARGS.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|m| m.get(window.label()).cloned())
}

/// Ferme toutes les fenêtres d'un type (ex. verrou Windows Hello des notes).
#[tauri::command]
pub fn window_close_kind(app: AppHandle, kind: String) {
    if !KINDS.contains(&kind.as_str()) {
        return;
    }
    for (label, w) in app.webview_windows() {
        if label.starts_with(&format!("{kind}-")) {
            let _ = w.close();
        }
    }
}
