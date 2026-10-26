// Commandes perso d'un projet (« cargo xtask dev »…) : épinglées par projet dans le stockage
// de l'extension (partagé par la vue et le lanceur), et suggestions pour le champ de saisie
// (commandes connues du projet, alias cargo, sous-commandes d'un xtask en clap).

import type { Island } from "@island/sdk";
import { CARGO, composeCommand, inTerminal, packageManager, pathKey, runnables, type Project } from "./projects";

const KEY = "custom";

export type CustomMap = Record<string, string[]>;

export const loadCustom = async (island: Island): Promise<CustomMap> => (await island.storage.get<CustomMap>(KEY)) ?? {};

export const customOf = (map: CustomMap, p: Project) => map[pathKey(p.path)] ?? [];

/** Épingle (ou retire) une commande ; prévient le lanceur. */
export async function setPinned(island: Island, p: Project, command: string, pinned: boolean): Promise<string[]> {
  const map = await loadCustom(island);
  const k = pathKey(p.path);
  const list = (map[k] ?? []).filter((c) => c !== command);
  if (pinned) list.push(command);
  if (list.length) map[k] = list;
  else delete map[k];
  await island.storage.set(KEY, map);
  await island.events.emit("projects:custom");
  return list;
}

export const runCommand = (island: Island, p: Project, command: string) => inTerminal(island, p, command, `${p.name} · ${command}`);

// --- Suggestions ----------------------------------------------------------------------------

/** `BuildAll` → `build-all` (nom par défaut d'une sous-commande clap). */
const kebab = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

/** Sous-commandes d'un `enum` dérivant `Subcommand` (clap) dans ce source. */
export function clapSubcommands(src: string): string[] {
  const out: string[] = [];
  for (const m of src.matchAll(/#\[derive\([^)]*\bSubcommand\b[^)]*\)\][\s\S]*?enum\s+\w+\s*\{([\s\S]*?)\n\}/g)) {
    // Variantes : en début de ligne (après attributs et commentaires), `Nom`, `Nom(…)`, `Nom {`.
    for (const v of m[1].matchAll(/^\s*(?:#\[[^\]]*\]\s*)*([A-Z][A-Za-z0-9]*)\s*[({,]/gm)) {
      // `#[command(name = "…")]` posé sur la variante (dans les attributs captés devant elle).
      const name = v[0].match(/#\[command\([^)]*\bname\s*=\s*"([^"]+)"/)?.[1];
      out.push(name ?? kebab(v[1]));
    }
  }
  return [...new Set(out)];
}

async function cargoAliases(island: Island, p: Project): Promise<string[]> {
  let toml = "";
  for (const f of ["config.toml", "config"]) {
    toml = await island.files.readText(`${p.path}\\.cargo\\${f}`).catch(() => "");
    if (toml) break;
  }
  const table = toml.match(/^\[alias\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1] ?? "";
  const out: string[] = [];
  for (const a of table.matchAll(/^\s*([\w-]+)\s*=\s*(?:"([^"]*)"|\[([^\]]*)\])/gm)) {
    const alias = a[1];
    const value = a[2] ?? (a[3] ?? "").replace(/["',]/g, " ");
    out.push(`cargo ${alias}`);
    // Alias qui lance une crate du workspace (xtask…) : ses sous-commandes clap.
    const pkg = value.match(/(?:--package|-p)\s+([\w-]+)/)?.[1];
    const crate = pkg ? p.crates.find((c) => c.name === pkg) : undefined;
    if (!crate) continue;
    const main = await island.files.readText(`${crate.dir}\\src\\main.rs`).catch(() => "");
    for (const sub of clapSubcommands(main)) out.push(`cargo ${alias} ${sub}`);
  }
  return out;
}

/** Commandes proposées dans le champ : alias et sous-commandes d'abord, puis le reste. */
export async function suggestions(island: Island, p: Project): Promise<string[]> {
  const out: string[] = [];
  if (p.kinds.includes("rust")) {
    out.push(...(await cargoAliases(island, p)));
    out.push(...runnables(p).map((r) => r.command), ...CARGO.map((c) => `cargo ${c}`));
  }
  if (p.scripts.length) {
    const pm = await packageManager(island, p);
    out.push(...p.scripts.map((s) => `${pm} run ${s}`));
  }
  if (p.compose.length) out.push(composeCommand(p, "up -d"), composeCommand(p, "down"), composeCommand(p, "logs -f"));
  return [...new Set(out)];
}
