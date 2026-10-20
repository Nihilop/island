// Réglages de compilation du magasin, lus dans l'environnement ou, à défaut, dans le `.env`
// à la racine du dépôt (build local ; jamais commité) :
//   ISLAND_STORE_URL   adresse du magasin (ex. https://store.example.com)
//   ISLAND_STORE_KEYS  clés publiques du magasin, séparées par des virgules
// En CI (release), ils viennent des variables du dépôt GitHub (cf. .github/workflows/release.yml).
// Ils sont lus dans le code par `option_env!` (market.rs, extensions/install.rs).

const VARS: &[&str] = &["ISLAND_STORE_URL", "ISLAND_STORE_KEYS"];

fn dotenv(name: &str) -> Option<String> {
    let text = std::fs::read_to_string("../.env").ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.trim().split_once('=')?;
        (k.trim() == name).then(|| v.trim().trim_matches('"').trim_matches('\'').to_string())
    })
}

fn main() {
    println!("cargo:rerun-if-changed=../.env");
    for name in VARS {
        println!("cargo:rerun-if-env-changed={name}");
        let value = std::env::var(name).ok().filter(|v| !v.trim().is_empty()).or_else(|| dotenv(name)).filter(|v| !v.is_empty());
        if let Some(v) = value {
            println!("cargo:rustc-env={name}={v}");
        }
    }
    tauri_build::build()
}
