#!/usr/bin/env node
// Release en une commande : `pnpm publish:app`
// Affiche la version courante, demande la suivante (ou patch/minor/major), met à jour
// les 3 fichiers de version (tauri.conf.json, Cargo.toml, package.json), commit, tag et
// push → déclenche la CI GitHub qui build/signe/publie la release.
import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { createInterface } from "node:readline/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const FILES = {
  tauri: join(ROOT, "src-tauri", "tauri.conf.json"),
  cargo: join(ROOT, "src-tauri", "Cargo.toml"),
  pkg: join(ROOT, "package.json"),
};
const SEMVER = /^\d+\.\d+\.\d+$/;

const read = (p) => readFileSync(p, "utf8");
const sh = (cmd) => execSync(cmd, { cwd: ROOT, stdio: "inherit" });
const shOut = (cmd) => execSync(cmd, { cwd: ROOT }).toString().trim();

// La version de l'app fait foi côté tauri.conf.json (= celle qu'utilise l'updater).
function currentVersion() {
  const m = read(FILES.tauri).match(/"version":\s*"([^"]+)"/);
  return m ? m[1] : null;
}

function bump(v, kind) {
  const [a, b, c] = v.split(".").map(Number);
  if (kind === "major") return `${a + 1}.0.0`;
  if (kind === "minor") return `${a}.${b + 1}.0`;
  return `${a}.${b}.${c + 1}`; // patch
}

function setVersion(next) {
  // 1ʳᵉ clé "version" des JSON (= version du package, jamais une dépendance).
  for (const p of [FILES.tauri, FILES.pkg]) {
    writeFileSync(p, read(p).replace(/("version":\s*")[^"]+(")/, `$1${next}$2`));
  }
  // Ligne `version = "…"` du [package] de Cargo.toml (la 1ʳᵉ ancrée en début de ligne).
  writeFileSync(FILES.cargo, read(FILES.cargo).replace(/^version = "[^"]+"/m, `version = "${next}"`));
}

const rl = createInterface({ input: process.stdin, output: process.stdout });
try {
  const cur = currentVersion();
  if (!cur) {
    console.error("✗ Version introuvable dans tauri.conf.json.");
    process.exit(1);
  }

  console.log(`Version actuelle : ${cur}`);

  // Niveau de bump depuis les ARGS : rien = patch (cas courant), --minor, --major.
  // Un semver explicite (ex. `1.2.0`) en argument l'emporte sur tout.
  const args = process.argv.slice(2);
  const explicit = args.find((a) => SEMVER.test(a));
  const kind = args.includes("--major") ? "major" : args.includes("--minor") ? "minor" : "patch";
  const next = explicit ?? bump(cur, kind);
  console.log(`Bump : ${explicit ? "explicite" : kind}`);

  if (!SEMVER.test(next)) {
    console.error(`✗ « ${next} » n'est pas un semver valide (X.Y.Z).`);
    process.exit(1);
  }
  if (next === cur) {
    console.error(`✗ La nouvelle version est identique à l'actuelle (${cur}).`);
    process.exit(1);
  }

  // TOUT l'état courant du repo part dans la release (pas seulement les fichiers de version).
  const pending = shOut("git status --short");
  if (pending) console.log("\nSeront committés (en plus du bump de version) :\n" + pending);

  const tag = `v${next}`;
  const ok = (await rl.question(`\n→ ${cur} → ${next} : git add -A + commit + push + tag ${tag} + push. Continuer ? [y/N] `)).trim().toLowerCase();
  if (!["y", "o", "yes", "oui"].includes(ok)) {
    console.log("Annulé. (Aucun fichier modifié.)");
    process.exit(0);
  }

  setVersion(next);
  sh("git add -A"); // tout l'état courant, pas juste les 3 fichiers de version
  sh(`git commit -m "release ${tag}"`);
  sh("git push"); // pousse le commit AVANT le tag
  sh(`git tag ${tag}`);
  sh(`git push origin ${tag}`); // le tag déclenche la CI (son commit est déjà sur origin)

  const repo = shOut("git remote get-url origin")
    .replace(/\.git$/, "")
    .replace(/^git@github\.com:/, "https://github.com/");
  console.log(`\n✓ ${tag} poussé. La CI build et publie la release → ${repo}/actions`);
} finally {
  rl.close();
}
