// Logique de fond : rafraîchit l'état GitHub toutes les N minutes, met à jour l'activité
// en direct du notch, notifie les nouveautés, répond au launcher (« gh … ») et à la
// commande « Rafraîchir ».

import { connect } from "@island/sdk";
import { fetchSnapshot, repos, type Snapshot } from "./github";

const island = await connect();
let timer: ReturnType<typeof setTimeout> | undefined;
let previous: Snapshot | null = await island.storage.get<Snapshot>("snapshot");

async function refresh() {
  clearTimeout(timer);
  const res = await fetchSnapshot(island);
  if (typeof res === "object" && "reviews" in res) {
    await notifyChanges(previous, res);
    previous = res;
    await updateLive(res);
  } else {
    await island.live.set("status", null);
    if (res !== "noToken") await island.log("rafraîchissement impossible :", res);
  }
  const minutes = Number(await island.settings.get<string>("interval")) || 5;
  timer = setTimeout(refresh, minutes * 60_000);
}

/** Notch fermé : revues en attente (orange), CI échouée (rouge) ou en cours (bleu). */
async function updateLive(s: Snapshot) {
  const failed = s.mine.filter((p) => p.ci === "failure").length;
  const running = s.mine.filter((p) => p.ci === "pending").length;
  if (failed) return island.live.set("status", { icon: "icons/ci.svg", text: String(failed), color: "red", priority: "high" });
  if (s.reviews.length) return island.live.set("status", { icon: "icons/review.svg", text: String(s.reviews.length), color: "orange" });
  if (running) return island.live.set("status", { icon: "icons/ci.svg", text: String(running), color: "blue" });
  return island.live.set("status", null);
}

/** Notifications : nouvelle revue demandée, CI passée en échec. */
async function notifyChanges(before: Snapshot | null, now: Snapshot) {
  if (!before || !(await island.settings.get<boolean>("notify"))) return;
  const known = new Set(before.reviews.map((p) => p.id));
  for (const pr of now.reviews.filter((p) => !known.has(p.id))) {
    await island.notify({ title: island.t("notify.review", { repo: pr.repo }), body: pr.title });
  }
  const wasFailing = new Set(before.mine.filter((p) => p.ci === "failure").map((p) => p.id));
  for (const pr of now.mine.filter((p) => p.ci === "failure" && !wasFailing.has(p.id))) {
    await island.notify({ title: island.t("notify.ciFailed", { repo: pr.repo }), body: pr.title });
  }
}

// Launcher : « gh island » → dépôts dont le nom contient « island ».
island.launcher.provide("repos", async (q) => {
  const list = await repos(island).catch(() => []);
  const low = q.toLowerCase();
  return list
    .filter((r) => !low || r.name.toLowerCase().includes(low))
    .slice(0, 20)
    .map((r) => ({ id: r.url, title: r.name, subtitle: r.description ?? undefined, icon: "icons/github.svg" }));
});
island.launcher.onRun("repos", (url) => void island.openUrl(url));

island.commands.on("refresh", () => void refresh());
island.settings.onChange((key) => {
  if (key === "token") {
    previous = null;
    void island.storage.set("repos", null);
  }
  void refresh();
});

await refresh();
