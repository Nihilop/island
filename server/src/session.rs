//! Cookie de session du panneau d'administration, SIGNÉ (HMAC-SHA256 avec `SESSION_KEY`) :
//! `<id du compte>.<expiration>.<signature>`. Pas de table de sessions : le serveur reste
//! sans état. Le rôle et le bannissement sont relus en base à chaque requête : retirer un
//! modérateur ou bannir un compte prend effet tout de suite, même avec un cookie valide.
//!
//! Le même mécanisme signe le `state` de l'OAuth web (anti-falsification du retour).

use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const COOKIE: &str = "island_admin";
pub const STATE_COOKIE: &str = "island_oauth_state";
/// Durée d'une session : 7 jours.
pub const TTL_SECS: i64 = 7 * 24 * 3600;

fn mac(key: &[u8], data: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("clé HMAC");
    m.update(data.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(m.finalize().into_bytes())
}

/// Comparaison en temps constant (pas d'indice sur la signature attendue).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Valeur signée qui expire à `exp` (secondes Unix).
pub fn sign(key: &[u8], value: &str, exp: i64) -> String {
    let data = format!("{value}.{exp}");
    format!("{data}.{}", mac(key, &data))
}

/// Valeur d'origine si la signature est bonne et non expirée.
pub fn verify(key: &[u8], signed: &str) -> Option<String> {
    let (data, sig) = signed.rsplit_once('.')?;
    let (value, exp) = data.rsplit_once('.')?;
    if !same(&mac(key, data), sig) || exp.parse::<i64>().ok()? < now() {
        return None;
    }
    Some(value.to_string())
}

/// Valeur d'un cookie dans l'en-tête `Cookie`.
pub fn cookie<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// En-tête `Set-Cookie` (HttpOnly, Secure sauf en local, chemin `/`).
pub fn set_cookie(name: &str, value: &str, max_age: i64, same_site: &str, secure: bool) -> String {
    format!("{name}={value}; Path=/; HttpOnly; SameSite={same_site}; Max-Age={max_age}{}", if secure { "; Secure" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_values_resist_tampering_and_expire() {
        let key = b"cle-de-test";
        let s = sign(key, "42", now() + 60);
        assert_eq!(verify(key, &s).as_deref(), Some("42"));
        assert_eq!(verify(b"autre-cle", &s), None);
        assert_eq!(verify(key, &s.replacen("42", "1", 1)), None);
        assert_eq!(verify(key, &sign(key, "42", now() - 1)), None);
        assert_eq!(verify(key, "n'importe quoi"), None);
    }
}
