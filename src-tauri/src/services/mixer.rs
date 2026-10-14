// Mixer : volume par application. Sessions WASAPI de la sortie par défaut, REGROUPÉES par
// exécutable (Chrome = plusieurs processus, une ligne : niveau = max, muet = tous muets ;
// un réglage s'applique à toutes les sessions de l'exe). Exclus : sons système, sessions
// expirées, PID 0 et notre propre processus. Nom = FileDescription de l'exe (sinon nom de
// fichier), icône = icône du programme ; les deux mis en cache par chemin.

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixerApp {
    /// Chemin de l'exécutable : identifiant de la ligne.
    pub exe: String,
    pub name: String,
    pub icon: Option<String>,
    pub level: f32,
    pub muted: bool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixerState {
    pub system: Option<MixerApp>,
    pub apps: Vec<MixerApp>,
}

/// Processus en cours (pid, chemin de l'exe).
pub fn running_exes() -> Vec<(u32, String)> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new().with_exe(UpdateKind::Always));
    sys.processes()
        .iter()
        .filter_map(|(pid, p)| Some((pid.as_u32(), p.exe()?.to_string_lossy().into_owned())))
        .collect()
}

#[tauri::command]
pub async fn mixer_list() -> Result<MixerState, String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(imp::list).await.map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    Ok(MixerState { system: None, apps: vec![] })
}

/// Règle le niveau et/ou le muet d'une app (`exe`) ou du système (`exe = None`). Monter le
/// niveau sans préciser le muet démute (comme Windows).
#[tauri::command]
pub async fn mixer_set(exe: Option<String>, level: Option<f32>, muted: Option<bool>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(move || imp::set(exe, level, muted))
        .await
        .map_err(|e| e.to_string())?;
    #[allow(unreachable_code)]
    {
        let _ = (exe, level, muted);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{MixerApp, MixerState};
    use std::collections::HashMap;
    use std::sync::Mutex;

    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, AudioSessionStateExpired, IAudioSessionControl2, IAudioSessionManager2,
        IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// Nom + icône par chemin d'exe.
    static META: Mutex<Option<HashMap<String, (String, Option<String>)>>> = Mutex::new(None);

    pub fn exe_path(pid: u32) -> Option<String> {
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

    /// FileDescription de la ressource de version, sinon nom de fichier capitalisé.
    fn display_name(path: &str) -> String {
        let fallback = || {
            let base = path.rsplit(['\\', '/']).next().unwrap_or(path);
            let stem = base.strip_suffix(".exe").or_else(|| base.strip_suffix(".EXE")).unwrap_or(base);
            let mut c = stem.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        unsafe {
            let wpath = HSTRING::from(path);
            let size = GetFileVersionInfoSizeW(&wpath, None);
            if size == 0 {
                return fallback();
            }
            let mut data = vec![0u8; size as usize];
            if GetFileVersionInfoW(&wpath, None, size, data.as_mut_ptr() as *mut _).is_err() {
                return fallback();
            }
            // Première traduction disponible.
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            let trans = HSTRING::from("\\VarFileInfo\\Translation");
            if !VerQueryValueW(data.as_ptr() as *const _, PCWSTR(trans.as_ptr()), &mut ptr, &mut len).as_bool() || len < 4 {
                return fallback();
            }
            let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
            let key = HSTRING::from(format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", pair[0], pair[1]));
            if !VerQueryValueW(data.as_ptr() as *const _, PCWSTR(key.as_ptr()), &mut ptr, &mut len).as_bool() || len == 0 {
                return fallback();
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16, len as usize))
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if s.is_empty() { fallback() } else { s }
        }
    }

    fn meta(path: &str) -> (String, Option<String>) {
        let mut g = META.lock().unwrap_or_else(|p| p.into_inner());
        let map = g.get_or_insert_with(HashMap::new);
        map.entry(path.to_string())
            .or_insert_with(|| (display_name(path), crate::services::apps::windows::icon_data_url(path, 32)))
            .clone()
    }

    struct Session {
        exe: String,
        vol: ISimpleAudioVolume,
    }

    unsafe fn device() -> Option<windows::Win32::Media::Audio::IMMDevice> {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        e.GetDefaultAudioEndpoint(eRender, eConsole).ok()
    }

    unsafe fn sessions() -> Vec<Session> {
        let me = GetCurrentProcessId();
        let Some(dev) = device() else { return vec![] };
        let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return vec![] };
        let Ok(list) = mgr.GetSessionEnumerator() else { return vec![] };
        let mut out = Vec::new();
        for i in 0..list.GetCount().unwrap_or(0) {
            let Ok(ctrl) = list.GetSession(i) else { continue };
            let Ok(c2) = ctrl.cast::<IAudioSessionControl2>() else { continue };
            if c2.IsSystemSoundsSession().0 == 0 {
                continue; // S_OK = session « sons système »
            }
            if c2.GetState().is_ok_and(|s| s == AudioSessionStateExpired) {
                continue;
            }
            let pid = c2.GetProcessId().unwrap_or(0);
            if pid == 0 || pid == me {
                continue;
            }
            let Some(exe) = exe_path(pid) else { continue };
            let Ok(vol) = c2.cast::<ISimpleAudioVolume>() else { continue };
            out.push(Session { exe, vol });
        }
        out
    }

    unsafe fn endpoint() -> Option<IAudioEndpointVolume> {
        device()?.Activate(CLSCTX_ALL, None).ok()
    }

    pub fn list() -> Result<MixerState, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let system = endpoint().and_then(|ep| {
                Some(MixerApp {
                    exe: String::new(),
                    name: String::new(),
                    icon: None,
                    level: ep.GetMasterVolumeLevelScalar().ok()?,
                    muted: ep.GetMute().ok()?.as_bool(),
                })
            });
            // Regroupement par exe, en gardant l'ordre Windows.
            let mut apps: Vec<MixerApp> = Vec::new();
            for s in sessions() {
                let level = s.vol.GetMasterVolume().unwrap_or(0.0);
                let muted = s.vol.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                let key = s.exe.to_lowercase();
                if let Some(a) = apps.iter_mut().find(|a| a.exe.to_lowercase() == key) {
                    a.level = a.level.max(level);
                    a.muted &= muted;
                } else {
                    let (name, icon) = meta(&s.exe);
                    apps.push(MixerApp { exe: s.exe, name, icon, level, muted });
                }
            }
            Ok(MixerState { system, apps })
        }
    }

    pub fn set(exe: Option<String>, level: Option<f32>, muted: Option<bool>) -> Result<(), String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let muted = muted.or_else(|| level.filter(|l| *l > 0.0).map(|_| false));
            match exe {
                None => {
                    let ep = endpoint().ok_or("sortie audio introuvable")?;
                    if let Some(l) = level {
                        ep.SetMasterVolumeLevelScalar(l.clamp(0.0, 1.0), std::ptr::null()).map_err(|e| e.to_string())?;
                    }
                    if let Some(m) = muted {
                        ep.SetMute(m, std::ptr::null()).map_err(|e| e.to_string())?;
                    }
                }
                Some(exe) => {
                    let key = exe.to_lowercase();
                    for s in sessions().into_iter().filter(|s| s.exe.to_lowercase() == key) {
                        if let Some(l) = level {
                            let _ = s.vol.SetMasterVolume(l.clamp(0.0, 1.0), std::ptr::null());
                        }
                        if let Some(m) = muted {
                            let _ = s.vol.SetMute(m, std::ptr::null());
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
