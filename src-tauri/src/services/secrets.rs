// Service `secrets` : stockage CHIFFRÉ de petites valeurs sensibles (tokens d'API,
// mots de passe…), isolé par extension. Contrairement à `storage` (JSON en clair),
// les valeurs vivent dans le coffre du système (Windows Credential Manager via
// `keyring` ; Keychain/Secret Service au portage) — jamais sur disque en clair.
//
// Cloisonné par id d'extension (service = `island-ext:<id>`), comme `storage` : une
// extension ne lit que SES propres secrets. Pas de permission dédiée (ne donne accès
// qu'à ses propres données), fourni via `ctx.secrets`.

use keyring::Entry;

fn entry(ext: &str, key: &str) -> Result<Entry, String> {
    Entry::new(&format!("island-ext:{ext}"), key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn secret_get(ext: String, key: String) -> Result<Option<String>, String> {
    match entry(&ext, &key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn secret_set(ext: String, key: String, value: String) -> Result<(), String> {
    entry(&ext, &key)?.set_password(&value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn secret_delete(ext: String, key: String) -> Result<(), String> {
    match entry(&ext, &key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
