#!/usr/bin/env node
// Outil des extensions Island v2 (Node seul, aucune dépendance). Sortie en anglais : c'est
// l'outil public du SDK.
//
//   island keygen <fichier-clé.json>             crée une clé de signature ed25519
//   island pack [dossier] [--key <clé.json>] [--out <paquet.island>]
//   island login | logout | whoami                compte du magasin (GitHub)
//   island publish <fichier> [--item <id>] [--new] [--summary …] [--tags a,b] [--changelog …]
//
// Magasin : `--store <url>`, sinon ISLAND_STORE_URL, sinon https://store.streemkit.com.
// Le jeton du magasin est rangé dans ~/.island/credentials.json (droits 600), une entrée
// par adresse de magasin.
//
// Paquet `.island` = zip : manifest.json + dist/, icons/, locales/, assets/ et les fichiers
// référencés par le manifeste. Avec --key, ajoute ISLAND-SIGNATURE.json : signature
// ed25519 de l'empreinte du contenu (lignes « <sha256>  <chemin> » triées par chemin) —
// même calcul que la vérification d'Island (src-tauri/src/extensions/install.rs).

import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { homedir, hostname } from "node:os";
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

/** Options suivies d'une valeur : tout le reste (hors `--…`) est positionnel. */
const VALUED = new Set(["--key", "--out", "--store", "--item", "--summary", "--description", "--tags", "--changelog", "--homepage", "--repository", "--license"]);
function positionals() {
  const out = [];
  const a = process.argv.slice(2);
  for (let i = 0; i < a.length; i++) {
    if (VALUED.has(a[i])) i++;
    else if (!a[i].startsWith("--")) out.push(a[i]);
  }
  return out;
}

// --- keygen ----------------------------------------------------------------------------

function keygen(out) {
  if (!out) die("usage: island keygen <key-file.json>");
  if (existsSync(out)) die(`${out} already exists (a key is never overwritten)`);
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const raw = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  const key = { public: raw.toString("base64"), private: privateKey.export({ type: "pkcs8", format: "pem" }) };
  writeFileSync(out, JSON.stringify(key, null, 2), { mode: 0o600 });
  console.log(`key created: ${out}\npublic key: ${key.public}\n⚠ Keep this file secret and backed up: without it, no more signed updates.`);
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
    if (!existsSync(join(root, r))) die(`referenced file not found: ${r} (run the build first?)`);
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
  if (!existsSync(manifestPath)) die(`no manifest.json in ${root}`);
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
  console.log(`${basename(out)}: ${entries.length} files, ${size} KB, ${keyFile ? "signed" : "UNSIGNED (installable in developer mode only)"}`);
}

// --- Magasin -----------------------------------------------------------------------------

const STORE = (arg("--store") ?? process.env.ISLAND_STORE_URL ?? "https://store.streemkit.com").replace(/\/+$/, "");
const HOME = join(homedir(), ".island");
const CREDENTIALS = join(HOME, "credentials.json");
/** Fichiers déjà publiés : chemin absolu → { store, item } (nouvelle version par défaut). */
const PUBLISHED = join(HOME, "published.json");

const readJson = (f) => (existsSync(f) ? JSON.parse(readFileSync(f, "utf8")) : {});
function writeJson(f, v) {
  mkdirSync(HOME, { recursive: true });
  writeFileSync(f, JSON.stringify(v, null, 2), { mode: 0o600 });
}
const token = () => readJson(CREDENTIALS)[STORE];

const MESSAGES = {
  unauthorized: "session expired or revoked: run `island login` again",
  publishingPaused: "publishing is paused by the store moderators. Nothing was uploaded; try again later.",
  idTaken: "this extension id belongs to another author",
  versionNotGreater: "the manifest version must be higher than the last published one",
  invalidSignature: "the package signature is wrong (package modified after signing?)",
  idMismatch: "the manifest id does not match --item",
  reservedId: "dev.island.* ids are reserved for official extensions",
  forbidden: "this item does not belong to you",
  notFound: "item not found (deleted?): publish it as a new one with --new",
  kindMismatch: "the file is not the same kind as the target item",
  banned: "this account is banned from the store",
  malware: "the file was rejected by the store antivirus",
  tooLarge: "file too large",
  invalidTags: "invalid tags (8 at most, a-z 0-9 -, 24 characters)",
  corrupt: "this file is not a valid Island scene",
  sceneTooNew: "the scene was made by a newer Island than the store supports",
  nameRequired: "the scene has no name",
  expired: "the code expired: run `island login` again",
  denied: "sign-in was denied on GitHub",
};

async function api(method, path, { body, auth = true, headers = {} } = {}) {
  const h = { "user-agent": "island-cli", ...headers };
  if (auth) {
    const t = token();
    if (!t) die(`not signed in to ${STORE}: run \`island login\``);
    h.authorization = `Bearer ${t}`;
  }
  let res;
  try {
    res = await fetch(STORE + path, { method, headers: h, body });
  } catch (e) {
    die(`store unreachable (${STORE}): ${e.cause?.code ?? e.message}`);
  }
  const data = res.status === 204 ? null : await res.json().catch(() => null);
  return { status: res.status, data };
}

function fail({ status, data }) {
  const code = data?.error ?? `http${status}`;
  die(MESSAGES[code] ?? code);
}

const sleep = (s) => new Promise((r) => setTimeout(r, s * 1000));
const json = (v) => ({ body: JSON.stringify(v), headers: { "content-type": "application/json" } });

async function login() {
  const start = await api("POST", "/v1/auth/device", { auth: false, ...json({}) });
  if (start.status !== 200) fail(start);
  const { deviceCode, userCode, verificationUri, expiresIn } = start.data;
  let interval = start.data.interval;
  console.log(`Open ${verificationUri} and enter the code: ${userCode}`);
  const deadline = Date.now() + expiresIn * 1000;
  while (Date.now() < deadline) {
    await sleep(interval);
    const r = await api("POST", "/v1/auth/token", { auth: false, ...json({ deviceCode, name: `CLI sur ${hostname()}` }) });
    if (r.status === 202) {
      if (r.data?.status === "slowDown") interval += 5;
      continue;
    }
    if (r.status !== 200) fail(r);
    writeJson(CREDENTIALS, { ...readJson(CREDENTIALS), [STORE]: r.data.token });
    return console.log(`signed in as ${r.data.user.login} (${STORE})`);
  }
  die(MESSAGES.expired);
}

async function logout() {
  if (token()) await api("DELETE", "/v1/auth/token");
  const c = readJson(CREDENTIALS);
  delete c[STORE];
  writeJson(CREDENTIALS, c);
  console.log(`signed out of ${STORE}`);
}

async function whoami() {
  const r = await api("GET", "/v1/me");
  if (r.status !== 200) fail(r);
  console.log(`${r.data.login} (${r.data.role}) on ${STORE}`);
}

/** Arrête la commande si les publications sont suspendues (`known` : le serveur l'a dit). */
async function stopIfPaused(known) {
  const s = await api("GET", "/v1/store/status", { auth: false });
  if (!known && s.data?.publishing?.open !== false) return;
  const why = s.data?.publishing?.message;
  die(`${MESSAGES.publishingPaused}${why ? `\n  Message from the moderators: ${why}` : ""}`);
}

async function publish(file) {
  if (!file || file.startsWith("--")) die("usage: island publish <file.islandscene|file.island> [--item <id>] [--new] [--summary …] [--tags a,b] [--changelog …]");
  const path = resolve(file);
  if (!existsSync(path)) die(`file not found: ${file}`);
  if (!/\.(islandscene|island)$/i.test(path)) die("the file must be an .islandscene scene or an .island extension");
  const known = readJson(PUBLISHED)[path];
  const remembered = !process.argv.includes("--new") && known?.store === STORE ? known.item : undefined;
  const item = arg("--item") ?? remembered;
  const form = new FormData();
  if (item) form.set("item", item);
  for (const k of ["summary", "description", "changelog", "homepage", "repository", "license"]) if (arg(`--${k}`)) form.set(k, arg(`--${k}`));
  if (arg("--tags")) form.set("tags", JSON.stringify(arg("--tags").split(",").map((t) => t.trim()).filter(Boolean)));
  form.set("file", new Blob([readFileSync(path)]), basename(path));
  // Extension : l'élément est désigné par l'id de son manifeste (côté magasin).
  const isExtension = /\.island$/i.test(path);
  // Publications suspendues par la modération : on le dit AVANT d'envoyer le fichier.
  await stopIfPaused(false);
  console.log(isExtension ? "uploading the extension…" : item ? `uploading a new version of ${item}…` : "uploading a new item…");
  const r = await api("POST", "/v1/items", { body: form });
  if (r.status === 503 && r.data?.error === "publishingPaused") await stopIfPaused(true);
  if (r.status !== 201 && r.status !== 200 && r.status !== 202) {
    if (r.data?.error?.startsWith("fileType:")) die(`file not allowed in a store package: ${r.data.error.slice(9)} (web resources only)`);
    fail(r);
  }
  writeJson(PUBLISHED, { ...readJson(PUBLISHED), [path]: { store: STORE, item: r.data.id } });
  const scan = r.data.scan === "skipped" ? " (not scanned by the antivirus)" : "";
  if (r.data.status === "pending") console.log(`submitted: ${r.data.id} v${r.data.version}${scan}, waiting for review by the moderators`);
  else console.log(`published: ${r.data.id} v${r.data.version}${scan}\n  ${STORE}/v1/items/${r.data.id}`);
  if (!item && !isExtension) console.log("  publishing this file again sends a new version (--new to create another item)");
}

const [cmd, ...rest] = positionals();
const commands = {
  keygen: () => keygen(rest[0]),
  pack: () => pack(rest[0] ?? "."),
  login,
  logout,
  whoami,
  publish: () => publish(rest[0]),
};
if (commands[cmd]) await commands[cmd]();
else
  die(
    "commands: keygen <key.json> · pack [folder] [--key key.json] [--out package.island] · login · logout · whoami · " +
      "publish <file> [--item id] [--new] [--summary …] [--tags a,b] [--changelog …]   (--store <url>)",
  );
