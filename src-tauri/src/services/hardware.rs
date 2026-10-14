// Matériel (outil intégré) : CPU, RAM, GPU, ping.
//   - CPU/RAM : sysinfo ;
//   - charge GPU : compteur PDH « GPU Engine(*engtype_3D)\Utilization Percentage » (même
//     valeur que le Gestionnaire des tâches, toutes marques), sommé, plafonné à 100 ;
//   - VRAM utilisée : « GPU Adapter Memory(*)\Dedicated Usage » sommé ; nom + VRAM totale =
//     adaptateur DXGI matériel ayant le plus de VRAM dédiée ;
//   - ping ICMP vers 1.1.1.1 (timeout 1 s) sur son propre thread toutes les 2 s, en pause
//     après 5 s sans demande.
// Le front pilote la cadence : `full = false` (en fond) ne lit que CPU + RAM.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    cpu: f32,
    cpu_name: String,
    mem_used: u64,
    mem_total: u64,
    gpu: Option<f32>,
    gpu_name: Option<String>,
    vram_used: Option<u64>,
    vram_total: Option<u64>,
    /// ms ; None = pas de réponse (hors ligne).
    ping: Option<u32>,
}

struct State {
    sys: sysinfo::System,
    #[cfg(target_os = "windows")]
    pdh: Option<imp::Pdh>,
    #[cfg(target_os = "windows")]
    adapter: Option<(String, u64)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Dernier ping (-1 = hors ligne, -2 = inconnu) et dernière demande.
static PING: AtomicI64 = AtomicI64::new(-2);
static PING_WANTED: Mutex<Option<Instant>> = Mutex::new(None);

/// Nom nettoyé : sans (R)/(TM), « @ fréquence », préfixe de marque, « CPU »/« Processor ».
fn clean(name: &str) -> String {
    let mut s = name.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    if let Some(i) = s.find('@') {
        s.truncate(i);
    }
    for p in ["NVIDIA ", "AMD ", "Intel ", "Intel(R) "] {
        if let Some(rest) = s.trim_start().strip_prefix(p) {
            s = rest.to_string();
        }
    }
    s = s.replace(" CPU", "").replace(" Processor", "");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ping_loop() {
    std::thread::spawn(|| loop {
        let wanted = PING_WANTED.lock().unwrap_or_else(|p| p.into_inner()).is_some_and(|t| t.elapsed() < Duration::from_secs(5));
        if wanted {
            #[cfg(target_os = "windows")]
            PING.store(imp::ping().map(|ms| ms as i64).unwrap_or(-1), Ordering::SeqCst);
        }
        std::thread::sleep(Duration::from_secs(if wanted { 2 } else { 1 }));
    });
}

#[tauri::command]
pub async fn hw_sample(full: bool) -> Sample {
    tauri::async_runtime::spawn_blocking(move || {
        let mut g = STATE.lock().unwrap_or_else(|p| p.into_inner());
        let first = g.is_none();
        let st = g.get_or_insert_with(|| State {
            sys: sysinfo::System::new(),
            #[cfg(target_os = "windows")]
            pdh: None,
            #[cfg(target_os = "windows")]
            adapter: None,
        });
        if first {
            ping_loop();
        }
        st.sys.refresh_cpu_usage();
        st.sys.refresh_memory();
        let mut s = Sample {
            cpu: st.sys.global_cpu_usage(),
            cpu_name: st.sys.cpus().first().map(|c| clean(c.brand())).unwrap_or_default(),
            mem_used: st.sys.used_memory(),
            mem_total: st.sys.total_memory(),
            ..Default::default()
        };
        if full {
            *PING_WANTED.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
            let p = PING.load(Ordering::SeqCst);
            s.ping = (p >= 0).then_some(p as u32);
            #[cfg(target_os = "windows")]
            {
                if st.adapter.is_none() {
                    st.adapter = imp::adapter();
                }
                if let Some((name, vram)) = &st.adapter {
                    s.gpu_name = Some(clean(name));
                    s.vram_total = Some(*vram);
                }
                if st.pdh.is_none() {
                    st.pdh = imp::Pdh::open();
                }
                if let Some(pdh) = &st.pdh {
                    let (gpu, vram) = pdh.read();
                    s.gpu = gpu.map(|g| g.min(100.0) as f32);
                    s.vram_used = vram.map(|v| v as u64);
                }
            }
        }
        s
    })
    .await
    .unwrap_or_default()
}

/// Ping : -1 hors ligne, -2 inconnu (pour afficher l'état « hors ligne » en rouge).
#[tauri::command]
pub fn hw_ping_state() -> i64 {
    PING.load(Ordering::SeqCst)
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W,
        PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };

    pub struct Pdh {
        query: PDH_HQUERY,
        gpu: PDH_HCOUNTER,
        vram: PDH_HCOUNTER,
    }
    // Les handles PDH ne sont pas liés à un thread ; l'accès est sérialisé par le Mutex.
    unsafe impl Send for Pdh {}

    impl Pdh {
        pub fn open() -> Option<Self> {
            unsafe {
                let mut query = PDH_HQUERY::default();
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                    return None;
                }
                let mut gpu = PDH_HCOUNTER::default();
                let mut vram = PDH_HCOUNTER::default();
                PdhAddEnglishCounterW(query, w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"), 0, &mut gpu);
                PdhAddEnglishCounterW(query, w!("\\GPU Adapter Memory(*)\\Dedicated Usage"), 0, &mut vram);
                PdhCollectQueryData(query); // premier échantillon (les taux en demandent deux)
                Some(Self { query, gpu, vram })
            }
        }

        fn sum(&self, counter: PDH_HCOUNTER) -> Option<f64> {
            unsafe {
                let mut size = 0u32;
                let mut count = 0u32;
                PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
                if size == 0 {
                    return None;
                }
                let mut buf = vec![0u8; size as usize];
                let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
                    return None;
                }
                let list = std::slice::from_raw_parts(items, count as usize);
                Some(list.iter().map(|i| i.FmtValue.Anonymous.doubleValue).sum())
            }
        }

        /// (charge GPU %, VRAM utilisée en octets).
        pub fn read(&self) -> (Option<f64>, Option<f64>) {
            unsafe {
                PdhCollectQueryData(self.query);
            }
            (self.sum(self.gpu), self.sum(self.vram))
        }
    }

    /// Adaptateur DXGI matériel ayant le plus de VRAM dédiée : (nom, VRAM).
    pub fn adapter() -> Option<(String, u64)> {
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
        unsafe {
            let f: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let mut best: Option<(String, u64)> = None;
            let mut i = 0;
            while let Ok(a) = f.EnumAdapters1(i) {
                i += 1;
                let Ok(d) = a.GetDesc1() else { continue };
                if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                    continue;
                }
                let vram = d.DedicatedVideoMemory as u64;
                if best.as_ref().map(|b| vram > b.1).unwrap_or(true) {
                    let n = d.Description.iter().position(|c| *c == 0).unwrap_or(d.Description.len());
                    best = Some((String::from_utf16_lossy(&d.Description[..n]), vram));
                }
            }
            best
        }
    }

    /// Ping ICMP vers 1.1.1.1 (timeout 1 s), en ms.
    pub fn ping() -> Option<u32> {
        use windows::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};
        unsafe {
            let h = IcmpCreateFile().ok()?;
            let addr = u32::from_le_bytes([1, 1, 1, 1]);
            let data = [0u8; 32];
            let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
            let n = IcmpSendEcho(
                h,
                addr,
                data.as_ptr() as *const _,
                data.len() as u16,
                None,
                reply.as_mut_ptr() as *mut _,
                reply.len() as u32,
                1000,
            );
            let _ = IcmpCloseHandle(h);
            if n == 0 {
                return None;
            }
            let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
            (r.Status == 0).then_some(r.RoundTripTime)
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn reads_hardware() {
        let a = super::imp::adapter();
        println!("adapter = {a:?}");
        let pdh = super::imp::Pdh::open().expect("pdh");
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("gpu/vram = {:?}", pdh.read());
        println!("ping = {:?}", super::imp::ping());
        println!("privacy = {:?}", serde_json::to_string(&super::super::privacy::privacy_state()).unwrap());
        assert!(a.is_some());
    }
}
