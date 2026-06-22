# Distribution & auto-update (runbook)

## CI: release on every tag

`.github/workflows/release.yml` (in place): `git tag v0.5.0 && git push --tags` → Windows
build + signing + a **GitHub release** with the installers and `latest.json` (read by the
auto-updater).

## Prerequisites (one time)

1. **Git repo + GitHub**:
   - `git init` + a `.gitignore` (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`).
   - create the GitHub repo, `git remote add origin …`, push.
2. **Updater signing keys**:
   - `pnpm tauri signer generate -w island-updater.key`
   - → **private key** + password → GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` /
     `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (Settings → Secrets → Actions).
   - → **public key** → `tauri.conf.json` (below). ⚠️ NEVER commit the private key.
3. **Updater plugin**:
   - Rust: `tauri-plugin-updater` (dep + `.plugin(tauri_plugin_updater::Builder::new().build())`).
   - JS: `@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process` (for relaunch).
4. **`tauri.conf.json`**:
   ```json
   "bundle": { "createUpdaterArtifacts": true },
   "plugins": {
     "updater": {
       "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"],
       "pubkey": "<public key generated in step 2>"
     }
   }
   ```
5. **Capability** (`settings` window): `updater:default`.
6. **Front-end**: at launch (or a "Check for updates" button in Settings) → `check()` → if
   available → `downloadAndInstall()` → `relaunch()`. (Island exposes this in Settings →
   Updates, with a reactive status.)

## Full flow

`git push --tags` → CI builds + **signs** → GitHub release (installers + `latest.json`) →
the installed app reads `latest.json`, compares the version, downloads the **signed**
installer, verifies the signature with the public key, installs, relaunches.

## ⚠️ Do this BEFORE renaming

A rename changes `productName`, the `identifier` and the repo name → freeze the `endpoints`
and `pubkey` **after** the rename, otherwise you'll have to redo them. (See also the note
about the identifier = the `%APPDATA%` path of installed extensions.)
