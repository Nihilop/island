// Modèle de l'index du launcher : un élément = une chose qu'on peut ouvrir (app, fichier,
// dossier, page de réglages Windows…). Les chaînes de recherche sont normalisées UNE fois
// au chargement : la boucle de recherche n'alloue rien.

use serde::{Deserialize, Serialize};

use super::normalize::normalize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    App,
    Setting,
    Folder,
    File,
}

impl Kind {
    /// Biais de classement par type (une app passe avant un fichier homonyme).
    pub fn bias(self) -> i32 {
        match self {
            Kind::App => 220,
            Kind::Setting => 0,
            Kind::Folder => 0,
            Kind::File => -20,
        }
    }
}

/// Élément tel qu'il est indexé et persisté.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    /// Identifiant stable : `app:<cible>`, `set:<uri>`, `file:<chemin>`…
    pub id: String,
    pub kind: Kind,
    pub title: String,
    /// Ligne secondaire (chemin, catégorie de réglage…).
    pub subtitle: String,
    /// Ce qu'on ouvre : chemin, `shell:AppsFolder\…`, `steam://…`, `ms-settings:…`.
    pub target: String,
    /// Mots-clés supplémentaires (alias de réglages, FR + EN).
    #[serde(default)]
    pub keywords: String,
}

/// Élément prêt pour la recherche (chaînes normalisées pré-calculées).
pub struct Prepared {
    pub item: Item,
    pub title: Box<str>,
    pub subtitle: Box<str>,
    pub keywords: Box<str>,
    /// Profondeur du chemin (fichiers) : les éléments enfouis passent après.
    pub depth: u16,
}

impl Prepared {
    pub fn new(item: Item) -> Self {
        let depth = if matches!(item.kind, Kind::File | Kind::Folder) {
            item.target.matches(['\\', '/']).count() as u16
        } else {
            0
        };
        Self {
            title: normalize(&item.title).into(),
            subtitle: normalize(&item.subtitle).into(),
            keywords: normalize(&item.keywords).into(),
            depth,
            item,
        }
    }
}
