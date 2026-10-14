// Correspondance requête ↔ texte (tous deux déjà normalisés). Score de base = le meilleur
// des signaux : exact > préfixe > contient > tous les mots présents > approximatif.
//
// L'approximatif est une sous-séquence (les lettres de la requête apparaissent dans
// l'ordre) choisie par programmation dynamique pour maximiser : +10 par lettre, bonus en
// début de mot (ce qui fait marcher les initiales : « vsc » → « visual studio code »),
// bonus de lettres consécutives, bonus près du début, pénalité d'écart plafonnée.

pub const EXACT: i32 = 2000;
pub const PREFIX: i32 = 1500;
pub const CONTAINS: i32 = 1200;
pub const ALL_WORDS: i32 = 850;

/// Requêtes plus longues : glouton (la DP serait quadratique pour rien).
const DP_MAX: usize = 48;

fn is_boundary(prev: Option<u8>) -> bool {
    matches!(prev, None | Some(b' ' | b'-' | b'_' | b'.' | b'/' | b'\\' | b'(' | b'[' | b':'))
}

/// Meilleur score de base de `query` contre `text` (0 = aucune correspondance).
pub fn score(query: &str, text: &str) -> i32 {
    if query.is_empty() || text.is_empty() {
        return 0;
    }
    if text == query {
        return EXACT;
    }
    if let Some(rest) = text.strip_prefix(query) {
        return PREFIX - rest.len().min(200) as i32;
    }
    if let Some(at) = text.find(query) {
        // Au début d'un mot : presque un préfixe.
        let word = is_boundary(text.as_bytes().get(at.wrapping_sub(1)).copied().filter(|_| at > 0));
        return CONTAINS + if word { 150 } else { 0 } - (at.min(100) as i32);
    }
    if query.contains(' ') {
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.len() > 1 && words.iter().all(|w| text.contains(w)) {
            return ALL_WORDS;
        }
    }
    fuzzy(query.as_bytes(), text.as_bytes()).unwrap_or(0)
}

/// Sous-séquence pondérée ; None si une lettre manque.
pub fn fuzzy(q: &[u8], t: &[u8]) -> Option<i32> {
    let q: Vec<u8> = q.iter().copied().filter(|c| *c != b' ').collect();
    if q.is_empty() || q.len() > t.len() {
        return None;
    }
    if q.len() > DP_MAX {
        return greedy(&q, t);
    }
    let m = t.len();
    const NEG: i32 = i32::MIN / 4;
    // prev[j] = meilleur score avec la lettre précédente de la requête posée sur t[j] ;
    // prev_run[j] = longueur de la série de lettres consécutives qui finit en j.
    let mut prev = vec![NEG; m];
    let mut prev_run = vec![0u16; m];
    for (i, &qc) in q.iter().enumerate() {
        let mut cur = vec![NEG; m];
        let mut cur_run = vec![0u16; m];
        // Maximum de prev[k] pour k ≤ j − 21 (écart plafonné : pénalité fixe de 20).
        let mut far_max = NEG;
        for j in 0..m {
            if j >= 21 {
                far_max = far_max.max(prev[j - 21]);
            }
            if t[j] != qc {
                continue;
            }
            let boundary = is_boundary(if j == 0 { None } else { Some(t[j - 1]) });
            let base = 10 + if boundary { 8 } else { 0 } + if j < 5 { (5 - j) as i32 } else { 0 };
            if i == 0 {
                cur[j] = base;
                cur_run[j] = 1;
                continue;
            }
            let mut v = NEG;
            let mut run = 1u16;
            // Lettre précédente juste avant : série consécutive.
            if j > 0 && prev[j - 1] > NEG {
                let r = prev_run[j - 1] + 1;
                v = prev[j - 1] + base + 4 * r.min(6) as i32;
                run = r;
            }
            // Lettre précédente plus loin : pénalité d'écart.
            let near_from = j.saturating_sub(20);
            for k in near_from..j.saturating_sub(1) {
                if prev[k] > NEG {
                    let c = prev[k] + base - gap_penalty(k, j);
                    if c > v {
                        v = c;
                        run = 1;
                    }
                }
            }
            if far_max > NEG && far_max + base - 20 > v {
                v = far_max + base - 20;
                run = 1;
            }
            if v > NEG {
                cur[j] = v;
                cur_run[j] = run;
            }
        }
        prev = cur;
        prev_run = cur_run;
    }
    prev.into_iter().filter(|v| *v > NEG).max()
}

fn gap_penalty(from: usize, to: usize) -> i32 {
    ((to - from - 1) as i32).min(20)
}

fn greedy(q: &[u8], t: &[u8]) -> Option<i32> {
    let mut s = 0;
    let mut j = 0;
    for &qc in q {
        while j < t.len() && t[j] != qc {
            j += 1;
        }
        if j == t.len() {
            return None;
        }
        s += 10 + if is_boundary(if j == 0 { None } else { Some(t[j - 1]) }) { 8 } else { 0 };
        j += 1;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_beats_prefix_beats_contains() {
        assert!(score("code", "code") > score("code", "code insiders"));
        assert!(score("code", "code insiders") > score("code", "vs code"));
        assert!(score("code", "vs code") > score("code", "xcodex"));
    }

    #[test]
    fn initials_match_word_starts() {
        let vsc = score("vsc", "visual studio code");
        assert!(vsc > 0);
        // Les initiales battent des lettres prises au milieu d'un mot.
        assert!(vsc > score("vsc", "invasic"));
    }

    #[test]
    fn missing_letter_fails() {
        assert_eq!(score("xyz", "visual studio code"), 0);
        assert_eq!(fuzzy(b"abc", b"ab"), None);
    }

    #[test]
    fn consecutive_letters_preferred() {
        assert!(score("chr", "chrome") > score("chr", "cash register"));
        assert!(fuzzy(b"note", b"notepad").unwrap() > fuzzy(b"note", b"n o t e").unwrap());
    }

    #[test]
    fn all_words_in_any_order() {
        assert_eq!(score("ecran parametres", "parametres d'ecran"), ALL_WORDS);
    }
}
