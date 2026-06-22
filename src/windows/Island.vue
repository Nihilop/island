<script setup lang="ts">
// L'ÎLE = des slots agnostiques. Elle ne connaît AUCUNE extension.
// États : idle (registre d'indicateurs) · launcher (extensions + natifs) ·
// view (monte une surface d'extension) · + un sous-slot "goutte" dans une view.
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { idleState, idleCenter, idleActions, idleTap } from "../composables/idle";
import { hasProviders, launcherCells, loadBuiltins } from "../composables/launcher";
import {
  setHitRegion, activeView, activeViewSize, activeViewPersistent, activeViewSafeZone, closeView, dropContent, closeDrop,
  modalSpec, floatWindows, selecting, regionOutline, islandRect, type SafeZone,
} from "../composables/overlay";
import { DEV, devPin, installDevShortcuts, devRestore } from "../composables/dev";
import { theme, islandAccent } from "../composables/islandTheme";
import IslandSurface from "./IslandSurface.vue";
import NotifCard from "./NotifCard.vue";
import Launcher from "./Launcher.vue";
import { stack as notifStack, unread, unreadCount, lastPosted, setDnd as setNotifDnd, pauseStack, resumeStack, markRead, clearUnread, type Notif } from "../composables/notifications";

type Format = "idle" | "launcher" | "view" | "notifcenter";
type Phase = "stable" | "exit" | "morph" | "enter";

const format = ref<Format>("idle");
const phase = ref<Phase>("stable");
const hovered = ref(false);
const dnd = ref(false);

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

// --- Goutte (sous-slot d'une view) ---
const dropOpen = ref(false);
const dropWide = ref(false);
const dropShow = ref(false);
const dropW = ref(48);
const dropEl = ref<HTMLElement>();

const wrapEl = ref<HTMLElement>();
const contentEl = ref<HTMLElement>();

// --- Mesure du CENTRE idle : l'île se dimensionne autour du contenu central réel
// (point, ou composant `idle.center` d'une extension qui peut faire 100px+). Le centre
// reste absolument centré ; seule la largeur/hauteur de l'île dérive de cette mesure. ---
const centerEl = ref<HTMLElement>();
const centerW = ref(8);
const centerH = ref(8);
let centerRO: ResizeObserver | undefined;
function measureCenter() {
  const el = centerEl.value;
  if (!el) return;
  centerW.value = el.offsetWidth;
  centerH.value = el.offsetHeight;
}
watch(centerEl, (el) => {
  centerRO?.disconnect();
  if (el) { centerRO = new ResizeObserver(measureCenter); centerRO.observe(el); measureCenter(); }
});

const contentVisible = computed(() => phase.value === "stable" || phase.value === "enter");

// Style racine : ancrage (espace sous le bord haut, selon le LAYOUT) + couleur d'accent
// (override --primary dans toute l'île, valeur indépendante du layout). Anim/contenu inchangés.
const rootStyle = computed(() => {
  const s: Record<string, string> = { paddingTop: theme.value.anchor === "topbar" ? "0px" : "8px" };
  if (islandAccent.value) s["--primary"] = islandAccent.value;
  return s;
});

// L'île est-elle « ouverte » (déployée) ou au repos ? → pilote les tailles topbar + le surface.
const islandOpen = computed(() => format.value !== "idle" || showStack.value);

// Mode de zone haute ACTIF selon le format courant (cf. SafeZone). Pilote d'un seul
// endroit : la poignée de collapse, la réserve de padding, et le scrim.
//  - view → mode déclaré par l'extension · launcher → relative · notifs → hidden.
const safeZone = computed<SafeZone>(() => {
  if (showStack.value) return "hidden";
  if (format.value === "notifcenter") return "hidden";
  if (format.value === "view") return activeViewSafeZone.value;
  if (format.value === "launcher") return "relative";
  return "relative";
});

function islandDims(): [number, number, number] {
  // Taille TOUJOURS pilotée par le contenu (idem flottant) — le thème ne change que le
  // chrome (ancrage + congés), jamais la taille. Donc topbar s'adapte au contenu comme le reste.
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
  // Réserve SYMÉTRIQUE par côté (mini = marge) → le centre reste centré même si un côté
  // a plus d'icônes ; l'île grandit avec les actions.
  const maxSide = Math.max(a.left.length, a.right.length + (unreadCount.value ? 1 : 0));
  const sideReserve = Math.max(16, 14 + maxSide * 28);
  // Le CENTRE dicte sa taille (mesurée) : l'île l'enveloppe → un composant large ne
  // déborde plus. Plancher = la pilule idle confortable (120×38) ; plafond large.
  const w = Math.min(720, Math.max(120, centerW.value + 2 * sideReserve));
  const h = Math.max(38, centerH.value + 16);
  return [w, h, Math.round(h / 2)];
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
  // Une fenêtre MINIMISÉE (sphère) ne force pas le plein écran.
  return modalSpec.value !== null || floatWindows.value.some((w) => !w.minimized) || selecting.value || !!regionOutline.value;
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
  if (DEV && devPin.value) return; // dev : île épinglée → ne se referme pas (cf. composables/dev)
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
// DND basculé depuis une cellule native du launcher → bascule + referme.
function onToggleDnd() { dnd.value = !dnd.value; collapseToIdle(); }

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
  islandRect.value = rect; // ancre la barre des fenêtres minimisées
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

let unfocus: (() => void) | undefined;
let uninstallDev: (() => void) | undefined;
onMounted(async () => {
  raf = requestAnimationFrame(tick);
  loadBuiltins(); // actions natives du launcher (Réglages, DND…)

  // Dev : raccourcis (Ctrl+Alt+P/N/M/W/0) + restauration de la surface forcée après HMR.
  if (DEV) {
    const force = (f: string) => setFormat(f as Format);
    uninstallDev = installDevShortcuts(force);
    nextTick(() => devRestore(force));
  }

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
onUnmounted(() => { cancelAnimationFrame(raf); unfocus?.(); uninstallDev?.(); centerRO?.disconnect(); });
</script>

<template>
  <div class="relative z-1 flex h-screen items-start justify-center bg-transparent pointer-events-none select-none" :style="rootStyle">
    <svg width="0" height="0" class="absolute">
      <defs>
        <filter id="goo">
          <feGaussianBlur in="SourceGraphic" stdDeviation="6" result="b" />
          <feColorMatrix in="b" mode="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 20 -9" />
        </filter>
      </defs>
    </svg>

    <!-- Auto-hide : zone de survol (bord haut) + petit indice quand l'île est rétractée -->
    <div v-if="hiddenFs" class="fixed top-0 left-0 right-0 h-1.5 pointer-events-auto" @mouseenter="peekShow" @mouseleave="peekHideSoon"></div>
    <div v-if="slidUp" class="fixed top-[3px] left-1/2 h-1 w-10 -translate-x-1/2 rounded-full pointer-events-none bg-white/[0.22] transition-opacity duration-300"></div>

    <!-- Gouttelette DND : tombe sous l'île à l'arrivée d'une notif (sans déranger) -->
    <div v-if="dndDrop && dnd" :key="dndDrop.id" class="dnddrop absolute left-1/2 top-[34px] h-2.5 w-2.5 rounded-full pointer-events-none shadow-[0_0_6px_rgba(0,0,0,0.25)]" :style="{ background: dndDrop.color }"></div>

    <div
      ref="wrapEl"
      class="wrap"
      :class="['sz-' + safeZone, { up: slidUp }]"
      :style="wrapStyle"
      @click="onWrapClick"
      @mouseenter="peekShow"
      @mouseleave="peekHideSoon"
    >
      <div class="goo" :class="{ dropping: dropOpen }">
        <IslandSurface :open="islandOpen" />
        <div class="drop-bg" :class="{ wide: dropWide }" :style="dropWide ? { width: dropW + 'px' } : {}"></div>
      </div>

      <!-- Goutte : contenu fourni par l'extension (sous-slot d'une view) -->
      <div v-if="dropOpen" ref="dropEl" class="drop-content" :class="{ show: dropShow }">
        <component v-if="dropContent" :is="dropContent" />
      </div>

      <!-- Scrim sous la poignée en mode `absolute` : lisibilité de la poignée sur une bannière -->
      <div v-if="format !== 'idle' && safeZone === 'absolute'" class="absolute top-0 left-0 right-0 z-3 h-7 pointer-events-none rounded-[var(--r)_var(--r)_0_0] bg-[linear-gradient(to_bottom,rgba(0,0,0,0.38),transparent)]"></div>
      <!-- Poignée de collapse : masquée en `hidden` (notifs) -->
      <div v-if="format !== 'idle' && safeZone !== 'hidden'" class="absolute top-0 left-0 right-0 z-4 flex h-4 items-center justify-center cursor-pointer" @click.stop="collapseToIdle" aria-label="Retour">
        <span class="h-0.75 w-6.5 rounded-full bg-gray-600/75 transition-all hover:bg-gray-600"></span>
      </div>

      <!-- Dev : badge d'épinglage (DEV-only) -->
      <div v-if="DEV && devPin" class="absolute bottom-1 right-2 z-10 rounded bg-[#6366f1]/85 px-1.5 py-0.5 text-[9px] font-bold tracking-[0.04em] text-white pointer-events-none" title="Île épinglée (Ctrl+Alt+P) — Ctrl+Alt+0 pour libérer">DEV ⊙</div>

      <div ref="contentEl" class="absolute inset-0 flex items-center overflow-hidden rounded-(--r) transition-opacity duration-[220ms]" :style="{ opacity: contentVisible ? 1 : 0 }">
        <!-- PILE de notifications : l'île se déplie et liste les notifs (scroll au-delà de 5) -->
        <TransitionGroup
          v-if="showStack"
          name="ncard"
          tag="div"
          class="nstack relative flex h-full w-full flex-col gap-1.5 overflow-y-auto overflow-x-hidden p-1.5 [scrollbar-width:thin]"
          @click.stop
          @mouseenter="pauseStack"
          @mouseleave="resumeStack"
        >
          <NotifCard v-for="n in notifStack" :key="n.id" :notif="n" @click.stop="onNotifClick(n)" />
        </TransitionGroup>

        <!-- IDLE : centre ABSOLUMENT centré ; les slots s'étendent aux bords -->
        <div v-else-if="format === 'idle'" class="relative h-full w-full">
          <template v-if="dnd">
            <div class="absolute left-1/2 top-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center justify-center"><div class="m-auto h-2 w-2 rounded-full bg-white/35 transition-opacity duration-[250ms]" :style="{ opacity: hovered ? 1 : 0 }"></div></div>
          </template>
          <template v-else>
            <!-- Slot gauche (bord gauche) : icônes/textes CUMULÉS -->
            <div class="absolute top-1/2 left-[10px] flex -translate-y-1/2 items-center gap-0.5">
              <template v-for="(a, i) in idleActions.left" :key="'l' + i">
                <button v-if="a.onActivate" class="flex h-[26px] w-[26px] items-center justify-center border-none bg-transparent p-0 cursor-pointer opacity-[0.92] hover:opacity-100" :style="{ color: a.color }" @click.stop="a.onActivate()">
                  <span v-if="a.text" class="px-0.5 text-[11px] tabular-nums opacity-90">{{ a.text }}</span>
                  <span v-else class="flex h-[15px] w-[15px] [&_svg]:h-full [&_svg]:w-full" v-html="a.icon"></span>
                </button>
                <span v-else class="flex h-[26px] w-[26px] items-center justify-center px-0.5 text-[11px] tabular-nums opacity-90" :style="{ color: a.color }">{{ a.text }}</span>
              </template>
            </div>

            <!-- Centre : composant custom d'une extension (viz riche) sinon cercle coloré par état.
                 Mesuré (ResizeObserver) → l'île se dimensionne autour. -->
            <div ref="centerEl" class="absolute left-1/2 top-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center justify-center">
              <component v-if="idleCenter" :is="idleCenter" />
              <div v-else-if="idleState === 'recording'" class="rec m-auto h-2 w-2 rounded-full bg-[#ef4444]"></div>
              <div v-else class="dot m-auto h-2 w-2 rounded-full bg-primary"></div>
            </div>

            <!-- Slot droit (bord droit) : icônes/textes CUMULÉS + cloche -->
            <div class="absolute top-1/2 right-[10px] flex -translate-y-1/2 items-center gap-0.5">
              <template v-for="(a, i) in idleActions.right" :key="'r' + i">
                <button v-if="a.onActivate" class="flex h-[26px] w-[26px] items-center justify-center border-none bg-transparent p-0 cursor-pointer opacity-[0.92] hover:opacity-100" :style="{ color: a.color }" @click.stop="a.onActivate()">
                  <span v-if="a.text" class="px-0.5 text-[11px] tabular-nums opacity-90">{{ a.text }}</span>
                  <span v-else class="flex h-[15px] w-[15px] [&_svg]:h-full [&_svg]:w-full" v-html="a.icon"></span>
                </button>
                <span v-else class="flex h-[26px] w-[26px] items-center justify-center px-0.5 text-[11px] tabular-nums opacity-90" :style="{ color: a.color }">{{ a.text }}</span>
              </template>
              <button v-if="unreadCount" class="relative flex h-[26px] w-[26px] items-center justify-center border-none bg-transparent p-0 cursor-pointer opacity-[0.92] hover:opacity-100" @click.stop="openCenter" aria-label="Notifications">
                <span class="flex h-[15px] w-[15px] [&_svg]:h-full [&_svg]:w-full" v-html="BELL"></span>
                <span class="absolute top-[-3px] right-[-3px] grid h-3.5 min-w-3.5 place-items-center rounded-full bg-[#ff453a] px-0.5 text-[8px] font-bold leading-none text-white">{{ unreadCount }}</span>
              </button>
            </div>
          </template>
        </div>

        <!-- LAUNCHER : recherche (si provider) + grille extensions/natifs -->
        <Launcher v-else-if="format === 'launcher'" :dnd="dnd" @close="collapseToIdle" @toggle-dnd="onToggleDnd" />

        <!-- CENTRE de notifications : liste des non-lues (lues = retirées pour de bon) -->
        <div v-else-if="format === 'notifcenter'" class="relative h-full w-full" @click.stop>
          <div v-if="!unread.length" class="grid h-full w-full place-items-center text-[12px] text-muted-foreground">Aucune notification</div>
          <TransitionGroup v-else name="ncard" tag="div" class="nlist relative flex h-full w-full flex-col gap-1.5 overflow-y-auto overflow-x-hidden p-1.5 [scrollbar-width:thin]">
            <NotifCard v-for="n in unread" :key="n.id" :notif="n" @click.stop="onNotifClick(n)" />
          </TransitionGroup>
        </div>

        <!-- VIEW : la surface de l'extension active -->
        <div v-else-if="format === 'view'" class="view">
          <component v-if="activeView" :is="activeView" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference '@/style.css';
/* Ce bloc ne garde QUE ce qui n'est pas exprimable en classes : variables CSS,
   transitions multi-propriétés à béziers, filtre #goo, scrollbars custom,
   classes de <TransitionGroup>, et les keyframes/animations des indicateurs. */

/* Boîte de l'île : --safe-top (consommé par les enfants) + morph multi-prop. */
.wrap {
  @apply relative w-(--w) h-(--h) rounded-(--r) cursor-pointer pointer-events-auto!;
  --safe-top: 14px;
  transition: width 0.55s cubic-bezier(0.34, 1.4, 0.42, 1), height 0.55s cubic-bezier(0.34, 1.4, 0.42, 1), border-radius 0.45s ease, transform 0.42s cubic-bezier(0.4, 0, 0.2, 1);
}
.wrap.up { transform: translateY(calc(-100% - 18px)); } /* rétractée hors écran */

/* Gouttelette DND : layout en classes ; ici seulement l'animation (collée à ses
   keyframes, car Vue renomme les @keyframes scoped → l'animation doit rester ici). */
.dnddrop { animation: drip 1s cubic-bezier(0.4, 0, 0.6, 1) forwards; }
@keyframes drip {
  0%   { transform: translate(-50%, -6px) scale(0.2); opacity: 0; }
  25%  { transform: translate(-50%, 0) scale(1); opacity: 0.9; }
  55%  { transform: translate(-50%, 12px) scaleY(1.35) scaleX(0.8); opacity: 0.85; }
  100% { transform: translate(-50%, 36px) scale(0.35); opacity: 0; }
}

/* Effet metaball : le filtre fusionne l'île et la goutte. */
.goo { @apply absolute inset-0; }
.goo.dropping { filter: url(#goo); }
.drop-bg {
  @apply absolute left-1/2 top-[calc(100%-13px)] w-8.5 h-9.5 rounded-[19px] bg-background -translate-x-1/2 scale-0 origin-top opacity-0;
  transition: width 0.42s cubic-bezier(0.34,1.4,0.4,1), opacity 0.2s ease;
}
.goo.dropping .drop-bg { @apply -translate-x-1/2 scale-100 opacity-100; }
.drop-content {
  @apply absolute left-1/2 top-[calc(100%-13px)] -translate-x-1/2 h-9.5 flex items-center py-0 px-3.5 opacity-0 z-3 pointer-events-auto;
  transition: opacity 0.16s ease;
}
.drop-content.show { opacity: 1; }

/* Zone haute de la view : padding réservé seulement en mode "relative" (sélecteur descendant). */
.view { width: 100%; height: 100%; box-sizing: border-box; padding-top: 0; }
.sz-relative .view { padding-top: var(--safe-top); }
/* Pile / centre de notifs : layout en classes ; ici seulement les scrollbars custom
   (pseudo-éléments ::-webkit-scrollbar non inlinables). */
.nstack::-webkit-scrollbar, .nlist::-webkit-scrollbar { width: 6px; }
.nstack::-webkit-scrollbar-thumb, .nlist::-webkit-scrollbar-thumb { @apply rounded-full bg-foreground/20; }
/* Cartes (TransitionGroup) — s'applique à la racine .notif de NotifCard. */
.ncard-move, .ncard-enter-active, .ncard-leave-active { transition: opacity 0.28s ease, transform 0.3s cubic-bezier(0.22, 1, 0.36, 1); }
.ncard-enter-from { opacity: 0; transform: translateY(-10px); }
.ncard-leave-to { opacity: 0; transform: translateX(24px); }
.ncard-leave-active { position: absolute; left: 6px; right: 6px; }

/* Indicateurs animés du centre idle : layout en classes ; ici seulement les animations
   (+ keyframes), et les délais par enfant via :nth-child (non exprimables en classe). */
.dot { animation: breathe 2s ease-in-out infinite; }
.rec { animation: recpulse 1.5s ease-out infinite; }
@keyframes recpulse {
  0%   { box-shadow: 0 0 0 0 rgba(239,68,68,.55); }
  70%  { box-shadow: 0 0 0 6px rgba(239,68,68,0); }
  100% { box-shadow: 0 0 0 0 rgba(239,68,68,0); }
}
@keyframes breathe { 0%,100%{opacity:.5;transform:scale(.85)} 50%{opacity:1;transform:scale(1.15)} }
</style>
