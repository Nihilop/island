<script setup lang="ts">
// L'ÎLE = des slots agnostiques. Elle ne connaît AUCUNE extension.
// États : idle (registre d'indicateurs) · launcher (extensions + natifs) ·
// view (monte une surface d'extension) · + un sous-slot "goutte" dans une view.
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { idleState, idleActions, idleTap } from "../composables/idle";
import { launcherEntries, runProviders, hasProviders } from "../composables/launcher";
import {
  setHitRegion, activeView, activeViewSize, activeViewPersistent, activeViewSafeArea, closeView, dropContent, closeDrop,
  modalSpec, floatWindows, selecting, regionOutline,
} from "../composables/overlay";
import { stack as notifStack, unread, unreadCount, lastPosted, setDnd as setNotifDnd, pauseStack, resumeStack, markRead, clearUnread, type Notif } from "../composables/notifications";

type Format = "idle" | "launcher" | "view" | "notifcenter";
type Phase = "stable" | "exit" | "morph" | "enter";
interface Cell { id: string; label: string; icon: string; kind: string; toggle?: boolean; onActivate?: () => void }

const ICONS: Record<string, string> = {
  settings: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M19.14 12.94c.04-.31.06-.63.06-.94s-.02-.63-.06-.94l2.03-1.58a.48.48 0 0 0 .12-.61l-1.92-3.32a.49.49 0 0 0-.59-.22l-2.39.96a7 7 0 0 0-1.62-.94l-.36-2.54A.49.49 0 0 0 13.5 2h-3a.49.49 0 0 0-.48.42l-.36 2.54c-.59.24-1.13.56-1.62.94l-2.39-.96a.49.49 0 0 0-.59.22L2.74 8.48a.48.48 0 0 0 .12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58a.48.48 0 0 0-.12.61l1.92 3.32c.13.22.39.31.59.22l2.39-.96c.49.38 1.03.7 1.62.94l.36 2.54c.05.24.25.42.48.42h3c.23 0 .43-.18.48-.42l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.2.09.46 0 .59-.22l1.92-3.32a.48.48 0 0 0-.12-.61zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z'/></svg>",
  moon: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.5 5.5 0 0 1-7.54-7.54C12.92 3.04 12.46 3 12 3z'/></svg>",
  puzzle: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M20.5 11H19V7a2 2 0 0 0-2-2h-4V3.5a2.5 2.5 0 0 0-5 0V5H4a2 2 0 0 0-2 2v3.8h1.5a2.7 2.7 0 0 1 0 5.4H2V20a2 2 0 0 0 2 2h3.8v-1.5a2.7 2.7 0 0 1 5.4 0V22H17a2 2 0 0 0 2-2v-4h1.5a2.5 2.5 0 0 0 0-5z'/></svg>",
};
const ico = (name: string) => ICONS[name] || "";

const format = ref<Format>("idle");
const phase = ref<Phase>("stable");
const hovered = ref(false);
const dnd = ref(false);
const builtinActions = ref<Cell[]>([]);

// --- Auto-hide : l'île se rétracte vers le haut quand une app est en plein écran,
// et ressort quand on survole le bord haut de l'écran ---
const hiddenFs = ref(false); // une app plein écran est au premier plan
const peek = ref(false); // l'utilisateur survole le bord haut → on ressort
const slidUp = computed(() => hiddenFs.value && !peek.value);
let peekTimer = 0;
function peekShow() {
  if (!hiddenFs.value) return;
  window.clearTimeout(peekTimer);
  peek.value = true;
}
function peekHideSoon() {
  if (!hiddenFs.value) return;
  window.clearTimeout(peekTimer);
  peekTimer = window.setTimeout(() => (peek.value = false), 250);
}

// Pile de notifications : l'île se déplie vers le bas (idle, hors DND). 5 visibles
// max, le reste au scroll.
const NOTIF_MAX = 5;
const NOTIF_CARD = 56; // px
const NOTIF_GAP = 6;
const showStack = computed(() => notifStack.value.length > 0 && format.value === "idle" && !dnd.value);
const stackHeight = computed(() => {
  const n = Math.min(notifStack.value.length, NOTIF_MAX);
  return n * NOTIF_CARD + (n - 1) * NOTIF_GAP + 12; // + padding vertical
});
function onNotifClick(n: Notif) {
  markRead(n.id);
  n.onClick?.();
}
// Cloche (slot droit) quand il y a des non-lues → ouvre le centre.
const BELL =
  "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M12 2a6 6 0 0 0-6 6v3.4l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.6L18 11.4V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z'/></svg>";
function openCenter() {
  setFormat("notifcenter");
}

// Gouttelette DND : en Do Not Disturb, pas de bannière — juste une goutte discrète
// qui tombe pour signaler l'arrivée d'une notif (elle file en non-lue).
const dndDrop = ref<{ id: string; color: string } | null>(null);
let dropTimer = 0;
function triggerDrop(n: Notif) {
  window.clearTimeout(dropTimer);
  dndDrop.value = { id: n.id, color: n.color || "rgba(255,255,255,.6)" };
  dropTimer = window.setTimeout(() => (dndDrop.value = null), 1000);
}

// --- Launcher : recherche extensible (les extensions enregistrent un provider) ---
const query = ref("");
const providerResults = ref<Cell[]>([]);
const searchEl = ref<HTMLInputElement>();
let queryTimer: number | undefined;
const RESULT_ICON =
  "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='11' cy='11' r='7'/><path d='m21 21-4.3-4.3'/></svg>";

// Mode recherche actif uniquement si une extension fournit un provider ET qu'on tape.
const searching = computed(() => hasProviders.value && query.value.trim().length > 0);

// Débounce : interroge les providers quand l'utilisateur tape.
watch(query, (q) => {
  if (queryTimer) clearTimeout(queryTimer);
  const term = q.trim();
  if (!hasProviders.value || !term) { providerResults.value = []; return; }
  queryTimer = window.setTimeout(async () => {
    const res = await runProviders(term).catch(() => []);
    providerResults.value = res.map((r) => ({ id: r.id, label: r.title, icon: r.icon || RESULT_ICON, kind: "result", onActivate: r.onActivate }));
  }, 120);
});

const launcherCells = computed<Cell[]>(() => {
  // Recherche : résultats des providers + entrées dont le label matche.
  if (searching.value) {
    const q = query.value.trim().toLowerCase();
    const matched = launcherEntries.value
      .filter((e) => e.label.toLowerCase().includes(q))
      .map((e) => ({ id: e.id, label: e.label, icon: e.icon, kind: "entry", onActivate: e.onActivate }));
    return [...providerResults.value, ...matched];
  }
  // Au repos : built-ins (icône = nom → SVG) + entrées des extensions ACTIVES.
  return [
    ...builtinActions.value.map((a) => ({ ...a, icon: ico(a.icon) })),
    ...launcherEntries.value.map((e) => ({ id: e.id, label: e.label, icon: e.icon, kind: "entry", onActivate: e.onActivate })),
  ];
});

// --- Goutte (sous-slot d'une view) ---
const dropOpen = ref(false);
const dropWide = ref(false);
const dropShow = ref(false);
const dropW = ref(48);
const dropEl = ref<HTMLElement>();

const wrapEl = ref<HTMLElement>();
const contentEl = ref<HTMLElement>();

const contentVisible = computed(() => phase.value === "stable" || phase.value === "enter");

function islandDims(): [number, number, number] {
  if (showStack.value) return [372, stackHeight.value, 26]; // pile de notifications
  if (format.value === "notifcenter") {
    const n = Math.min(unread.value.length || 1, NOTIF_MAX);
    return [372, n * NOTIF_CARD + (n - 1) * NOTIF_GAP + 12, 26];
  }
  if (format.value === "view") {
    const s = activeViewSize.value;
    return [s.width, s.height, s.radius];
  }
  if (format.value === "launcher") {
    // Hauteur dynamique : 3 colonnes → on affiche jusqu'à 4 rangées avant de scroller.
    const rows = Math.min(4, Math.max(1, Math.ceil(launcherCells.value.length / 3)));
    const search = hasProviders.value ? 46 : 0; // champ de recherche en haut (si provider)
    return [360, 32 + search + rows * 64 + (rows - 1) * 8, 26];
  }
  // idle
  if (dnd.value) return hovered.value ? [110, 36, 18] : [34, 34, 17];
  const a = idleActions.value;
  // Largeur basée sur le côté le plus chargé → réservation symétrique → le centre
  // (absolu) reste centré quoi qu'il arrive ; l'île grandit avec les actions.
  const maxSide = Math.max(a.left.length, a.right.length + (unreadCount.value ? 1 : 0));
  const stateActive = idleState.value !== "idle";
  if (!stateActive && maxSide === 0) return [120, 38, 19];
  const w = (stateActive ? 56 : 24) + 2 * (14 + maxSide * 28);
  return [Math.min(340, w), 38, 19];
}
const wrapStyle = computed(() => {
  const [w, h, r] = islandDims();
  return { "--w": w + "px", "--h": h + "px", "--r": r + "px" };
});

// --- Empreinte de la fenêtre overlay (anti-gel des autres apps) ----------------------
// Une fenêtre topmost qui couvre TOUT l'écran fait que Windows met en pause les autres
// apps (elles se croient occultées). On garde donc l'overlay en PETITE boîte haut-centre
// (l'île morphe DEDANS → pas de redimensionnement pendant les animations, donc pas de
// saut), et on ne repasse PLEIN ÉCRAN que quand une surface le réclame : modal (fond),
// fenêtre flottante, sélection de zone, contour d'enregistrement.
const win = getCurrentWindow();
const BOX_W = 900; // CSS px — assez large pour la plus grande vue + halo
const BOX_H = 760; // CSS px — assez haut pour vue + goutte + pile de notifs
// Géométrie du moniteur en pixels PHYSIQUES, mesurée de façon SYNCHRONE (window.screen ;
// le moniteur principal est à l'origine 0,0). Pas de dépendance à primaryMonitor() qui
// peut échouer → sinon la boîte ne s'appliquerait jamais (fenêtre restée plein écran).
const monX = 0;
const monY = 0;
const monW = Math.round(window.screen.width * (window.devicePixelRatio || 1));
const monH = Math.round(window.screen.height * (window.devicePixelRatio || 1));

function needsFullscreen(): boolean {
  return modalSpec.value !== null || floatWindows.value.length > 0 || selecting.value || !!regionOutline.value;
}

interface Box { x: number; y: number; w: number; h: number }
const boxTarget = computed<Box>(() => {
  if (needsFullscreen()) return { x: monX, y: monY, w: monW, h: monH };
  const dpr = window.devicePixelRatio || 1;
  const w = Math.min(monW, Math.round(BOX_W * dpr));
  const h = Math.min(monH, Math.round(BOX_H * dpr));
  return { x: monX + Math.round((monW - w) / 2), y: monY, w, h };
});

let lastBoxArea = -1;
let lastBoxKey = "";
let boxShrinkTimer: number | undefined;
async function applyBox(b: Box) {
  const key = `${b.x},${b.y},${b.w},${b.h}`;
  if (key === lastBoxKey) return;
  lastBoxKey = key;
  lastBoxArea = b.w * b.h;
  // Repositionne PUIS redimensionne → la fenêtre reste centrée pendant le changement.
  try { await win.setPosition(new PhysicalPosition(b.x, b.y)); } catch { /* noop */ }
  win.setSize(new PhysicalSize(b.w, b.h)).catch(() => {});
}
// Agrandir = immédiat (pas de rognage) ; rétrécir = différé (après l'anim de fermeture).
watch(boxTarget, (b) => {
  if (!b) return;
  clearTimeout(boxShrinkTimer);
  if (b.w * b.h >= lastBoxArea) applyBox(b);
  else boxShrinkTimer = window.setTimeout(() => applyBox(b), 460);
}, { immediate: true });

// --- Orchestrateur d'animation (exit -> morph -> enter, sans chevauchement) ---
function afterTransition(el: HTMLElement | undefined, props: string[], fallback: number) {
  return new Promise<void>((resolve) => {
    if (!el) return void setTimeout(resolve, fallback);
    let done = false;
    const finish = () => { if (done) return; done = true; el.removeEventListener("transitionend", h); clearTimeout(t); resolve(); };
    const h = (e: TransitionEvent) => { if (e.target === el && props.includes(e.propertyName)) finish(); };
    const t = window.setTimeout(finish, fallback);
    el.addEventListener("transitionend", h);
  });
}

let seq = 0;
async function setFormat(f: Format) {
  const my = ++seq;
  phase.value = "exit";
  await afterTransition(contentEl.value, ["opacity"], 280);
  if (my !== seq) return;
  format.value = f;
  phase.value = "morph";
  await afterTransition(wrapEl.value, ["width", "height"], 700);
  if (my !== seq) return;
  phase.value = "enter";
  await afterTransition(contentEl.value, ["opacity"], 280);
  if (my !== seq) return;
  phase.value = "stable";
}

function collapseToIdle() {
  closeDrop();
  if (activeView.value) closeView(); // → le watch repasse en idle
  else if (format.value !== "idle") setFormat("idle");
}
// Fermeture AUTOMATIQUE (clic hors de l'île / perte de focus) : une view marquée
// `persistent` y résiste (elle ne se ferme qu'au « Retour » ou via view.close()).
function autoDismiss() {
  if (activeView.value && activeViewPersistent.value) return;
  collapseToIdle();
}
function onWrapClick() {
  if (format.value === "idle") {
    // Si une extension a posé un handler de tap (ex. enregistrement en cours),
    // le clic ouvre SON UI au lieu du launcher.
    if (idleTap.value) idleTap.value();
    else setFormat("launcher");
  } else if (format.value === "launcher") collapseToIdle();
}
function onAction(c: Cell) {
  if (c.kind === "settings") invoke("open_settings").catch(() => {});
  else if (c.kind === "dnd") { dnd.value = !dnd.value; collapseToIdle(); }
  else if (c.kind === "entry" || c.kind === "result") { c.onActivate?.(); query.value = ""; } // action de l'extension
}
// Recherche du launcher : Entrée = 1er résultat ; Échap = vide le champ puis referme.
function onEnter() { const first = launcherCells.value[0]; if (first) onAction(first); }
function onEsc() { if (query.value) query.value = ""; else collapseToIdle(); }

// --- Goutte : ouverture/fermeture + largeur adaptée au contenu ---
async function openDropAnim() {
  dropOpen.value = true;
  await nextTick();
  dropW.value = Math.min(320, Math.max(48, dropEl.value?.offsetWidth ?? 200));
  window.setTimeout(() => (dropWide.value = true), 60);
  window.setTimeout(() => (dropShow.value = true), 320);
}
function closeDropAnim() {
  dropShow.value = false;
  window.setTimeout(() => (dropWide.value = false), 150);
  window.setTimeout(() => { dropWide.value = false; dropOpen.value = false; }, 360);
}

// --- Hit region (click-through) : inclut la goutte quand elle est ouverte ---
let lastRegion = "";
function publishRegion() {
  const el = wrapEl.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  const extra = dropOpen.value ? 120 : 8;
  const rect = { x: Math.round(r.left - 6), y: Math.round(r.top - 4), w: Math.round(r.width + 12), h: Math.round(r.height + extra) };
  const key = `${rect.x},${rect.y},${rect.w},${rect.h}`;
  if (key !== lastRegion) { lastRegion = key; setHitRegion("island", rect); }
}
let raf = 0;
function tick() { publishRegion(); raf = requestAnimationFrame(tick); }

// --- Réactions aux stores (view + goutte pilotées par les extensions) ---
watch(activeView, (v) => {
  if (v) { if (format.value !== "view") setFormat("view"); }
  else if (format.value === "view") setFormat("idle");
});
watch(dropContent, (c) => { if (c) openDropAnim(); else closeDropAnim(); });
// Île rétractée : une fine bande en haut capte la souris (désactive le click-through
// là) pour pouvoir la faire ressortir au survol.
watch(hiddenFs, (h) => {
  setHitRegion("fs-peek", h ? { x: 0, y: 0, w: window.innerWidth, h: 6 } : null);
});
// Quitter le centre de notifs = on les a lues → elles disparaissent pour de bon.
watch(format, (f, prev) => { if (prev === "notifcenter" && f !== "notifcenter") clearUnread(); });
// Centre vidé (toutes effacées une à une) → on referme.
watch(unreadCount, (c) => { if (c === 0 && format.value === "notifcenter") collapseToIdle(); });
// DND : pas de bannière (le centre le sait) + une gouttelette discrète à l'arrivée.
watch(dnd, (v) => setNotifDnd(v), { immediate: true });
watch(lastPosted, (n) => { if (n && dnd.value) triggerDrop(n); });
// Launcher avec recherche : focus le champ à l'ouverture, vide le champ à la fermeture.
watch(format, (f, prev) => {
  if (f === "launcher" && hasProviders.value) {
    nextTick(() => { invoke("overlay_focus").catch(() => {}); searchEl.value?.focus(); });
  }
  if (prev === "launcher" && f !== "launcher") query.value = "";
});

let unfocus: (() => void) | undefined;
onMounted(async () => {
  raf = requestAnimationFrame(tick);
  try { builtinActions.value = await invoke<Cell[]>("list_launcher"); } catch { /* noop */ }

  await listen<boolean>("overlay://hover", (e) => { hovered.value = e.payload; });
  await listen("overlay://dismiss", () => autoDismiss());
  // Auto-hide : une app plein écran → l'île se rétracte (reveal au survol du haut).
  await listen<{ active: boolean }>("fullscreen://changed", (e) => {
    hiddenFs.value = e.payload.active;
    if (!e.payload.active) peek.value = false;
  });
  unfocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) autoDismiss();
  });
});
onUnmounted(() => { cancelAnimationFrame(raf); unfocus?.(); });
</script>

<template>
  <div class="root">
    <svg width="0" height="0" class="goo-def">
      <defs>
        <filter id="goo">
          <feGaussianBlur in="SourceGraphic" stdDeviation="6" result="b" />
          <feColorMatrix in="b" mode="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 20 -9" />
        </filter>
      </defs>
    </svg>

    <!-- Auto-hide : zone de survol (bord haut) + petit indice quand l'île est rétractée -->
    <div v-if="hiddenFs" class="peek-zone" @mouseenter="peekShow" @mouseleave="peekHideSoon"></div>
    <div v-if="slidUp" class="peek-hint"></div>

    <!-- Gouttelette DND : tombe sous l'île à l'arrivée d'une notif (sans déranger) -->
    <div v-if="dndDrop && dnd" :key="dndDrop.id" class="dnddrop" :style="{ background: dndDrop.color }"></div>

    <div
      ref="wrapEl"
      class="wrap"
      :class="{ up: slidUp }"
      :style="wrapStyle"
      @click="onWrapClick"
      @mouseenter="peekShow"
      @mouseleave="peekHideSoon"
    >
      <div class="goo" :class="{ dropping: dropOpen }">
        <div class="bg"></div>
        <div class="drop-bg" :class="{ wide: dropWide }" :style="dropWide ? { width: dropW + 'px' } : {}"></div>
      </div>

      <!-- Goutte : contenu fourni par l'extension (sous-slot d'une view) -->
      <div v-if="dropOpen" ref="dropEl" class="drop-content" :class="{ show: dropShow }">
        <component v-if="dropContent" :is="dropContent" />
      </div>

      <div v-if="format !== 'idle'" class="topback" @click.stop="collapseToIdle" aria-label="Retour">
        <span class="grab"></span>
      </div>

      <div ref="contentEl" class="content" :style="{ opacity: contentVisible ? 1 : 0 }">
        <!-- PILE de notifications : l'île se déplie et liste les notifs (scroll au-delà de 5) -->
        <TransitionGroup
          v-if="showStack"
          name="ncard"
          tag="div"
          class="nstack"
          :class="{ safe: notifStack.length > 1 }"
          @click.stop
          @mouseenter="pauseStack"
          @mouseleave="resumeStack"
        >
          <div v-for="n in notifStack" :key="n.id" class="notif" @click.stop="onNotifClick(n)">
            <div
              class="nicon"
              :style="n.color ? { background: n.color + '26', color: n.color } : {}"
              v-html="n.icon"
            ></div>
            <div class="ntext">
              <div class="ntitle">{{ n.title }}</div>
              <div v-if="n.body" class="nbody">{{ n.body }}</div>
            </div>
            <div v-if="n.source" class="nsource">{{ n.source }}</div>
          </div>
        </TransitionGroup>

        <!-- IDLE : centre ABSOLUMENT centré ; les slots s'étendent aux bords -->
        <div v-else-if="format === 'idle'" class="idle">
          <template v-if="dnd">
            <div class="center"><div class="dot dim" :style="{ opacity: hovered ? 1 : 0 }"></div></div>
          </template>
          <template v-else>
            <!-- Slot gauche (bord gauche) : icônes/textes CUMULÉS -->
            <div class="slots slots-left">
              <template v-for="(a, i) in idleActions.left" :key="'l' + i">
                <button v-if="a.onActivate" class="slot" :style="{ color: a.color }" @click.stop="a.onActivate()">
                  <span v-if="a.text" class="stext">{{ a.text }}</span>
                  <span v-else class="sico" v-html="a.icon"></span>
                </button>
                <span v-else class="slot stext" :style="{ color: a.color }">{{ a.text }}</span>
              </template>
            </div>

            <!-- Centre : état géré par l'hôte, toujours au centre -->
            <div class="center">
              <div v-if="idleState === 'playing'" class="wave">
                <span></span><span></span><span></span><span></span><span></span><span></span>
              </div>
              <div v-else-if="idleState === 'busy'" class="busy"><span></span><span></span><span></span></div>
              <div v-else-if="idleState === 'recording'" class="rec"></div>
              <div v-else class="dot"></div>
            </div>

            <!-- Slot droit (bord droit) : icônes/textes CUMULÉS + cloche -->
            <div class="slots slots-right">
              <template v-for="(a, i) in idleActions.right" :key="'r' + i">
                <button v-if="a.onActivate" class="slot" :style="{ color: a.color }" @click.stop="a.onActivate()">
                  <span v-if="a.text" class="stext">{{ a.text }}</span>
                  <span v-else class="sico" v-html="a.icon"></span>
                </button>
                <span v-else class="slot stext" :style="{ color: a.color }">{{ a.text }}</span>
              </template>
              <button v-if="unreadCount" class="slot bell" @click.stop="openCenter" aria-label="Notifications">
                <span class="sico" v-html="BELL"></span>
                <span class="badge">{{ unreadCount }}</span>
              </button>
            </div>
          </template>
        </div>

        <!-- LAUNCHER : recherche (si provider) + grille extensions/natifs -->
        <div v-else-if="format === 'launcher'" class="lwrap">
          <input v-if="hasProviders" ref="searchEl" v-model="query" class="lsearch" type="text"
                 spellcheck="false" placeholder="Rechercher…"
                 @keydown.enter.prevent="onEnter" @keydown.esc.prevent="onEsc" @click.stop />
          <div class="grid">
            <button v-for="c in launcherCells" :key="c.id" class="cell"
                    :class="{ on: c.kind === 'dnd' && dnd }" @click.stop="onAction(c)">
              <span class="cico" v-html="c.icon"></span>
              <span class="clabel">{{ c.label }}</span>
            </button>
          </div>
        </div>

        <!-- CENTRE de notifications : liste des non-lues (lues = retirées pour de bon) -->
        <div v-else-if="format === 'notifcenter'" class="ncenter" @click.stop>
          <div v-if="!unread.length" class="nempty">Aucune notification</div>
          <TransitionGroup v-else name="ncard" tag="div" class="nlist" :class="{ safe: unread.length > 1 }">
            <div v-for="n in unread" :key="n.id" class="notif" @click.stop="onNotifClick(n)">
              <div class="nicon" :style="n.color ? { background: n.color + '26', color: n.color } : {}" v-html="n.icon"></div>
              <div class="ntext">
                <div class="ntitle">{{ n.title }}</div>
                <div v-if="n.body" class="nbody">{{ n.body }}</div>
              </div>
              <div v-if="n.source" class="nsource">{{ n.source }}</div>
            </div>
          </TransitionGroup>
        </div>

        <!-- VIEW : la surface de l'extension active -->
        <div v-else-if="format === 'view'" class="view" :class="{ 'no-safe': !activeViewSafeArea }">
          <component v-if="activeView" :is="activeView" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference '@/style.css';
.root {
  @apply relative h-screen flex justify-center items-start pt-2 bg-transparent pointer-events-none select-none z-1;
}
.goo-def { @apply absolute; }
.wrap {
  @apply relative w-(--w) h-(--h) rounded-(--r) cursor-pointer pointer-events-auto!;
  /* Zone haute réservée : l'île touche le bord d'écran + porte la poignée de
     collapse → on ne laisse pas le contenu (views/notifs) coller tout en haut. */
  --safe-top: 14px;
  transition: width 0.55s cubic-bezier(0.34, 1.4, 0.42, 1), height 0.55s cubic-bezier(0.34, 1.4, 0.42, 1), border-radius 0.45s ease, transform 0.42s cubic-bezier(0.4, 0, 0.2, 1);
}
/* Rétractée vers le haut, hors écran (au-delà du padding top de .root). */
.wrap.up { transform: translateY(calc(-100% - 18px)); }
/* Bande invisible en haut qui capte le survol pour faire ressortir l'île. */
.peek-zone { @apply fixed top-0 left-0 right-0 h-1.5 pointer-events-auto; }
/* Petit indice visuel quand l'île est cachée. */
.peek-hint {
  @apply fixed top-[3px] left-1/2 h-1 w-10 -translate-x-1/2 rounded-full pointer-events-none;
  background: rgba(255, 255, 255, 0.22);
  transition: opacity 0.3s ease;
}
/* Gouttelette DND : forme une goutte sous l'île puis tombe et s'efface. */
.dnddrop {
  @apply absolute left-1/2 h-2.5 w-2.5 rounded-full pointer-events-none;
  top: 34px;
  box-shadow: 0 0 6px rgba(0, 0, 0, 0.25);
  animation: drip 1s cubic-bezier(0.4, 0, 0.6, 1) forwards;
}
@keyframes drip {
  0%   { transform: translate(-50%, -6px) scale(0.2); opacity: 0; }
  25%  { transform: translate(-50%, 0) scale(1); opacity: 0.9; }
  55%  { transform: translate(-50%, 12px) scaleY(1.35) scaleX(0.8); opacity: 0.85; }
  100% { transform: translate(-50%, 36px) scale(0.35); opacity: 0; }
}
.goo { @apply absolute inset-0; }
.goo.dropping { filter: url(#goo); }
.bg { @apply absolute inset-0 bg-background rounded-(--r); box-shadow: 0 8px 24px rgba(0,0,0,.4), inset 0 0 0 0.5px rgba(255,255,255,.07); }

/* Goutte */
.drop-bg { 
  @apply absolute left-1/2 top-[calc(100%-13px)] w-8.5 h-9.5 rounded-[19px] bg-background -translate-x-1/2 scale-0 origin-top opacity-0;
  /* position: absolute; left: 50%; top: calc(100% - 13px); width: 34px; height: 38px; border-radius: 19px; background: #070708; transform: translateX(-50%) scale(0); transform-origin: top center; opacity: 0;  */
  transition: width 0.42s cubic-bezier(0.34,1.4,0.4,1), opacity 0.2s ease; 
}
.goo.dropping .drop-bg { @apply -translate-x-1/2 scale-100 opacity-100; /* transform: translateX(-50%) scale(1); opacity: 1; */ }
.drop-content { 
  @apply absolute left-1/2 top-[calc(100%-13px)] -translate-x-1/2 h-9.5 flex items-center py-0 px-3.5 opacity-0 z-3 pointer-events-auto;
  /* position: absolute; left: 50%; top: calc(100% - 13px); transform: translateX(-50%); height: 38px; display: flex; align-items: center; padding: 0 14px; box-sizing: border-box; opacity: 0; z-index: 3; pointer-events: auto; color: #fff;  */
  transition: opacity 0.16s ease; 
}
.drop-content.show { opacity: 1; }

.content { position: absolute; inset: 0; display: flex; align-items: center; border-radius: var(--r); overflow: hidden; transition: opacity 0.22s ease; }
.view { width: 100%; height: 100%; box-sizing: border-box; padding-top: var(--safe-top); }
.view.no-safe { padding-top: 0; } /* la view gère son propre haut (ex. bannière) */

/* Idle */
.idle { position: relative; width: 100%; height: 100%; }
/* Centre absolument centré : reste au milieu quel que soit le nombre d'actions. */
.center { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); display: flex; align-items: center; justify-content: center; }
.slot { width: 26px; height: 26px; border: none; background: transparent; padding: 0; display: flex; align-items: center; justify-content: center; cursor: pointer; opacity: .92; }
.slot:hover { opacity: 1; }
.sico { width: 15px; height: 15px; display: flex; }
.sico :deep(svg) {@apply w-full h-full; }
.spacer { @apply w-6.5 h-6.5; }
/* Pile de notifications + centre */
.nstack, .nlist { @apply relative flex h-full w-full flex-col gap-1.5 overflow-y-auto overflow-x-hidden p-1.5; scrollbar-width: thin; }
/* Safe-zone seulement au-delà d'1 notif : sinon 1 carte + padding déborde → scroll moche. */
.nstack.safe, .nlist.safe { padding-top: calc(0.375rem + var(--safe-top)); }
.nstack::-webkit-scrollbar, .nlist::-webkit-scrollbar { width: 6px; }
.nstack::-webkit-scrollbar-thumb, .nlist::-webkit-scrollbar-thumb { @apply rounded-full bg-foreground/20; }
.ncenter { @apply relative h-full w-full; }
.nempty { @apply grid h-full w-full place-items-center text-[12px] text-muted-foreground; }
.notif { @apply flex h-14 flex-none items-center gap-2.5 rounded-xl px-2.5 transition; }
.notif:hover { @apply bg-foreground/[0.06]; }
.nicon { @apply grid h-9 w-9 flex-none place-items-center rounded-[10px] bg-foreground/10 text-foreground; }
.nicon :deep(svg) { width: 18px; height: 18px; }
.ntext { @apply min-w-0 flex-1; }
.ntitle { @apply truncate text-[12.5px] font-semibold leading-tight; }
.nbody { @apply truncate text-[11px] leading-tight text-muted-foreground; margin-top: 1px; }
.nsource { @apply flex-none self-start text-[10px] text-muted-foreground; }
/* Lissage des cartes (insertion / réordonnancement / retrait) */
.ncard-move, .ncard-enter-active, .ncard-leave-active { transition: opacity 0.28s ease, transform 0.3s cubic-bezier(0.22, 1, 0.36, 1); }
.ncard-enter-from { opacity: 0; transform: translateY(-10px); }
.ncard-leave-to { opacity: 0; transform: translateX(24px); }
.ncard-leave-active { position: absolute; left: 6px; right: 6px; }

/* Slots idle cumulables + cloche */
/* Slots aux bords (absolus) : s'étendent vers l'extérieur sans bouger le centre. */
.slots { @apply absolute flex items-center gap-0.5; top: 50%; transform: translateY(-50%); }
.slots-left { left: 10px; }
.slots-right { right: 10px; }
.bell { position: relative; }
.badge { @apply absolute grid h-3.5 min-w-3.5 place-items-center rounded-full px-0.5 text-[8px] font-bold leading-none text-white; top: -3px; right: -3px; background: #ff453a; }

.dot { @apply w-2 h-2 bg-primary rounded-full m-auto; animation: breathe 2s ease-in-out infinite; }
.dot.dim { background: rgba(255,255,255,.35); animation: none; transition: opacity 0.25s ease; }
.rec { width: 8px; height: 8px; margin: auto; border-radius: 50%; background: #ef4444; animation: recpulse 1.5s ease-out infinite; }
@keyframes recpulse {
  0%   { box-shadow: 0 0 0 0 rgba(239,68,68,.55); }
  70%  { box-shadow: 0 0 0 6px rgba(239,68,68,0); }
  100% { box-shadow: 0 0 0 0 rgba(239,68,68,0); }
}
.stext { font-size: 11px; font-variant-numeric: tabular-nums; opacity: .9; padding: 0 2px; }
@keyframes breathe { 0%,100%{opacity:.5;transform:scale(.85)} 50%{opacity:1;transform:scale(1.15)} }
.wave { display: flex; align-items: center; gap: 3px; height: 20px; color: var(--foreground); }
.wave span { width: 3px; background: currentColor; border-radius: 2px; animation: wave 1s ease-in-out infinite; }
.wave span:nth-child(1){animation-delay:0s}.wave span:nth-child(2){animation-delay:.18s}.wave span:nth-child(3){animation-delay:.36s}.wave span:nth-child(4){animation-delay:.12s}.wave span:nth-child(5){animation-delay:.3s}.wave span:nth-child(6){animation-delay:.22s}
@keyframes wave { 0%,100%{height:5px} 50%{height:20px} }
.busy { display: flex; align-items: center; gap: 4px; }
.busy span { width: 6px; height: 6px; border-radius: 50%; background: var(--foreground); animation: busy 1.2s ease-in-out infinite; }
.busy span:nth-child(2){animation-delay:.15s}.busy span:nth-child(3){animation-delay:.3s}
@keyframes busy { 0%,100%{opacity:.3;transform:translateY(0)} 40%{opacity:1;transform:translateY(-3px)} }

/* Launcher */
.lwrap { @apply flex flex-col w-full h-full; }
.lsearch { @apply mx-3 mt-3 mb-0.5 rounded-lg border border-border bg-foreground/[0.04] px-3 py-2 text-[13px] text-foreground outline-none transition; }
.lsearch:focus { @apply border-primary; }
.lsearch::placeholder { @apply text-muted-foreground; }
.grid { @apply grid grid-cols-3 content-start gap-2 p-4 w-full flex-1 min-h-0 overflow-y-auto overflow-x-hidden; scrollbar-width: thin; }
.cell { @apply flex flex-col items-center gap-1.5 py-2.5 px-1 rounded-xl cursor-pointer hover:bg-card/50; }
.cell.on { @apply bg-primary text-primary-foreground; }
.cico { @apply w-5.5 h-5.5 flex; }
.cico :deep(svg) { @apply w-full h-full }
.clabel { @apply text-[11px] text-muted-foreground; }

/* Retour */
.topback { @apply absolute top-0 left-0 right-0 h-4 flex items-center justify-center cursor-pointer z-4; }
.grab { @apply w-6.5 h-0.75 bg-gray-600/75 hover:bg-gray-600 rounded-full transition-all; }
</style>
