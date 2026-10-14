// Quotas IA (outil intégré) : limites d'abonnement lues à partir des identifiants DÉJÀ
// présents des outils CLI connectés sur le PC (rien à saisir ; les jetons ne sont jamais
// réécrits, ni renvoyés au front, ni envoyés ailleurs qu'au service qui les a émis).
//   - Claude (Claude Code) : jeton OAuth de `~/.claude/.credentials.json` → endpoint d'usage
//     OAuth d'Anthropic : session 5 h, semaine, semaine Opus/Sonnet ;
//   - ChatGPT (Codex CLI) : jeton de `~/.codex/auth.json` → endpoint d'usage du backend
//     ChatGPT ; repli : dernier événement de limites dans les sessions JSONL récentes.
// Endpoints non documentés : on garde toujours le dernier bon résultat (persisté sur
// disque) et on signale l'erreur. Cache 5 min (Claude) / 2 min ; après un 429, pause
// Retry-After (10 min minimum).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tauri::{AppHandle, Manager};

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    /// session | week | weekOpus | weekSonnet | day | month
    kind: String,
    used: f64,
    /// Reset (secondes epoch), si connu.
    resets_at: Option<u64>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    provider: String,
    plan: Option<String>,
    windows: Vec<Window>,
    /// notFound | notLoggedIn | expired | rateLimited | offline | failed
    error: Option<String>,
    /// Date (s) du dernier bon résultat.
    fetched_at: u64,
}

struct Slot {
    last_try: u64,
    pause_until: u64,
    data: Option<ProviderUsage>,
}

static CACHE: Mutex<Option<HashMap<String, Slot>>> = Mutex::new(None);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
}

fn iso_to_epoch(s: &str) -> Option<u64> {
    // AAAA-MM-JJTHH:MM:SS(.fff)(Z|±HH:MM) → secondes epoch (UTC).
    let (date, rest) = s.split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
    let mut t = time.split(':').map(|x| x.parse::<i64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    let tz = &rest[time.len()..];
    let tz = tz.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = if let Some(o) = tz.strip_prefix('+').or_else(|| tz.strip_prefix('-')) {
        let sign = if tz.starts_with('-') { -1 } else { 1 };
        let mut p = o.split(':').map(|x| x.parse::<i64>().unwrap_or(0));
        sign * (p.next().unwrap_or(0) * 3600 + p.next().unwrap_or(0) * 60)
    } else {
        0
    };
    // Jours depuis l'epoch (algorithme civil de Howard Hinnant).
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some((days * 86400 + hh * 3600 + mm * 60 + ss - offset).max(0) as u64)
}

fn get(url: &str, headers: &[(&str, &str)]) -> Result<Value, (String, u64)> {
    let mut req = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build().get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.call() {
        Ok(r) => r.into_json().map_err(|_| ("failed".into(), 0)),
        Err(ureq::Error::Status(429, r)) => {
            let retry = r.header("retry-after").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            Err(("rateLimited".into(), retry.max(600)))
        }
        Err(ureq::Error::Status(401 | 403, _)) => Err(("expired".into(), 0)),
        Err(ureq::Error::Status(_, _)) => Err(("failed".into(), 0)),
        Err(_) => Err(("offline".into(), 0)),
    }
}

// --- Claude --------------------------------------------------------------------------

fn claude() -> Result<ProviderUsage, (String, u64)> {
    let raw = std::fs::read_to_string(home().join(".claude").join(".credentials.json")).map_err(|_| ("notFound".to_string(), 0))?;
    let v: Value = serde_json::from_str(&raw).map_err(|_| ("notLoggedIn".to_string(), 0))?;
    let o = &v["claudeAiOauth"];
    let token = o["accessToken"].as_str().ok_or(("notLoggedIn".to_string(), 0))?;
    if o["expiresAt"].as_u64().is_some_and(|ms| ms / 1000 < now()) {
        return Err(("expired".into(), 0));
    }
    let u = get(
        "https://api.anthropic.com/api/oauth/usage",
        &[("authorization", &format!("Bearer {token}")), ("anthropic-beta", "oauth-2025-04-20")],
    )?;
    let windows = [("five_hour", "session"), ("seven_day", "week"), ("seven_day_opus", "weekOpus"), ("seven_day_sonnet", "weekSonnet")]
        .iter()
        .filter_map(|(k, kind)| {
            let w = u.get(*k).filter(|w| w.is_object())?;
            Some(Window {
                kind: kind.to_string(),
                used: w["utilization"].as_f64()?,
                resets_at: w["resets_at"].as_str().and_then(iso_to_epoch),
            })
        })
        .collect();
    Ok(ProviderUsage {
        provider: "claude".into(),
        plan: o["subscriptionType"].as_str().map(str::to_string),
        windows,
        error: None,
        fetched_at: now(),
    })
}

// --- ChatGPT (Codex) -----------------------------------------------------------------

fn window_kind(secs: u64) -> &'static str {
    match secs {
        0..=21_600 => "session",
        21_601..=86_400 => "day",
        86_401..=691_200 => "week",
        _ => "month",
    }
}

/// Dernier événement de limites dans les sessions JSONL les plus récentes (le nom contient
/// l'horodatage ; l'mtime n'est pas fiable).
fn codex_from_sessions() -> Option<Vec<Window>> {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "jsonl") {
                    out.push(p);
                }
            }
        }
    }
    let mut files = Vec::new();
    walk(&home().join(".codex").join("sessions"), &mut files);
    files.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));
    for f in files.iter().rev().take(5) {
        let text = std::fs::read_to_string(f).ok()?;
        for line in text.lines().rev() {
            if !line.contains("rate_limits") {
                continue;
            }
            let v: Value = serde_json::from_str(line).ok()?;
            let rl = &v["payload"]["rate_limits"];
            let ts = v["timestamp"].as_str().and_then(iso_to_epoch).unwrap_or(now());
            let mut out = Vec::new();
            for key in ["primary", "secondary"] {
                let w = &rl[key];
                let (Some(used), Some(mins)) = (w["used_percent"].as_f64(), w["window_minutes"].as_u64()) else { continue };
                let reset = w["resets_in_seconds"].as_u64().map(|s| ts + s);
                // Fenêtre expirée → retombe à 0.
                let used = if reset.is_some_and(|r| r < now()) { 0.0 } else { used };
                out.push(Window { kind: window_kind(mins * 60).into(), used, resets_at: reset });
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

fn chatgpt() -> Result<ProviderUsage, (String, u64)> {
    let raw = std::fs::read_to_string(home().join(".codex").join("auth.json")).map_err(|_| ("notFound".to_string(), 0))?;
    let v: Value = serde_json::from_str(&raw).map_err(|_| ("notLoggedIn".to_string(), 0))?;
    let token = v["tokens"]["access_token"].as_str().ok_or(("notLoggedIn".to_string(), 0))?;
    let account = v["tokens"]["account_id"].as_str().unwrap_or_default();
    let api = get(
        "https://chatgpt.com/backend-api/wham/usage",
        &[("authorization", &format!("Bearer {token}")), ("chatgpt-account-id", account)],
    );
    let (plan, windows) = match api {
        Ok(u) => {
            let rl = &u["rate_limit"];
            let windows: Vec<Window> = ["primary_window", "secondary_window"]
                .iter()
                .filter_map(|k| {
                    let w = rl.get(*k).filter(|w| w.is_object())?;
                    Some(Window {
                        kind: window_kind(w["limit_window_seconds"].as_u64().unwrap_or(18_000)).into(),
                        used: w["used_percent"].as_f64()?,
                        resets_at: w["reset_after_seconds"].as_u64().map(|s| now() + s),
                    })
                })
                .collect();
            (u["plan_type"].as_str().map(str::to_string), windows)
        }
        Err(e) => match codex_from_sessions() {
            Some(w) => (None, w),
            None => return Err(e),
        },
    };
    Ok(ProviderUsage { provider: "chatgpt".into(), plan, windows, error: None, fetched_at: now() })
}

// --- Cache + commande ----------------------------------------------------------------

fn disk(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("usage-cache.json")
}

fn load_disk(app: &AppHandle) -> HashMap<String, ProviderUsage> {
    std::fs::read(disk(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn fetch(provider: &str) -> Result<ProviderUsage, (String, u64)> {
    match provider {
        "claude" => claude(),
        "chatgpt" => chatgpt(),
        _ => Err(("unavailable".into(), 0)),
    }
}

/// Quotas des fournisseurs demandés. `force` ignore le cache (bouton rafraîchir), mais
/// jamais la pause après un 429.
#[tauri::command]
pub async fn usage_get(app: AppHandle, providers: Vec<String>, force: bool) -> Vec<ProviderUsage> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut g = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        let first = g.is_none();
        let cache = g.get_or_insert_with(HashMap::new);
        if first {
            for (k, v) in load_disk(&app) {
                cache.insert(k, Slot { last_try: v.fetched_at, pause_until: 0, data: Some(v) });
            }
        }
        let mut out = Vec::new();
        let mut changed = false;
        for p in &providers {
            let ttl = if p == "claude" { 300 } else { 120 };
            let slot = cache.entry(p.clone()).or_insert(Slot { last_try: 0, pause_until: 0, data: None });
            let due = now() >= slot.pause_until && (force || now() >= slot.last_try + ttl);
            if due {
                slot.last_try = now();
                match fetch(p) {
                    Ok(d) => {
                        slot.data = Some(d);
                        changed = true;
                    }
                    Err((code, pause)) => {
                        if pause > 0 {
                            slot.pause_until = now() + pause;
                        }
                        // On garde le dernier bon résultat et on signale l'erreur.
                        let mut d = slot.data.clone().unwrap_or(ProviderUsage { provider: p.clone(), ..Default::default() });
                        d.error = Some(code);
                        slot.data = Some(d);
                    }
                }
            }
            if let Some(d) = &slot.data {
                out.push(d.clone());
            }
        }
        if changed {
            let snapshot: HashMap<String, ProviderUsage> = cache
                .iter()
                .filter_map(|(k, s)| s.data.as_ref().filter(|d| d.error.is_none()).map(|d| (k.clone(), d.clone())))
                .collect();
            let _ = std::fs::write(disk(&app), serde_json::to_vec(&snapshot).unwrap_or_default());
        }
        out
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn iso() {
        assert_eq!(super::iso_to_epoch("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(super::iso_to_epoch("2026-10-13T12:00:00+02:00"), Some(1_791_885_600));
    }
}
