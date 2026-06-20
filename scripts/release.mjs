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

  const dirty = shOut("git status --porcelain");
  if (dirty) {
    console.log("\n⚠️  Changements non commités — ils NE seront PAS dans la release tant");
    console.log("    qu'ils ne sont pas commités à part :\n" + dirty + "\n");
  }

  console.log(`Version actuelle : ${cur}`);
  const suggested = bump(cur, "patch");
  const ans = (await rl.question(`Nouvelle version (X.Y.Z, ou patch/minor/major) [${suggested}] : `)).trim();
  const next = !ans ? suggested : ["patch", "minor", "major"].includes(ans) ? bump(cur, ans) : ans;

  if (!SEMVER.test(next)) {
    console.error(`✗ « ${next} » n'est pas un semver valide (X.Y.Z).`);
    process.exit(1);
  }
  if (next === cur) {
    console.error(`✗ La nouvelle version est identique à l'actuelle (${cur}).`);
    process.exit(1);
  }

  const tag = `v${next}`;
  const ok = (await rl.question(`\n→ ${cur} → ${next} : update fichiers + commit + tag ${tag} + push. Continuer ? [y/N] `)).trim().toLowerCase();
  if (!["y", "o", "yes", "oui"].includes(ok)) {
    console.log("Annulé. (Aucun fichier modifié.)");
    process.exit(0);
  }

  setVersion(next);
  sh("git add src-tauri/tauri.conf.json src-tauri/Cargo.toml package.json");
  sh(`git commit -m "release ${tag}"`);
  sh(`git tag ${tag}`);
  sh("git push");
  sh(`git push origin ${tag}`);

  const repo = shOut("git remote get-url origin")
    .replace(/\.git$/, "")
    .replace(/^git@github\.com:/, "https://github.com/");
  console.log(`\n✓ ${tag} poussé. La CI build et publie la release → ${repo}/actions`);
} finally {
  rl.close();
}
