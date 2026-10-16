// Marchés (outil intégré) : cours de bourse (Yahoo Finance, API non officielle, sans clé)
// et crypto (CoinGecko, API publique, sans clé). Les requêtes passent par Rust (liste des
// hôtes contactés fermée, ici) et sont faites en `spawn_blocking`.
//
//   - bourse : cours + courbe du jour (15 min) par symbole, recherche de valeurs ;
//   - crypto : cours, variation 24 h et courbe 7 jours (sous-échantillonnée), recherche ;
//   - cache mémoire 30 s par valeur ; en cas de limite de débit (429), on renvoie les
//     dernières valeurs connues, sinon l'erreur courte `rateLimited`.
//
// Yahoo refuse les clients sans User-Agent de navigateur (429/403) : on en présente un.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

const CACHE_TTL: Duration = Duration::from_secs(30);
/// Nombre de points visé pour les mini-graphiques.
const POINTS: usize = 40;
const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Cache par clé (`stock:{symbole}` ou `crypto:{devise}:{id}`) → dernière cotation.
static CACHE: Mutex<Option<HashMap<String, (Instant, Quote)>>> = Mutex::new(None);

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Symbole Yahoo ou id CoinGecko (sert de clé dans la liste de suivi).
    id: String,
    name: String,
    symbol: String,
    price: f64,
    /// Variation en % (séance du jour pour la bourse, 24 h pour la crypto).
    change: f64,
    currency: String,
    /// Mini-graphique (cours successifs, sans trous).
    points: Vec<f64>,
    image: Option<String>,
    exchange: Option<String>,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    id: String,
    name: String,
    symbol: String,
    /// Place de cotation / type (bourse) ou rang (crypto).
    detail: String,
    image: Option<String>,
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .user_agent(BROWSER_UA)
        .build()
}

fn get_json(url: &str, timeout: u64) -> Result<Value, String> {
    let body = match agent(timeout).get(url).set("Accept", "application/json").call() {
        Ok(r) => r.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(429, _)) => return Err("rateLimited".into()),
        Err(ureq::Error::Status(404, _)) => return Err("notFound".into()),
        Err(_) => return Err("network".into()),
    };
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

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

/// Premier champ texte non vide parmi `keys`.
fn first(v: &Value, keys: &[&str]) -> String {
    keys.iter().map(|k| s(v, k)).find(|x| !x.is_empty()).unwrap_or_default()
}

/// Ramène une série à `n` points au plus (pas régulier, dernier point conservé).
fn downsample(v: Vec<f64>, n: usize) -> Vec<f64> {
    if v.len() <= n || n < 2 {
        return v;
    }
    let step = (v.len() - 1) as f64 / (n - 1) as f64;
    (0..n).map(|i| v[((i as f64 * step).round() as usize).min(v.len() - 1)]).collect()
}

fn cache_get(key: &str, fresh_only: bool) -> Option<Quote> {
    let guard = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let (at, q) = guard.as_ref()?.get(key)?;
    (!fresh_only || at.elapsed() < CACHE_TTL).then(|| q.clone())
}

fn cache_put(key: String, q: &Quote) {
    CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key, (Instant::now(), q.clone()));
}

// ---------------------------------------------------------------- bourse (Yahoo)

/// Réponse `v8/finance/chart` → cotation.
fn parse_chart(v: &Value) -> Option<Quote> {
    let r = v.pointer("/chart/result/0")?;
    let meta = r.get("meta")?;
    let price = meta.get("regularMarketPrice")?.as_f64()?;
    let prev = meta
        .get("chartPreviousClose")
        .and_then(|x| x.as_f64())
        .or_else(|| meta.get("previousClose").and_then(|x| x.as_f64()));
    let change = match prev {
        Some(p) if p != 0.0 => (price - p) / p * 100.0,
        _ => meta.get("regularMarketChangePercent").and_then(|x| x.as_f64()).unwrap_or(0.0),
    };
    let points = r
        .pointer("/indicators/quote/0/close")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
        .unwrap_or_default();
    let symbol = s(meta, "symbol");
    let exchange = first(meta, &["fullExchangeName", "exchangeName"]);
    Some(Quote {
        id: symbol.clone(),
        name: first(meta, &["shortName", "longName", "symbol"]),
        symbol,
        price,
        change,
        currency: s(meta, "currency"),
        points: downsample(points, POINTS),
        image: None,
        exchange: (!exchange.is_empty()).then_some(exchange),
    })
}

fn fetch_stock(symbol: &str) -> Result<Quote, String> {
    let key = format!("stock:{symbol}");
    if let Some(q) = cache_get(&key, true) {
        return Ok(q);
    }
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=15m", enc(symbol));
    match get_json(&url, 8) {
        Ok(v) => {
            let mut q = parse_chart(&v).ok_or_else(|| "notFound".to_string())?;
            // On garde le symbole demandé comme clé (la liste de suivi s'y réfère).
            q.id = symbol.to_string();
            cache_put(key, &q);
            Ok(q)
        }
        Err(e) => cache_get(&key, false).ok_or(e),
    }
}

/// Cours des symboles demandés (ordre conservé ; un symbole introuvable est ignoré).
/// Erreur seulement si aucun n'a pu être lu.
#[tauri::command]
pub async fn markets_stocks(symbols: Vec<String>) -> Result<Vec<Quote>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        let mut err = None;
        for sym in symbols.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
            match fetch_stock(sym) {
                Ok(q) => out.push(q),
                // Symbole introuvable : ignoré ; on retient la première vraie erreur.
                Err(e) if e != "notFound" => err = err.or(Some(e)),
                Err(_) => {}
            }
        }
        match err {
            Some(e) if out.is_empty() => Err(e),
            _ => Ok(out),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Réponse `v1/finance/search` → résultats (actions, ETF, indices, fonds…).
fn parse_stock_search(v: &Value) -> Vec<SearchHit> {
    v.get("quotes")
        .and_then(|q| q.as_array())
        .map(|list| {
            list.iter()
                .filter(|q| !s(q, "symbol").is_empty())
                .map(|q| {
                    let detail = [s(q, "exchDisp"), first(q, &["typeDisp", "quoteType"])]
                        .into_iter()
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    SearchHit {
                        id: s(q, "symbol"),
                        name: first(q, &["shortname", "longname", "symbol"]),
                        symbol: s(q, "symbol"),
                        detail,
                        image: None,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Recherche de valeurs boursières (8 résultats).
#[tauri::command]
pub async fn markets_stock_search(query: String) -> Result<Vec<SearchHit>, String> {
    let url = format!(
        "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=8&newsCount=0",
        enc(query.trim())
    );
    let v = tauri::async_runtime::spawn_blocking(move || get_json(&url, 6)).await.map_err(|e| e.to_string())??;
    Ok(parse_stock_search(&v))
}

// ---------------------------------------------------------------- crypto (CoinGecko)

/// Réponse `coins/markets` → cotations.
fn parse_coins(v: &Value, currency: &str) -> Vec<Quote> {
    v.as_array()
        .map(|list| {
            list.iter()
                .filter_map(|c| {
                    let points = c
                        .pointer("/sparkline_in_7d/price")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                        .unwrap_or_default();
                    let image = s(c, "image");
                    Some(Quote {
                        id: s(c, "id"),
                        name: s(c, "name"),
                        symbol: s(c, "symbol").to_uppercase(),
                        price: c.get("current_price")?.as_f64()?,
                        change: c.get("price_change_percentage_24h").and_then(|x| x.as_f64()).unwrap_or(0.0),
                        currency: currency.to_uppercase(),
                        points: downsample(points, POINTS),
                        image: (!image.is_empty()).then_some(image),
                        exchange: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Cours des cryptos demandées (ids CoinGecko), dans la devise `eur` ou `usd`.
#[tauri::command]
pub async fn markets_crypto(ids: Vec<String>, currency: String) -> Result<Vec<Quote>, String> {
    let cur = if currency.eq_ignore_ascii_case("usd") { "usd" } else { "eur" };
    let ids: Vec<String> = ids.into_iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let key = |id: &str| format!("crypto:{cur}:{id}");
    // Tout est frais en cache : pas d'appel.
    let cached: Vec<Option<Quote>> = ids.iter().map(|id| cache_get(&key(id), true)).collect();
    if cached.iter().all(|q| q.is_some()) {
        return Ok(cached.into_iter().flatten().collect());
    }
    let url = format!(
        "https://api.coingecko.com/api/v3/coins/markets?vs_currency={cur}&ids={}&sparkline=true&price_change_percentage=24h",
        ids.iter().map(|x| enc(x)).collect::<Vec<_>>().join(",")
    );
    match tauri::async_runtime::spawn_blocking(move || get_json(&url, 8)).await.map_err(|e| e.to_string())? {
        Ok(v) => {
            let mut by_id: HashMap<String, Quote> = parse_coins(&v, cur).into_iter().map(|q| (q.id.clone(), q)).collect();
            Ok(ids
                .iter()
                .filter_map(|id| {
                    let q = by_id.remove(id)?;
                    cache_put(key(id), &q);
                    Some(q)
                })
                .collect())
        }
        Err(e) => {
            // Limite de débit / réseau : dernières valeurs connues si on en a.
            let stale: Vec<Quote> = ids.iter().filter_map(|id| cache_get(&key(id), false)).collect();
            if stale.is_empty() {
                Err(e)
            } else {
                Ok(stale)
            }
        }
    }
}

/// Réponse `search` → résultats (8 premiers).
fn parse_coin_search(v: &Value) -> Vec<SearchHit> {
    v.get("coins")
        .and_then(|c| c.as_array())
        .map(|list| {
            list.iter()
                .filter(|c| !s(c, "id").is_empty())
                .take(8)
                .map(|c| {
                    let thumb = s(c, "thumb");
                    SearchHit {
                        id: s(c, "id"),
                        name: s(c, "name"),
                        symbol: s(c, "symbol").to_uppercase(),
                        detail: c
                            .get("market_cap_rank")
                            .and_then(|x| x.as_u64())
                            .map(|r| format!("#{r}"))
                            .unwrap_or_default(),
                        image: (!thumb.is_empty()).then_some(thumb),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Recherche de cryptos (8 résultats).
#[tauri::command]
pub async fn markets_crypto_search(query: String) -> Result<Vec<SearchHit>, String> {
    let url = format!("https://api.coingecko.com/api/v3/search?query={}", enc(query.trim()));
    let v = tauri::async_runtime::spawn_blocking(move || get_json(&url, 6)).await.map_err(|e| e.to_string())??;
    Ok(parse_coin_search(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chart_parse() {
        let v = json!({ "chart": { "result": [{
            "meta": { "currency": "EUR", "symbol": "CW8.PA", "exchangeName": "PAR", "fullExchangeName": "Paris",
                      "regularMarketPrice": 110.0, "chartPreviousClose": 100.0, "shortName": "Amundi MSCI World" },
            "indicators": { "quote": [{ "close": [100.0, null, 105.5, 110.0] }] }
        }], "error": null } });
        let q = parse_chart(&v).unwrap();
        assert_eq!(q.symbol, "CW8.PA");
        assert_eq!(q.name, "Amundi MSCI World");
        assert_eq!(q.currency, "EUR");
        assert_eq!(q.exchange.as_deref(), Some("Paris"));
        assert!((q.change - 10.0).abs() < 1e-9);
        assert_eq!(q.points, vec![100.0, 105.5, 110.0]);
        // Symbole inconnu : résultat vide.
        assert!(parse_chart(&json!({ "chart": { "result": null, "error": { "code": "Not Found" } } })).is_none());
    }

    #[test]
    fn coins_parse() {
        let v = json!([{
            "id": "bitcoin", "symbol": "btc", "name": "Bitcoin", "image": "https://x/btc.png",
            "current_price": 60000.5, "price_change_percentage_24h": -1.25,
            "sparkline_in_7d": { "price": (0..168).map(|i| i as f64).collect::<Vec<_>>() }
        }, { "id": "broken", "symbol": "x", "name": "X", "current_price": null }]);
        let q = parse_coins(&v, "eur");
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].symbol, "BTC");
        assert_eq!(q[0].currency, "EUR");
        assert_eq!(q[0].change, -1.25);
        assert_eq!(q[0].points.len(), POINTS);
        assert_eq!(q[0].points.first(), Some(&0.0));
        assert_eq!(q[0].points.last(), Some(&167.0));
    }

    #[test]
    fn search_parse() {
        let stocks = parse_stock_search(&json!({ "quotes": [
            { "symbol": "AAPL", "shortname": "Apple Inc.", "exchDisp": "NASDAQ", "quoteType": "EQUITY", "typeDisp": "Equity" },
            { "shortname": "sans symbole" }
        ] }));
        assert_eq!(stocks.len(), 1);
        assert_eq!(stocks[0].detail, "NASDAQ · Equity");
        let coins = parse_coin_search(&json!({ "coins": [
            { "id": "solana", "name": "Solana", "symbol": "sol", "market_cap_rank": 7, "thumb": "https://x/sol.png" }
        ] }));
        assert_eq!(coins[0].symbol, "SOL");
        assert_eq!(coins[0].detail, "#7");
        assert_eq!(coins[0].image.as_deref(), Some("https://x/sol.png"));
    }
}
