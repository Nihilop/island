// Scaffolding d'extension (template → dossier de dev). Templates embarqués dans
// l'exe. Cross-platform.

use tauri::{AppHandle, Manager};

use super::sanitize;

// Fichiers d'un template, embarqués dans l'exe. (chemin relatif, contenu brut)
fn template_files(kind: &str) -> Result<Vec<(&'static str, &'static str)>, String> {
    match kind {
        "minimal" => Ok(vec![
            ("manifest.json", include_str!("../../templates/minimal/manifest.json")),
            ("package.json", include_str!("../../templates/minimal/package.json")),
            ("vite.config.ts", include_str!("../../templates/minimal/vite.config.ts")),
            ("tailwind.css", include_str!("../../templates/minimal/tailwind.css")),
            ("index.ts", include_str!("../../templates/minimal/index.ts")),
            ("View.vue", include_str!("../../templates/minimal/View.vue")),
            (".gitignore", include_str!("../../templates/minimal/.gitignore")),
        ]),
        "complete" => Ok(vec![
            ("manifest.json", include_str!("../../templates/complete/manifest.json")),
            ("package.json", include_str!("../../templates/complete/package.json")),
            ("vite.config.ts", include_str!("../../templates/complete/vite.config.ts")),
            ("tailwind.css", include_str!("../../templates/complete/tailwind.css")),
            ("index.ts", include_str!("../../templates/complete/index.ts")),
            ("store.ts", include_str!("../../templates/complete/store.ts")),
            ("View.vue", include_str!("../../templates/complete/View.vue")),
            ("Config.vue", include_str!("../../templates/complete/Config.vue")),
            ("README.md", include_str!("../../templates/complete/README.md")),
            (".gitignore", include_str!("../../templates/complete/.gitignore")),
        ]),
        other => Err(format!("template inconnu: {other}")),
    }
}

// slug = minuscules alphanumériques (sert d'id reverse-DNS + nom de paquet npm).
fn slugify(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldResult {
    id: String,
    dir: String,
}

/// Crée une nouvelle extension depuis un template dans le dossier des extensions.
#[tauri::command]
pub fn scaffold_extension(app: AppHandle, name: String, template: String) -> Result<ScaffoldResult, String> {
    // On retire ce qui casserait un littéral JSON/TS, on garde accents & espaces.
    let name = name.replace(['"', '\\', '`', '\n', '\r'], "");
    let name = name.trim();
    if name.is_empty() {
        return Err("Le nom est vide.".into());
    }
    let slug = slugify(name);
    if slug.is_empty() {
        return Err("Le nom doit contenir au moins une lettre ou un chiffre.".into());
    }
    let id = format!("com.island.{slug}");

    let base = app.path().app_config_dir().map_err(|e| e.to_string())?.join("extensions");
    let dir = base.join(sanitize(&id));
    if dir.exists() {
        return Err(format!("Une extension « {id} » existe déjà."));
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    for (rel, raw) in template_files(&template)? {
        let content = raw
            .replace("__EXT_ID__", &id)
            .replace("__EXT_NAME__", name)
            .replace("__EXT_SLUG__", &slug);
        let out = dir.join(rel);
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&out, content).map_err(|e| e.to_string())?;
    }

    Ok(ScaffoldResult { id, dir: dir.to_string_lossy().to_string() })
}

/// Ouvre la fenêtre de création d'extension.
#[tauri::command]
pub fn open_create(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("create") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}
