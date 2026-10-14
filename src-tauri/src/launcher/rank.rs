// Classement : score de correspondance + type + usage (fréquence et récence) − profondeur.
// Requête vide : on montre ce qu'on ouvre le plus et le plus récemment.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use super::matcher::score;
use super::model::{Kind, Prepared};

/// Usage d'un élément : nombre d'ouvertures + dernière ouverture (ms Unix).
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub count: u32,
    pub last: i64,
}

pub type UsageMap = HashMap<String, Usage>;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Bonus d'usage : fréquence (log, bornée) + récence par paliers.
fn frecency(u: Option<&Usage>, now: i64) -> i32 {
    let Some(u) = u.filter(|u| u.count > 0) else { return 0 };
    let freq = ((u.count as f32 + 1.0).log2() * 40.0).min(240.0) as i32;
    let age_h = (now - u.last).max(0) / 3_600_000;
    let recent = match age_h {
        0 => 120,
        1..=23 => 80,
        24..=167 => 40,
        168..=719 => 12,
        _ => 0,
    };
    freq + recent
}

/// Score de base d'un élément (0 = pas de correspondance).
fn base(q: &str, p: &Prepared) -> i32 {
    let title = score(q, &p.title);
    // Mots-clés (alias, autre langue) : un peu moins forts que le titre.
    let kw = if p.keywords.is_empty() { 0 } else { score(q, &p.keywords) * 3 / 4 };
    // Ligne secondaire (chemin…) : seulement si elle contient la requête telle quelle.
    let sub = if q.len() >= 3 && p.subtitle.contains(q) { 700 } else { 0 };
    title.max(kw).max(sub)
}

/// Filtre de portée (préfixes `a:` apps, `f:` fichiers, `d:` dossiers, `s:` réglages).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Only(Kind),
}

/// Les `limit` meilleurs éléments (indices dans `items`), meilleur d'abord.
pub fn top(items: &[Prepared], q: &str, scope: Scope, usage: &UsageMap, limit: usize) -> Vec<(usize, i32)> {
    let now = now_ms();
    let mut heap: BinaryHeap<Reverse<(i32, usize)>> = BinaryHeap::with_capacity(limit + 1);
    for (i, p) in items.iter().enumerate() {
        if let Scope::Only(k) = scope {
            if p.item.kind != k {
                continue;
            }
        }
        let used = usage.get(&p.item.id);
        let s = if q.is_empty() {
            // Navigation : apps et éléments déjà utilisés seulement.
            let f = frecency(used, now);
            if f == 0 && p.item.kind != Kind::App {
                continue;
            }
            f * 3 + if p.item.kind == Kind::App { 100 } else { 0 }
        } else {
            let b = base(q, p);
            if b == 0 {
                continue;
            }
            b + p.item.kind.bias() + frecency(used, now) - (p.depth as i32) / 2
        };
        heap.push(Reverse((s, i)));
        if heap.len() > limit {
            heap.pop();
        }
    }
    let mut out: Vec<(usize, i32)> = heap.into_iter().map(|Reverse((s, i))| (i, s)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| items[a.0].title.len().cmp(&items[b.0].title.len())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::model::Item;

    fn it(kind: Kind, title: &str, target: &str) -> Prepared {
        Prepared::new(Item { id: format!("{title}:{target}"), kind, title: title.into(), subtitle: String::new(), target: target.into(), keywords: String::new() })
    }

    #[test]
    fn app_beats_file_with_same_name() {
        let items = vec![it(Kind::File, "Discord", "C:\\x\\Discord"), it(Kind::App, "Discord", "C:\\Discord.lnk")];
        let r = top(&items, "discord", Scope::All, &UsageMap::new(), 5);
        assert_eq!(r[0].0, 1);
    }

    #[test]
    fn usage_lifts_a_result() {
        let items = vec![it(Kind::App, "Notepad", "a"), it(Kind::App, "Notion", "b")];
        let mut usage = UsageMap::new();
        usage.insert(items[1].item.id.clone(), Usage { count: 20, last: now_ms() });
        let r = top(&items, "no", Scope::All, &usage, 5);
        assert_eq!(r[0].0, 1);
    }

    #[test]
    fn scope_filters_kinds() {
        let items = vec![it(Kind::File, "chrome.txt", "a"), it(Kind::App, "Chrome", "b")];
        let r = top(&items, "chrome", Scope::Only(Kind::File), &UsageMap::new(), 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 0);
    }
}
