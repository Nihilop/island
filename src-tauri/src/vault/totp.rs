// TOTP (RFC 6238) calculé dans Rust : les graines ne quittent jamais le natif, seuls le
// code, les secondes restantes et la période remontent à l'UI.
//
// Formats acceptés : clé base32 (RFC 4648, insensible à la casse, espaces/tirets/padding
// ignorés, ≥ 80 bits) ou lien `otpauth://totp/...` (HOTP refusé ; SHA1/256/512 ; 6–8
// chiffres ; période 1–300 s).

use hmac::{Hmac, Mac};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Algo {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Clone, Debug)]
pub struct Otp {
    pub secret: Vec<u8>,
    pub algo: Algo,
    pub digits: u32,
    pub period: u64,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

pub fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let mut bits: u64 = 0;
    let mut n = 0u32;
    let mut out = Vec::new();
    for c in input.chars() {
        if c == '=' || c == ' ' || c == '-' {
            continue;
        }
        let v = match c.to_ascii_uppercase() {
            ch @ 'A'..='Z' => ch as u64 - 'A' as u64,
            ch @ '2'..='7' => ch as u64 - '2' as u64 + 26,
            _ => return None,
        };
        bits = (bits << 5) | v;
        n += 5;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
            bits &= (1 << n) - 1;
        }
    }
    (out.len() * 8 >= 80).then_some(out)
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Analyse une graine Bitwarden (base32 ou otpauth://). `Err` = code d'erreur pour l'UI.
pub fn parse(input: &str) -> Result<Otp, &'static str> {
    let input = input.trim();
    if let Some(rest) = input.strip_prefix("otpauth://") {
        let (kind, rest) = rest.split_once('/').ok_or("invalidSecret")?;
        if !kind.eq_ignore_ascii_case("totp") {
            return Err("notTotp");
        }
        let (label, query) = rest.split_once('?').unwrap_or((rest, ""));
        let label = pct_decode(label);
        let (mut issuer, account) = match label.split_once(':') {
            Some((i, a)) => (Some(i.trim().to_string()), Some(a.trim().to_string())),
            None => (None, Some(label.trim().to_string()).filter(|s| !s.is_empty())),
        };
        let mut otp = Otp { secret: vec![], algo: Algo::Sha1, digits: 6, period: 30, issuer: None, account };
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            let v = pct_decode(v);
            match k.to_ascii_lowercase().as_str() {
                "secret" => otp.secret = base32_decode(&v).ok_or("invalidSecret")?,
                "issuer" if !v.is_empty() => issuer = Some(v),
                "algorithm" => {
                    otp.algo = match v.to_ascii_uppercase().as_str() {
                        "SHA1" => Algo::Sha1,
                        "SHA256" => Algo::Sha256,
                        "SHA512" => Algo::Sha512,
                        _ => return Err("invalidSecret"),
                    }
                }
                "digits" => otp.digits = v.parse().ok().filter(|d| (6..=8).contains(d)).ok_or("invalidSecret")?,
                "period" => otp.period = v.parse().ok().filter(|p| (1..=300).contains(p)).ok_or("invalidSecret")?,
                _ => {}
            }
        }
        if otp.secret.is_empty() {
            return Err("invalidSecret");
        }
        otp.issuer = issuer;
        return Ok(otp);
    }
    if input.starts_with("steam://") {
        return Err("notTotp");
    }
    Ok(Otp {
        secret: base32_decode(input).ok_or("invalidSecret")?,
        algo: Algo::Sha1,
        digits: 6,
        period: 30,
        issuer: None,
        account: None,
    })
}

fn hmac(algo: Algo, key: &[u8], msg: &[u8]) -> Vec<u8> {
    macro_rules! run {
        ($h:ty) => {{
            let mut m = <Hmac<$h>>::new_from_slice(key).expect("HMAC accepte toute taille de clé");
            m.update(msg);
            m.finalize().into_bytes().to_vec()
        }};
    }
    match algo {
        Algo::Sha1 => run!(sha1::Sha1),
        Algo::Sha256 => run!(sha2::Sha256),
        Algo::Sha512 => run!(sha2::Sha512),
    }
}

/// Code courant + secondes restantes dans la période.
pub fn code(otp: &Otp, unix: u64) -> (String, u64) {
    let counter = unix / otp.period;
    let h = hmac(otp.algo, &otp.secret, &counter.to_be_bytes());
    let off = (h[h.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([h[off] & 0x7f, h[off + 1], h[off + 2], h[off + 3]]);
    let code = bin % 10u32.pow(otp.digits);
    (format!("{:0width$}", code, width = otp.digits as usize), otp.period - unix % otp.period)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vecteurs de la RFC 6238 (annexe B), SHA1, 8 chiffres.
    #[test]
    fn rfc6238_sha1() {
        let otp = Otp {
            secret: b"12345678901234567890".to_vec(),
            algo: Algo::Sha1,
            digits: 8,
            period: 30,
            issuer: None,
            account: None,
        };
        assert_eq!(code(&otp, 59).0, "94287082");
        assert_eq!(code(&otp, 1111111109).0, "07081804");
        assert_eq!(code(&otp, 2000000000).0, "69279037");
    }

    #[test]
    fn otpauth() {
        let o = parse("otpauth://totp/GitHub:me%40x.io?secret=JBSWY3DPEHPK3PXPJBSWY3DP&issuer=GitHub&digits=6").unwrap();
        assert_eq!(o.issuer.as_deref(), Some("GitHub"));
        assert_eq!(o.account.as_deref(), Some("me@x.io"));
        assert!(parse("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
    }
}
