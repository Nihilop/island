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

export const catalog = (p: { q?: string; tag?: string; sort?: "recent" | "popular"; page?: number }) =>
  get<Page>("/v1/items", { kind: "scene", q: p.q, tag: p.tag, sort: p.sort, page: p.page ? String(p.page) : undefined });

export const item = (id: string) => get<Detail>(`/v1/items/${encodeURIComponent(id)}`);

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
