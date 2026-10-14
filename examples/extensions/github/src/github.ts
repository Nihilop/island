// Accès à l'API GitHub, partagé par la logique de fond (main.ts) et la vue (inbox.ts).
// Tout passe par `island.http` : Island n'autorise que api.github.com (manifeste).

import { HttpError, type Island } from "@island/sdk";

const API = "https://api.github.com";

export type Ci = "success" | "failure" | "pending" | null;

export interface Pr {
  id: number;
  repo: string;
  number: number;
  title: string;
  url: string;
  draft: boolean;
  updated: string;
  ci: Ci;
}

export interface Snapshot {
  reviews: Pr[];
  mine: Pr[];
  at: number;
}

export type Failure = "noToken" | "badToken" | { error: string };

interface SearchItem {
  id: number;
  number: number;
  title: string;
  html_url: string;
  draft?: boolean;
  updated_at: string;
  repository_url: string;
  pull_request?: { url: string };
}

async function headers(island: Island): Promise<Record<string, string> | null> {
  const token = await island.settings.get<string | null>("token");
  if (!token) return null;
  return { Authorization: `Bearer ${token}`, "X-GitHub-Api-Version": "2022-11-28" };
}

const repoOf = (url: string) => url.replace(`${API}/repos/`, "");

function toPr(i: SearchItem): Pr {
  return { id: i.id, repo: repoOf(i.repository_url), number: i.number, title: i.title, url: i.html_url, draft: !!i.draft, updated: i.updated_at, ci: null };
}

async function search(island: Island, h: Record<string, string>, q: string): Promise<Pr[]> {
  const r = await island.http.json<{ items: SearchItem[] }>(`${API}/search/issues?q=${encodeURIComponent(q)}&per_page=30&sort=updated`, { headers: h });
  return r.items.map(toPr);
}

/** État de la CI d'une PR : checks (Actions…) + statuts classiques du dernier commit. */
async function ci(island: Island, h: Record<string, string>, pr: Pr): Promise<Ci> {
  const p = await island.http.json<{ head: { sha: string } }>(`${API}/repos/${pr.repo}/pulls/${pr.number}`, { headers: h });
  const [checks, status] = await Promise.all([
    island.http.json<{ check_runs: { status: string; conclusion: string | null }[] }>(`${API}/repos/${pr.repo}/commits/${p.head.sha}/check-runs?per_page=50`, { headers: h }),
    island.http.json<{ state: string; total_count: number }>(`${API}/repos/${pr.repo}/commits/${p.head.sha}/status`, { headers: h }),
  ]);
  const runs = checks.check_runs;
  if (runs.some((r) => r.conclusion === "failure" || r.conclusion === "timed_out") || status.state === "failure" || status.state === "error") return "failure";
  if (runs.some((r) => r.status !== "completed") || (status.total_count > 0 && status.state === "pending")) return "pending";
  if (runs.length || status.total_count) return "success";
  return null;
}

/** Récupère l'état complet ; échec typé (pas de jeton, jeton refusé, réseau). */
export async function fetchSnapshot(island: Island): Promise<Snapshot | Failure> {
  const h = await headers(island);
  if (!h) return "noToken";
  try {
    const [reviews, mine] = await Promise.all([
      search(island, h, "is:open is:pr archived:false review-requested:@me"),
      search(island, h, "is:open is:pr archived:false author:@me"),
    ]);
    // CI des 10 PR les plus récentes (coût : 3 appels par PR).
    await Promise.all(mine.slice(0, 10).map(async (pr) => (pr.ci = await ci(island, h, pr).catch(() => null))));
    const snap: Snapshot = { reviews, mine, at: Date.now() };
    await island.storage.set("snapshot", snap);
    return snap;
  } catch (e) {
    if (e instanceof HttpError && (e.status === 401 || e.status === 403)) return "badToken";
    return { error: String((e as Error).message ?? e) };
  }
}

export interface Repo {
  name: string;
  description: string | null;
  url: string;
}

/** Dépôts accessibles (pour le launcher), mis en cache 1 h. */
export async function repos(island: Island): Promise<Repo[]> {
  const cached = await island.storage.get<{ at: number; list: Repo[] }>("repos");
  if (cached && Date.now() - cached.at < 3_600_000) return cached.list;
  const h = await headers(island);
  if (!h) return cached?.list ?? [];
  const list: Repo[] = [];
  for (let page = 1; page <= 3; page++) {
    const r = await island.http.json<{ full_name: string; description: string | null; html_url: string }[]>(
      `${API}/user/repos?per_page=100&sort=pushed&page=${page}`,
      { headers: h },
    );
    list.push(...r.map((x) => ({ name: x.full_name, description: x.description, url: x.html_url })));
    if (r.length < 100) break;
  }
  await island.storage.set("repos", { at: Date.now(), list });
  return list;
}

/** Âge relatif (« il y a 3 h »). */
export function age(island: Island, iso: string | number): string {
  const m = Math.floor((Date.now() - new Date(iso).getTime()) / 60_000);
  if (m < 1) return island.t("age.now");
  if (m < 60) return island.t("age.min", { n: m });
  const h = Math.floor(m / 60);
  return h < 24 ? island.t("age.hour", { n: h }) : island.t("age.day", { n: Math.floor(h / 24) });
}
