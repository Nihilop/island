# Distribution & auto-update (runbook)

## CI : release à chaque tag

[`.github/workflows/release.yml`](../.github/workflows/release.yml) (en place) :
`git tag v0.5.0 && git push --tags` → build Windows + signature + **release GitHub** avec
les installeurs et `latest.json` (lu par l'auto-updater).

## Prérequis (une seule fois)

1. **Dépôt git + GitHub** (le projet n'est pas encore versionné) :
   - `git init` + un `.gitignore` (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`).
   - créer le dépôt GitHub, `git remote add origin …`, push.
2. **Clés de signature de l'updater** :
   - `pnpm tauri signer generate -w island-updater.key`
   - → **clé privée** + mot de passe → secrets GitHub `TAURI_SIGNING_PRIVATE_KEY` /
     `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (Settings → Secrets → Actions).
   - → **clé publique** → `tauri.conf.json` (ci-dessous). ⚠️ ne JAMAIS committer la clé privée.
3. **Plugin updater** :
   - Rust : `tauri-plugin-updater` (dep + `.plugin(tauri_plugin_updater::Builder::new().build())`).
   - JS : `@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process` (pour relaunch).
4. **`tauri.conf.json`** :
   ```json
   "bundle": { "createUpdaterArtifacts": true },
   "plugins": {
     "updater": {
       "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"],
       "pubkey": "<clé publique générée à l'étape 2>"
     }
   }
   ```
5. **Capability** (fenêtre `settings`) : `updater:default`.
6. **Front** : au lancement (ou bouton « Vérifier les mises à jour » dans Réglages) →
   `check()` → si dispo → `downloadAndInstall()` → `relaunch()`.

## Flux complet

`git push --tags` → CI compile + **signe** → release GitHub (installeurs + `latest.json`)
→ l'app installée lit `latest.json`, compare la version, télécharge l'installeur **signé**,
vérifie la signature avec la clé publique, installe, relance.

## ⚠️ À faire AVANT le rename

Le rename change `productName`, l'`identifier` et le nom du dépôt → fige les `endpoints`
et le `pubkey` **après** le rename, sinon il faudra les refaire. (Voir aussi la note sur
l'identifier = chemin `%APPDATA%` des extensions installées.)
