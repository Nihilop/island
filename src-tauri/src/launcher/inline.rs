// Réponses directes, calculées à chaque frappe (avant les résultats de l'index) :
// - calculatrice : + − × ÷ % ^, parenthèses, virgule ou point décimal, fonctions
//   (sqrt, sin, cos, tan, ln, log, abs, round, floor, ceil) et constantes (pi, e) ;
// - URL tapée (`exemple.com`, `https://…`) → l'ouvrir.

/// Résultat de la calculatrice, formaté (None = pas une expression).
pub fn calc(input: &str) -> Option<String> {
    let s = input.trim().trim_start_matches('=').trim();
    // Il faut au moins un chiffre ET un opérateur ou une fonction : « 2024 » seul n'est pas un calcul.
    if !s.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let has_op = s.chars().skip(1).any(|c| "+-*/×÷%^".contains(c)) || s.contains('(');
    if !has_op {
        return None;
    }
    let mut p = Parser { s: s.as_bytes(), i: 0 };
    let v = p.expr()?;
    p.ws();
    if p.i != p.s.len() || !v.is_finite() {
        return None;
    }
    Some(format_num(v))
}

fn format_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i] == b' ' {
            self.i += 1;
        }
    }

    /// Opérateur suivant (× et ÷ en UTF-8 acceptés).
    fn op(&mut self, ops: &[&str]) -> Option<String> {
        self.ws();
        for o in ops {
            if self.s[self.i..].starts_with(o.as_bytes()) {
                self.i += o.len();
                return Some((*o).to_string());
            }
        }
        None
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(o) = self.op(&["+", "-"]) {
            let r = self.term()?;
            v = if o == "+" { v + r } else { v - r };
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(o) = self.op(&["*", "/", "×", "÷", "%"]) {
            let r = self.power()?;
            v = match o.as_str() {
                "*" | "×" => v * r,
                "%" => v % r,
                _ => v / r,
            };
        }
        Some(v)
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.unary()?;
        if self.op(&["^"]).is_some() {
            // Associatif à droite : 2^3^2 = 2^9.
            return Some(base.powf(self.power()?));
        }
        Some(base)
    }

    fn unary(&mut self) -> Option<f64> {
        if self.op(&["-"]).is_some() {
            return Some(-self.unary()?);
        }
        if self.op(&["+"]).is_some() {
            return self.unary();
        }
        self.atom()
    }

    fn atom(&mut self) -> Option<f64> {
        self.ws();
        if self.op(&["("]).is_some() {
            let v = self.expr()?;
            self.op(&[")"])?;
            return Some(v);
        }
        let start = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.' || self.s[self.i] == b',') {
            self.i += 1;
        }
        if self.i > start {
            let txt = std::str::from_utf8(&self.s[start..self.i]).ok()?.replace(',', ".");
            return txt.parse().ok();
        }
        while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.s[start..self.i]).ok()?.to_ascii_lowercase();
        match name.as_str() {
            "pi" => Some(std::f64::consts::PI),
            "e" => Some(std::f64::consts::E),
            "" => None,
            f => {
                self.op(&["("])?;
                let x = self.expr()?;
                self.op(&[")"])?;
                Some(match f {
                    "sqrt" => x.sqrt(),
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "ln" => x.ln(),
                    "log" => x.log10(),
                    "abs" => x.abs(),
                    "round" => x.round(),
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    _ => return None,
                })
            }
        }
    }
}

const TLDS: &[&str] = &[
    "com", "net", "org", "io", "dev", "app", "fr", "eu", "de", "uk", "co", "me", "ai", "gg", "tv", "info", "be", "ch", "ca",
    "us", "xyz", "tech", "gov", "edu", "es", "it", "nl", "jp", "so", "sh", "to", "ly", "fm",
];

/// URL à ouvrir si la saisie en est une (`https://…`, `www.…`, `exemple.com/…`).
pub fn url(input: &str) -> Option<String> {
    let s = input.trim();
    if s.contains(' ') || s.len() < 4 {
        return None;
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Some(s.to_string());
    }
    let host = s.split('/').next()?;
    let tld = host.rsplit('.').next()?;
    let ok_host = host.contains('.')
        && !host.starts_with('.')
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
        // TLD courant, ou `www.` : « notes.txt » reste un nom de fichier.
        && (host.starts_with("www.") || TLDS.contains(&tld.to_ascii_lowercase().as_str()));
    ok_host.then(|| format!("https://{s}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes() {
        assert_eq!(calc("2+2").as_deref(), Some("4"));
        assert_eq!(calc("2 × (3 + 4)").as_deref(), Some("14"));
        assert_eq!(calc("10 / 4").as_deref(), Some("2.5"));
        assert_eq!(calc("1,5*2").as_deref(), Some("3"));
        assert_eq!(calc("2^3^2").as_deref(), Some("512"));
        assert_eq!(calc("-3+5").as_deref(), Some("2"));
        assert_eq!(calc("sqrt(16)+1").as_deref(), Some("5"));
    }

    #[test]
    fn ignores_non_expressions() {
        assert_eq!(calc("2024"), None);
        assert_eq!(calc("chrome"), None);
        assert_eq!(calc("1/0"), None);
        assert_eq!(calc("2+"), None);
    }

    #[test]
    fn detects_urls() {
        assert_eq!(url("github.com/tauri").as_deref(), Some("https://github.com/tauri"));
        assert_eq!(url("https://x.io").as_deref(), Some("https://x.io"));
        assert_eq!(url("notes.txt"), None);
        assert_eq!(url("www.exemple.test").as_deref(), Some("https://www.exemple.test"));
        assert_eq!(url("visual studio"), None);
    }
}
