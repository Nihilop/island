// Recherche de fichiers/dossiers pour le launcher. Moteur HYBRIDE :
//  - index MAISON (toujours dispo) : scan de racines configurables, mis en cache mémoire,
//    reconstruit quand les racines changent ;
//  - backend EVERYTHING (auto) : si l'app voidtools Everything tourne, on l'interroge en
//    IPC (WM_COPYDATA) → recherche instantanée sur tout le disque. Sinon → index maison.
use super::FileEntry;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ---------- Index maison ------------------------------------------------------

struct HomeIndex {
    roots: Vec<String>,
    entries: Vec<FileEntry>,
    built: std::time::Instant,
}
static INDEX: Mutex<Option<HomeIndex>> = Mutex::new(None);

const MAX_ENTRIES: usize = 50_000;
const MAX_DEPTH: u8 = 6;
const TTL: std::time::Duration = std::time::Duration::from_secs(300); // réindex auto après 5 min

/// Racines par défaut (si l'utilisateur n'en a configuré aucune) : dossiers usuels.
fn default_roots() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(home) = std::env::var("USERPROFILE") {
        for sub in ["Desktop", "Documents", "Downloads"] {
            let p = PathBuf::from(&home).join(sub);
            if p.is_dir() {
                v.push(p.to_string_lossy().to_string());
            }
        }
    }
    v
}

fn scan(dir: &Path, out: &mut Vec<FileEntry>, depth: u8) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() >= MAX_ENTRIES {
            return;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue; // caché / système (.git, etc.)
        }
        let path = e.path();
        let is_dir = path.is_dir();
        out.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
        });
        if is_dir {
            scan(&path, out, depth + 1);
        }
    }
}

fn home_search(query: &str, roots: &[String], limit: usize) -> Vec<FileEntry> {
    let mut guard = INDEX.lock().unwrap_or_else(|e| e.into_inner());
    let stale = match &*guard {
        Some(ix) => ix.roots != roots || ix.built.elapsed() > TTL,
        None => true,
    };
    if stale {
        let mut entries = Vec::new();
        for r in roots {
            scan(Path::new(r), &mut entries, 0);
        }
        *guard = Some(HomeIndex { roots: roots.to_vec(), entries, built: std::time::Instant::now() });
    }
    let ix = guard.as_ref().unwrap();
    let q = query.to_lowercase();
    let mut hits: Vec<(&FileEntry, i32)> = ix
        .entries
        .iter()
        .filter_map(|e| {
            let n = e.name.to_lowercase();
            let score = if n == q {
                1000
            } else if n.starts_with(&q) {
                600 - n.len() as i32
            } else if n.contains(&q) {
                300 - n.len() as i32
            } else {
                return None;
            };
            Some((e, score))
        })
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1));
    hits.into_iter().take(limit).map(|(e, _)| e.clone()).collect()
}

// ---------- Façade --------------------------------------------------------

/// Recherche fichiers/dossiers : Everything (tout-disque) si dispo et non vide, sinon
/// l'index maison sur les racines fournies (ou les racines par défaut).
pub fn search(query: &str, roots: Vec<String>, limit: usize) -> Vec<FileEntry> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    if let Some(res) = everything::search(q, limit) {
        if !res.is_empty() {
            return res;
        }
    }
    let roots = if roots.is_empty() { default_roots() } else { roots };
    home_search(q, &roots, limit)
}

/// True si une instance Everything tourne (pour l'afficher dans les réglages de Flow).
pub fn everything_available() -> bool {
    everything::is_running()
}

// ---------- Backend Everything (IPC WM_COPYDATA) ------------------------------

mod everything {
    use super::FileEntry;
    use std::sync::Mutex;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW,
        PeekMessageW, RegisterClassW, SendMessageW, TranslateMessage, HWND_MESSAGE, MSG, PM_REMOVE,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_COPYDATA, WNDCLASSW,
    };

    // Type de WM_COPYDATA pour une requête Unicode (cf. ipc d'Everything).
    const COPYDATA_QUERYW: usize = 2;
    const REPLY_ID: usize = 0; // dwData que renverra Everything

    // Tampon de réponse (une requête à la fois — sérialisée par le verrou).
    static REPLY: Mutex<Option<Vec<u8>>> = Mutex::new(None);
    static QUERY_LOCK: Mutex<()> = Mutex::new(());

    pub fn is_running() -> bool {
        unsafe { FindWindowW(w!("EVERYTHING"), PCWSTR::null()).is_ok() }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_COPYDATA {
            let cds = lp.0 as *const COPYDATASTRUCT;
            if !cds.is_null() {
                let cds = unsafe { &*cds };
                if cds.dwData == REPLY_ID {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)
                    }
                    .to_vec();
                    *REPLY.lock().unwrap_or_else(|e| e.into_inner()) = Some(bytes);
                }
            }
            return LRESULT(1);
        }
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    /// Interroge Everything. None = Everything pas lancé / échec (→ fallback maison).
    pub fn search(query: &str, limit: usize) -> Option<Vec<FileEntry>> {
        let _serialize = QUERY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let ev = FindWindowW(w!("EVERYTHING"), PCWSTR::null()).ok()?;
            if ev.0.is_null() {
                return None;
            }

            // Fenêtre message-only pour recevoir la réponse.
            let cls = w!("IslandEverythingIpc");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                lpszClassName: cls,
                ..Default::default()
            };
            RegisterClassW(&wc); // idempotent : ignore « déjà enregistrée »
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                cls,
                w!("island-ev"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .ok()?;

            // EVERYTHING_IPC_QUERYW = 5 DWORD + chaîne UTF-16 terminée par 0.
            let search_w: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(&(hwnd.0 as u32).to_le_bytes()); // reply_hwnd
            buf.extend_from_slice(&(REPLY_ID as u32).to_le_bytes()); // reply_copydata_message
            buf.extend_from_slice(&0u32.to_le_bytes()); // search_flags
            buf.extend_from_slice(&0u32.to_le_bytes()); // offset
            buf.extend_from_slice(&(limit as u32).to_le_bytes()); // max_results
            for c in &search_w {
                buf.extend_from_slice(&c.to_le_bytes());
            }

            *REPLY.lock().unwrap_or_else(|e| e.into_inner()) = None;

            let cds = COPYDATASTRUCT {
                dwData: COPYDATA_QUERYW,
                cbData: buf.len() as u32,
                lpData: buf.as_ptr() as *mut _,
            };
            SendMessageW(
                ev,
                WM_COPYDATA,
                Some(WPARAM(hwnd.0 as usize)),
                Some(LPARAM(&cds as *const _ as isize)),
            );

            // Pompe les messages jusqu'à la réponse (ou timeout : ne jamais bloquer l'UI).
            let start = std::time::Instant::now();
            let mut reply = None;
            while start.elapsed() < std::time::Duration::from_millis(800) {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, Some(hwnd), 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if let Some(b) = REPLY.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    reply = Some(b);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }

            let _ = DestroyWindow(hwnd);
            Some(parse_list(&reply?, limit))
        }
    }

    /// Décode EVERYTHING_IPC_LISTW : en-tête de 7 DWORD puis `numitems` items de 3 DWORD
    /// (flags, filename_offset, path_offset) ; les offsets pointent des WCHAR* (octets,
    /// depuis le début de la struct).
    fn parse_list(b: &[u8], limit: usize) -> Vec<FileEntry> {
        if b.len() < 28 {
            return Vec::new();
        }
        let rd = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let numitems = rd(20) as usize; // 6e DWORD
        let mut out = Vec::new();
        for i in 0..numitems.min(limit) {
            let base = 28 + i * 12;
            if base + 12 > b.len() {
                break;
            }
            let flags = rd(base);
            let filename = read_wstr(b, rd(base + 4) as usize);
            let path = read_wstr(b, rd(base + 8) as usize);
            if filename.is_empty() {
                continue;
            }
            let full = if path.is_empty() {
                filename.clone()
            } else {
                format!("{path}\\{filename}")
            };
            out.push(FileEntry { name: filename, path: full, is_dir: (flags & 1) != 0 });
        }
        out
    }

    fn read_wstr(b: &[u8], byte_off: usize) -> String {
        let mut u16s = Vec::new();
        let mut o = byte_off;
        while o + 2 <= b.len() {
            let c = u16::from_le_bytes([b[o], b[o + 1]]);
            if c == 0 {
                break;
            }
            u16s.push(c);
            o += 2;
        }
        String::from_utf16_lossy(&u16s)
    }
}
