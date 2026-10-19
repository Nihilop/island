<script setup lang="ts">
// Pinceau de masque, posé sur l'aperçu : on peint la zone (affichée teintée) d'un masque de
// calque ou d'effet, dans le repère du calque (`box` = son rectangle en px CSS, rotation
// comprise). Le masque est tenu dans le canal alpha d'un canvas (1024 px sur le grand
// côté) ; à l'export, alpha → niveaux de gris (blanc = zone).
//
// Raccourcis : B pinceau, E gomme, [ ] taille, Ctrl+Z / Ctrl+Y annuler / rétablir, Entrée
// terminer, Échap annuler.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { PhArrowClockwise, PhArrowCounterClockwise, PhCheck, PhEraser, PhPaintBrush, PhSwap, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { Button } from "@/sdk/ui/button";
import { Slider } from "@/sdk/ui/slider";
import { t } from "../notch/i18n";

const props = defineProps<{
  box: { x: number; y: number; w: number; h: number; rot: number };
  /** Masque existant (niveaux de gris) ; null = nouveau. */
  source: ImageBitmap | null;
  /** Nouveau masque : plein (tout visible) ou vide (rien). */
  full: boolean;
  title: string;
}>();
const emit = defineEmits<{ live: [bitmap: ImageBitmap]; done: [png: string, bitmap: ImageBitmap]; cancel: [] }>();

const LONG = 1024;
const W = props.box.w >= props.box.h ? LONG : Math.max(16, Math.round((LONG * props.box.w) / props.box.h));
const H = props.box.h > props.box.w ? LONG : Math.max(16, Math.round((LONG * props.box.h) / props.box.w));

const canvas = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;

const tool = ref<"paint" | "erase">("paint");
const size = ref(120);
const hardness = ref(0.5);
const opacity = ref(0.8);
const busy = ref(false);

const undo: ImageData[] = [];
const redo: ImageData[] = [];
const canUndo = ref(false);
const canRedo = ref(false);
const sync = () => ((canUndo.value = undo.length > 0), (canRedo.value = redo.length > 0));

// Teinte d'affichage (le masque lui-même est l'alpha).
const TINT = "255, 72, 96";

function snapshot() {
  if (!ctx) return;
  undo.push(ctx.getImageData(0, 0, W, H));
  if (undo.length > 30) undo.shift();
  redo.length = 0;
  sync();
}

function restore(from: ImageData[], to: ImageData[]) {
  if (!ctx || !from.length) return;
  to.push(ctx.getImageData(0, 0, W, H));
  ctx.putImageData(from.pop()!, 0, 0);
  sync();
  void pushLive(true);
}

/** Pixel du canvas sous le pointeur (rotation du calque compensée). */
function local(e: PointerEvent): [number, number] {
  const r = canvas.value!.getBoundingClientRect();
  const cx = r.left + r.width / 2;
  const cy = r.top + r.height / 2;
  const a = (-props.box.rot * Math.PI) / 180;
  const dx = e.clientX - cx;
  const dy = e.clientY - cy;
  const x = dx * Math.cos(a) - dy * Math.sin(a) + props.box.w / 2;
  const y = dx * Math.sin(a) + dy * Math.cos(a) + props.box.h / 2;
  return [(x * W) / props.box.w, (y * H) / props.box.h];
}

function stamp(x: number, y: number) {
  if (!ctx) return;
  const r = size.value / 2;
  const g = ctx.createRadialGradient(x, y, r * Math.min(hardness.value, 0.98), x, y, r);
  const a = opacity.value * 0.35;
  g.addColorStop(0, `rgba(${TINT}, ${a})`);
  g.addColorStop(1, `rgba(${TINT}, 0)`);
  ctx.globalCompositeOperation = tool.value === "paint" ? "source-over" : "destination-out";
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fill();
}

let drawing = false;
let last: [number, number] | null = null;

function down(e: PointerEvent) {
  e.stopPropagation();
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  snapshot();
  drawing = true;
  last = local(e);
  stamp(...last);
}

const cursor = ref<{ x: number; y: number } | null>(null);
function move(e: PointerEvent) {
  const host = canvas.value?.parentElement?.getBoundingClientRect();
  if (host) cursor.value = { x: e.clientX - host.left, y: e.clientY - host.top };
  if (!drawing || !last) return;
  const p = local(e);
  const d = Math.hypot(p[0] - last[0], p[1] - last[1]);
  const step = Math.max(1, size.value * 0.15);
  for (let i = step; i <= d; i += step) {
    const k = i / d;
    stamp(last[0] + (p[0] - last[0]) * k, last[1] + (p[1] - last[1]) * k);
  }
  if (d >= step) last = p;
  void pushLive();
}

function up() {
  drawing = false;
  last = null;
  void pushLive(true);
}

// --- Export : alpha → niveaux de gris ------------------------------------------------------

function grayscale(): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = W;
  out.height = H;
  const src = ctx!.getImageData(0, 0, W, H);
  const o = out.getContext("2d")!;
  const img = o.createImageData(W, H);
  for (let i = 0; i < src.data.length; i += 4) {
    const v = src.data[i + 3];
    img.data[i] = img.data[i + 1] = img.data[i + 2] = v;
    img.data[i + 3] = 255;
  }
  o.putImageData(img, 0, 0);
  return out;
}

let liveAt = 0;
let liveTimer: number | undefined;
/** Met à jour l'aperçu (au plus ~8 fois par seconde pendant le tracé). */
async function pushLive(now = false) {
  window.clearTimeout(liveTimer);
  const wait = now ? 0 : Math.max(0, 120 - (performance.now() - liveAt));
  liveTimer = window.setTimeout(async () => {
    liveAt = performance.now();
    emit("live", await createImageBitmap(grayscale()));
  }, wait);
}

function fill(full: boolean) {
  if (!ctx) return;
  snapshot();
  ctx.globalCompositeOperation = "source-over";
  ctx.clearRect(0, 0, W, H);
  if (full) {
    ctx.fillStyle = `rgb(${TINT})`;
    ctx.fillRect(0, 0, W, H);
  }
  void pushLive(true);
}

function invert() {
  if (!ctx) return;
  snapshot();
  const d = ctx.getImageData(0, 0, W, H);
  for (let i = 0; i < d.data.length; i += 4) {
    d.data[i] = 255;
    d.data[i + 1] = 72;
    d.data[i + 2] = 96;
    d.data[i + 3] = 255 - d.data[i + 3];
  }
  ctx.putImageData(d, 0, 0);
  void pushLive(true);
}

async function done() {
  if (busy.value || !ctx) return;
  busy.value = true;
  const gray = grayscale();
  const blob = await new Promise<Blob | null>((r) => gray.toBlob(r, "image/png"));
  const bitmap = await createImageBitmap(gray);
  if (!blob) return void (busy.value = false);
  const b64 = await new Promise<string>((resolve) => {
    const fr = new FileReader();
    fr.onload = () => resolve(String(fr.result).replace(/^data:[^,]*,/, ""));
    fr.readAsDataURL(blob);
  });
  emit("done", b64, bitmap);
}

function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement | null)?.closest("input, textarea")) return;
  const k = e.key.toLowerCase();
  if ((e.ctrlKey || e.metaKey) && k === "z" && !e.shiftKey) restore(undo, redo);
  else if ((e.ctrlKey || e.metaKey) && (k === "y" || (k === "z" && e.shiftKey))) restore(redo, undo);
  else if (k === "b") tool.value = "paint";
  else if (k === "e") tool.value = "erase";
  else if (k === "[") size.value = Math.max(4, Math.round(size.value / 1.25));
  else if (k === "]") size.value = Math.min(600, Math.round(size.value * 1.25));
  else if (k === "enter") void done();
  else if (k === "escape") emit("cancel");
  else return;
  e.preventDefault();
  e.stopPropagation();
}

/** Diamètre du pinceau à l'écran (le cercle qui suit le curseur). */
const cursorSize = computed(() => (size.value * props.box.w) / W);

onMounted(() => {
  const c = canvas.value!;
  c.width = W;
  c.height = H;
  ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return;
  if (props.source) {
    // Niveaux de gris → alpha.
    ctx.drawImage(props.source, 0, 0, W, H);
    const d = ctx.getImageData(0, 0, W, H);
    for (let i = 0; i < d.data.length; i += 4) {
      const v = d.data[i];
      d.data[i] = 255;
      d.data[i + 1] = 72;
      d.data[i + 2] = 96;
      d.data[i + 3] = v;
    }
    ctx.putImageData(d, 0, 0);
  } else if (props.full) {
    ctx.fillStyle = `rgb(${TINT})`;
    ctx.fillRect(0, 0, W, H);
  }
  void pushLive(true);
  window.addEventListener("keydown", onKey, true);
});

onBeforeUnmount(() => {
  window.clearTimeout(liveTimer);
  window.removeEventListener("keydown", onKey, true);
});
</script>

<template>
  <div class="absolute inset-0 cursor-none" @pointerleave="cursor = null">
    <canvas
      ref="canvas"
      class="absolute opacity-50"
      :style="{ left: `${box.x}px`, top: `${box.y}px`, width: `${box.w}px`, height: `${box.h}px`, transform: `rotate(${box.rot}deg)` }"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="up"
      @pointercancel="up"
    />
    <!-- Contour de la zone peignable (le calque) -->
    <div class="pointer-events-none absolute border border-dashed border-white/60" :style="{ left: `${box.x}px`, top: `${box.y}px`, width: `${box.w}px`, height: `${box.h}px`, transform: `rotate(${box.rot}deg)` }" />
    <!-- Pinceau -->
    <div
      v-if="cursor"
      class="pointer-events-none absolute rounded-full border-2"
      :class="tool === 'paint' ? 'border-white' : 'border-white/60 border-dashed'"
      :style="{ left: `${cursor.x - cursorSize / 2}px`, top: `${cursor.y - cursorSize / 2}px`, width: `${cursorSize}px`, height: `${cursorSize}px` }"
    />

    <!-- Barre d'outils -->
    <div class="absolute top-2 left-1/2 flex -translate-x-1/2 cursor-default items-center gap-1.5 rounded-[12px] border border-white/10 bg-black/75 px-2 py-1.5 text-white shadow-xl backdrop-blur" @pointerdown.stop @pointermove.stop>
      <span class="type-caption max-w-[140px] truncate px-1 font-medium">{{ title }}</span>
      <div class="h-4 w-px bg-white/20" />
      <IconButton :icon="PhPaintBrush" :label="t('wall.brush.paint')" :active="tool === 'paint'" class="text-white!" @click="tool = 'paint'" />
      <IconButton :icon="PhEraser" :label="t('wall.brush.erase')" :active="tool === 'erase'" class="text-white!" @click="tool = 'erase'" />
      <label class="type-caption flex items-center gap-1.5 pl-1">{{ t("wall.brush.size") }}<Slider class="w-20" :model-value="[size]" :min="4" :max="600" :step="2" @update:model-value="(v) => v && (size = v[0])" /></label>
      <label class="type-caption flex items-center gap-1.5">{{ t("wall.brush.hardness") }}<Slider class="w-14" :model-value="[hardness]" :min="0" :max="1" :step="0.05" @update:model-value="(v) => v && (hardness = v[0])" /></label>
      <label class="type-caption flex items-center gap-1.5">{{ t("wall.brush.opacity") }}<Slider class="w-14" :model-value="[opacity]" :min="0.05" :max="1" :step="0.05" @update:model-value="(v) => v && (opacity = v[0])" /></label>
      <div class="h-4 w-px bg-white/20" />
      <IconButton :icon="PhArrowCounterClockwise" :label="t('notes.undo')" :disabled="!canUndo" class="text-white!" @click="restore(undo, redo)" />
      <IconButton :icon="PhArrowClockwise" :label="t('notes.redo')" :disabled="!canRedo" class="text-white!" @click="restore(redo, undo)" />
      <IconButton :icon="PhSwap" :label="t('wall.brush.invert')" class="text-white!" @click="invert" />
      <Button size="sm" variant="ghost" class="text-white/80!" @click="fill(true)">{{ t("wall.brush.fill") }}</Button>
      <Button size="sm" variant="ghost" class="text-white/80!" @click="fill(false)">{{ t("wall.brush.clear") }}</Button>
      <div class="h-4 w-px bg-white/20" />
      <IconButton :icon="PhX" :label="t('common.cancel')" class="text-white!" @click="emit('cancel')" />
      <Button size="sm" variant="default" :loading="busy" @click="done"><PhCheck />{{ t("wall.brush.done") }}</Button>
    </div>
  </div>
</template>
