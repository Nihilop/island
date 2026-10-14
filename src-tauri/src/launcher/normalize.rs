// Normalisation des chaînes de recherche : minuscules + accents retirés (« Café » =
// « cafe »), chemin rapide quand tout est ASCII. Table maison des lettres latines
// accentuées courantes (FR, DE, ES, PT, IT, nordiques) : pas de dépendance Unicode lourde.

pub fn normalize(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        for l in c.to_lowercase() {
            match fold(l) {
                Some(f) => out.push_str(f),
                None => out.push(l),
            }
        }
    }
    out
}

fn fold(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => "o",
        'œ' => "oe",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => "u",
        'ý' | 'ÿ' => "y",
        'ß' => "ss",
        'ś' | 'š' => "s",
        'ź' | 'ż' | 'ž' => "z",
        'ł' => "l",
        'ř' => "r",
        'ť' => "t",
        'ď' => "d",
        '’' | '‘' => "'",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn folds_accents_and_case() {
        assert_eq!(normalize("Café Crème"), "cafe creme");
        assert_eq!(normalize("Paramètres d’Écran"), "parametres d'ecran");
        assert_eq!(normalize("VS Code"), "vs code");
        assert_eq!(normalize("Straße"), "strasse");
    }
}
