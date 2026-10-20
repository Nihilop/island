// Magasin d'Island (commandes `market_*` de src-tauri/src/market.rs) : catalogue, fiche,
// installation. Le Rust fait les appels réseau ; les images viennent directement du CDN.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { reactive } from "vue";
import { t } from "../notch/i18n";
import { shareError } from "../wallpaper/api";

export interface Card {
  id: string;
  kind: "scene" | "extension";
  name: string;
  summary: string;
  tags: string[];
  downloads: number;
  updatedAt: string;
  author: string;
  authorAvatar: string | null;
  version: string | null;
  thumbnail: string | null;
}

export interface Version {
  version: string;
  size: number;
  changelog: string;
  createdAt: string;
}

export interface Detail extends Card {
  description: string;
  homepage: string | null;
  repository: string | null;
  license: string | null;
  media: { thumbnail: string | null; banner: string | null; screenshots: string[] };
  versions: Version[];
  /** Extension : permissions de la dernière version. */
  permissions: string[] | null;
}

export interface Page {
  items: Card[];
  page: number;
  total: number;
}

/** Taille d'une page du catalogue (côté serveur). */
export const PAGE = 24;

let base = "";
/** URL d'une image : absolue (CDN) telle quelle, relative (serveur local) complétée. */
export function mediaUrl(u: string | null | undefined): string | null {
  if (!u) return null;
  return u.startsWith("/") ? base + u : u;
}

async function get<T>(path: string, query?: Record<string, string | undefined>): Promise<T> {
  if (!base) base = await invoke<string>("market_base");
  const q = Object.entries(query ?? {}).filter((e): e is [string, string] => !!e[1]);
  return invoke<T>("market_get", { path, query: q });
}

/** `kind` : scènes par défaut ; `null` = tout le magasin. */
export const catalog = (p: { kind?: "scene" | "extension" | null; q?: string; tag?: string; sort?: "recent" | "popular"; page?: number }) =>
  get<Page>("/v1/items", { kind: p.kind === null ? undefined : (p.kind ?? "scene"), q: p.q, tag: p.tag, sort: p.sort, page: p.page ? String(p.page) : undefined });

export const item = (id: string) => get<Detail>(`/v1/items/${encodeURIComponent(id)}`);

/** Étiquettes les plus utilisées (filtres du magasin). */
export const tags = (kind?: "scene" | "extension") => get<{ tag: string; count: number }[]>("/v1/tags", { kind });

export interface Installed {
  scene: string;
  version: string;
}

/** Éléments installés (id du magasin → scène locale), tenus à jour. */
export const market = reactive({ installed: {} as Record<string, Installed> });

export async function refreshInstalled() {
  market.installed = await invoke<Record<string, Installed>>("market_installed").catch(() => market.installed);
}

let watching = false;
export function watchInstalled() {
  if (watching) return;
  watching = true;
  void refreshInstalled();
  void listen("market://installed", () => void refreshInstalled());
  // Une scène supprimée n'est plus « installée ».
  void listen("wallpaper://changed", () => void refreshInstalled());
}

export const install = (id: string, version?: string) => invoke<{ id: string; name: string }>("market_install", { id, version: version ?? null });

/** Message d'erreur lisible (codes du serveur ou du Rust). */
export function marketError(e: unknown): string {
  const code = String(e);
  const k = `market.err.${code}`;
  const msg = t(k);
  if (msg !== k) return msg;
  // Erreurs du format (paquet invalide…), partagées avec l'import d'un fichier.
  const share = shareError(code);
  return share !== code ? share : t("market.err.other", { e: code });
}

// --- Compte (connexion GitHub, device flow) ------------------------------------------------

export interface Account {
  id: number;
  login: string;
  avatarUrl: string | null;
  role: "user" | "moderator" | "admin";
}

export interface LoginStart {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export const session = reactive({ account: null as Account | null, loaded: false });

export async function refreshAccount() {
  try {
    session.account = await invoke<Account | null>("market_account");
  } finally {
    session.loaded = true;
  }
}

let followingAccount = false;
/** Charge le compte et suit les connexions / déconnexions (autre fenêtre). */
export function watchAccount() {
  if (followingAccount) return;
  followingAccount = true;
  void refreshAccount().catch(() => {});
  void listen("market://account", () => void refreshAccount().catch(() => {}));
}

/** Démarre la connexion ; le Rust copie le code, le notifie et attend la validation. */
export const loginStart = (lang: string) => invoke<LoginStart>("market_login_start", { lang });
/** Connexion en cours (retrouvée quand le panneau se rouvre), sinon null. */
export const loginState = () => invoke<LoginStart | null>("market_login_state");
export const loginCancel = () => invoke("market_login_cancel");
export const logout = () => invoke("market_logout");

// --- Publication (éditeur) ------------------------------------------------------------------

export interface Publication {
  item: string;
  version: string;
}

/** Élément du magasin d'une scène déjà publiée, sinon null. */
export const published = (scene: string) => invoke<Publication | null>("market_published", { scene });

/** Publie la scène enregistrée : nouvel élément, ou nouvelle version de son élément. */
export const publish = (scene: string, p: { summary: string; tags: string[]; changelog: string }) =>
  invoke<Publication>("market_publish", { scene, summary: p.summary, tags: p.tags, changelog: p.changelog });

export const storeStatus = () => get<{ publishing: { open: boolean; message: string } }>("/v1/store/status");

/** Étiquettes saisies librement → format du magasin (8 au plus, `a-z0-9-`, 24 caractères). */
export function cleanTags(raw: string): string[] {
  const out: string[] = [];
  for (const w of raw.split(/[,\s]+/)) {
    const tag = w
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9-]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 24);
    if (tag && !out.includes(tag)) out.push(tag);
  }
  return out.slice(0, 8);
}

// --- Mises à jour -------------------------------------------------------------------------

export interface Update {
  id: string;
  kind: "scene" | "extension";
  name: string;
  version: string;
  changelog: string;
  permissions: string[] | null;
  size: number;
}

/** Mises à jour connues (vérifiées par le Rust au démarrage puis toutes les 24 h). */
export const updates = reactive({ list: [] as Update[], checking: false, checkedAt: 0, error: null as string | null });

let followingUpdates = false;
export function watchUpdates() {
  if (followingUpdates) return;
  followingUpdates = true;
  const load = async () => {
    updates.list = await invoke<Update[]>("market_updates").catch(() => updates.list);
  };
  void load();
  void listen("market://updates", () => void load());
}

/** Vérifie tout de suite. */
export async function checkUpdates() {
  if (updates.checking) return;
  updates.checking = true;
  updates.error = null;
  try {
    updates.list = await invoke<Update[]>("market_check_updates");
    updates.checkedAt = Date.now();
  } catch (e) {
    updates.error = marketError(e);
  } finally {
    updates.checking = false;
  }
}

export const updateFor = (id: string) => updates.list.find((u) => u.id === id) ?? null;

/** La scène installée a été modifiée depuis (la mettre à jour écraserait ces changements). */
export const sceneModified = (item: string) => invoke<boolean>("market_scene_modified", { item });
