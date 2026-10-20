#!/usr/bin/env node
// Outil des extensions Island v2 (Node seul, aucune dépendance).
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
const VALUED = new Set(["--key", "--out", "--store", "--item", "--summary", "--tags", "--changelog"]);
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
  unauthorized: "session expirée ou révoquée : relance « island login »",
  publishingPaused: "les publications sont suspendues pour le moment",
  extensionsNotYet: "le magasin n'accepte pas encore les extensions (seulement les scènes .islandscene)",
  forbidden: "cet élément ne t'appartient pas",
  notFound: "élément introuvable (supprimé ?) : publie-le comme nouveau avec --new",
  kindMismatch: "le fichier n'est pas du même type que l'élément visé",
  banned: "ce compte est banni du magasin",
  malware: "le fichier a été refusé par l'antivirus du magasin",
  tooLarge: "fichier trop gros",
  invalidTags: "étiquettes invalides (8 au plus, a-z 0-9 -, 24 caractères)",
  corrupt: "ce fichier n'est pas une scène Island valide",
  sceneTooNew: "scène créée par une version d'Island plus récente que celle du magasin",
  nameRequired: "la scène n'a pas de nom",
  expired: "le code a expiré : relance « island login »",
  denied: "connexion refusée sur GitHub",
};

async function api(method, path, { body, auth = true, headers = {} } = {}) {
  const h = { "user-agent": "island-cli", ...headers };
  if (auth) {
    const t = token();
    if (!t) die(`pas connecté à ${STORE} : lance « island login »`);
    h.authorization = `Bearer ${t}`;
  }
  let res;
  try {
    res = await fetch(STORE + path, { method, headers: h, body });
  } catch (e) {
    die(`magasin injoignable (${STORE}) : ${e.cause?.code ?? e.message}`);
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
  console.log(`Ouvre ${verificationUri} et saisis le code : ${userCode}`);
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
    return console.log(`connecté en tant que ${r.data.user.login} (${STORE})`);
  }
  die(MESSAGES.expired);
}

async function logout() {
  if (token()) await api("DELETE", "/v1/auth/token");
  const c = readJson(CREDENTIALS);
  delete c[STORE];
  writeJson(CREDENTIALS, c);
  console.log(`déconnecté de ${STORE}`);
}

async function whoami() {
  const r = await api("GET", "/v1/me");
  if (r.status !== 200) fail(r);
  console.log(`${r.data.login} (${r.data.role}) sur ${STORE}`);
}

async function publish(file) {
  if (!file || file.startsWith("--")) die("usage : island publish <fichier.islandscene> [--item <id>] [--new] [--summary …] [--tags a,b] [--changelog …]");
  const path = resolve(file);
  if (!existsSync(path)) die(`fichier introuvable : ${file}`);
  if (!/\.(islandscene|island)$/i.test(path)) die("le fichier doit être une scène .islandscene (ou, plus tard, une extension .island)");
  const known = readJson(PUBLISHED)[path];
  const remembered = !process.argv.includes("--new") && known?.store === STORE ? known.item : undefined;
  const item = arg("--item") ?? remembered;
  const form = new FormData();
  if (item) form.set("item", item);
  for (const k of ["summary", "changelog"]) if (arg(`--${k}`)) form.set(k, arg(`--${k}`));
  if (arg("--tags")) form.set("tags", JSON.stringify(arg("--tags").split(",").map((t) => t.trim()).filter(Boolean)));
  form.set("file", new Blob([readFileSync(path)]), basename(path));
  console.log(item ? `envoi d'une nouvelle version de ${item}…` : "envoi d'un nouvel élément…");
  const r = await api("POST", "/v1/items", { body: form });
  if (r.status === 503 && r.data?.error === "publishingPaused") {
    const s = await api("GET", "/v1/store/status", { auth: false });
    const why = s.data?.publishing?.message;
    die(`${MESSAGES.publishingPaused}${why ? ` : ${why}` : ""}`);
  }
  if (r.status !== 201 && r.status !== 200) fail(r);
  writeJson(PUBLISHED, { ...readJson(PUBLISHED), [path]: { store: STORE, item: r.data.id } });
  const scan = r.data.scan === "skipped" ? " (non analysé par l'antivirus)" : "";
  console.log(`publié : ${r.data.id} v${r.data.version}${scan}\n  ${STORE}/v1/items/${r.data.id}`);
  if (!item) console.log("  republier ce fichier enverra une nouvelle version (--new pour créer un autre élément)");
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
    "commandes : keygen <clé.json> · pack [dossier] [--key clé.json] [--out paquet.island] · login · logout · whoami · " +
      "publish <fichier> [--item id] [--new] [--summary …] [--tags a,b] [--changelog …]   (--store <url>)",
  );
