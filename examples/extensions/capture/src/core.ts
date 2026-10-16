// Actions de l'extension Capture, partagées par la logique (raccourcis) et la vue de
// l'outil. Island affiche la sélection, capture et encode (permission `capture`) ;
// l'extension choisit les options et tient l'historique des dernières captures.

import type { CaptureTarget, Island, RecordOptions } from "@island/sdk";

export type Mode = "region" | "window" | "screen";

export interface Recent {
  path: string;
  kind: "image" | "video";
  at: number;
  /** Dimensions (image) ou durée (vidéo, ms). */
  width?: number;
  height?: number;
  durationMs?: number;
}

const RECENT_KEY = "recent";
const RECENT_MAX = 8;

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Options d'enregistrement : réglages de l'extension, son remplacé par le dernier choix
 *  rapide de l'outil (`audio`) s'il existe. */
export async function recordOptions(island: Island, audio?: { systemAudio: boolean; mic: boolean }): Promise<RecordOptions> {
  const quick = audio ?? (await island.storage.get<{ systemAudio: boolean; mic: boolean }>("audio")) ?? undefined;
  const [systemAudio, mic, quality, fps, codec] = await Promise.all([
    island.settings.get<boolean>("systemAudio"),
    island.settings.get<boolean>("mic"),
    island.settings.get<string>("quality"),
    island.settings.get<string>("fps"),
    island.settings.get<string>("codec"),
  ]);
  return {
    systemAudio: quick ? quick.systemAudio : systemAudio !== false,
    mic: quick ? quick.mic : !!mic,
    quality: (quality as RecordOptions["quality"]) ?? "medium",
    fps: fps === "60" ? 60 : 30,
    codec: codec === "hevc" ? "hevc" : "h264",
  };
}

export async function recent(island: Island): Promise<Recent[]> {
  return (await island.storage.get<Recent[]>(RECENT_KEY)) ?? [];
}

async function remember(island: Island, item: Recent) {
  const list = [item, ...(await recent(island)).filter((r) => r.path !== item.path)].slice(0, RECENT_MAX);
  await island.storage.set(RECENT_KEY, list);
  await island.events.emit("recent", list);
}

/** Capture d'écran : sélection par Island (zone, fenêtre, écran), puis PNG + presse-papiers. */
export async function shoot(island: Island, mode: Mode, target?: CaptureTarget): Promise<Recent | null> {
  const t = target ?? (await island.capture.select(mode));
  if (!t) return null;
  const clipboard = (await island.settings.get<boolean>("clipboard")) !== false;
  const r = await island.capture.screenshot(t, { clipboard, save: true });
  if (!r.path) return null;
  const item: Recent = { path: r.path, kind: "image", at: Date.now(), width: r.width, height: r.height };
  await remember(island, item);
  await island.notify({
    title: island.t("notify.shot"),
    body: clipboard ? island.t("notify.shotClip", { name: fileName(r.path) }) : fileName(r.path),
  });
  return item;
}

/** Démarre un enregistrement (sélection par Island), ou rien si annulé. */
export async function startRecording(island: Island, mode: Mode, audio?: { systemAudio: boolean; mic: boolean }): Promise<boolean> {
  const t = await island.capture.select(mode);
  if (!t) return false;
  await island.capture.startRecording(t, await recordOptions(island, audio));
  return true;
}

export async function stopRecording(island: Island): Promise<Recent> {
  const r = await island.capture.stopRecording();
  const item: Recent = { path: r.path, kind: "video", at: Date.now(), durationMs: r.durationMs };
  await remember(island, item);
  return item;
}

/** « 1:05 » / « 1:02:05 ». */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}
