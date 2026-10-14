// FPS du jeu au premier plan, via ETW (version très simplifiée de PresentMon).
//
// Session temps réel nommée ; fournisseurs : Microsoft-Windows-DXGI (Present_Start,
// PresentMultiplaneOverlay_Start), Microsoft-Windows-D3D9 (Present_Start) et
// Microsoft-Windows-DxgKrnl (mot-clé Present uniquement, très bavard) — toutes les API
// passent par DxgKrnl, donc OpenGL et Vulkan comptent aussi. Comptage par (PID, type
// d'événement) ; on prend le MAX des types, jamais la somme (une frame émet plusieurs
// événements). FPS = frames du process au premier plan / temps écoulé, lissé 40/60.
// La session s'arrête après 10 s sans demande ; une session orpheline du même nom est
// arrêtée au démarrage ; erreurs réessayées toutes les 5 s.
//
// ETW temps réel exige admin ou le groupe « Utilisateurs du journal de performances »
// (SID S-1-5-32-559). `fps_unlock` relance l'exe élevé (UAC) qui ajoute le compte au
// groupe ; Windows ne l'applique qu'aux nouvelles sessions → « reconnexion nécessaire ».

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Argument de la relance élevée : `--island-fps-elevate <SID utilisateur>`.
pub const ELEVATE_ARG: &str = "--island-fps-elevate";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsSample {
    /// ok | noAdmin | relogin | error | starting
    status: &'static str,
    fps: Option<f32>,
}

#[cfg(target_os = "windows")]
pub use imp::elevated_main;

#[cfg(not(target_os = "windows"))]
pub mod imp {
    use super::FpsSample;
    #[tauri::command]
    pub fn fps_sample() -> FpsSample {
        FpsSample { status: "error", fps: None }
    }
    #[tauri::command]
    pub fn fps_unlock() -> &'static str {
        "error"
    }
}

#[cfg(target_os = "windows")]
pub mod imp {
    use super::*;
    use windows::core::{w, GUID, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_SUCCESS};
    use windows::Win32::System::Diagnostics::Etw::{
        CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW, CONTROLTRACE_HANDLE,
        EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW,
        EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, PROCESSTRACE_HANDLE, PROCESS_TRACE_MODE_EVENT_RECORD,
        PROCESS_TRACE_MODE_REAL_TIME, WNODE_FLAG_TRACED_GUID,
    };

    const SESSION: PCWSTR = w!("IslandFpsSession");
    const SESSION_GUID: GUID = GUID::from_u128(0x6f1a2c3e_8b7d_4e5f_9a01_2b3c4d5e6f70);
    const DXGI: GUID = GUID::from_u128(0xca11c036_0102_4a2d_a6ad_f03cfed5d3c9);
    const D3D9: GUID = GUID::from_u128(0x783aca0a_790e_4d7f_8451_aa850511c6b9);
    const DXGKRNL: GUID = GUID::from_u128(0x802ec45a_1e99_4b83_9920_87c98277ba9d);
    /// Mot-clé « Present » de DxgKrnl.
    const DXGKRNL_PRESENT: u64 = 0x8000000;

    const IDLE_STOP: Duration = Duration::from_secs(10);
    const RETRY: Duration = Duration::from_secs(5);

    /// Compteurs par (pid, type d'événement).
    static COUNTS: Mutex<Option<HashMap<(u32, u32), u64>>> = Mutex::new(None);

    struct Runtime {
        running: bool,
        status: &'static str,
        last_poll: Instant,
        last_error_at: Option<Instant>,
        /// Dernier échantillon : (instant, pid, compteurs par type) + FPS lissé par pid.
        prev: Option<(Instant, u32, HashMap<u32, u64>)>,
        smooth: HashMap<u32, f32>,
    }
    static RT: Mutex<Option<Runtime>> = Mutex::new(None);

    /// Clé de type : fournisseur (2 bits) + id d'événement.
    fn kind(provider: &GUID, id: u16) -> Option<u32> {
        let p = if *provider == DXGI {
            matches!(id, 42 | 55).then_some(1)?
        } else if *provider == D3D9 {
            (id == 1).then_some(2)?
        } else if *provider == DXGKRNL {
            // Blit_Info, PresentHistory_Start, Present_Info, PresentHistoryDetailed_Start.
            matches!(id, 166 | 171 | 184 | 215).then_some(3)?
        } else {
            return None;
        };
        Some(p << 16 | id as u32)
    }

    unsafe extern "system" fn on_event(rec: *mut EVENT_RECORD) {
        let r = unsafe { &*rec };
        let h = &r.EventHeader;
        if let Some(k) = kind(&h.ProviderId, h.EventDescriptor.Id) {
            let mut g = COUNTS.lock().unwrap_or_else(|p| p.into_inner());
            *g.get_or_insert_with(HashMap::new).entry((h.ProcessId, k)).or_insert(0) += 1;
        }
    }

    /// Propriétés de session (+ place pour le nom à la suite).
    fn props() -> Vec<u8> {
        let size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + 1024;
        let mut buf = vec![0u8; size];
        let p = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        unsafe {
            (*p).Wnode.BufferSize = size as u32;
            (*p).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*p).Wnode.ClientContext = 1; // QPC
            (*p).Wnode.Guid = SESSION_GUID;
            (*p).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*p).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        }
        buf
    }

    fn stop_session() {
        let mut p = props();
        unsafe {
            let _ = ControlTraceW(CONTROLTRACE_HANDLE { Value: 0 }, SESSION, p.as_mut_ptr() as *mut _, EVENT_TRACE_CONTROL_STOP);
        }
    }

    /// Démarre la session et le thread de lecture. Erreur : « noAdmin » ou « error ».
    fn start_session() -> Result<(), &'static str> {
        stop_session(); // session orpheline du même nom
        let mut p = props();
        let mut handle = CONTROLTRACE_HANDLE::default();
        let r = unsafe { StartTraceW(&mut handle, SESSION, p.as_mut_ptr() as *mut _) };
        if r == ERROR_ACCESS_DENIED {
            return Err("noAdmin");
        }
        if r != ERROR_SUCCESS && r != ERROR_ALREADY_EXISTS {
            return Err("error");
        }
        unsafe {
            for (guid, kw) in [(DXGI, 0u64), (D3D9, 0), (DXGKRNL, DXGKRNL_PRESENT)] {
                let _ = EnableTraceEx2(handle, &guid, EVENT_CONTROL_CODE_ENABLE_PROVIDER.0, 4, kw, 0, 0, None);
            }
        }
        std::thread::spawn(|| unsafe {
            let mut name: Vec<u16> = "IslandFpsSession\0".encode_utf16().collect();
            let mut log = EVENT_TRACE_LOGFILEW::default();
            log.LoggerName = PWSTR(name.as_mut_ptr());
            log.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
            log.Anonymous2.EventRecordCallback = Some(on_event);
            let h: PROCESSTRACE_HANDLE = OpenTraceW(&mut log);
            if h.Value == u64::MAX {
                return;
            }
            let _ = ProcessTrace(&[h], None, None); // bloque jusqu'à l'arrêt de la session
            let _ = CloseTrace(h);
            if let Some(rt) = RT.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                rt.running = false;
            }
        });
        // Arrêt après 10 s sans demande.
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(2));
            let idle = RT.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|r| r.last_poll.elapsed() > IDLE_STOP).unwrap_or(true);
            if idle {
                stop_session();
                if let Some(rt) = RT.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                    rt.running = false;
                    rt.prev = None;
                }
                *COUNTS.lock().unwrap_or_else(|p| p.into_inner()) = None;
                return;
            }
        });
        Ok(())
    }

    fn foreground_pid() -> u32 {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid)) };
        pid
    }

    /// Le compte est-il dans le groupe (sans élévation, via le jeton courant) ?
    fn in_group_token() -> bool {
        use windows::Win32::Security::{CheckTokenMembership, CreateWellKnownSid, WinBuiltinPerfLoggingUsersSid, PSID};
        let mut buf = [0u8; 68];
        let mut size = buf.len() as u32;
        let sid = PSID(buf.as_mut_ptr() as *mut _);
        let mut member = windows::core::BOOL(0);
        unsafe {
            CreateWellKnownSid(WinBuiltinPerfLoggingUsersSid, None, Some(sid), &mut size).is_ok()
                && CheckTokenMembership(None, sid, &mut member).is_ok()
                && member.as_bool()
        }
    }

    /// FPS du jeu au premier plan. Le front l'appelle chaque seconde en mode jeu.
    #[tauri::command]
    pub fn fps_sample() -> FpsSample {
        let mut g = RT.lock().unwrap_or_else(|p| p.into_inner());
        let rt = g.get_or_insert_with(|| Runtime {
            running: false,
            status: "starting",
            last_poll: Instant::now(),
            last_error_at: None,
            prev: None,
            smooth: HashMap::new(),
        });
        rt.last_poll = Instant::now();
        if !rt.running {
            let retry = rt.last_error_at.map(|t| t.elapsed() >= RETRY).unwrap_or(true);
            if !retry {
                return FpsSample { status: rt.status, fps: None };
            }
            drop(g);
            let r = start_session();
            let mut g = RT.lock().unwrap_or_else(|p| p.into_inner());
            let rt = g.as_mut().unwrap();
            match r {
                Ok(()) => {
                    rt.running = true;
                    rt.status = "ok";
                    rt.last_error_at = None;
                }
                Err(e) => {
                    // Déjà ajouté au groupe mais pas encore dans le jeton : reconnexion.
                    rt.status = if e == "noAdmin" && super::member_stored() { "relogin" } else { e };
                    rt.last_error_at = Some(Instant::now());
                }
            }
            return FpsSample { status: rt.status, fps: None };
        }

        let pid = foreground_pid();
        let now = Instant::now();
        let counts: HashMap<u32, u64> = COUNTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|m| m.iter().filter(|((p, _), _)| *p == pid).map(|((_, k), v)| (*k, *v)).collect())
            .unwrap_or_default();
        let fps = match &rt.prev {
            Some((t, prev_pid, prev)) if *prev_pid == pid && t.elapsed() >= Duration::from_millis(250) => {
                let dt = t.elapsed().as_secs_f32();
                let frames = counts.iter().map(|(k, v)| v.saturating_sub(*prev.get(k).unwrap_or(&0))).max().unwrap_or(0);
                (frames > 0).then(|| {
                    let raw = frames as f32 / dt;
                    let s = rt.smooth.get(&pid).map(|old| old * 0.4 + raw * 0.6).unwrap_or(raw);
                    rt.smooth.insert(pid, s);
                    s
                })
            }
            _ => None,
        };
        rt.prev = Some((now, pid, counts));
        FpsSample { status: "ok", fps }
    }

    /// Relance l'exe élevé (UAC au nom de l'app) pour ajouter l'utilisateur au groupe.
    /// Résultat : ok | relogin | cancelled | error.
    #[tauri::command]
    pub async fn fps_unlock() -> &'static str {
        tauri::async_runtime::spawn_blocking(|| {
            use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
            use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
            use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
            let Some(sid) = super::current_user_sid() else { return "error" };
            let exe: Vec<u16> = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
                .encode_utf16()
                .chain([0])
                .collect();
            let params: Vec<u16> = format!("{ELEVATE_ARG} {sid}").encode_utf16().chain([0]).collect();
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS,
                lpVerb: w!("runas"),
                lpFile: PCWSTR(exe.as_ptr()),
                lpParameters: PCWSTR(params.as_ptr()),
                nShow: 0,
                ..Default::default()
            };
            unsafe {
                if ShellExecuteExW(&mut info).is_err() {
                    let e = windows::Win32::Foundation::GetLastError();
                    return if e == ERROR_CANCELLED { "cancelled" } else { "error" };
                }
                WaitForSingleObject(info.hProcess, INFINITE);
                let mut code = 1u32;
                let _ = GetExitCodeProcess(info.hProcess, &mut code);
                let _ = CloseHandle(info.hProcess);
                if code != 0 {
                    return "error";
                }
            }
            // Autorisation : relancer la session tout de suite ; sinon reconnexion requise.
            if let Some(rt) = RT.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                rt.last_error_at = None;
            }
            if in_group_token() { "ok" } else { "relogin" }
        })
        .await
        .unwrap_or("error")
    }

    /// Point d'entrée de la relance élevée : ajoute le SID au groupe puis quitte.
    /// Code de sortie 0 = succès (« déjà membre » compris).
    pub fn elevated_main(sid: &str) -> i32 {
        if super::add_to_group(sid) { 0 } else { 1 }
    }
}

// --- Groupe « Utilisateurs du journal de performances » -------------------------------

#[cfg(target_os = "windows")]
fn group_name() -> Option<Vec<u16>> {
    use windows::Win32::Security::{CreateWellKnownSid, LookupAccountSidW, WinBuiltinPerfLoggingUsersSid, PSID, SID_NAME_USE};
    use windows::core::{PCWSTR, PWSTR};
    let mut buf = [0u8; 68];
    let mut size = buf.len() as u32;
    let sid = PSID(buf.as_mut_ptr() as *mut _);
    let mut name = vec![0u16; 256];
    let mut name_len = name.len() as u32;
    let mut domain = vec![0u16; 256];
    let mut domain_len = domain.len() as u32;
    let mut use_ = SID_NAME_USE::default();
    unsafe {
        CreateWellKnownSid(WinBuiltinPerfLoggingUsersSid, None, Some(sid), &mut size).ok()?;
        LookupAccountSidW(
            PCWSTR::null(),
            sid,
            Some(PWSTR(name.as_mut_ptr())),
            &mut name_len,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_len,
            &mut use_,
        )
        .ok()?;
    }
    name.truncate(name_len as usize);
    name.push(0);
    Some(name)
}

/// SID (chaîne) de l'utilisateur courant.
#[cfg(target_os = "windows")]
fn current_user_sid() -> Option<String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut size);
        let mut buf = vec![0u8; size as usize];
        let ok = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), size, &mut size).is_ok();
        let _ = CloseHandle(token);
        if !ok {
            return None;
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = windows::core::PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s).ok()?;
        let out = s.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(s.0 as *mut _)));
        out
    }
}

/// Ajoute le SID au groupe (« déjà membre » = succès). Appelé dans le process élevé.
#[cfg(target_os = "windows")]
fn add_to_group(sid: &str) -> bool {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::NetworkManagement::NetManagement::{NetLocalGroupAddMembers, LOCALGROUP_MEMBERS_INFO_0};
    use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows::Win32::Security::PSID;
    const ERROR_MEMBER_IN_ALIAS: u32 = 1378;
    let Some(group) = group_name() else { return false };
    let mut psid = PSID::default();
    unsafe {
        if ConvertStringSidToSidW(&HSTRING::from(sid), &mut psid).is_err() {
            return false;
        }
        let info = LOCALGROUP_MEMBERS_INFO_0 { lgrmi0_sid: psid };
        let r = NetLocalGroupAddMembers(PCWSTR::null(), PCWSTR(group.as_ptr()), 0, &info as *const _ as *const u8, 1);
        r == 0 || r == ERROR_MEMBER_IN_ALIAS
    }
}

/// Le compte figure-t-il déjà dans le groupe (même si le jeton ne le reflète pas encore) ?
#[cfg(target_os = "windows")]
fn member_stored() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetLocalGroupGetMembers, LOCALGROUP_MEMBERS_INFO_0};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    let (Some(group), Some(me)) = (group_name(), current_user_sid()) else { return false };
    unsafe {
        let mut buf: *mut u8 = std::ptr::null_mut();
        let (mut read, mut total) = (0u32, 0u32);
        if NetLocalGroupGetMembers(PCWSTR::null(), PCWSTR(group.as_ptr()), 0, &mut buf, u32::MAX, &mut read, &mut total, None) != 0 {
            return false;
        }
        if buf.is_null() || read == 0 {
            if !buf.is_null() {
                let _ = NetApiBufferFree(Some(buf as *const _));
            }
            return false; // groupe vide
        }
        let list = std::slice::from_raw_parts(buf as *const LOCALGROUP_MEMBERS_INFO_0, read as usize);
        let found = list.iter().any(|m| {
            let mut s = windows::core::PWSTR::null();
            ConvertSidToStringSidW(m.lgrmi0_sid, &mut s).is_ok() && s.to_string().map(|x| x == me).unwrap_or(false)
        });
        let _ = NetApiBufferFree(Some(buf as *const _));
        found
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn probe_session() {
        let a = super::imp::fps_sample();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let b = super::imp::fps_sample();
        println!("first = {}", serde_json::to_string(&a).unwrap());
        println!("second = {}", serde_json::to_string(&b).unwrap());
        println!("member_stored = {}", super::member_stored());
        println!("user sid = {:?}", super::current_user_sid());
    }
}
