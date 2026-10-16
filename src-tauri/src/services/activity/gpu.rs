// Signal « jeu probable » : utilisation GPU 3D par process via PDH (compteur générique
// `\GPU Engine(*engtype_3D)\Utilization Percentage`, instances `pid_<pid>_…`). Requête
// ouverte UNIQUEMENT pendant qu'un candidat est au premier plan, fermée sinon.

use std::collections::HashMap;

/// Seuil d'utilisation 3D (%) et durée soutenue (échantillons de 2 s → 10 s).
pub const THRESHOLD: f64 = 25.0;
pub const SUSTAIN: u32 = 5;

/// `pid_1234_luid_0x…_engtype_3D` → 1234.
pub fn parse_pid(instance: &str) -> Option<u32> {
    let rest = instance.strip_prefix("pid_")?;
    rest.split('_').next()?.parse().ok()
}

/// Compteur d'échantillons consécutifs au-dessus du seuil pour UN pid.
#[derive(Default)]
pub struct Sustain {
    pid: u32,
    hits: u32,
}

impl Sustain {
    /// Vrai quand le seuil est tenu `SUSTAIN` échantillons d'affilée.
    pub fn feed(&mut self, pid: u32, util: f64) -> bool {
        if pid != self.pid {
            self.pid = pid;
            self.hits = 0;
        }
        self.hits = if util >= THRESHOLD { self.hits + 1 } else { 0 };
        self.hits >= SUSTAIN
    }
    pub fn reset(&mut self) {
        self.hits = 0;
    }
}

#[cfg(target_os = "windows")]
pub struct Gpu {
    query: windows::Win32::System::Performance::PDH_HQUERY,
    counter: windows::Win32::System::Performance::PDH_HCOUNTER,
}

// Handles PDH non liés à un thread ; utilisés par le seul fil de relevé.
#[cfg(target_os = "windows")]
unsafe impl Send for Gpu {}

#[cfg(target_os = "windows")]
impl Gpu {
    pub fn open() -> Option<Self> {
        use windows::core::{w, PCWSTR};
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhOpenQueryW, PDH_HCOUNTER, PDH_HQUERY,
        };
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return None;
            }
            let mut counter = PDH_HCOUNTER::default();
            if PdhAddEnglishCounterW(query, w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"), 0, &mut counter) != 0 {
                PdhCloseQuery(query);
                return None;
            }
            PdhCollectQueryData(query); // référence (un taux demande deux relevés)
            Some(Self { query, counter })
        }
    }

    /// Utilisation 3D (%) par pid (somme des moteurs / adaptateurs).
    pub fn sample(&self) -> HashMap<u32, f64> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterArrayW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
        };
        let mut out = HashMap::new();
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return out;
            }
            let mut size = 0u32;
            let mut count = 0u32;
            PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if size == 0 {
                return out;
            }
            // u64 : alignement des items (contiennent des f64 / pointeurs).
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
                return out;
            }
            for it in std::slice::from_raw_parts(items, count as usize) {
                if it.FmtValue.CStatus != 0 {
                    continue;
                }
                let Ok(name) = it.szName.to_string() else { continue };
                if let Some(pid) = parse_pid(&name) {
                    *out.entry(pid).or_insert(0.0) += it.FmtValue.Anonymous.doubleValue;
                }
            }
        }
        out
    }
}

#[cfg(target_os = "windows")]
impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Performance::PdhCloseQuery(self.query);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub struct Gpu;

#[cfg(not(target_os = "windows"))]
impl Gpu {
    pub fn open() -> Option<Self> {
        None
    }
    pub fn sample(&self) -> HashMap<u32, f64> {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_and_sustain() {
        assert_eq!(parse_pid("pid_1234_luid_0x00000000_0x0000D1F3_phys_0_eng_0_engtype_3D"), Some(1234));
        assert_eq!(parse_pid("luid_0x0"), None);
        let mut s = Sustain::default();
        for _ in 0..4 {
            assert!(!s.feed(7, 60.0));
        }
        assert!(s.feed(7, 60.0));
        assert!(!s.feed(7, 10.0)); // chute → remise à zéro
        assert!(!s.feed(8, 99.0)); // autre pid → repart de zéro
    }
}
