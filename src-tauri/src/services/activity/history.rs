// Historique : secondes au premier plan par jeu (toujours) et par app (si `trackApps`),
// agrégées par jour local dans `<app_data>/activity/AAAA-MM.json`
// (`{ "AAAA-MM-JJ": { games: { key: { name, seconds } }, apps: { … } } }`). Mois touchés
// gardés en mémoire, écrits atomiquement (temp + rename) ; purge au-delà de 12 mois.
// Calendrier civil fait main (pas de dépendance date).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub name: String,
    pub seconds: u64,
    /// Reste en millisecondes (non persisté).
    #[serde(skip)]
    pub ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Day {
    #[serde(default)]
    pub games: BTreeMap<String, Entry>,
    #[serde(default)]
    pub apps: BTreeMap<String, Entry>,
}

pub type Month = BTreeMap<String, Day>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bucket {
    Games,
    Apps,
}

// --- Calendrier ----------------------------------------------------------------------------

/// Jours depuis 1970-01-01 → (année, mois, jour) (algorithme de H. Hinnant).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Date locale `AAAA-MM-JJ` d'un instant (ms epoch) pour un décalage UTC donné.
pub fn date_of(epoch_ms: u64, offset_s: i64) -> String {
    let local = (epoch_ms / 1000) as i64 + offset_s;
    let (y, m, d) = civil_from_days(local.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Découpe [from, to) en morceaux par jour local : (date, ms).
pub fn split_days(from_ms: u64, to_ms: u64, offset_s: i64) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut t = from_ms;
    let off_ms = offset_s * 1000;
    while t < to_ms {
        let local = t as i64 + off_ms;
        let next_midnight = (local.div_euclid(86_400_000) + 1) * 86_400_000 - off_ms;
        let end = (next_midnight.max(0) as u64).min(to_ms).max(t + 1);
        out.push((date_of(t, offset_s), end - t));
        t = end;
    }
    out
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// `AAAA-MM` → index de mois (année × 12 + mois - 1).
fn month_index(ym: &str) -> Option<i64> {
    let (y, m) = ym.split_once('-')?;
    let (y, m): (i64, i64) = (y.parse().ok()?, m.parse().ok()?);
    (ym.len() == 7 && (1..=12).contains(&m)).then_some(y * 12 + m - 1)
}

fn month_name(idx: i64) -> String {
    format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1)
}

/// Fichiers de mois (`AAAA-MM.json`) à supprimer : plus de 12 mois avant `current`.
pub fn months_to_purge(files: &[String], current: &str) -> Vec<String> {
    let Some(cur) = month_index(current) else { return Vec::new() };
    files
        .iter()
        .filter(|f| {
            f.strip_suffix(".json").and_then(month_index).is_some_and(|i| i < cur - 12)
        })
        .cloned()
        .collect()
}

// --- Stats (réponse de `activity_stats`) ---------------------------------------------------

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub name: String,
    pub seconds: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DayStats {
    pub date: String,
    pub games: Vec<Item>,
    pub apps: Vec<Item>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Stats {
    pub days: Vec<DayStats>,
}

fn items(m: &BTreeMap<String, Entry>) -> Vec<Item> {
    let mut v: Vec<Item> = m
        .iter()
        .filter(|(_, e)| e.seconds > 0)
        .map(|(k, e)| Item { key: k.clone(), name: e.name.clone(), seconds: e.seconds })
        .collect();
    v.sort_by(|a, b| b.seconds.cmp(&a.seconds).then_with(|| a.name.cmp(&b.name)));
    v
}

// --- Stockage ------------------------------------------------------------------------------

pub struct History {
    dir: Option<PathBuf>,
    months: HashMap<String, Month>,
    dirty: HashSet<String>,
}

pub fn write_atomic(path: &std::path::Path, content: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

impl History {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir, months: HashMap::new(), dirty: HashSet::new() }
    }

    fn file(&self, ym: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{ym}.json")))
    }

    fn read_month(&self, ym: &str) -> Month {
        self.file(ym)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn month_mut(&mut self, ym: &str) -> &mut Month {
        if !self.months.contains_key(ym) {
            // Charger l'existant AVANT d'ajouter (sinon on écraserait le fichier).
            let m = self.read_month(ym);
            self.months.insert(ym.to_string(), m);
        }
        self.months.get_mut(ym).expect("mois chargé")
    }

    /// Ajoute [from, to) à `key`, découpé par jour local.
    pub fn add(&mut self, bucket: Bucket, key: &str, name: &str, from_ms: u64, to_ms: u64, offset_s: i64) {
        for (date, ms) in split_days(from_ms, to_ms, offset_s) {
            let ym = date[..7].to_string();
            let day = self.month_mut(&ym).entry(date).or_default();
            let map = match bucket {
                Bucket::Games => &mut day.games,
                Bucket::Apps => &mut day.apps,
            };
            let e = map.entry(key.to_string()).or_default();
            e.name = name.to_string();
            e.ms += ms;
            e.seconds += e.ms / 1000;
            e.ms %= 1000;
            self.dirty.insert(ym);
        }
    }

    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Écrit les mois modifiés ; ne garde en mémoire que le mois courant.
    pub fn flush(&mut self, current_ym: &str) {
        for ym in std::mem::take(&mut self.dirty) {
            let (Some(p), Some(m)) = (self.file(&ym), self.months.get(&ym)) else { continue };
            if let Ok(json) = serde_json::to_vec(m) {
                if let Err(e) = write_atomic(&p, &json) {
                    eprintln!("[activity] historique {ym} : {e}");
                    self.dirty.insert(ym);
                }
            }
        }
        let dirty = &self.dirty;
        self.months.retain(|ym, _| ym == current_ym || dirty.contains(ym));
    }

    /// Supprime les mois de plus de 12 mois (fichiers et mémoire).
    pub fn purge(&mut self, current_ym: &str) {
        let Some(dir) = self.dir.clone() else { return };
        let names: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        for f in months_to_purge(&names, current_ym) {
            let _ = std::fs::remove_file(dir.join(&f));
            let ym = f.trim_end_matches(".json");
            self.months.remove(ym);
            self.dirty.remove(ym);
        }
    }

    /// Jours de [from, to] (inclus) ayant des données, triés par date.
    pub fn stats(&self, from: &str, to: &str) -> Result<Stats, String> {
        if !valid_date(from) || !valid_date(to) || from > to {
            return Err("badRange".into());
        }
        let (a, b) = (month_index(&from[..7]).ok_or("badRange")?, month_index(&to[..7]).ok_or("badRange")?);
        if b - a > 24 {
            return Err("badRange".into());
        }
        let mut days = Vec::new();
        for i in a..=b {
            let ym = month_name(i);
            let disk;
            let m = match self.months.get(&ym) {
                Some(m) => m,
                None => {
                    disk = self.read_month(&ym);
                    &disk
                }
            };
            for (date, d) in m.range(from.to_string()..=to.to_string()) {
                let (games, apps) = (items(&d.games), items(&d.apps));
                if !games.is_empty() || !apps.is_empty() {
                    days.push(DayStats { date: date.clone(), games, apps });
                }
            }
        }
        Ok(Stats { days })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400_000;
    // 2026-10-16T00:00:00Z
    const T0: u64 = 1_792_108_800_000;

    #[test]
    fn calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(date_of(T0, 0), "2026-10-16");
        assert_eq!(date_of(T0 - 1, 0), "2026-10-15");
        // UTC+2 : 23h UTC = 1h le lendemain.
        assert_eq!(date_of(T0 - 3_600_000, 7200), "2026-10-16");
        // UTC-5.
        assert_eq!(date_of(T0 + 3_600_000, -18_000), "2026-10-15");
        // Bissextile.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn day_rollover() {
        // 23:59:00 → 00:01:00 locale (UTC+2).
        let off = 7200;
        let midnight_local = T0 + DAY - 2 * 3_600_000; // 2026-10-17 00:00 locale
        let s = split_days(midnight_local - 60_000, midnight_local + 60_000, off);
        assert_eq!(s, vec![("2026-10-16".to_string(), 60_000), ("2026-10-17".to_string(), 60_000)]);
        assert_eq!(split_days(T0, T0 + 1000, 0), vec![("2026-10-16".to_string(), 1000)]);
        assert!(split_days(T0, T0, 0).is_empty());
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("island-activity-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn aggregation_and_flush() {
        let dir = tmpdir("agg");
        let mut h = History::new(Some(dir.clone()));
        h.add(Bucket::Games, "steam:1", "Elden Ring", T0, T0 + 1500, 0);
        h.add(Bucket::Games, "steam:1", "Elden Ring", T0 + 1500, T0 + 3000, 0);
        h.add(Bucket::Apps, "c:\\code.exe", "VS Code", T0, T0 + 10_000, 0);
        // À cheval sur fin de mois.
        let nov1 = T0 + 16 * DAY;
        h.add(Bucket::Games, "steam:1", "Elden Ring", nov1 - 5000, nov1 + 5000, 0);
        h.flush("2026-11");
        assert!(dir.join("2026-10.json").is_file() && dir.join("2026-11.json").is_file());
        assert!(!dir.join("2026-10.tmp").exists());

        // Rechargement depuis le disque (nouvelle instance) puis ajout : pas d'écrasement.
        let mut h2 = History::new(Some(dir.clone()));
        h2.add(Bucket::Games, "steam:1", "Elden Ring", T0 + 10_000, T0 + 12_000, 0);
        let s = h2.stats("2026-10-16", "2026-11-01").unwrap();
        assert_eq!(s.days.len(), 3);
        assert_eq!(s.days[0].date, "2026-10-16");
        assert_eq!(s.days[0].games, vec![Item { key: "steam:1".into(), name: "Elden Ring".into(), seconds: 5 }]);
        assert_eq!(s.days[0].apps[0].seconds, 10);
        assert_eq!(s.days[1].date, "2026-10-31");
        assert_eq!(s.days[1].games[0].seconds, 5);
        assert_eq!(s.days[2].date, "2026-11-01");
        assert!(h2.stats("2026-11-02", "2026-10-01").is_err());
        assert!(h2.stats("2026-1-01", "2026-10-01").is_err());
        assert_eq!(h2.stats("2025-01-01", "2025-01-31").unwrap().days.len(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn purge() {
        let files: Vec<String> =
            ["2025-09.json", "2025-10.json", "2025-11.json", "2026-10.json", "settings.json", "detectable.json"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert_eq!(months_to_purge(&files, "2026-10"), vec!["2025-09.json".to_string()]);
        assert_eq!(months_to_purge(&files, "2026-11"), vec!["2025-09.json".to_string(), "2025-10.json".to_string()]);

        let dir = tmpdir("purge");
        std::fs::write(dir.join("2024-01.json"), "{}").unwrap();
        std::fs::write(dir.join("2026-10.json"), "{}").unwrap();
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let mut h = History::new(Some(dir.clone()));
        h.purge("2026-10");
        assert!(!dir.join("2024-01.json").exists());
        assert!(dir.join("2026-10.json").exists() && dir.join("settings.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
