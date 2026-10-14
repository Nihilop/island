// Apps ouvertes, pour choisir celles qui suspendent le launcher : process qui ont une
// fenêtre visible, hors Windows lui-même (`C:\Windows\…`) et hors Island. Un exécutable =
// une ligne (nom de fichier, titre de fenêtre, icône).

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proc {
    /// Nom de l'exécutable (`UnrealEditor.exe`) : c'est lui qui sert de filtre.
    exe: String,
    /// Titre d'une de ses fenêtres (pour reconnaître l'app).
    title: String,
    icon: Option<String>,
}

#[tauri::command]
pub async fn launcher_processes() -> Result<Vec<Proc>, String> {
    tauri::async_runtime::spawn_blocking(imp::list).await.map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
mod imp {
    use std::collections::HashMap;

    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    use super::Proc;

    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
        let out = unsafe { &mut *(lp.0 as *mut Vec<(u32, String)>) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
                return true.into();
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return true.into();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(hwnd, &mut buf);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            out.push((pid, String::from_utf16_lossy(&buf[..n as usize])));
        }
        true.into()
    }

    fn image_path(pid: u32) -> Option<String> {
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

    pub fn list() -> Vec<Proc> {
        let mut wins: Vec<(u32, String)> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut wins as *mut _ as isize));
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let me = std::process::id();
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()).to_lowercase();
        let mut by_exe: HashMap<String, Proc> = HashMap::new();
        for (pid, title) in wins {
            if pid == me {
                continue;
            }
            let Some(path) = image_path(pid) else { continue };
            if path.to_lowercase().starts_with(&windir) {
                continue;
            }
            let exe = path.rsplit('\\').next().unwrap_or(&path).to_string();
            by_exe.entry(exe.to_lowercase()).or_insert_with(|| Proc {
                icon: crate::services::apps::windows::icon_data_url(&path, 32),
                exe,
                title,
            });
        }
        let mut v: Vec<Proc> = by_exe.into_values().collect();
        v.sort_by_key(|p| p.exe.to_lowercase());
        v
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn list() -> Vec<super::Proc> {
        Vec::new()
    }
}
