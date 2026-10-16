// Primitives natives : relevé des process (Toolhelp + chemin image, accès
// `PROCESS_QUERY_LIMITED_INFORMATION` SEULEMENT), fenêtres, hook de premier plan
// (`SetWinEventHook`, hors contexte, fil avec boucle de messages), inactivité, plein écran,
// description de fichier (ressources de l'exe SUR DISQUE). Jamais de lecture mémoire ni
// d'énumération de modules d'un autre process (sûr vis-à-vis des anti-triche).

#[cfg(target_os = "windows")]
pub use imp::*;
#[cfg(not(target_os = "windows"))]
pub use stub::*;

#[cfg(target_os = "windows")]
mod imp {
    use windows::core::{BOOL, HSTRING, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM, RECT};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EnumWindows, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindow,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, TranslateMessage,
        EVENT_SYSTEM_FOREGROUND, GW_OWNER, MSG, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    };

    /// (pid, nom d'exe) de tous les process.
    pub fn snapshot() -> Vec<(u32, String)> {
        let mut out = Vec::with_capacity(400);
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            if Process32FirstW(snap, &mut e).is_ok() {
                loop {
                    let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                    out.push((e.th32ProcessID, String::from_utf16_lossy(&e.szExeFile[..n])));
                    if Process32NextW(snap, &mut e).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    /// Chemin complet de l'exe (accès limité : pas de lecture mémoire).
    pub fn image_path(pid: u32) -> Option<String> {
        if pid == 0 || pid == 4 {
            return None;
        }
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; 1024];
            let mut len = buf.len() as u32;
            let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(h);
            r.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    pub fn foreground() -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }

    pub fn window_pid(hwnd: isize) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid)) };
        pid
    }

    pub fn window_title(hwnd: isize) -> String {
        unsafe {
            let h = HWND(hwnd as *mut _);
            let len = GetWindowTextLengthW(h);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(h, &mut buf);
            String::from_utf16_lossy(&buf[..n.max(0) as usize])
        }
    }

    /// Bureau / barre des tâches : pas une « app au premier plan ».
    pub fn is_shell_window(hwnd: isize) -> bool {
        let mut cls = [0u16; 64];
        let n = unsafe { GetClassNameW(HWND(hwnd as *mut _), &mut cls) };
        let c = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
        matches!(c.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
    }

    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
        let out = unsafe { &mut *(lp.0 as *mut Vec<(u32, isize, String)>) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
                return true.into();
            }
        }
        let title = window_title(hwnd.0 as isize);
        if !title.is_empty() {
            out.push((window_pid(hwnd.0 as isize), hwnd.0 as isize, title));
        }
        true.into()
    }

    /// Fenêtres de premier niveau visibles et titrées : (pid, hwnd, titre).
    pub fn visible_windows() -> Vec<(u32, isize, String)> {
        let mut v: Vec<(u32, isize, String)> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut v as *mut _ as isize));
        }
        v
    }

    /// Millisecondes depuis la dernière entrée clavier/souris (session).
    pub fn idle_ms() -> u64 {
        use windows::Win32::System::SystemInformation::GetTickCount;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        unsafe {
            let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
            if !GetLastInputInfo(&mut li).as_bool() {
                return 0;
            }
            GetTickCount().wrapping_sub(li.dwTime) as u64
        }
    }

    /// Décalage local − UTC en secondes (heure d'été comprise), arrondi à la minute.
    pub fn local_offset_secs() -> i64 {
        use windows::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
        use windows::Win32::System::Time::SystemTimeToFileTime;
        fn ft(f: FILETIME) -> i64 {
            ((f.dwHighDateTime as i64) << 32) | f.dwLowDateTime as i64
        }
        unsafe {
            let (l, u) = (GetLocalTime(), GetSystemTime());
            let (mut fl, mut fu) = (FILETIME::default(), FILETIME::default());
            if SystemTimeToFileTime(&l, &mut fl).is_err() || SystemTimeToFileTime(&u, &mut fu).is_err() {
                return 0;
            }
            let secs = (ft(fl) - ft(fu)) / 10_000_000;
            ((secs as f64 / 60.0).round() as i64) * 60
        }
    }

    /// Fenêtre plein écran (exclusif ou sans bordure) : signal officiel de Windows
    /// (`SHQueryUserNotificationState`) + la fenêtre couvre son moniteur.
    pub fn is_fullscreen(hwnd: isize) -> bool {
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        unsafe {
            let Ok(state) = SHQueryUserNotificationState() else { return false };
            if !(state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE) {
                return false;
            }
            let h = HWND(hwnd as *mut _);
            let mut r = RECT::default();
            if GetWindowRect(h, &mut r).is_err() {
                return false;
            }
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut mi).as_bool() {
                return false;
            }
            let m = mi.rcMonitor;
            r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
        }
    }

    /// `FileDescription` des ressources de version de l'exe (lecture du fichier).
    pub fn file_description(path: &str) -> Option<String> {
        use windows::core::w;
        use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
        unsafe {
            let wp = HSTRING::from(path);
            let size = GetFileVersionInfoSizeW(&wp, None);
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            GetFileVersionInfoW(&wp, None, size, buf.as_mut_ptr() as _).ok()?;
            let mut subs = Vec::new();
            let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut len = 0u32;
            if VerQueryValueW(buf.as_ptr() as _, w!("\\VarFileInfo\\Translation"), &mut p, &mut len).as_bool()
                && len >= 4
                && !p.is_null()
            {
                let t = p as *const u16;
                subs.push(format!("{:04x}{:04x}", *t, *t.add(1)));
            }
            subs.extend(["040904b0".to_string(), "040904e4".to_string(), "000004b0".to_string()]);
            for s in subs {
                let q = HSTRING::from(format!("\\StringFileInfo\\{s}\\FileDescription"));
                let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
                let mut len = 0u32;
                if VerQueryValueW(buf.as_ptr() as _, &q, &mut p, &mut len).as_bool() && len > 0 && !p.is_null() {
                    let sl = std::slice::from_raw_parts(p as *const u16, len as usize);
                    let n = sl.iter().position(|&c| c == 0).unwrap_or(sl.len());
                    let d = String::from_utf16_lossy(&sl[..n]).trim().to_string();
                    if !d.is_empty() {
                        return Some(d);
                    }
                }
            }
            None
        }
    }

    /// Icône de l'exe (PNG data URL). COM doit être initialisé sur le fil appelant.
    pub fn icon(path: &str) -> Option<String> {
        crate::services::apps::windows::icon_data_url(path, 32)
    }

    pub fn com_init() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
    }

    static CALLBACK: std::sync::OnceLock<fn(isize)> = std::sync::OnceLock::new();

    unsafe extern "system" fn on_event(
        _h: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        idobject: i32,
        _idchild: i32,
        _thread: u32,
        _time: u32,
    ) {
        // OBJID_WINDOW = 0 ; jamais de panique à travers la frontière FFI.
        if idobject == 0 && !hwnd.0.is_null() {
            if let Some(cb) = CALLBACK.get() {
                let h = hwnd.0 as isize;
                let _ = std::panic::catch_unwind(|| cb(h));
            }
        }
    }

    /// Fil dédié : hook `EVENT_SYSTEM_FOREGROUND` hors contexte + boucle de messages
    /// (bloquée dans `GetMessageW` : coût nul au repos). COM initialisé (icônes).
    pub fn spawn_foreground_hook(cb: fn(isize)) {
        let _ = CALLBACK.set(cb);
        let _ = std::thread::Builder::new().name("activity-fg".into()).spawn(move || unsafe {
            com_init();
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if hook.is_invalid() {
                eprintln!("[activity] SetWinEventHook a échoué");
                return;
            }
            // État initial.
            let fg = foreground();
            if fg != 0 {
                let _ = std::panic::catch_unwind(|| cb(fg));
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }
}

#[cfg(not(target_os = "windows"))]
mod stub {
    pub fn snapshot() -> Vec<(u32, String)> {
        Vec::new()
    }
    pub fn image_path(_pid: u32) -> Option<String> {
        None
    }
    pub fn foreground() -> isize {
        0
    }
    pub fn window_pid(_hwnd: isize) -> u32 {
        0
    }
    pub fn window_title(_hwnd: isize) -> String {
        String::new()
    }
    pub fn is_shell_window(_hwnd: isize) -> bool {
        false
    }
    pub fn visible_windows() -> Vec<(u32, isize, String)> {
        Vec::new()
    }
    pub fn idle_ms() -> u64 {
        0
    }
    pub fn local_offset_secs() -> i64 {
        0
    }
    pub fn is_fullscreen(_hwnd: isize) -> bool {
        false
    }
    pub fn file_description(_path: &str) -> Option<String> {
        None
    }
    pub fn icon(_path: &str) -> Option<String> {
        None
    }
    pub fn com_init() {}
    pub fn spawn_foreground_hook(_cb: fn(isize)) {}
}
