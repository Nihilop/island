// Conscience des fenêtres via Win32 : premier plan, énumération, focus.

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

fn title_of(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

/// Nom d'exécutable (sans chemin ni extension) du process propriétaire de `hwnd`.
fn app_of(hwnd: HWND) -> String {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..size as usize]).to_lowercase();
        let base = full.rsplit(['\\', '/']).next().unwrap_or(&full);
        base.strip_suffix(".exe").unwrap_or(base).to_string()
    }
}

pub fn foreground() -> Option<(i64, String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        Some((hwnd.0 as i64, title_of(hwnd), app_of(hwnd)))
    }
}

struct Collector(Vec<(i64, String, String)>);

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let col = &mut *(lparam.0 as *mut Collector);
    if IsWindowVisible(hwnd).as_bool() {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let title = title_of(hwnd);
        // top-level visible, titrée, et pas une "tool window" (palettes, etc.)
        if !title.is_empty() && ex & WS_EX_TOOLWINDOW.0 == 0 {
            col.0.push((hwnd.0 as i64, title, app_of(hwnd)));
        }
    }
    BOOL(1) // continuer l'énumération
}

pub fn list() -> Vec<(i64, String, String)> {
    let mut col = Collector(Vec::new());
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut col as *mut _ as isize));
    }
    col.0
}

pub fn focus(id: i64) -> Result<(), String> {
    unsafe {
        let hwnd = HWND(id as *mut core::ffi::c_void);
        if SetForegroundWindow(hwnd).as_bool() {
            Ok(())
        } else {
            Err("windows: mise au premier plan refusée".into())
        }
    }
}
