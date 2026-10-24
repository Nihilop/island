// Projets : détection dans les dossiers donnés par l'utilisateur, et leurs actions. Partagé
// par la logique (lanceur `p:`) et la vue.
//
// Un dossier ajouté est lui-même un projet s'il en porte les marques (package.json,
// .uproject, .sln, fichier compose) ; sinon ses sous-dossiers qui en portent sont des projets
// (ajouter « F:\Web_Projects » liste tout ce qu'il contient).

import type { FileEntry, Island, RunSpec } from "@island/sdk";
import { launchCommand } from "./unreal";

export type Kind = "unreal" | "node" | "compose" | "sln";
export type PackageManager = "npm" | "pnpm" | "yarn" | "bun";

export interface Project {
  /** Chemin du dossier (sert d'identifiant). */
  path: string;
  name: string;
  /** Dossier ajouté par l'utilisateur dont il vient. */
  root: string;
  kinds: Kind[];
  uprojects: string[];
  solutions: string[];
  /** Node : scripts du package.json, dans l'ordre. */
  scripts: string[];
  pm: PackageManager | null;
  /** Docker Compose : fichiers, et nom de projet compose (étiquettes des conteneurs). */
  compose: string[];
  composeName: string | null;
}

/** Dossiers où l'on ne descend jamais (lourds, générés). */
export const SKIP = [
  "node_modules",
  ".git",
  ".svn",
  ".vs",
  ".idea",
  ".vscode",
  "dist",
  "build",
  "target",
  "vendor",
  "intermediate",
  "binaries",
  "saved",
  "deriveddatacache",
  "content",
  ".next",
  ".nuxt",
  ".output",
  ".turbo",
  ".cache",
];

const COMPOSE = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];

const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const parent = (p: string) => p.replace(/[\\/][^\\/]+$/, "");
const low = (s: string) => s.toLowerCase();
/** Clé de comparaison d'un chemin (casse, séparateurs, barre finale). */
export const pathKey = (p: string) => low(p).replace(/\//g, "\\").replace(/\\+$/, "");

/** Nom de projet compose par défaut : celui du dossier, normalisé comme le fait Docker. */
const composeDefault = (dir: string) => low(base(dir)).replace(/[^a-z0-9_-]/g, "");

function marks(files: FileEntry[]) {
  const names = files.filter((f) => !f.dir).map((f) => ({ f, n: low(f.name) }));
  return {
    uprojects: names.filter((x) => x.n.endsWith(".uproject")).map((x) => x.f.path),
    solutions: names.filter((x) => x.n.endsWith(".sln")).map((x) => x.f.path),
    pkg: names.find((x) => x.n === "package.json")?.f.path ?? null,
    compose: names.filter((x) => COMPOSE.includes(x.n)).map((x) => x.f.path),
    locks: new Set(names.map((x) => x.n)),
  };
}

const isProject = (m: ReturnType<typeof marks>) => !!(m.uprojects.length || m.solutions.length || m.pkg || m.compose.length);

async function detail(island: Island, dir: string, root: string, files: FileEntry[]): Promise<Project> {
  const m = marks(files);
  const p: Project = { path: dir, name: base(dir), root, kinds: [], uprojects: m.uprojects, solutions: m.solutions, scripts: [], pm: null, compose: m.compose, composeName: null };
  if (m.uprojects.length) {
    p.kinds.push("unreal");
    p.name = base(m.uprojects[0]).replace(/\.uproject$/i, "");
  } else if (m.solutions.length) p.kinds.push("sln");
  if (m.pkg) {
    p.kinds.push("node");
    try {
      const pkg = JSON.parse(await island.files.readText(m.pkg));
      if (pkg.scripts && typeof pkg.scripts === "object") p.scripts = Object.keys(pkg.scripts);
      if (!m.uprojects.length && typeof pkg.name === "string" && pkg.name) p.name = pkg.name;
      const declared = typeof pkg.packageManager === "string" ? pkg.packageManager.split("@")[0] : "";
      p.pm = (["npm", "pnpm", "yarn", "bun"].includes(declared) ? declared : null) as PackageManager | null;
    } catch {
      // package.json illisible : le projet reste listé, sans scripts.
    }
    p.pm ??= m.locks.has("pnpm-lock.yaml")
      ? "pnpm"
      : m.locks.has("yarn.lock")
        ? "yarn"
        : m.locks.has("bun.lockb") || m.locks.has("bun.lock")
          ? "bun"
          : m.locks.has("package-lock.json")
            ? "npm"
            : null;
  }
  if (m.compose.length) {
    p.kinds.push("compose");
    p.composeName = composeDefault(dir);
    try {
      const top = (await island.files.readText(m.compose[0])).match(/^name:\s*["']?([\w.-]+)/m);
      if (top) p.composeName = low(top[1]);
    } catch {
      // Fichier illisible : nom par défaut.
    }
  }
  return p;
}

/** Projets de tous les dossiers donnés, triés par nom. */
export async function scan(island: Island): Promise<Project[]> {
  const roots = await island.files.folders();
  const found: Project[] = [];
  await Promise.all(
    roots.map(async (root) => {
      const entries = await island.files.list(root, { depth: 2, skip: SKIP }).catch(() => [] as FileEntry[]);
      // Fichiers par dossier parent : la racine, puis chaque sous-dossier direct.
      const byDir = new Map<string, FileEntry[]>();
      for (const e of entries) {
        const dir = pathKey(parent(e.path));
        if (!byDir.has(dir)) byDir.set(dir, []);
        byDir.get(dir)!.push(e);
      }
      const own = byDir.get(pathKey(root)) ?? [];
      if (isProject(marks(own))) return void found.push(await detail(island, root, root, own));
      const subdirs = own.filter((e) => e.dir && !SKIP.includes(low(e.name)) && !e.name.startsWith("."));
      for (const d of subdirs) {
        const files = byDir.get(pathKey(d.path)) ?? [];
        if (isProject(marks(files))) found.push(await detail(island, d.path, root, files));
      }
    }),
  );
  return found.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
}

// --- Actions --------------------------------------------------------------------------------

export async function packageManager(island: Island, p: Project): Promise<PackageManager> {
  const choice = await island.settings.get<string>("packageManager").catch(() => "auto");
  if (choice && choice !== "auto") return choice as PackageManager;
  return p.pm ?? "npm";
}

type Shell = NonNullable<RunSpec["shell"]>;
/** Commande dans un onglet de Windows Terminal, avec le shell choisi dans les réglages. */
const inTerminal = async (island: Island, p: Project, command: string, title: string) =>
  island.terminal.run({ cwd: p.path, command, title, shell: ((await island.settings.get<string>("terminal").catch(() => "")) || "default") as Shell });

export async function runScript(island: Island, p: Project, script: string) {
  const pm = await packageManager(island, p);
  await inTerminal(island, p, `${pm} run ${script}`, `${p.name} · ${script}`);
}

export async function openEditor(island: Island, p: Project) {
  const editor = ((await island.settings.get<string>("editor").catch(() => "")) || "code").trim();
  await island.terminal.launch({ cwd: p.path, command: `${editor} .` });
}

export const openTerminal = (island: Island, p: Project) => inTerminal(island, p, "", p.name);

export const compose = (island: Island, p: Project, args: string) => inTerminal(island, p, `docker compose ${args}`, `${p.name} · compose ${args.split(" ")[0]}`);

/** Unreal : compile la cible éditeur puis ouvre l'éditeur (« Exécuter sans débogage »). */
export async function launchUnreal(island: Island, p: Project, uproject: string) {
  const { command, title } = await launchCommand(island, p, uproject);
  await inTerminal(island, p, command, title);
}

/** Action principale (Entrée dans le lanceur) : Unreal → compiler et lancer, sinon l'éditeur. */
export async function primary(island: Island, p: Project) {
  if (p.uprojects[0]) return launchUnreal(island, p, p.uprojects[0]);
  if (p.solutions[0] && !p.kinds.includes("node")) return island.files.open(p.solutions[0]);
  return openEditor(island, p);
}

export const fileName = base;
