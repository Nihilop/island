// Indexation et lancement d'applications (pour un launcher type Flow).
// On scanne les raccourcis `.lnk` du menu Démarrer (machine + utilisateur) — c'est
// l'approche standard ; lancer le `.lnk` exécute sa cible (pas besoin de parser).
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<super::AppEntry>, seen: &mut HashSet<String>, depth: u8) {
    if depth > 5 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out, seen, depth + 1);
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let low = name.to_lowercase();
            // Bruit courant : désinstalleurs, fichiers d'aide.
            if low.contains("uninstall") || low.contains("désinstall") || low.contains("readme") {
                continue;
            }
            if seen.insert(low) {
                out.push(super::AppEntry { name, path: p.to_string_lossy().to_string() });
            }
        }
    }
}

/// Bruit fréquent à écarter (désinstalleurs, aide).
fn is_noise(low: &str) -> bool {
    low.contains("uninstall") || low.contains("désinstall") || low.contains("readme")
}

/// Énumère le dossier shell `AppsFolder` : apps Win32 du menu Démarrer **+ UWP/Store +
/// items Panneau de config**, en une passe. Le `path` stocké est le *parsing name* —
/// chemin `.lnk`/exe pour le Win32, `shell:AppsFolder\<AUMID>` pour l'UWP — il sert
/// AUSSI bien au lancement (`ShellExecuteW open`) qu'à l'icône (`SHCreateItemFromParsingName`).
/// Nécessite COM initialisé par l'appelant. Renvoie vide en cas d'échec (→ fallback `.lnk`).
unsafe fn enum_appsfolder(out: &mut Vec<super::AppEntry>, seen: &mut HashSet<String>) {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        SHGetKnownFolderItem, IEnumShellItems, IShellItem, BHID_EnumItems, FOLDERID_AppsFolder,
        KF_FLAG_DEFAULT, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
    };

    unsafe fn name_of(item: &IShellItem, kind: windows::Win32::UI::Shell::SIGDN) -> String {
        match item.GetDisplayName(kind) {
            Ok(p) if !p.is_null() => {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            }
            _ => String::new(),
        }
    }

    let apps: IShellItem =
        match SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) {
            Ok(i) => i,
            Err(_) => return,
        };
    let en: IEnumShellItems = match apps.BindToHandler(None, &BHID_EnumItems) {
        Ok(e) => e,
        Err(_) => return,
    };

    loop {
        let mut items: [Option<IShellItem>; 1] = [None];
        let mut fetched = 0u32;
        let _ = en.Next(&mut items, Some(&mut fetched));
        if fetched == 0 {
            break;
        }
        let Some(item) = items[0].take() else { break };

        let name = name_of(&item, SIGDN_NORMALDISPLAY);
        if name.is_empty() || is_noise(&name.to_lowercase()) {
            continue;
        }
        let parsing = name_of(&item, SIGDN_DESKTOPABSOLUTEPARSING);
        if parsing.is_empty() {
            continue;
        }
        // Chemin du système de fichiers (Win32) → tel quel ; sinon AUMID UWP → shell:AppsFolder.
        let path = if parsing.contains(":\\") || parsing.starts_with("\\\\") {
            parsing
        } else {
            format!("shell:AppsFolder\\{parsing}")
        };
        if seen.insert(name.to_lowercase()) {
            out.push(super::AppEntry { name, path });
        }
    }
}

/// Jeux Steam installés : `steam://rungameid/<appid>` (lançables via ShellExecute).
/// Localise Steam (registre) → bibliothèques (`libraryfolders.vdf`) → manifestes (`*.acf`).
fn steam_games(out: &mut Vec<super::AppEntry>, seen: &mut HashSet<String>) {
    let Some(steam) = steam_dir() else { return };

    // Racines de bibliothèques : Steam lui-même + celles listées dans libraryfolders.vdf.
    let mut libs: Vec<PathBuf> = vec![steam.clone()];
    let vdf = steam.join("steamapps").join("libraryfolders.vdf");
    if let Ok(txt) = std::fs::read_to_string(&vdf) {
        // On extrait les valeurs "path" sans parser tout le VDF.
        for line in txt.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("\"path\"") {
                if let Some(p) = rest.split('"').nth(1) {
                    libs.push(PathBuf::from(p.replace("\\\\", "\\")));
                }
            }
        }
    }

    for lib in libs {
        let dir = lib.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let is_acf = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"))
                .unwrap_or(false);
            if !is_acf {
                continue;
            }
            let Ok(txt) = std::fs::read_to_string(&p) else { continue };
            let appid = acf_value(&txt, "appid");
            let name = acf_value(&txt, "name");
            if let (Some(appid), Some(name)) = (appid, name) {
                if name.is_empty() || !seen.insert(name.to_lowercase()) {
                    continue;
                }
                out.push(super::AppEntry { name, path: format!("steam://rungameid/{appid}") });
            }
        }
    }
}

/// Dossier d'install de Steam via `HKCU\Software\Valve\Steam\SteamPath`.
fn steam_dir() -> Option<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Valve\\Steam")
        .ok()?;
    let p: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(p))
}

/// Extrait une valeur `"key"  "value"` d'un fichier ACF (KeyValues plat).
fn acf_value(txt: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    for line in txt.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix(&needle) {
            if let Some(v) = rest.split('"').nth(1) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Liste les applications, triées par nom, dédupliquées : AppsFolder (Win32 + UWP) en
/// priorité (fallback sur les `.lnk` du menu Démarrer si l'énumération échoue) + jeux Steam.
/// L'appelant doit avoir initialisé COM (pour AppsFolder).
pub fn list_apps() -> Vec<super::AppEntry> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    unsafe { enum_appsfolder(&mut out, &mut seen) };

    // Fallback : si AppsFolder n'a rien donné, on retombe sur le scan des `.lnk`.
    if out.is_empty() {
        let roots = [
            std::env::var("ProgramData").ok().map(|p| {
                PathBuf::from(p).join("Microsoft").join("Windows").join("Start Menu").join("Programs")
            }),
            std::env::var("APPDATA").ok().map(|p| {
                PathBuf::from(p).join("Microsoft").join("Windows").join("Start Menu").join("Programs")
            }),
        ];
        for root in roots.into_iter().flatten() {
            walk(&root, &mut out, &mut seen, 0);
        }
    }

    steam_games(&mut out, &mut seen);

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Extrait l'icône d'un fichier (`.lnk`/exe) → PNG en data URL (base64), ou None.
/// Utilise IShellItemImageFactory (même rendu qu'Explorer : alpha propre, .lnk résolu).
pub fn icon_data_url(path: &str, size: i32) -> Option<String> {
    use base64::Engine as _;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    };

    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
        let hbitmap: HBITMAP = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .ok()?;

        let mut bm = BITMAP::default();
        let got = GetObjectW(
            HGDIOBJ(hbitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut std::ffi::c_void),
        );
        if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);

        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = 0; // BI_RGB

        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        let hdc = GetDC(None);
        let scan = GetDIBits(
            hdc,
            hbitmap,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            &mut bi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        let _ = DeleteObject(HGDIOBJ(hbitmap.0));
        if scan == 0 {
            return None;
        }

        // BGRA (premultiplié) → RGBA (droit). Si aucun alpha → icône opaque.
        let has_alpha = buf.chunks_exact(4).any(|px| px[3] != 0);
        for px in buf.chunks_exact_mut(4) {
            if !has_alpha {
                px.swap(0, 2); // B<->R
                px[3] = 255;
            } else {
                let a = px[3];
                if a == 0 {
                    px[0] = 0;
                    px[1] = 0;
                    px[2] = 0;
                    continue;
                }
                let (b, g, r) = (px[0], px[1], px[2]);
                let un = |c: u8| ((c as u32 * 255 / a as u32).min(255)) as u8;
                px[0] = un(r);
                px[1] = un(g);
                px[2] = un(b);
            }
        }

        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w as u32, h as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().ok()?;
            writer.write_image_data(&buf).ok()?;
        }
        Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png)))
    }
}

/// Lance un fichier/app via ShellExecute avec un verbe (`open`, `runas`…) et des
/// paramètres optionnels. Un `.lnk` exécute sa cible ; `runas` demande l'élévation.
fn shell_exec(verb: &str, path: &str, args: Option<&str>) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let op = HSTRING::from(verb);
    let file = HSTRING::from(path);
    let params = args.map(HSTRING::from);
    let res = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            params.as_ref().map(|h| PCWSTR(h.as_ptr())).unwrap_or(PCWSTR::null()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute renvoie une « valeur » > 32 en cas de succès.
    if res.0 as isize > 32 {
        Ok(())
    } else {
        Err("échec du lancement".to_string())
    }
}

/// Lance un fichier/app (verbe `open`).
pub fn launch(path: &str) -> Result<(), String> {
    shell_exec("open", path, None)
}

/// Lance un fichier/app **en administrateur** (verbe `runas` → UAC).
pub fn launch_admin(path: &str) -> Result<(), String> {
    shell_exec("runas", path, None)
}
