#!/usr/bin/env node
// Outil des extensions Island v2 (Node seul, aucune dépendance).
//
//   island keygen <fichier-clé.json>             crée une clé de signature ed25519
//   island pack [dossier] [--key <clé.json>] [--out <paquet.island>]
//
// Paquet `.island` = zip : manifest.json + dist/, icons/, locales/, assets/ et les fichiers
// référencés par le manifeste. Avec --key, ajoute ISLAND-SIGNATURE.json : signature
// ed25519 de l'empreinte du contenu (lignes « <sha256>  <chemin> » triées par chemin) —
// même calcul que la vérification d'Island (src-tauri/src/extensions/install.rs).

import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, join, relative, resolve, sep } from "node:path";
import { deflateRawSync } from "node:zlib";

const SIGNATURE_FILE = "ISLAND-SIGNATURE.json";
const DIRS = ["dist", "icons", "locales", "assets"];

function die(msg) {
  console.error(`island: ${msg}`);
  process.exit(1);
}

function arg(name) {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
}

// --- keygen ----------------------------------------------------------------------------

function keygen(out) {
  if (!out) die("usage : island keygen <fichier-clé.json>");
  if (existsSync(out)) die(`${out} existe déjà (on n'écrase jamais une clé)`);
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const raw = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  const key = { public: raw.toString("base64"), private: privateKey.export({ type: "pkcs8", format: "pem" }) };
  writeFileSync(out, JSON.stringify(key, null, 2), { mode: 0o600 });
  console.log(`clé créée : ${out}\nclé publique : ${key.public}\n⚠ Garde ce fichier secret et sauvegardé : sans lui, plus de mise à jour signée.`);
}

// --- pack ------------------------------------------------------------------------------

function walk(dir, root, out) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, root, out);
    else out.add(relative(root, p).split(sep).join("/"));
  }
}

function collect(root, manifest) {
  const files = new Set(["manifest.json"]);
  for (const d of DIRS) if (existsSync(join(root, d))) walk(join(root, d), root, files);
  const refs = [manifest.main, manifest.icon, ...Object.values(manifest.locales ?? {})];
  for (const t of manifest.contributes?.tools ?? []) refs.push(t.view, t.icon);
  for (const r of refs.filter(Boolean)) {
    if (!existsSync(join(root, r))) die(`fichier référencé introuvable : ${r} (lancer le build ?)`);
    files.add(r);
  }
  return [...files].sort();
}

const sha256 = (b) => createHash("sha256").update(b).digest("hex");

/** Empreinte signée (identique à `digest` côté Rust). */
function digest(entries) {
  return entries
    .filter(([p]) => p !== SIGNATURE_FILE)
    .map(([p, b]) => [p, `${sha256(b)}  ${p}\n`])
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([, line]) => line)
    .join("");
}

// Zip minimal (deflate), suffisant pour un paquet d'extension.
const CRC = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function zip(entries) {
  const chunks = [];
  const central = [];
  let offset = 0;
  for (const [name, data] of entries) {
    const nameBuf = Buffer.from(name, "utf8");
    const comp = deflateRawSync(data);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6); // UTF-8
    local.writeUInt16LE(8, 8); // deflate
    local.writeUInt32LE(0, 10); // heure/date
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(comp.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    chunks.push(local, nameBuf, comp);
    const cd = Buffer.alloc(46);
    cd.writeUInt32LE(0x02014b50, 0);
    cd.writeUInt16LE(20, 4);
    cd.writeUInt16LE(20, 6);
    cd.writeUInt16LE(0x0800, 8);
    cd.writeUInt16LE(8, 10);
    cd.writeUInt32LE(0, 12);
    cd.writeUInt32LE(crc, 16);
    cd.writeUInt32LE(comp.length, 20);
    cd.writeUInt32LE(data.length, 24);
    cd.writeUInt16LE(nameBuf.length, 28);
    cd.writeUInt32LE(offset, 42);
    central.push(cd, nameBuf);
    offset += local.length + nameBuf.length + comp.length;
  }
  const cdBuf = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(cdBuf.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...chunks, cdBuf, end]);
}

function pack(dir = ".") {
  const root = resolve(dir);
  const manifestPath = join(root, "manifest.json");
  if (!existsSync(manifestPath)) die(`pas de manifest.json dans ${root}`);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const entries = collect(root, manifest).map((p) => [p, readFileSync(join(root, p))]);
  const keyFile = arg("--key");
  if (keyFile) {
    const key = JSON.parse(readFileSync(keyFile, "utf8"));
    const priv = createPrivateKey(key.private);
    const pub = createPublicKey(priv).export({ type: "spki", format: "der" }).subarray(-32).toString("base64");
    const sig = sign(null, Buffer.from(digest(entries), "utf8"), priv).toString("base64");
    entries.push([SIGNATURE_FILE, Buffer.from(JSON.stringify({ key: pub, sig }))]);
  }
  const out = resolve(arg("--out") ?? `${manifest.id}-${manifest.version}.island`);
  writeFileSync(out, zip(entries));
  const size = (statSync(out).size / 1024).toFixed(1);
  console.log(`${basename(out)} : ${entries.length} fichiers, ${size} Ko, ${keyFile ? "signé" : "NON signé (installable seulement en mode développeur)"}`);
}

const [cmd, ...rest] = process.argv.slice(2);
if (cmd === "keygen") keygen(rest[0]);
else if (cmd === "pack") pack(rest[0] && !rest[0].startsWith("--") ? rest[0] : ".");
else die("commandes : keygen <clé.json> · pack [dossier] [--key clé.json] [--out paquet.island]");
