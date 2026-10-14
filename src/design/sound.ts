// Sons d'alarme synthétisés en WebAudio (aucun fichier audio). Chaque son est une phrase
// répétée jusqu'à la durée choisie ; enveloppe : attaque 8 ms puis déclin exponentiel.

export type SoundId = "gong" | "bell" | "digital" | "radar" | "marimba" | "soft";
export const SOUNDS: SoundId[] = ["gong", "bell", "digital", "radar", "marimba", "soft"];

/** Durée de lecture : 0 = une fois, -1 = jusqu'à l'arrêt (plafonné à 120 s), sinon secondes. */
export type SoundDuration = 0 | 5 | 15 | 30 | -1;

type Note = { f: number; at: number; decay: number; type?: OscillatorType; gain?: number };

/** Phrase de chaque son (notes) et sa longueur avant répétition (s). */
const PHRASES: Record<SoundId, { notes: Note[]; length: number }> = {
  gong: {
    length: 1.8,
    notes: [
      { f: 880, at: 0, decay: 1.4 },
      { f: 1318.5, at: 0.16, decay: 1.4 },
    ],
  },
  bell: {
    length: 1.6,
    notes: [1, 2.76, 5.4, 8.93, 13.34].map((ratio, i) => ({
      f: 660 * ratio,
      at: 0,
      decay: 2.4 / (1 + i * 0.6),
      gain: 1 / (1 + i * 1.5),
    })),
  },
  digital: {
    length: 1,
    notes: [0, 0.12, 0.24, 0.36].map((at) => ({ f: 2000, at, decay: 0.07, type: "square" as const, gain: 0.35 })),
  },
  radar: {
    length: 1.1,
    notes: [0, 1, 2, 3].map((i) => ({ f: 880 * 1.06 ** i, at: i * 0.14, decay: 0.12, type: "triangle" as const })),
  },
  marimba: {
    length: 1.5,
    notes: [523.25, 659.25, 783.99, 1046.5].flatMap((f, i) => [
      { f, at: i * 0.12, decay: 0.5 },
      { f: f * 4, at: i * 0.12, decay: 0.12, gain: 0.25 },
    ]),
  },
  soft: {
    length: 2.8,
    notes: [
      { f: 523.25, at: 0, decay: 1.6, gain: 0.7 },
      { f: 659.25, at: 0.5, decay: 1.8, gain: 0.7 },
    ],
  },
};

const MAX_LOOP = 120;

let ctx: AudioContext | null = null;
let current: { master: GainNode; timer?: number } | null = null;

function playPhrase(ac: AudioContext, out: AudioNode, id: SoundId, t0: number) {
  for (const n of PHRASES[id].notes) {
    const osc = ac.createOscillator();
    const g = ac.createGain();
    osc.type = n.type ?? "sine";
    osc.frequency.value = n.f;
    const start = t0 + n.at;
    const peak = n.gain ?? 1;
    g.gain.setValueAtTime(0, start);
    g.gain.linearRampToValueAtTime(peak, start + 0.008);
    g.gain.exponentialRampToValueAtTime(0.0001, start + 0.008 + n.decay);
    osc.connect(g).connect(out);
    osc.start(start);
    osc.stop(start + 0.01 + n.decay);
  }
}

/** Joue `id` au volume 0..1 pendant `duration`. Un nouveau son arrête l'ancien. */
export function playSound(id: SoundId, volume: number, duration: SoundDuration = 0) {
  stopSound();
  ctx ??= new AudioContext();
  const ac = ctx;
  void ac.resume();
  const master = ac.createGain();
  master.gain.value = volume * 0.5;
  master.connect(ac.destination);
  current = { master };

  const { length } = PHRASES[id];
  const total = duration === -1 ? MAX_LOOP : duration;
  let next = ac.currentTime;
  const end = next + Math.max(total, length);
  const tick = () => {
    if (current?.master !== master) return;
    // Programme les phrases qui démarrent dans la seconde à venir.
    while (next < ac.currentTime + 1 && (next < end || total === 0)) {
      playPhrase(ac, master, id, next);
      next += length;
      if (total === 0) return;
    }
    if (next < end) current.timer = window.setTimeout(tick, 500);
  };
  tick();
}

/** Arrête le son en cours avec un fondu doux (~50 ms). */
export function stopSound() {
  if (!current || !ctx) return;
  window.clearTimeout(current.timer);
  const { master } = current;
  master.gain.setTargetAtTime(0, ctx.currentTime, 0.05);
  window.setTimeout(() => master.disconnect(), 300);
  current = null;
}
