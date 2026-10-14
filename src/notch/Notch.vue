<script setup lang="ts">
// Le notch : une forme noire collée en haut-centre de la fenêtre fixe (760 × 560).
// Ses 4 dimensions (w, h, r, ear) sont des valeurs animées indépendantes, pilotées par
// springs interruptibles (la vitesse est conservée lors des allers-retours de survol),
// écrites en variables CSS hors du cycle de rendu Vue. La zone de hit (forme + marge)
// est republiée à Rust à chaque frame où elle change.

import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { animate, motionValue, type MotionValue } from "motion-v";
import { invoke } from "@tauri-apps/api/core";
import { spring, type SpringName } from "../design/motion";
import { HIT_MARGIN, SHAPES, WINDOW, openShape, snapEven, type Shape } from "./geometry";
import { activeTool, hold, initNotchState, liveTool, open, state, type NotchState } from "./state";
import { settings } from "./settings";
import { display, initDisplay, setDisplay } from "./display";
import { sizeRequest, tools } from "../tools/registry";
import { initTray } from "./tray";
import { initPortals, portalRects } from "./portals";
import { np } from "../tools/media/store";
import { fmt as fmtTimer, left as timerLeft, ringing, running as timerRunning, timer } from "../tools/timer/store";
import { PhAlarm, PhCheck, PhTimer } from "@phosphor-icons/vue";
import { t } from "./i18n";
import { PhMusicNotes } from "@phosphor-icons/vue";
import Equalizer from "../ui/Equalizer.vue";
import Header from "./Header.vue";
import { privacy } from "./presentation";
import { fps, gamingActive } from "../tools/system/gaming";
import { hw, tone } from "../tools/system/store";
import { PhLockSimple, PhSignOut } from "@phosphor-icons/vue";
import { discord, inCall, meSpeaking, someoneSpeaking, initials } from "../tools/discord/store";
import { PhMicrophoneSlash, PhSpeakerSimpleSlash } from "@phosphor-icons/vue";
import Dock from "./Dock.vue";
import { PhBell } from "@phosphor-icons/vue";
import { notifs } from "../tools/notifications/store";
import { extLive, liveIconUrl } from "../extensions/slots";

const rootEl = ref<HTMLElement>();

const tool = computed(() => tools.value.find((x) => x.id === activeTool.value));
const openSize = computed(() => {
  const req = sizeRequest.value;
  if (req && req.tool === activeTool.value) return req.size;
  return tool.value?.size ?? { w: 520, h: 252 };
});

/** Le contenu d'un outil est posé à sa taille FINALE (centré) : pendant le morph, la
 *  forme le découvre sans le faire refluer. Header et dock, eux, suivent la forme. */
const HEADER_H = 36;
const DOCK_H = 46;
const bare = computed(() => !!tool.value?.bare);
const viewStyle = computed(() => {
  const s = openShape(openSize.value);
  return { width: `${s.w}px`, height: `${bare.value ? s.h : s.h - HEADER_H - DOCK_H}px` };
});

/** Forme « ligne » : style fermé ligne, ou mode focus rendu en ligne. */
const lineMode = computed(() => (settings.focusMode && settings.focusStyle === "line") || settings.idleStyle === "line");

/** Live activity (notch fermé) : une seule à la fois, par priorité — alerte de minuteur >
 *  notification > gaming > minuteur > appel > musique. Rien en style « ligne » sauf les
 *  alertes (minuteur terminé, notification). */
const live = computed<"notifications" | "gaming" | "timer" | "discord" | "media" | "ext" | null>(() => {
  if (timer.finished) return "timer"; // alerte : même en style « ligne »
  if (notifs.banner) return "notifications";
  if (lineMode.value) return null;
  if (gamingActive.value) return "gaming";
  // Extensions : « high » avant le minuteur / l'appel / la musique, sinon après.
  if (extLive.value?.priority === "high") return "ext";
  if (timerRunning.value) return "timer";
  if (inCall.value) return "discord";
  if (np.active && np.playing) return "media";
  if (extLive.value) return "ext";
  return null;
});
const EXT_COLOR = { label: "var(--label)", green: "var(--sys-green)", orange: "var(--sys-orange)", red: "var(--sys-red)", blue: "var(--sys-link)" } as const;
const ramPct = computed(() => (hw.last && hw.last.memTotal ? (hw.last.memUsed / hw.last.memTotal) * 100 : null));
const pct = (v: number | null | undefined) => (v === null || v === undefined ? "–" : `${Math.round(v)}%`);
const fpsColor = computed(() => {
  const f = fps.value;
  if (f === null) return "var(--label)";
  return f < 30 ? "var(--sys-red)" : f < 50 ? "var(--sys-orange)" : "var(--label)";
});
/** Minuteur terminé en style ligne : la forme repasse en notch le temps de l'alerte. */
const alert = computed(() => timer.finished && lineMode.value);
// Outil ouvert « à froid » sur la live activity (extension : l'outil qu'elle désigne).
watch(
  () => (live.value === "ext" ? (extLive.value?.tool ?? null) : live.value),
  (id) => (liveTool.value = id),
  { immediate: true },
);

/** Bannière de notification : forme complète, ou forme « live » pour le style minimal. */
const banner = computed<Shape | null>(() =>
  live.value === "notifications" ? (settings.notifStyle === "full" ? SHAPES.notif : SHAPES.live) : null,
);

const targetShape = computed<Shape>(() => {
  if (state.value === "open") return openShape(openSize.value);
  if (banner.value) {
    const b = banner.value;
    return state.value === "peek" ? { ...b, w: b.w + 16, h: b.h + 4, r: b.r + 2 } : b;
  }
  if (settings.focusMode)
    return settings.focusStyle === "line" ? SHAPES.line : live.value === "gaming" ? SHAPES.gaming : live.value ? SHAPES.live : SHAPES.closed;
  if (alert.value) return state.value === "peek" ? { ...SHAPES.peek, w: SHAPES.live.w + 16 } : SHAPES.live;
  if (state.value === "peek") {
    if (lineMode.value) return SHAPES.linePeek;
    // Survol depuis une live activity : on garde sa largeur + l'écart peek − closed.
    const base = live.value === "gaming" ? SHAPES.gaming.w : SHAPES.live.w;
    return live.value ? { ...SHAPES.peek, w: base + (SHAPES.peek.w - SHAPES.closed.w) } : SHAPES.peek;
  }
  if (lineMode.value) return SHAPES.line;
  if (live.value === "gaming") return SHAPES.gaming;
  return live.value ? SHAPES.live : SHAPES.closed;
});

const isLineShape = computed(() => state.value !== "open" && lineMode.value && !alert.value && !banner.value);

const mv: Record<keyof Shape, MotionValue<number>> = {
  w: motionValue(SHAPES.closed.w),
  h: motionValue(SHAPES.closed.h),
  r: motionValue(SHAPES.closed.r),
  ear: motionValue(SHAPES.closed.ear),
};

/** Ressort selon la transition : open en arrivant ouvert, morph si déjà ouvert,
 *  peek en survol, close en quittant l'état ouvert, sinon morph. */
function pickSpring(from: NotchState, to: NotchState): SpringName {
  if (to === "open") return from === "open" ? "morph" : "open";
  if (to === "peek") return "peek";
  return from === "open" ? "close" : "morph";
}

// État lors du dernier changement de forme : choisit le ressort de la transition.
let prevState: NotchState = state.value;
/** Zone collante : taille minimale de la zone de hit (largeur, hauteur), 0 = inactive.
 *  Quand la forme rétrécit (changement d'onglet), la zone garde l'ancienne taille tant
 *  que le curseur n'est pas revenu sur la forme réelle : sinon il se retrouve dehors et
 *  le notch se ferme. */
let stickyW = 0;
let stickyH = 0;

// La transition ne change que si la forme cible change réellement.
watch(targetShape, (shape, old) => {
  if (old && shape.w === old.w && shape.h === old.h && shape.r === old.r && shape.ear === old.ear) return;
  // Zone « collante » : un onglet plus petit ne doit pas faire sortir le curseur.
  if (state.value === "open" && prevState === "open" && old) {
    if (shape.h < old.h) stickyH = Math.max(stickyH, old.h + HIT_MARGIN.normal.bottom + 12);
    if (shape.w < old.w) stickyW = Math.max(stickyW, old.w + HIT_MARGIN.normal.side * 2 + 24);
  }
  const opts = spring(pickSpring(prevState, state.value));
  prevState = state.value;
  for (const k of Object.keys(mv) as (keyof Shape)[]) {
    if (mv[k].get() !== shape[k]) animate(mv[k], shape[k], opts);
  }
});

watch(state, (s) => {
  if (s !== "open") stickyW = stickyH = 0;
});

// --- Écriture des variables CSS + zone de hit, une fois par frame -----------------
let raf = 0;
let lastHit = "";

function flush() {
  raf = 0;
  const el = rootEl.value;
  if (!el) return;
  const w = snapEven(mv.w.get());
  const h = snapEven(mv.h.get());
  el.style.setProperty("--w", `${w}px`);
  el.style.setProperty("--h", `${h}px`);
  el.style.setProperty("--r", `${mv.r.get()}px`);
  el.style.setProperty("--ear", `${mv.ear.get()}px`);
  // Les côtés de la live activity suivent les bords de la forme par transform (pas par
  // layout : sinon les fins de spring « rampent » par pas de 1 px).
  el.style.setProperty("--edge", `${(w - (live.value === "gaming" ? SHAPES.gaming.w : SHAPES.live.w)) / 2}px`);

  const m = isLineShape.value ? HIT_MARGIN.line : HIT_MARGIN.normal;
  const hitW = Math.ceil(Math.max(w + m.side * 2, stickyW));
  const hit = {
    x: Math.floor(WINDOW.w / 2 - hitW / 2),
    y: 0,
    w: hitW,
    h: Math.ceil(Math.max(h + m.bottom, stickyH)),
  };
  // Menus / tooltips téléportés hors du notch : zones de hit en plus.
  const extra = portalRects();
  const key = `${hit.x},${hit.w},${hit.h}|${extra.map((r) => `${r.x},${r.y},${r.w},${r.h}`).join(";")}`;
  if (key !== lastHit) {
    lastHit = key;
    el.style.setProperty("--hit-w", `${hit.w}px`);
    el.style.setProperty("--hit-h", `${hit.h}px`);
    invoke("set_hit_regions", { regions: [hit, ...extra] }).catch(() => {});
  }
}

function schedule() {
  if (!raf) raf = requestAnimationFrame(flush);
}

const unsubs = Object.values(mv).map((v) => v.on("change", schedule));
watch(isLineShape, schedule);

/** Le curseur revient sur la forme réelle → fin de la zone collante. */
function onPointerMove(e: PointerEvent) {
  if (stickyW || stickyH) {
    const half = mv.w.get() / 2;
    const cx = WINDOW.w / 2;
    if (e.clientY < mv.h.get() && e.clientX > cx - half && e.clientX < cx + half) {
      stickyW = stickyH = 0;
      schedule();
    }
  }
  if (drag) onDragMove(e);
}

// --- Drag horizontal : fermé = n'importe où ; ouvert = espace vide du header -------
const DRAG_THRESHOLD = 6;
const SNAP = 24;
let drag: { id: number; x0: number; o0: number; moving: boolean; release?: () => void } | null = null;
let suppressClick = false;
let dragRaf = 0;

const NO_DRAG = "button, input, textarea, select, a, [role=slider], [data-no-drag]";

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0 || settings.focusMode) return;
  const el = e.target as HTMLElement;
  if (el.closest(NO_DRAG)) return;
  if (state.value === "open" && !el.closest("[data-drag-handle]")) return;
  drag = { id: e.pointerId, x0: e.screenX, o0: display.offset, moving: false };
}

function onDragMove(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return;
  const dx = e.screenX - drag.x0;
  if (!drag.moving) {
    if (Math.abs(dx) < DRAG_THRESHOLD) return;
    drag.moving = true;
    drag.release = hold();
    rootEl.value?.setPointerCapture(e.pointerId);
    rootEl.value?.classList.add("grabbing");
  }
  const o = drag.o0 + dx;
  display.offset = Math.abs(o) < SNAP ? 0 : Math.round(o);
  if (!dragRaf)
    dragRaf = requestAnimationFrame(() => {
      dragRaf = 0;
      setDisplay({}, false);
    });
}

function onPointerUp(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return;
  if (drag.moving) {
    suppressClick = true;
    setDisplay({}, true);
    drag.release?.();
    rootEl.value?.classList.remove("grabbing");
  }
  drag = null;
}

function onClick() {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  if (state.value !== "open") open();
}

onMounted(() => {
  initNotchState();
  initDisplay();
  flush();
  initTray();
  initPortals(schedule);
});
onBeforeUnmount(() => unsubs.forEach((u) => u()));
</script>

<template>
  <!-- Racine : `grabbing` (posée par le drag) force le curseur sur tout l'arbre. -->
  <div
    ref="rootEl"
    class="notch-root pointer-events-none fixed inset-0 transition-opacity duration-200 ease-out [&.grabbing]:cursor-grabbing! [&.grabbing_*]:cursor-grabbing!"
    :class="{ 'opacity-50': settings.focusMode && settings.focusStyle === 'dim' }"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    @click="onClick"
  >
    <!-- Zone de hit (forme + marge) : capte les clics quand Rust lève le click-through. -->
    <div class="pointer-events-auto absolute top-0 left-1/2 h-(--hit-h) w-(--hit-w) -translate-x-1/2"></div>
    <!-- Voyants de confidentialité : à droite de la « caméra » (centre + 24 px). -->
    <TransitionGroup
      v-if="settings.privacyDots && !isLineShape && !(state === 'open' && bare)"
      tag="div"
      class="pointer-events-none absolute top-0 left-[calc(50%_+_24px)] z-3 flex h-[30px] items-center gap-[5px]"
      enter-active-class="transition-[opacity,scale] duration-200 ease-out"
      enter-from-class="opacity-0 scale-90"
      leave-active-class="transition-[opacity,scale] duration-200 ease-out"
      leave-to-class="opacity-0 scale-90"
    >
      <span
        v-if="privacy.mic || privacy.camera || (inCall && !discord.mute)"
        key="g"
        class="size-1.5 rounded-full bg-sys-green"
        :class="{ 'animate-talk': meSpeaking }"
      />
      <span v-if="privacy.screen" key="r" class="size-1.5 rounded-full bg-sys-red" />
    </TransitionGroup>
    <div class="pointer-events-none absolute top-0 left-1/2 h-(--h) w-(--w) -translate-x-1/2">
      <!-- Ombre double, visible seulement ouvert (fondu 300 ms). -->
      <div
        class="absolute inset-0 rounded-b-(--r) shadow-(--notch-shadow) transition-opacity duration-300 ease-out"
        :class="state === 'open' ? 'opacity-100' : 'opacity-0'"
      ></div>
      <!-- Oreilles : carré noir dont un quart de cercle est transparent → raccord concave
           avec le bord haut de l'écran. Chevauchement de 0,5 px avec le corps. -->
      <div
        class="absolute top-0 right-[calc(100%_-_0.5px)] size-(--ear) [background:radial-gradient(circle_at_0_100%,transparent_calc(var(--ear)_-_0.75px),var(--notch)_var(--ear))]"
      ></div>
      <div
        class="absolute top-0 left-[calc(100%_-_0.5px)] size-(--ear) [background:radial-gradient(circle_at_100%_100%,transparent_calc(var(--ear)_-_0.75px),var(--notch)_var(--ear))]"
      ></div>
      <div class="pointer-events-auto absolute inset-0 overflow-clip rounded-b-(--r) bg-notch">
        <!-- Live activity : boîte de la taille du notch « live », centrée ; les côtés suivent
             les bords de la forme via --edge. -->
        <Transition
          enter-active-class="transition-[opacity,filter] duration-200 ease-out"
          enter-from-class="opacity-0 blur-[4px]"
          leave-active-class="transition-[opacity,filter] duration-200 ease-out"
          leave-to-class="opacity-0 blur-[4px]"
        >
          <div
            v-if="state !== 'open' && live === 'notifications' && settings.notifStyle === 'full'"
            :key="`n${notifs.banner!.id}`"
            class="absolute top-0 left-1/2 flex h-[72px] w-[400px] -translate-x-1/2 items-center gap-2.5 px-4 pt-1"
          >
            <img v-if="notifs.banner!.icon" :src="notifs.banner!.icon" alt="" class="size-8 shrink-0 rounded-[8px] object-contain" />
            <span v-else class="grid size-8 shrink-0 place-items-center rounded-[8px] bg-fill-3"><PhBell :size="15" weight="fill" /></span>
            <div class="min-w-0 flex-1 leading-tight">
              <div class="type-caption truncate text-label-3">{{ notifs.banner!.app }}</div>
              <div class="type-footnote truncate font-medium">{{ notifs.banner!.title || notifs.banner!.app }}</div>
              <div v-if="notifs.banner!.body" class="type-caption truncate text-label-2">{{ notifs.banner!.body }}</div>
            </div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'notifications'"
            :key="`m${notifs.banner!.id}`"
            class="absolute top-0 left-1/2 flex h-[30px] w-[284px] -translate-x-1/2 items-center justify-between px-[9px]"
          >
            <div class="relative grid size-5 translate-x-[calc(-1*var(--edge,0px))] place-items-center">
              <img v-if="notifs.banner!.icon" :src="notifs.banner!.icon" alt="" class="size-5 rounded-[5px] object-contain" />
              <PhBell v-else :size="14" weight="fill" />
            </div>
            <div class="grid size-5 translate-x-[var(--edge,0px)] place-items-center">
              <span class="size-2 rounded-full bg-sys-link shadow-[0_0_8px_var(--sys-link)] animate-pulse" />
            </div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'ext' && extLive"
            class="absolute top-0 left-1/2 flex h-[30px] w-[284px] -translate-x-1/2 items-center justify-between px-[11px]"
            :style="{ color: EXT_COLOR[extLive.color ?? 'label'] }"
          >
            <div class="relative grid size-5 translate-x-[calc(-1*var(--edge,0px))] place-items-center">
              <span
                v-if="liveIconUrl(extLive)"
                class="size-3.5 bg-current [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain]"
                :style="{ maskImage: `url('${liveIconUrl(extLive)}')` }"
              />
            </div>
            <div class="type-footnote max-w-[48px] translate-x-[var(--edge,0px)] truncate font-semibold tabular-nums">{{ extLive.text }}</div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'media'"
            class="absolute top-0 left-1/2 flex h-[30px] w-[284px] -translate-x-1/2 items-center justify-between px-[9px]"
          >
            <div class="relative grid size-5 translate-x-[calc(-1*var(--edge,0px))] place-items-center">
              <Transition
                enter-active-class="transition-[opacity,scale,filter] duration-450 ease-in-out"
                enter-from-class="opacity-0 scale-[0.94] blur-[3px]"
                leave-active-class="transition-[opacity,filter] duration-300 ease-out delay-200"
                leave-to-class="opacity-0 blur-[2px]"
              >
                <img v-if="np.art" :key="np.art" :src="np.art" alt="" class="absolute inset-0 size-5 rounded-[5px] object-cover" />
                <PhMusicNotes v-else :size="14" weight="fill" class="text-tint" />
              </Transition>
            </div>
            <div class="translate-x-[var(--edge,0px)]"><Equalizer /></div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'gaming'"
            class="absolute top-0 left-1/2 flex h-[30px] w-[436px] -translate-x-1/2 items-center justify-between px-3"
          >
            <div class="relative flex translate-x-[calc(-1*var(--edge,0px))] items-center gap-3">
              <span class="inline-flex items-baseline gap-1">
                <span class="inline-flex min-w-[26px] justify-end text-[12px] font-semibold tabular-nums" :style="{ color: fpsColor }">
                  <PhLockSimple v-if="fps.status === 'noAdmin'" :size="11" weight="fill" class="text-label-3" />
                  <PhSignOut v-else-if="fps.status === 'relogin'" :size="11" weight="bold" class="text-sys-orange" />
                  <template v-else>{{ fps.value === null ? "–" : Math.round(fps.value) }}</template>
                </span>
                <span class="text-[11px] text-label-3">FPS</span>
              </span>
              <span class="inline-flex items-baseline gap-1">
                <span class="text-[11px] text-label-3">CPU</span>
                <span class="text-[12px] font-semibold tabular-nums" :style="{ color: tone(hw.last?.cpu) }">{{ pct(hw.last?.cpu) }}</span>
              </span>
            </div>
            <div class="flex translate-x-[var(--edge,0px)] items-center gap-3">
              <span class="inline-flex items-baseline gap-1">
                <span class="text-[11px] text-label-3">GPU</span>
                <span class="text-[12px] font-semibold tabular-nums" :style="{ color: tone(hw.last?.gpu) }">{{ pct(hw.last?.gpu) }}</span>
              </span>
              <span class="inline-flex items-baseline gap-1">
                <span class="text-[11px] text-label-3">RAM</span>
                <span class="text-[12px] font-semibold tabular-nums" :style="{ color: tone(ramPct) }">{{ pct(ramPct) }}</span>
              </span>
            </div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'discord'"
            class="absolute top-0 left-1/2 flex h-[30px] w-[284px] -translate-x-1/2 items-center justify-between px-[9px]"
          >
            <div class="relative grid size-5 translate-x-[calc(-1*var(--edge,0px))] place-items-center">
              <img v-if="discord.call?.guildIcon" :src="discord.call.guildIcon" alt="" class="absolute inset-0 size-5 rounded-[5px] object-cover" />
              <span
                v-else
                class="absolute inset-0 grid size-5 place-items-center rounded-[5px] bg-fill-3 text-[8px] font-semibold text-label"
              >{{ initials(discord.call?.guild ?? null) || "DM" }}</span>
            </div>
            <div class="translate-x-[var(--edge,0px)]">
              <PhSpeakerSimpleSlash v-if="discord.deaf" :size="14" weight="fill" class="text-sys-red" />
              <PhMicrophoneSlash v-else-if="discord.mute && !someoneSpeaking" :size="14" weight="fill" class="text-sys-red" />
              <Equalizer v-else color="var(--sys-green)" />
            </div>
          </div>
          <div
            v-else-if="state !== 'open' && live === 'timer'"
            class="absolute top-0 left-1/2 flex h-[30px] w-[284px] -translate-x-1/2 items-center justify-between px-[11px]"
            :style="{ color: timer.mode === 'focus' && !timer.finished ? 'var(--sys-orange)' : 'var(--sys-green)' }"
          >
            <div class="relative grid size-5 translate-x-[calc(-1*var(--edge,0px))] place-items-center">
              <PhAlarm v-if="ringing" :size="14" weight="fill" class="animate-throb" />
              <PhCheck v-else-if="timer.finished" :size="14" weight="bold" />
              <PhTimer v-else :size="14" weight="fill" />
            </div>
            <div class="type-footnote translate-x-[var(--edge,0px)] font-semibold tabular-nums">
              {{ timer.finished ? t("timer.done") : fmtTimer(timerLeft) }}
            </div>
          </div>
        </Transition>
        <!-- Contenu : entre 240 ms après 50 ms (la forme bouge d'abord), sort en 120 ms. Pas
             de scale (re-rasterise les petites icônes du dock). Le layout suit la forme. -->
        <Transition
          enter-active-class="transition-[opacity,translate,filter] duration-240 ease-out delay-50"
          enter-from-class="opacity-0 -translate-y-[6px] blur-[6px]"
          leave-active-class="transition-[opacity,filter] duration-120 ease-out"
          leave-to-class="opacity-0 blur-[4px]"
        >
          <div v-if="state === 'open'" class="absolute inset-0 flex flex-col">
            <Header v-if="!bare" />
            <div class="relative min-h-0 flex-1 overflow-clip">
              <!-- Changement d'outil : l'ancien sort tout de suite, le nouveau entre une fois
                   la forme lancée. -->
              <Transition
                enter-active-class="transition-[opacity,filter] duration-320 ease-out delay-80"
                enter-from-class="opacity-0 blur-[4px]"
                leave-active-class="transition-[opacity,filter] duration-180 ease-out"
                leave-to-class="opacity-0 blur-[4px]"
              >
                <component
                  :is="tool.view"
                  v-if="tool"
                  :key="tool.id"
                  class="absolute top-0 left-1/2 -translate-x-1/2"
                  :style="viewStyle"
                />
              </Transition>
            </div>
            <Dock v-if="!bare" :width="openShape(openSize).w" />
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>
