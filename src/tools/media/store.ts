// Lecture en cours, côté front. Rust émet `np://update` (poll 400 ms) ; ici on ajoute :
//   - mise à jour OPTIMISTE au clic, puis resynchronisation 1,5 s après chaque commande ;
//   - garde de seek 1,2 s (on ignore les anciennes positions juste après un seek) ;
//   - maintien de pochette 1,5 s (une nouvelle piste arrive souvent sans pochette) ; une
//     pochette absente ne « compte » qu'après 800 ms ; l'image n'est échangée qu'une fois
//     décodée (le fondu démarre avec une image prête).

import { reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setAccentFrom } from "../../design/accent";

interface Raw {
  active: boolean;
  title: string;
  artist: string;
  art: string | null;
  playing: boolean;
  position: number;
  duration: number;
  canSeek: boolean;
  appId: string;
}

export const np = reactive({
  active: false,
  title: "",
  artist: "",
  playing: false,
  duration: 0,
  canSeek: false,
  appId: "",
  /** Pochette affichée (décodée), null = icône note. */
  art: null as string | null,
});

/** Position : base + instant de référence ; `position()` extrapole en lecture. */
let posBase = 0;
let posAt = performance.now();
export function position(): number {
  const p = np.playing ? posBase + (performance.now() - posAt) / 1000 : posBase;
  return np.duration > 0 ? Math.min(Math.max(0, p), np.duration) : Math.max(0, p);
}
function setPosition(p: number) {
  posBase = p;
  posAt = performance.now();
}

/** Incrémenté à chaque changement de piste (pour les transitions de titre). */
export const trackKey = ref(0);

const SEEK_GUARD = 1200;
const RESYNC = 1500;
const ART_HOLD = 1500;
const ART_MISSING = 800;

let seekGuardUntil = 0;
let resyncTimer: number | undefined;
let artTimer: number | undefined;
let lastTrack = "";
let trackChangedAt = 0;

function swapArt(url: string | null) {
  window.clearTimeout(artTimer);
  if (!url) {
    // Absente : on garde l'ancienne un moment (elle arrive souvent après le titre).
    const wait = Math.max(ART_MISSING, ART_HOLD - (performance.now() - trackChangedAt));
    artTimer = window.setTimeout(() => {
      np.art = null;
    }, wait);
    return;
  }
  if (url === np.art) return;
  const img = new Image();
  img.src = url;
  img
    .decode()
    .then(() => {
      np.art = url;
      setAccentFrom(url);
    })
    .catch(() => {});
}

function apply(r: Raw) {
  const track = `${r.appId}\u0001${r.title}\u0001${r.artist}`;
  if (track !== lastTrack) {
    lastTrack = track;
    trackChangedAt = performance.now();
    trackKey.value++;
  }
  np.active = r.active;
  np.title = r.title;
  np.artist = r.artist;
  np.playing = r.playing;
  np.duration = r.duration;
  np.canSeek = r.canSeek;
  np.appId = r.appId;
  if (performance.now() >= seekGuardUntil) setPosition(r.position);
  swapArt(r.active ? r.art : null);
}

function resyncSoon() {
  window.clearTimeout(resyncTimer);
  resyncTimer = window.setTimeout(async () => {
    const r = await invoke<Raw | null>("np_state").catch(() => null);
    if (r) apply(r);
  }, RESYNC);
}

export function togglePlay() {
  setPosition(position());
  np.playing = !np.playing;
  invoke("np_toggle").catch(() => {});
  resyncSoon();
}

export function next() {
  invoke("np_next").catch(() => {});
  resyncSoon();
}

export function prev() {
  invoke("np_prev").catch(() => {});
  resyncSoon();
}

export function seek(seconds: number) {
  setPosition(seconds);
  seekGuardUntil = performance.now() + SEEK_GUARD;
  invoke("np_seek", { seconds }).catch(() => {});
  resyncSoon();
}

export const openSource = () => invoke("np_open_source").catch(() => {});

let started = false;
export async function initNowPlaying() {
  if (started) return;
  started = true;
  listen<Raw>("np://update", (e) => apply(e.payload));
  const r = await invoke<Raw | null>("np_state").catch(() => null);
  if (r) apply(r);
}

/** Nom lisible de l'app source (AUMID → « Spotify », « Chrome »…). */
export function appLabel(appId: string): string {
  const head = appId.split("!")[0];
  const base = head.split(/[\\/]/).pop() ?? head;
  const stem = base.replace(/\.exe$/i, "").split(/[._]/).filter(Boolean).pop() ?? base;
  return stem.charAt(0).toUpperCase() + stem.slice(1);
}

export function fmtTime(s: number): string {
  const t = Math.max(0, Math.floor(s));
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const sec = String(t % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
