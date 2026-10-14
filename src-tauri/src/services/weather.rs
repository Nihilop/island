// Météo (outil intégré) : Open-Meteo (gratuit, sans clé) + géolocalisation IP au niveau
// ville. Les requêtes passent par Rust (une WebView servie en https ne peut pas appeler
// les services http, et on garde la liste des hôtes contactés ici, fermée).
//
//   - prévisions : actuel (température, ressenti, code WMO, vent, jour/nuit) + 7 jours
//     (code, max, min, probabilité de pluie), fuseau automatique ; cache 10 min ;
//   - recherche de lieu : géocodage Open-Meteo (4 résultats, langue de l'UI) ;
//   - lieu automatique : ipapi.co, repli ipwho.is (timeout 5 s chacun).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

const CACHE_TTL: Duration = Duration::from_secs(600);
static CACHE: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);

fn agent(timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .user_agent("Island (weather)")
        .build()
}

fn get_json(url: &str, timeout: u64) -> Result<Value, String> {
    let body = agent(timeout)
        .get(url)
        .call()
        .map_err(|_| "network".to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Prévisions brutes Open-Meteo (le front met en forme).
#[tauri::command]
pub async fn weather_forecast(lat: f64, lon: f64, fahrenheit: bool) -> Result<Value, String> {
    let key = format!("{lat:.3},{lon:.3},{fahrenheit}");
    if let Some((at, v)) = CACHE.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        if at.elapsed() < CACHE_TTL {
            return Ok(v.clone());
        }
    }
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
         &current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
         &timezone=auto&forecast_days=7{}",
        if fahrenheit { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" }
    );
    let v = tauri::async_runtime::spawn_blocking(move || get_json(&url, 8)).await.map_err(|e| e.to_string())??;
    CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key, (Instant::now(), v.clone()));
    Ok(v)
}

#[derive(serde::Serialize)]
pub struct Place {
    name: String,
    detail: String,
    lat: f64,
    lon: f64,
}

/// Recherche de lieu (4 résultats, langue de l'UI).
#[tauri::command]
pub async fn weather_geocode(query: String, lang: String) -> Result<Vec<Place>, String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=4&language={}&format=json",
        enc(query.trim()),
        enc(&lang)
    );
    let v = tauri::async_runtime::spawn_blocking(move || get_json(&url, 6)).await.map_err(|e| e.to_string())??;
    let s = |r: &Value, k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    Ok(v.get("results")
        .and_then(|r| r.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    let detail = [s(r, "admin1"), s(r, "country")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                    Some(Place {
                        name: s(r, "name"),
                        detail,
                        lat: r.get("latitude")?.as_f64()?,
                        lon: r.get("longitude")?.as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Lieu approximatif (ville) d'après l'adresse IP.
#[tauri::command]
pub async fn weather_locate() -> Result<Place, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let f = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_f64());
        let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        for url in ["https://ipapi.co/json/", "https://ipwho.is/"] {
            if let Ok(v) = get_json(url, 5) {
                if let (Some(lat), Some(lon)) = (f(&v, "latitude"), f(&v, "longitude")) {
                    let country = if url.contains("ipapi") { s(&v, "country_name") } else { s(&v, "country") };
                    return Ok(Place { name: s(&v, "city"), detail: [s(&v, "region"), country].join(", "), lat, lon });
                }
            }
        }
        Err("network".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
