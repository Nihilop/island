// Marchés : liste de suivi persistée (bourse : symboles Yahoo ; crypto : ids CoinGecko,
// devise EUR/USD) et dernières cotations lues via Rust (cache 30 s côté hôte). Un seul
// appel en vol par onglet (une demande faite pendant l'appel est rejouée à la fin) ; les
// cotations restent affichées pendant un rafraîchissement.

import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { persisted } from "../../notch/persist";

export type MarketTab = "stocks" | "crypto";

export interface Quote {
  id: string;
  name: string;
  symbol: string;
  price: number;
  change: number;
  currency: string;
  points: number[];
  image: string | null;
  exchange: string | null;
}

export interface SearchHit {
  id: string;
  name: string;
  symbol: string;
  detail: string;
  image: string | null;
}

export const { state: watchlist, ready } = persisted("markets", {
  tab: "stocks" as MarketTab,
  stocks: ["^FCHI", "CW8.PA", "AAPL"],
  crypto: ["bitcoin", "ethereum"],
  currency: "eur" as "eur" | "usd",
});

const blank = () => ({ quotes: [] as Quote[], loaded: false, loading: false, error: null as string | null });
export const markets = reactive({ stocks: blank(), crypto: blank() });
const pending = { stocks: false, crypto: false };

/** Rafraîchit l'onglet `tab` ; pendant un appel en cours, la demande est rejouée à la fin
 *  (liste ou devise changée entre-temps). */
export async function refresh(tab: MarketTab = watchlist.tab) {
  const m = markets[tab];
  if (m.loading) {
    pending[tab] = true;
    return;
  }
  m.loading = true;
  m.error = null;
  try {
    m.quotes =
      tab === "stocks"
        ? await invoke<Quote[]>("markets_stocks", { symbols: [...watchlist.stocks] })
        : await invoke<Quote[]>("markets_crypto", { ids: [...watchlist.crypto], currency: watchlist.currency });
    m.loaded = true;
  } catch (e) {
    m.error = typeof e === "string" ? e : "network";
  } finally {
    m.loading = false;
  }
  if (pending[tab]) {
    pending[tab] = false;
    await refresh(tab);
  }
}

/** Cotations dans l'ordre de la liste de suivi. */
export function ordered(tab: MarketTab): Quote[] {
  const by = new Map(markets[tab].quotes.map((q) => [q.id, q]));
  return watchlist[tab].map((id) => by.get(id)).filter((q): q is Quote => !!q);
}

export function search(tab: MarketTab, query: string): Promise<SearchHit[]> {
  return invoke<SearchHit[]>(tab === "stocks" ? "markets_stock_search" : "markets_crypto_search", { query });
}

export async function add(tab: MarketTab, id: string) {
  if (!watchlist[tab].includes(id)) watchlist[tab].push(id);
  // Un appel déjà en vol ne connaît pas la nouvelle valeur : on attend la fin (rejouée).
  while (markets[tab].loading) await new Promise((r) => setTimeout(r, 100));
  if (!markets[tab].quotes.some((q) => q.id === id)) await refresh(tab);
}

export function remove(tab: MarketTab, id: string) {
  watchlist[tab] = watchlist[tab].filter((x) => x !== id);
  markets[tab].quotes = markets[tab].quotes.filter((q) => q.id !== id);
}
