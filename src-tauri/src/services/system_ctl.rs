// Actions système des automatisations : sortie audio par défaut (liste + bascule via
// IPolicyConfig), fermeture propre d'une app (WM_CLOSE, jamais TerminateProcess) et
// lancement d'une app / d'un jeu (ShellExecute `open`, sans arguments).
//
// Garde-fous : la sortie visée doit être un endpoint de rendu actif ; on ne ferme ni
// Island ni un processus de %SystemRoot% (explorer, services…) ; on ne lance qu'un
// .exe/.lnk/.url/.appref-ms existant (chemin absolu) ou une URL steam:// / Epic précise.
// Erreurs = codes courts (`invalidExe`, `forbidden`, `invalidTarget`, `unknownOutput`…).

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutput {
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// Sorties audio actives ; `default` = sortie console par défaut.
#[tauri::command]
pub async fn audio_outputs() -> Result<Vec<AudioOutput>, String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(imp::outputs).await.map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    Ok(vec![])
}

/// Sortie par défaut pour les trois rôles (console, multimédia, communications).
#[tauri::command]
pub async fn audio_set_output(id: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(move || imp::set_output(&id))
        .await
        .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    {
        let _ = id;
        Err("unsupported".into())
    }
}

/// WM_CLOSE aux fenêtres visibles des processus `exe` (nom de fichier) ; renvoie le nombre
/// de fenêtres signalées.
#[tauri::command]
pub async fn app_close(exe: String) -> Result<u32, String> {
    let exe = valid_exe(&exe).ok_or("invalidExe")?;
    if is_protected_exe(&exe) {
        return Err("forbidden".into());
    }
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(move || imp::close(&exe))
        .await
        .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    Err("unsupported".into())
}

/// Lance une app / un jeu (ShellExecute `open`, sans arguments).
#[tauri::command]
pub async fn app_launch(path: String) -> Result<(), String> {
    let target = launch_target(&path).ok_or("invalidTarget")?;
    if let LaunchTarget::File(p) = &target {
        if !std::path::Path::new(p).is_file() {
            return Err("invalidTarget".into());
        }
    }
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(move || {
        let s = match &target {
            LaunchTarget::File(s) | LaunchTarget::Url(s) => s,
        };
        crate::services::apps::windows::launch(s).map_err(|_| "launchFailed".to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    {
        let _ = target;
        Err("unsupported".into())
    }
}

/// Nom d'exe normalisé (minuscules) : nom de fichier seul, en `.exe`, sans séparateur ni
/// caractère de contrôle.
fn valid_exe(exe: &str) -> Option<String> {
    let exe = exe.trim();
    let lower = exe.to_lowercase();
    let ok = lower.len() > 4
        && lower.len() <= 260
        && lower.ends_with(".exe")
        && !lower.starts_with('.')
        && !lower.chars().any(|c| matches!(c, '\\' | '/' | ':' | '"' | '*' | '?' | '<' | '>' | '|') || c.is_control());
    ok.then_some(lower)
}

/// Exe refusés d'office (shell + processus système), plus Island lui-même. Le contrôle
/// %SystemRoot% du chemin réel est fait à l'énumération.
fn is_protected_exe(exe: &str) -> bool {
    const SYSTEM: &[&str] = &[
        "explorer.exe", "csrss.exe", "winlogon.exe", "wininit.exe", "services.exe", "lsass.exe", "smss.exe",
        "svchost.exe", "dwm.exe", "sihost.exe", "fontdrvhost.exe", "ctfmon.exe", "taskhostw.exe", "runtimebroker.exe",
        "searchhost.exe", "startmenuexperiencehost.exe", "shellexperiencehost.exe", "textinputhost.exe",
        "lockapp.exe", "logonui.exe", "system", "registry",
    ];
    if SYSTEM.contains(&exe) {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .is_some_and(|own| own == exe)
}

#[derive(Debug, PartialEq)]
enum LaunchTarget {
    File(String),
    Url(String),
}

/// Cible de lancement autorisée (syntaxe seule ; l'existence du fichier est vérifiée à part).
fn launch_target(raw: &str) -> Option<LaunchTarget> {
    let s = raw.trim();
    if s.is_empty() || s.len() > 1024 || s.chars().any(|c| c.is_control() || c == '"') {
        return None;
    }
    let lower = s.to_lowercase();
    if let Some(id) = lower.strip_prefix("steam://rungameid/") {
        let ok = !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit());
        return ok.then(|| LaunchTarget::Url(format!("steam://rungameid/{id}")));
    }
    if let Some(rest) = lower.strip_prefix("com.epicgames.launcher://apps/") {
        let rest = &s[s.len() - rest.len()..];
        let ok = !rest.is_empty()
            && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b"%:._-?=&".contains(&b));
        return ok.then(|| LaunchTarget::Url(format!("com.epicgames.launcher://apps/{rest}")));
    }
    // Chemin local absolu `X:\…` (pas d'UNC, pas de `..`), extension autorisée.
    let b = s.as_bytes();
    let drive = b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    if !drive || s.contains("://") {
        return None;
    }
    let p = std::path::Path::new(s);
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    let ext = p.extension()?.to_string_lossy().to_lowercase();
    matches!(ext.as_str(), "exe" | "lnk" | "url" | "appref-ms").then(|| LaunchTarget::File(s.to_string()))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::AudioOutput;
    use std::collections::HashMap;
    use std::ffi::c_void;

    use windows::core::{GUID, HRESULT, HSTRING, IUnknown, Interface, PCWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
    use windows::Win32::Media::Audio::{
        eCommunications, eConsole, eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
    use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GW_OWNER, WM_CLOSE,
    };

    // ---- Sorties audio ----

    fn enumerator() -> Result<IMMDeviceEnumerator, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|_| "audioUnavailable".to_string())
        }
    }

    unsafe fn device_id(d: &IMMDevice) -> Option<String> {
        let id = d.GetId().ok()?;
        let s = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const _));
        s
    }

    unsafe fn friendly_name(d: &IMMDevice) -> Option<String> {
        let store = d.OpenPropertyStore(STGM_READ).ok()?;
        let mut pv = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let s = PropVariantToStringAlloc(&pv).ok().map(|p| {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        });
        let _ = PropVariantClear(&mut pv);
        s.filter(|s| !s.is_empty())
    }

    /// (id, nom) des endpoints de rendu actifs.
    unsafe fn active(e: &IMMDeviceEnumerator) -> Result<Vec<(String, String)>, String> {
        let col = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).map_err(|_| "audioUnavailable".to_string())?;
        let n = col.GetCount().unwrap_or(0);
        Ok((0..n)
            .filter_map(|i| {
                let d = col.Item(i).ok()?;
                let id = device_id(&d)?;
                let name = friendly_name(&d).unwrap_or_else(|| id.clone());
                Some((id, name))
            })
            .collect())
    }

    pub fn outputs() -> Result<Vec<AudioOutput>, String> {
        unsafe {
            let e = enumerator()?;
            let def = e.GetDefaultAudioEndpoint(eRender, eConsole).ok().and_then(|d| device_id(&d));
            Ok(active(&e)?
                .into_iter()
                .map(|(id, name)| AudioOutput { default: def.as_deref() == Some(id.as_str()), id, name })
                .collect())
        }
    }

    // IPolicyConfig (non documentée, stable depuis Win7) : vtable déclarée jusqu'à
    // SetDefaultEndpoint inclus ; seules les positions comptent pour les entrées inutilisées.
    const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

    #[repr(C)]
    #[allow(non_snake_case, dead_code)]
    struct IPolicyConfigVtbl {
        base: windows::core::IUnknown_Vtbl,
        GetMixFormat: *const c_void,
        GetDeviceFormat: *const c_void,
        ResetDeviceFormat: *const c_void,
        SetDeviceFormat: *const c_void,
        GetProcessingPeriod: *const c_void,
        SetProcessingPeriod: *const c_void,
        GetShareMode: *const c_void,
        SetShareMode: *const c_void,
        GetPropertyValue: *const c_void,
        SetPropertyValue: *const c_void,
        SetDefaultEndpoint: unsafe extern "system" fn(this: *mut c_void, id: PCWSTR, role: i32) -> HRESULT,
        SetEndpointVisibility: *const c_void,
    }

    pub fn set_output(id: &str) -> Result<(), String> {
        unsafe {
            let e = enumerator()?;
            if !active(&e)?.iter().any(|(d, _)| d == id) {
                return Err("unknownOutput".into());
            }
            let unk: IUnknown =
                CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).map_err(|_| "policyUnavailable".to_string())?;
            let mut raw: *mut c_void = std::ptr::null_mut();
            unk.query(&IID_POLICY_CONFIG, &mut raw).ok().map_err(|_| "policyUnavailable".to_string())?;
            // Possède la référence obtenue par QueryInterface (Release au drop).
            let pc = IUnknown::from_raw(raw);
            let vt = &**(raw as *const *const IPolicyConfigVtbl);
            let wid = HSTRING::from(id);
            for role in [eConsole, eMultimedia, eCommunications] {
                (vt.SetDefaultEndpoint)(pc.as_raw(), PCWSTR(wid.as_ptr()), role.0)
                    .ok()
                    .map_err(|_| "setOutputFailed".to_string())?;
            }
            Ok(())
        }
    }

    // ---- Fermeture d'app ----

    fn image_path(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut size)
                .is_ok();
            let _ = CloseHandle(h);
            ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))
        }
    }

    /// `c:\windows\` en minuscules (repli : %SystemRoot%).
    fn windows_dir() -> String {
        let mut buf = [0u16; 260];
        let n = unsafe { GetSystemWindowsDirectoryW(Some(&mut buf)) } as usize;
        let dir = if n > 0 && n < buf.len() {
            String::from_utf16_lossy(&buf[..n])
        } else {
            std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())
        };
        let mut dir = dir.to_lowercase();
        if !dir.ends_with('\\') {
            dir.push('\\');
        }
        dir
    }

    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> windows::core::BOOL {
        let out = &mut *(lp.0 as *mut Vec<(HWND, u32)>);
        // Fenêtres visibles de premier niveau sans propriétaire (pas les dialogues).
        if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map_or(true, |o| o.is_invalid()) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != 0 {
                out.push((hwnd, pid));
            }
        }
        true.into()
    }

    pub fn close(exe: &str) -> Result<u32, String> {
        let mut wins: Vec<(HWND, u32)> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut wins as *mut _ as isize));
        }
        let me = unsafe { GetCurrentProcessId() };
        let win_dir = windows_dir();
        // pid → Some(autorisé) si l'image correspond à `exe`, None sinon.
        let mut cache: HashMap<u32, Option<bool>> = HashMap::new();
        let mut forbidden = false;
        let mut count = 0u32;
        for (hwnd, pid) in wins {
            if pid == me {
                continue;
            }
            let verdict = *cache.entry(pid).or_insert_with(|| {
                let path = image_path(pid)?.to_lowercase();
                let name = path.rsplit(['\\', '/']).next().unwrap_or(&path);
                (name == exe).then(|| !path.starts_with(&win_dir))
            });
            match verdict {
                Some(true) => {
                    if unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok() {
                        count += 1;
                    }
                }
                Some(false) => forbidden = true,
                None => {}
            }
        }
        if count == 0 && forbidden {
            return Err("forbidden".into());
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_names() {
        assert_eq!(valid_exe("Discord.exe").as_deref(), Some("discord.exe"));
        assert_eq!(valid_exe("  chrome.EXE ").as_deref(), Some("chrome.exe"));
        for bad in ["", ".exe", "chrome", "C:\\x\\a.exe", "a/b.exe", "..\\a.exe", "c:a.exe", "a\".exe", "a\n.exe", "a?.exe"] {
            assert!(valid_exe(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn protected_exes() {
        assert!(is_protected_exe("explorer.exe"));
        assert!(is_protected_exe("svchost.exe"));
        assert!(!is_protected_exe("discord.exe"));
        // Le binaire de test lui-même tient lieu d'« Island ».
        let own = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().to_lowercase();
        assert!(is_protected_exe(&own));
    }

    /// Lecture seule, dépend du matériel : `cargo test --lib system_ctl -- --ignored --nocapture`.
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore]
    fn lists_outputs() {
        let outs = imp::outputs().unwrap();
        for o in &outs {
            println!("{} | {} | default={}", o.name, o.id, o.default);
        }
        assert!(outs.iter().filter(|o| o.default).count() <= 1);
    }

    #[test]
    fn launch_targets() {
        assert_eq!(launch_target("steam://rungameid/570"), Some(LaunchTarget::Url("steam://rungameid/570".into())));
        assert_eq!(launch_target("STEAM://rungameid/570"), Some(LaunchTarget::Url("steam://rungameid/570".into())));
        assert_eq!(
            launch_target("com.epicgames.launcher://apps/fn%3A4fe7%3AFortnite?action=launch&silent=true"),
            Some(LaunchTarget::Url("com.epicgames.launcher://apps/fn%3A4fe7%3AFortnite?action=launch&silent=true".into()))
        );
        assert_eq!(launch_target(r"C:\Games\x.exe"), Some(LaunchTarget::File(r"C:\Games\x.exe".into())));
        assert_eq!(launch_target(r"d:/a/b.LNK"), Some(LaunchTarget::File(r"d:/a/b.LNK".into())));
        assert!(launch_target(r"C:\a\b.url").is_some());
        assert!(launch_target(r"C:\a\b.appref-ms").is_some());
        for bad in [
            "",
            "steam://rungameid/",
            "steam://rungameid/570 -console",
            "steam://install/570",
            "com.epicgames.launcher://apps/",
            "com.epicgames.launcher://apps/a b",
            "com.epicgames.launcher://store/x",
            "https://example.com/a.exe",
            "file:///C:/a.exe",
            "x.exe",
            r"\\server\share\a.exe",
            r"C:\a\..\b.exe",
            r"C:\a\b.bat",
            r"C:\a\b.ps1",
            r"C:\a\b",
            "C:\\a\\b.exe\"",
            "cmd.exe /c calc",
        ] {
            assert!(launch_target(bad).is_none(), "{bad:?}");
        }
    }
}
