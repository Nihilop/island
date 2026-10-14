// Ask (outil intégré) : chat rapide en streaming avec le fournisseur choisi.
//   - OpenAI, OpenRouter, Custom (compatible OpenAI) : `/chat/completions` en SSE ;
//   - Anthropic : API Messages en SSE (`content_block_delta` / `text_delta`), effort dans
//     `output_config.effort`, repli serveur `fallbacks: "default"` sur les modèles qui le
//     prennent en charge.
// La clé vit dans le Gestionnaire d'identifiants et n'est utilisée que dans l'en-tête HTTP
// côté Rust ; le front ne la relit jamais. Les deltas sont émis sur `ai://delta`, la fin
// sur `ai://done` (avec un code d'erreur éventuel), identifiés par un id de requête ;
// `ai_cancel(id)` arrête le flux au prochain morceau.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

const SYSTEM: &str = "You are Ask, a quick assistant living in a small notch at the top of the user's screen. \
Answer briefly and directly, in the language of the question. Use little Markdown.";

static CANCELLED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn key_entry(provider: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new("island-core:ai", provider)
}

fn key(provider: &str) -> Option<String> {
    key_entry(provider).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

#[tauri::command]
pub fn ai_set_key(provider: String, key: String) {
    if let Ok(e) = key_entry(&provider) {
        let _ = if key.trim().is_empty() { e.delete_credential() } else { e.set_password(key.trim()) };
    }
}

#[tauri::command]
pub fn ai_has_key(provider: String) -> bool {
    key(&provider).is_some()
}

#[tauri::command]
pub fn ai_cancel(id: String) {
    CANCELLED.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashSet::new).insert(id);
}

fn cancelled(id: &str) -> bool {
    CANCELLED.lock().unwrap_or_else(|p| p.into_inner()).as_ref().is_some_and(|s| s.contains(id))
}

#[derive(serde::Deserialize)]
pub struct Msg {
    role: String,
    content: String,
}

#[derive(Clone, serde::Serialize)]
struct Delta<'a> {
    id: &'a str,
    text: &'a str,
}

#[derive(Clone, serde::Serialize)]
struct Done {
    id: String,
    error: Option<String>,
}

/// Modèles Claude qui acceptent le repli serveur `fallbacks: "default"`.
fn supports_fallback(model: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-fable-5-1", "claude-sonnet-5-5"].contains(&model)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        // Les modèles de raisonnement peuvent être lents au premier token.
        .timeout_read(Duration::from_secs(180))
        .build()
}

/// Erreur HTTP → code court (+ détail tronqué).
fn http_error(status: u16, body: String) -> String {
    let detail: String = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| {
            v["error"]["message"].as_str().or(v["error"].as_str()).or(v["message"].as_str()).map(str::to_string)
        })
        .unwrap_or(body)
        .chars()
        .take(160)
        .collect();
    match status {
        401 | 403 => format!("auth:{detail}"),
        404 => format!("notFound:{detail}"),
        429 => "rateLimited".into(),
        _ => format!("http{status}:{detail}"),
    }
}

/// Lit un flux SSE ligne par ligne ; `on_data` reçoit chaque `data:` (renvoie Err pour
/// arrêter avec une erreur, Ok(false) pour arrêter normalement).
fn read_sse(id: &str, resp: ureq::Response, mut on_data: impl FnMut(&str) -> Result<bool, String>) -> Result<(), String> {
    let reader = BufReader::new(resp.into_reader());
    for line in reader.lines() {
        if cancelled(id) {
            return Ok(());
        }
        let line = line.map_err(|_| "network".to_string())?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(());
        }
        if !on_data(data)? {
            return Ok(());
        }
    }
    Ok(())
}

fn post(req: ureq::Request, body: Value) -> Result<ureq::Response, String> {
    match req.set("content-type", "application/json").send_string(&body.to_string()) {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, r)) => Err(http_error(code, r.into_string().unwrap_or_default())),
        Err(_) => Err("network".into()),
    }
}

fn run(app: &AppHandle, id: &str, provider: &str, model: &str, effort: &str, base_url: &str, messages: Vec<Msg>) -> Result<(), String> {
    let key = key(provider);
    if key.is_none() && provider != "custom" {
        return Err("noKey".into());
    }
    let emit = |text: &str| {
        let _ = app.emit("ai://delta", Delta { id, text });
    };

    if provider == "anthropic" {
        let mut body = json!({
            "model": if model.is_empty() { "claude-opus-5-5" } else { model },
            "max_tokens": 64000,
            "stream": true,
            "system": SYSTEM,
            "messages": messages.iter().map(|m| json!({ "role": m.role, "content": m.content })).collect::<Vec<_>>(),
            "output_config": { "effort": effort },
        });
        let mut req = agent()
            .post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", key.as_deref().unwrap_or_default())
            .set("anthropic-version", "2023-06-01");
        if supports_fallback(body["model"].as_str().unwrap_or_default()) {
            body["fallbacks"] = json!("default");
            req = req.set("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        let resp = post(req, body)?;
        return read_sse(id, resp, |data| {
            let v: Value = serde_json::from_str(data).unwrap_or(Value::Null);
            match v["type"].as_str() {
                Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
                    emit(v["delta"]["text"].as_str().unwrap_or_default());
                    Ok(true)
                }
                Some("message_delta") if v["delta"]["stop_reason"] == "refusal" => Err("refusal".into()),
                Some("message_stop") => Ok(false),
                Some("error") => Err(format!("api:{}", v["error"]["message"].as_str().unwrap_or("error"))),
                _ => Ok(true),
            }
        });
    }

    // OpenAI / OpenRouter / Custom (compatible OpenAI).
    let url = match provider {
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        _ => {
            let base = base_url.trim().trim_end_matches('/');
            let local = base.starts_with("http://localhost") || base.starts_with("http://127.0.0.1");
            if !(base.starts_with("https://") || local) {
                return Err("baseUrl".into());
            }
            format!("{base}/chat/completions")
        }
    };
    if model.is_empty() {
        return Err("noModel".into());
    }
    let mut msgs = vec![json!({ "role": "system", "content": SYSTEM })];
    msgs.extend(messages.iter().map(|m| json!({ "role": m.role, "content": m.content })));
    let mut body = json!({ "model": model, "stream": true, "messages": msgs });
    let mut req = agent().post(&url);
    if let Some(k) = key.as_deref() {
        req = req.set("authorization", &format!("Bearer {k}"));
    }
    match provider {
        "openrouter" => {
            body["reasoning"] = json!({ "effort": effort });
            req = req.set("X-Title", "Island").set("HTTP-Referer", "https://github.com/Nihilop/island");
        }
        "openai" => body["reasoning_effort"] = json!(effort),
        _ => {}
    }
    let resp = post(req, body)?;
    read_sse(id, resp, |data| {
        let v: Value = serde_json::from_str(data).unwrap_or(Value::Null);
        if let Some(e) = v["error"]["message"].as_str() {
            return Err(format!("api:{e}"));
        }
        if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
            emit(t);
        }
        Ok(true)
    })
}

/// Lance une réponse en streaming. Les morceaux arrivent sur `ai://delta`, la fin sur
/// `ai://done`.
#[tauri::command]
pub fn ai_chat(
    app: AppHandle,
    id: String,
    provider: String,
    model: String,
    effort: String,
    base_url: String,
    messages: Vec<Msg>,
) {
    std::thread::spawn(move || {
        let r = run(&app, &id, &provider, &model, &effort, &base_url, messages);
        CANCELLED.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashSet::new).remove(&id);
        let _ = app.emit("ai://done", Done { id, error: r.err() });
    });
}
