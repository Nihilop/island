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

/// Liste les applications (raccourcis du menu Démarrer), triées par nom, dédupliquées.
pub fn list_apps() -> Vec<super::AppEntry> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
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

/// Lance un fichier/app via ShellExecute (un `.lnk` exécute sa cible).
pub fn launch(path: &str) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let op = HSTRING::from("open");
    let file = HSTRING::from(path);
    let res = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
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
