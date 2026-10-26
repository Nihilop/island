// Logique de l'extension Projets (iframe cachée) : le fournisseur `p:` du lanceur. Garde la
// liste des projets en mémoire (dernier état connu au démarrage, puis rescan), rafraîchie
// quand la vue change les dossiers et toutes les 5 minutes.
//
//   p:            tous les projets (Entrée = action principale)
//   p:jeu         projets et actions qui correspondent (« p:site dev » → script dev)

import { connect, type LauncherItem } from "@island/sdk";
import { CARGO, cargo, compose, fileName, launchUnreal, primary, runScript, runnables, scan, type Project } from "./projects";
import { customOf, loadCustom, runCommand, type CustomMap } from "./custom";

const island = await connect();

let projects: Project[] = (await island.storage.get<Project[]>("projects")) ?? [];
let scanning: Promise<void> | null = null;

function rescan() {
  scanning ??= scan(island)
    .then(async (list) => {
      projects = list;
      await island.storage.set("projects", list);
    })
    .catch((e) => void island.log("scan", String(e)))
    .finally(() => (scanning = null));
  return scanning;
}

void rescan();
setInterval(() => void rescan(), 5 * 60_000);
// La vue vient de rescanner (dossier ajouté, retiré…) : on reprend sa liste.
island.events.on("projects:changed", async () => {
  projects = (await island.storage.get<Project[]>("projects")) ?? projects;
});

// Commandes épinglées dans la vue (« cargo xtask dev »…).
let custom: CustomMap = await loadCustom(island);
island.events.on("projects:custom", async () => void (custom = await loadCustom(island)));

interface Action {
  item: LauncherItem;
  run: () => Promise<unknown>;
  /** Texte cherché. */
  text: string;
}

function actions(p: Project): Action[] {
  const sub = (s: string) => `${p.name} · ${s}`;
  const out: Action[] = [];
  const kinds = p.kinds.map((k) => island.t(`kind.${k}`)).join(", ");
  const main = p.uprojects.length ? `${island.t("unreal.launch")} — ` : `${kinds} — `;
  out.push({ item: { id: `open|${p.path}`, title: p.name, subtitle: `${main}${p.path}` }, run: () => primary(island, p), text: p.name });
  for (const u of p.uprojects)
    out.push({ item: { id: `launch|${u}`, title: sub(island.t("unreal.launchShort")), subtitle: island.t("unreal.launch") }, run: () => launchUnreal(island, p, u), text: `${p.name} lancer launch run unreal` });
  for (const u of p.uprojects) out.push({ item: { id: `file|${u}`, title: sub(fileName(u)), subtitle: island.t("open.uproject") }, run: () => island.files.open(u), text: `${p.name} ${fileName(u)} unreal` });
  for (const s of p.solutions) out.push({ item: { id: `file|${s}`, title: sub(fileName(s)), subtitle: island.t("open.sln") }, run: () => island.files.open(s), text: `${p.name} ${fileName(s)} sln visual` });
  for (const s of p.scripts) out.push({ item: { id: `script|${p.path}|${s}`, title: sub(s), subtitle: island.t("run.script") }, run: () => runScript(island, p, s), text: `${p.name} ${s}` });
  for (const c of customOf(custom, p))
    out.push({ item: { id: `custom|${p.path}|${c}`, title: sub(c), subtitle: island.t("custom.hint") }, run: () => runCommand(island, p, c), text: `${p.name} ${c}` });
  for (const r of runnables(p))
    out.push({ item: { id: `cargo-run|${p.path}|${r.command}`, title: sub(`run ${r.label}`), subtitle: r.command }, run: () => cargo(island, p, r.command, `run ${r.label}`), text: `${p.name} cargo run ${r.label}` });
  if (p.kinds.includes("rust"))
    for (const c of CARGO) out.push({ item: { id: `cargo|${p.path}|${c}`, title: sub(`cargo ${c}`), subtitle: island.t("run.cargo") }, run: () => cargo(island, p, `cargo ${c}`, c), text: `${p.name} cargo ${c}` });
  if (p.compose.length) {
    out.push({ item: { id: `up|${p.path}`, title: sub("compose up"), subtitle: island.t("compose.upHint") }, run: () => compose(island, p, "up -d"), text: `${p.name} docker compose up` });
    out.push({ item: { id: `down|${p.path}`, title: sub("compose down"), subtitle: island.t("compose.downHint") }, run: () => compose(island, p, "down"), text: `${p.name} docker compose down` });
  }
  out.push({ item: { id: `view|${p.path}`, title: sub(island.t("show")), subtitle: island.t("showHint") }, run: () => show(p), text: `${p.name} ${island.t("show")}` });
  return out;
}

async function show(p: Project) {
  await island.storage.set("selected", p.path);
  await island.events.emit("projects:select", p.path);
  await island.tools.open("projects");
}

let index = new Map<string, Action>();

island.launcher.provide("projects", async (q) => {
  if (!projects.length && scanning) await scanning;
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  const all = projects.flatMap(actions);
  index = new Map(all.map((a) => [a.item.id, a]));
  // Sans recherche : un résultat par projet ; sinon tout ce qui contient chaque mot.
  const hits = words.length ? all.filter((a) => words.every((w) => a.text.toLowerCase().includes(w))) : all.filter((a) => a.item.id.startsWith("open|"));
  if (!projects.length) return [{ id: "add", title: island.t("launcher.empty"), subtitle: island.t("launcher.emptyHint") }];
  return hits.slice(0, 40).map((a) => a.item);
});

island.launcher.onRun("projects", (id) => {
  if (id === "add") return void island.tools.open("projects");
  const a = index.get(id);
  a?.run().catch((e) => island.log("run", String(e)));
});
