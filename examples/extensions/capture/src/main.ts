// Logique de fond de Capture : raccourcis globaux (zone → capture d'écran ; enregistrer /
// arrêter), activité en direct « ● 0:42 » pendant un enregistrement (clic = ouvre l'outil
// et son bouton Arrêter), notification à la fin.

import { connect } from "@island/sdk";
import { clock, fileName, shoot, startRecording, stopRecording, type Mode } from "./core";

const island = await connect();

let ticker: ReturnType<typeof setInterval> | undefined;

function showRec(startedAt: number) {
  clearInterval(ticker);
  const tick = () => void island.live.set("rec", { icon: "icons/capture.svg", text: `● ${clock(Date.now() - startedAt)}`, color: "red", priority: "high" });
  tick();
  ticker = setInterval(tick, 1000);
}

function hideRec() {
  clearInterval(ticker);
  ticker = undefined;
  void island.live.set("rec", null);
}

island.capture.onRecording((s) => {
  if (s.recording && s.startedAt) showRec(s.startedAt);
  else {
    hideRec();
    if (s.path) void island.notify({ title: island.t("notify.video"), body: fileName(s.path) });
    else if (s.error) void island.notify({ title: island.t("notify.error"), body: island.t(`error.${s.error}`) });
  }
});

// Enregistrement déjà en cours (rechargement de l'extension) : on reprend l'affichage.
const state = await island.capture.state().catch(() => null);
if (state?.recording && state.startedAt) showRec(state.startedAt);

// Toutes les actions passent par ici (raccourcis ET outil) : la vue de l'outil est détruite
// dès que l'île se ferme — c'est-à-dire dès que la sélection s'affiche —, elle ne peut donc
// pas attendre la sélection elle-même. Cette page de fond, elle, vit tant que l'extension tourne.
type Action = { kind: "shot" | "video" | "stop" | "toggle"; mode?: Mode; audio?: { systemAudio: boolean; mic: boolean } };

let busy = false;
async function run(a: Action) {
  if (busy) return;
  busy = true;
  try {
    if (a.kind === "shot") await shoot(island, a.mode ?? "region");
    else if (a.kind === "video") await startRecording(island, a.mode ?? "region", a.audio);
    else if (a.kind === "stop") await stopRecording(island);
    else {
      const s = await island.capture.state();
      if (s.recording) await stopRecording(island);
      else await startRecording(island, "region");
    }
  } catch (e) {
    const msg = String(e instanceof Error ? e.message : e);
    void island.log("capture", msg);
    void island.notify({ title: island.t("notify.error"), body: island.t(`error.${msg}`) === `error.${msg}` ? msg : island.t(`error.${msg}`) });
  } finally {
    busy = false;
  }
}

island.events.on<Action>("action", (a) => void run(a));
island.commands.on("shot", () => void run({ kind: "shot", mode: "region" }));
island.commands.on("record", () => void run({ kind: "toggle" }));
