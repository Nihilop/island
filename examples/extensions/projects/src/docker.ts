// Conteneurs Docker d'un projet compose, par l'API du moteur (permission `docker`) : ceux
// dont l'étiquette `working_dir` est le dossier du projet, ou dont le nom de projet compose
// correspond.

import type { Island } from "@island/sdk";
import { pathKey, type Project } from "./projects";

export interface Container {
  id: string;
  name: string;
  service: string;
  image: string;
  state: "running" | "exited" | "paused" | "restarting" | "created" | "dead" | string;
  status: string;
}

interface Raw {
  Id: string;
  Names: string[];
  Image: string;
  State: string;
  Status: string;
  Labels: Record<string, string> | null;
}

export type DockerResult = { ok: true; list: Container[] } | { ok: false; error: "unavailable" | string };

export async function containers(island: Island, p: Project): Promise<DockerResult> {
  try {
    const all = await island.docker.json<Raw[]>("/containers/json?all=1");
    const dir = pathKey(p.path);
    const list = all
      .filter((c) => {
        const l = c.Labels ?? {};
        const wd = l["com.docker.compose.project.working_dir"];
        return (wd && pathKey(wd) === dir) || (!!p.composeName && l["com.docker.compose.project"] === p.composeName);
      })
      .map((c) => ({
        id: c.Id,
        name: (c.Names[0] ?? c.Id.slice(0, 12)).replace(/^\//, ""),
        service: c.Labels?.["com.docker.compose.service"] ?? "",
        image: c.Image,
        state: c.State,
        status: c.Status,
      }))
      .sort((a, b) => (a.service || a.name).localeCompare(b.service || b.name));
    return { ok: true, list };
  } catch (e) {
    const msg = String((e as Error)?.message ?? e);
    return { ok: false, error: msg.includes("dockerUnavailable") ? "unavailable" : msg };
  }
}

export async function act(island: Island, id: string, action: "start" | "stop" | "restart") {
  const r = await island.docker.request({ method: "POST", path: `/containers/${encodeURIComponent(id)}/${action}` });
  // 304 : déjà dans l'état voulu.
  if (r.status >= 400) throw new Error(r.body || `HTTP ${r.status}`);
}
