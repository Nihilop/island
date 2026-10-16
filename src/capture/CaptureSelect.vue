<script setup lang="ts">
// Sélection de capture (fenêtre `capture-select`, cf. docs/internal/capture-spec.md) :
// dessinée par Island, jamais par une extension. Fenêtre transparente sur le moniteur sous
// la souris, exclue des captures. Fond assombri, zone découpée en clair.
//  - Zone : glisser pour dessiner, poignées pour ajuster, glisser l'intérieur pour déplacer ;
//    Entrée / double-clic / « Valider » = choisir.
//  - Fenêtre : la fenêtre sous la souris est surlignée ; clic = choisir.
//  - Écran : clic = tout l'écran.
// Échap / clic droit = annuler. Coordonnées renvoyées en pixels PHYSIQUES, relatives au
// moniteur (la page travaille en px CSS : × échelle).
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhAppWindow, PhCheck, PhMonitor, PhSelection, PhX } from "@phosphor-icons/vue";
import { t } from "../notch/i18n";

type Mode = "region" | "window" | "screen";
interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}
interface Init {
  mode: Mode;
  display: { index: number; x: number; y: number; width: number; height: number; scale: number };
  windows: { hwnd: number; title: string; x: number; y: number; w: number; h: number }[];
}

const init = ref<Init | null>(null);
const mode = ref<Mode>("region");
/** Échelle physique / CSS (celle du moniteur, sinon celle de la page). */
const scale = computed(() => init.value?.display.scale || window.devicePixelRatio || 1);
const MIN = 8;

// --- Zone (px CSS) ---------------------------------------------------------------------------

const sel = reactive<Rect>({ x: 0, y: 0, w: 0, h: 0 });
const hasSel = computed(() => sel.w >= MIN && sel.h >= MIN);
type Drag = { kind: "draw" | "move" | "resize"; sx: number; sy: number; start: Rect; edge?: string };
let drag: Drag | null = null;
const dragging = ref(false);

const clampRect = (r: Rect): Rect => {
  const W = window.innerWidth;
  const H = window.innerHeight;
  const x = Math.max(0, Math.min(r.x, W));
  const y = Math.max(0, Math.min(r.y, H));
  return { x, y, w: Math.max(0, Math.min(r.w, W - x)), h: Math.max(0, Math.min(r.h, H - y)) };
};

function onDown(e: PointerEvent) {
  if (e.button === 2) return cancel();
  if (e.button !== 0) return;
  if (mode.value === "screen") return finish({ kind: "screen", display: init.value!.display.index });
  if (mode.value === "window") {
    const w = hovered.value;
    return w ? finish({ kind: "window", hwnd: w.hwnd }) : undefined;
  }
  const target = e.target as HTMLElement;
  const edge = target.dataset.edge;
  const start = { ...sel };
  if (edge) drag = { kind: "resize", sx: e.clientX, sy: e.clientY, start, edge };
  else if (target.dataset.sel !== undefined && hasSel.value) drag = { kind: "move", sx: e.clientX, sy: e.clientY, start };
  else {
    drag = { kind: "draw", sx: e.clientX, sy: e.clientY, start };
    Object.assign(sel, { x: e.clientX, y: e.clientY, w: 0, h: 0 });
  }
  dragging.value = true;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onMove(e: PointerEvent) {
  pointer.x = e.clientX;
  pointer.y = e.clientY;
  if (!drag) return;
  const dx = e.clientX - drag.sx;
  const dy = e.clientY - drag.sy;
  const s = drag.start;
  if (drag.kind === "draw") {
    Object.assign(sel, clampRect({ x: Math.min(drag.sx, e.clientX), y: Math.min(drag.sy, e.clientY), w: Math.abs(dx), h: Math.abs(dy) }));
  } else if (drag.kind === "move") {
    const x = Math.max(0, Math.min(s.x + dx, window.innerWidth - s.w));
    const y = Math.max(0, Math.min(s.y + dy, window.innerHeight - s.h));
    Object.assign(sel, { x, y });
  } else {
    let { x, y, w, h } = s;
    const ed = drag.edge!;
    if (ed.includes("w")) [x, w] = [s.x + dx, s.w - dx];
    if (ed.includes("e")) w = s.w + dx;
    if (ed.includes("n")) [y, h] = [s.y + dy, s.h - dy];
    if (ed.includes("s")) h = s.h + dy;
    // Poignée tirée au-delà du bord opposé : le rectangle se retourne proprement.
    if (w < 0) [x, w] = [x + w, -w];
    if (h < 0) [y, h] = [y + h, -h];
    Object.assign(sel, clampRect({ x, y, w, h }));
  }
}

function onUp() {
  drag = null;
  dragging.value = false;
}

const physical = computed(() => ({
  x: Math.round(sel.x * scale.value),
  y: Math.round(sel.y * scale.value),
  w: Math.round(sel.w * scale.value),
  h: Math.round(sel.h * scale.value),
}));

function confirmRegion() {
  if (!hasSel.value || !init.value) return;
  finish({ kind: "region", display: init.value.display.index, rect: physical.value });
}

// --- Fenêtres --------------------------------------------------------------------------------

const pointer = reactive({ x: -1, y: -1 });
/** Fenêtre la plus haute (ordre Z) sous la souris, en px CSS. */
const hovered = computed(() => {
  if (mode.value !== "window" || !init.value) return null;
  const px = pointer.x * scale.value;
  const py = pointer.y * scale.value;
  return init.value.windows.find((w) => px >= w.x && py >= w.y && px < w.x + w.w && py < w.y + w.h) ?? null;
});
const hoveredCss = computed(() => {
  const w = hovered.value;
  if (!w) return null;
  const s = scale.value;
  return clampRect({ x: w.x / s, y: w.y / s, w: w.w / s, h: w.h / s });
});

// --- Fin -------------------------------------------------------------------------------------

let done = false;
function finish(result: unknown) {
  if (done) return;
  done = true;
  void invoke("capture_select_done", { result }).catch(() => {});
}
const cancel = () => finish(null);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") cancel();
  else if (e.key === "Enter" && mode.value === "region") confirmRegion();
  else if (e.key === "1") mode.value = "region";
  else if (e.key === "2") mode.value = "window";
  else if (e.key === "3") mode.value = "screen";
}

/** Rectangle « découpé » affiché (zone, fenêtre survolée, ou tout l'écran). */
const cut = computed<Rect | null>(() => {
  if (mode.value === "screen") return { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  if (mode.value === "window") return hoveredCss.value;
  return hasSel.value || dragging.value ? { ...sel } : null;
});

const label = computed(() => {
  if (mode.value === "region") return `${physical.value.w} × ${physical.value.h}`;
  if (mode.value === "window") return hovered.value?.title || "";
  const d = init.value?.display;
  return d ? `${d.width} × ${d.height}` : "";
});

const MODES: { id: Mode; icon: typeof PhSelection; key: string }[] = [
  { id: "region", icon: PhSelection, key: "1" },
  { id: "window", icon: PhAppWindow, key: "2" },
  { id: "screen", icon: PhMonitor, key: "3" },
];
const EDGES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
const edgeStyle = (ed: string) => ({
  left: ed.includes("w") ? "0%" : ed.includes("e") ? "100%" : "50%",
  top: ed.includes("n") ? "0%" : ed.includes("s") ? "100%" : "50%",
  cursor: `${ed}-resize`,
});

onMounted(async () => {
  document.documentElement.classList.add("dark");
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
  window.addEventListener("keydown", onKey);
  window.addEventListener("blur", () => window.focus());
  try {
    init.value = await invoke<Init>("capture_select_init");
    mode.value = init.value.mode;
  } catch {
    cancel();
  }
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div
    class="fixed inset-0 select-none"
    :class="mode === 'region' ? 'cursor-crosshair' : 'cursor-pointer'"
    @pointerdown="onDown"
    @pointermove="onMove"
    @pointerup="onUp"
    @dblclick="mode === 'region' && confirmRegion()"
    @contextmenu.prevent
  >
    <!-- Voile : tout l'écran, ou autour du rectangle découpé (ombre géante). -->
    <div v-if="!cut" class="pointer-events-none absolute inset-0 bg-black/45" />
    <div
      v-else
      data-sel
      class="absolute shadow-[0_0_0_9999px_rgb(0_0_0/0.45)] ring-1 ring-white/90 transition-[left,top,width,height] duration-75"
      :class="[mode === 'region' && hasSel && !dragging ? 'cursor-move' : '', mode !== 'region' ? 'pointer-events-none ring-2 ring-sys-link' : '']"
      :style="{ left: `${cut.x}px`, top: `${cut.y}px`, width: `${cut.w}px`, height: `${cut.h}px` }"
    >
      <!-- Poignées (zone dessinée seulement). -->
      <template v-if="mode === 'region' && hasSel && !dragging">
        <span
          v-for="ed in EDGES"
          :key="ed"
          :data-edge="ed"
          class="absolute size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-black/40 bg-white shadow"
          :style="edgeStyle(ed)"
        />
      </template>
      <!-- Dimensions / titre. -->
      <span
        v-if="label"
        class="type-caption pointer-events-none absolute -top-7 left-0 max-w-[60vw] truncate rounded-[6px] bg-black/75 px-2 py-0.5 font-medium text-white tabular-nums"
      >{{ label }}</span>
      <!-- Valider (zone). -->
      <button
        v-if="mode === 'region' && hasSel && !dragging"
        type="button"
        class="type-footnote absolute right-0 -bottom-10 flex items-center gap-1.5 rounded-full bg-white px-3 py-1.5 font-semibold text-black shadow-lg"
        @pointerdown.stop
        @click.stop="confirmRegion"
      >
        <PhCheck :size="14" weight="bold" />{{ t("capture.select.confirm") }}
      </button>
    </div>

    <!-- Barre du haut : mode + aide. -->
    <div class="absolute top-4 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-full bg-[#1c1c1e]/95 p-1 text-white shadow-2xl ring-1 ring-white/10" @pointerdown.stop>
      <button
        v-for="m in MODES"
        :key="m.id"
        type="button"
        class="type-footnote flex h-8 items-center gap-1.5 rounded-full px-3 font-medium transition-colors"
        :class="mode === m.id ? 'bg-white text-black' : 'text-white/70 hover:bg-white/10 hover:text-white'"
        :title="m.key"
        @click="mode = m.id"
      >
        <component :is="m.icon" :size="15" :weight="mode === m.id ? 'fill' : 'bold'" />{{ t(`capture.select.${m.id}`) }}
      </button>
      <span class="type-caption px-2 text-white/50">{{ t(`capture.select.help.${mode}`) }}</span>
      <button type="button" class="grid size-8 place-items-center rounded-full text-white/70 hover:bg-white/10 hover:text-white" :title="t('common.cancel')" @click="cancel">
        <PhX :size="15" weight="bold" />
      </button>
    </div>
  </div>
</template>
