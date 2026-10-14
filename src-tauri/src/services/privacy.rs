// Voyants de confidentialité : micro, caméra, capture d'écran en cours d'utilisation.
// Source : le registre que Windows lui-même utilise pour « activité récente » des
// paramètres de confidentialité — `HKCU\…\CapabilityAccessManager\ConsentStore\<capacité>`,
// une sous-clé par app (apps de bureau un niveau plus bas, sous `NonPackaged`) avec
// `LastUsedTimeStart` / `LastUsedTimeStop` : en cours = Start > 0 et Stop = 0.
// Capacités : microphone, webcam, graphicsCaptureProgrammatic (Windows.Graphics.Capture :
// Teams, Discord, Outil Capture, OBS selon la source). Notre propre exe est exclu.

#[derive(serde::Serialize, Default)]
pub struct Privacy {
    mic: bool,
    camera: bool,
    screen: bool,
}

#[tauri::command]
pub fn privacy_state() -> Privacy {
    #[cfg(target_os = "windows")]
    return Privacy {
        mic: imp::in_use("microphone"),
        camera: imp::in_use("webcam"),
        screen: imp::in_use("graphicsCaptureProgrammatic"),
    };
    #[allow(unreachable_code)]
    Privacy::default()
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    const BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

    fn open(parent: HKEY, path: &str) -> Option<HKEY> {
        let mut k = HKEY::default();
        unsafe { RegOpenKeyExW(parent, &HSTRING::from(path), Some(0), KEY_READ, &mut k).ok().ok()? };
        Some(k)
    }

    fn subkeys(k: HKEY) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        loop {
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let r = unsafe { RegEnumKeyExW(k, i, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None) };
            if r.is_err() {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
            i += 1;
        }
        out
    }

    fn qword(k: HKEY, name: &str) -> u64 {
        let mut v = 0u64;
        let mut size = 8u32;
        let r = unsafe {
            RegQueryValueExW(k, PCWSTR(HSTRING::from(name).as_ptr()), None, None, Some(&mut v as *mut u64 as *mut u8), Some(&mut size))
        };
        if r.is_ok() { v } else { 0 }
    }

    /// L'app sous `k` utilise-t-elle la capacité en ce moment ?
    fn active(k: HKEY) -> bool {
        qword(k, "LastUsedTimeStart") > 0 && qword(k, "LastUsedTimeStop") == 0
    }

    fn me() -> String {
        std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "#").to_lowercase()).unwrap_or_default()
    }

    pub fn in_use(capability: &str) -> bool {
        let Some(root) = open(HKEY_CURRENT_USER, &format!(r"{BASE}\{capability}")) else { return false };
        let me = me();
        let mut found = false;
        for name in subkeys(root) {
            let Some(k) = open(root, &name) else { continue };
            if name == "NonPackaged" {
                for app in subkeys(k) {
                    if app.to_lowercase() == me {
                        continue;
                    }
                    if let Some(a) = open(k, &app) {
                        found |= active(a);
                        unsafe {
                            let _ = RegCloseKey(a);
                        }
                    }
                }
            } else {
                found |= active(k);
            }
            unsafe {
                let _ = RegCloseKey(k);
            }
            if found {
                break;
            }
        }
        unsafe {
            let _ = RegCloseKey(root);
        }
        found
    }
}
