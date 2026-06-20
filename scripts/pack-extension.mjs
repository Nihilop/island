// Empaquette une extension en .island (= archive zip avec manifest.json à la racine).
//
// Usage :  node scripts/pack-extension.mjs <dossier-extension> [dossier-sortie]
// Exemple : node scripts/pack-extension.mjs extensions/spotify dist-extensions
//
// Sous Windows on s'appuie sur Compress-Archive (aucune dépendance npm).

import { execFileSync } from "node:child_process";
import { readFileSync, existsSync, mkdirSync, rmSync, renameSync } from "node:fs";
import { join, resolve, basename } from "node:path";

const srcArg = process.argv[2];
const outArg = process.argv[3] ?? "dist-extensions";

if (!srcArg) {
  console.error("Usage: node scripts/pack-extension.mjs <dossier-extension> [dossier-sortie]");
  process.exit(1);
}

const srcDir = resolve(srcArg);
const manifestPath = join(srcDir, "manifest.json");

if (!existsSync(manifestPath)) {
  console.error(`✗ manifest.json introuvable dans ${srcDir}`);
  process.exit(1);
}

const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const id = manifest.id ?? basename(srcDir);

const outDir = resolve(outArg);
mkdirSync(outDir, { recursive: true });

const zipPath = join(outDir, `${id}.zip`);
const islandPath = join(outDir, `${id}.island`);

// Nettoie d'anciens artefacts.
for (const p of [zipPath, islandPath]) if (existsSync(p)) rmSync(p);

// Un .island ne contient QUE l'artefact buildé : manifest.json + dist/.
// (Le code source reste côté dev ; Island ne charge jamais que le build.)
const distDir = join(srcDir, "dist");
if (!existsSync(distDir)) {
  console.error(`✗ ${distDir} introuvable — build d'abord : node scripts/build-extension.mjs ${srcArg}`);
  process.exit(1);
}

// Compress-Archive accepte une liste de chemins ; manifest.json + dist/ → racine de l'archive.
execFileSync(
  "powershell",
  [
    "-NoProfile",
    "-Command",
    `Compress-Archive -Path '${manifestPath}','${distDir}' -DestinationPath '${zipPath}' -Force`,
  ],
  { stdio: "inherit" }
);

renameSync(zipPath, islandPath);
console.log(`✓ ${manifest.name ?? id} → ${islandPath}`);
