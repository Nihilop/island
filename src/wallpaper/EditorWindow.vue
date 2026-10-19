<script setup lang="ts">
// Éditeur de fonds animés (fenêtre détachée `wallpaper-editor`), à calques : outliner à
// gauche (Scène + calques), aperçu au centre (WebGPU, mêmes shaders que le fond natif) avec
// poignées pour déplacer / redimensionner un calque placé, propriétés de la sélection à
// droite. Arguments d'ouverture : `{ id }` (scène à éditer ; un préréglage s'édite en copie)
// ou rien (nouvelle scène).
import { computed, onBeforeUnmount, onMounted, ref, toRaw, watch } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { PhCheck, PhMinus, PhMonitor, PhWarning, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { Button } from "@/sdk/ui/button";
import Outliner from "./Outliner.vue";
import LayerPanel from "./LayerPanel.vue";
import ScenePanel from "./ScenePanel.vue";
import { Preview } from "./preview";
import { layerBox, layerId, MAX_LAYERS, newLayer, newScene, type Layer, type LayerKind, type Scene } from "./scene";
import { apply, getScene, imageBitmap, importImage, newId, saveScene } from "./api";
import { t } from "../notch/i18n";

const win = getCurrentWindow();
const id = ref(newId());
const scene = ref<Scene>(newScene(t("wall.untitled"), t("wall.kind.fill")));
/** État de référence (à l'ouverture ou au dernier enregistrement). */
const baseline = ref("");
/** Jamais enregistrée (nouvelle scène, copie d'un préréglage). */
const isNew = ref(true);
const dirty = computed(() => JSON.stringify(scene.value) !== baseline.value);
const selected = ref<string>("scene");
const error = ref<string | null>(null);
const saving = ref(false);
const applying = ref(false);
const importing = ref(false);
const noGpu = ref(false);

const canvas = ref<HTMLCanvasElement | null>(null);
const stage = ref<HTMLElement | null>(null);
let preview: Preview | null = null;

const layer = computed(() => scene.value.layers.find((l) => l.id === selected.value) ?? null);

// --- Thème (comme les notes) ---------------------------------------------------------------

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}

// --- Chargement -----------------------------------------------------------------------------

const loadImage = (name: string) => imageBitmap(id.value, name);

async function open(args: { id?: string | null } | null) {
  error.value = null;
  try {
    if (args?.id) {
      const s = await getScene(args.id);
      if (args.id.startsWith("preset-")) {
        // Préréglage : on édite une copie.
        id.value = newId();
        s.name = t("wall.copyOf", { name: s.name });
        scene.value = s;
        isNew.value = true;
      } else {
        id.value = args.id;
        scene.value = s;
        isNew.value = false;
      }
    } else {
      id.value = newId();
      scene.value = newScene(t("wall.untitled"), t("wall.kind.fill"));
      isNew.value = true;
    }
    baseline.value = JSON.stringify(scene.value);
    selected.value = "scene";
    preview?.resetImages();
    preview?.setScene(scene.value);
  } catch (e) {
    error.value = String(e);
  }
}

watch(scene, (s) => preview?.setScene(s), { deep: true });

// --- Calques --------------------------------------------------------------------------------

async function pickFile(): Promise<{ name: string; file: string; size: [number, number] } | null> {
  const path = await openDialog({ multiple: false, filters: [{ name: t("wall.images"), extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }] });
  if (typeof path !== "string") return null;
  importing.value = true;
  try {
    const name = await importImage(id.value, path);
    const bmp = await imageBitmap(id.value, name);
    const file = (path.split(/[\\/]/).pop() ?? "").replace(/\.[^.]+$/, "");
    return { name, file, size: [bmp.width, bmp.height] };
  } catch (e) {
    error.value = String(e);
    return null;
  } finally {
    importing.value = false;
  }
}

async function addLayer(kind: LayerKind) {
  if (scene.value.layers.length >= MAX_LAYERS) return;
  const l = newLayer(kind, t(`wall.kind.${kind}`));
  if (kind === "image") {
    const f = await pickFile();
    if (!f) return;
    l.image = f.name;
    l.name = f.file.slice(0, 80) || l.name;
    // Grande image aux proportions de l'écran (ou premier calque) : en fond ; sinon placée.
    const [pw, ph] = preview?.pixelSize() ?? [16, 9];
    const close = Math.abs(f.size[0] / f.size[1] - pw / ph) < 0.35;
    if (!scene.value.layers.length || (f.size[0] >= 1280 && close)) l.transform.fit = "cover";
    else l.transform = { fit: "free", anchor: "c", x: 0, y: 0, width: 0.3, rotation: 0 };
  }
  if (kind === "particles") l.blend = "normal";
  scene.value.layers.push(l);
  selected.value = l.id;
}

async function changeImage() {
  const l = layer.value;
  if (!l) return;
  const f = await pickFile();
  if (f) l.image = f.name;
}

function duplicate(lid: string) {
  const list = scene.value.layers;
  const i = list.findIndex((l) => l.id === lid);
  if (i < 0 || list.length >= MAX_LAYERS) return;
  const copy: Layer = JSON.parse(JSON.stringify(toRaw(list[i])));
  copy.id = layerId();
  copy.name = t("wall.copyOf", { name: list[i].name });
  list.splice(i + 1, 0, copy);
  selected.value = copy.id;
}

function remove(lid: string) {
  scene.value.layers = scene.value.layers.filter((l) => l.id !== lid);
  if (selected.value === lid) selected.value = "scene";
}

// --- Poignées sur l'aperçu ------------------------------------------------------------------

/** Rafraîchit les poignées (images qui finissent de charger, canvas redimensionné). */
const tick = ref(0);
let tickTimer: number | undefined;

/** Un calque placé à la main : image en mode « libre ». */
const movable = (l: Layer) => l.kind === "image" && l.transform.fit === "free" && l.visible && !l.locked && !!l.image;

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
  rot: number;
}

/** Rectangle d'un calque en px CSS du canvas (sans la parallaxe, qui bouge). */
function cssBox(l: Layer): Box | null {
  void tick.value;
  if (!preview || !canvas.value) return null;
  const [pw, ph] = preview.pixelSize();
  const dpr = pw / Math.max(canvas.value.clientWidth, 1);
  const [cx, cy, hw, hh] = layerBox({ ...scene.value, parallax: null }, l, { width: pw, height: ph, time: 0, mouse: null, ripples: [], trail: [] }, preview.texSize(l));
  return { x: (cx - hw) / dpr, y: (cy - hh) / dpr, w: (hw * 2) / dpr, h: (hh * 2) / dpr, rot: l.transform.rotation };
}

const gizmo = computed(() => (layer.value && movable(layer.value) ? cssBox(layer.value) : null));

type Drag = { mode: "move" | "resize"; sign: number; x0: number; y0: number; t0: { x: number; y: number; width: number } };
let drag: Drag | null = null;

function startDrag(e: PointerEvent, mode: Drag["mode"], sign = 1) {
  const l = layer.value;
  if (!l) return;
  e.stopPropagation();
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  drag = { mode, sign, x0: e.clientX, y0: e.clientY, t0: { x: l.transform.x, y: l.transform.y, width: l.transform.width } };
  if (preview) preview.interactive = false;
}

function onDrag(e: PointerEvent) {
  const l = layer.value;
  if (!drag || !l || !canvas.value) return;
  const cw = canvas.value.clientWidth;
  const ch = canvas.value.clientHeight;
  const dx = e.clientX - drag.x0;
  const dy = e.clientY - drag.y0;
  if (drag.mode === "move") {
    l.transform.x = +(drag.t0.x + dx / cw).toFixed(4);
    l.transform.y = +(drag.t0.y + dy / ch).toFixed(4);
  } else {
    l.transform.width = +Math.min(1.5, Math.max(0.02, drag.t0.width + (drag.sign * dx) / cw)).toFixed(4);
  }
}

function endDrag() {
  drag = null;
  if (preview) preview.interactive = true;
}

/** Clic sur l'aperçu : sélectionne le calque placé le plus haut sous le curseur. */
function pickAt(e: PointerEvent) {
  if (!stage.value) return;
  const r = stage.value.getBoundingClientRect();
  const x = e.clientX - r.left;
  const y = e.clientY - r.top;
  for (const l of [...scene.value.layers].reverse()) {
    if (!movable(l)) continue;
    const b = cssBox(l);
    if (b && x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h) {
      selected.value = l.id;
      return;
    }
  }
}

// --- Enregistrer / appliquer ---------------------------------------------------------------

async function save(): Promise<boolean> {
  if (saving.value) return false;
  saving.value = true;
  error.value = null;
  try {
    const s = JSON.parse(JSON.stringify(toRaw(scene.value))) as Scene;
    s.name = s.name.trim() || t("wall.untitled");
    await saveScene(id.value, s);
    scene.value.name = s.name;
    baseline.value = JSON.stringify(scene.value);
    isNew.value = false;
    return true;
  } catch (e) {
    error.value = String(e);
    return false;
  } finally {
    saving.value = false;
  }
}

async function saveAndApply() {
  if (applying.value) return;
  applying.value = true;
  try {
    if ((dirty.value || isNew.value) && !(await save())) return;
    await apply(id.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    applying.value = false;
  }
}

async function close() {
  if (dirty.value && !window.confirm(t("wall.discard"))) return;
  await win.close();
}

// --- Cycle de vie --------------------------------------------------------------------------

const unlisten: UnlistenFn[] = [];
function onKey(e: KeyboardEvent) {
  const inField = (e.target as HTMLElement | null)?.closest("input, textarea");
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void save();
  } else if (e.key === "Delete" && !inField && layer.value) {
    remove(layer.value.id);
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");
  unlisten.push(
    await listen<{ id?: string }>("window://args", (e) => {
      if (dirty.value && !window.confirm(t("wall.discard"))) return;
      void open(e.payload);
    }),
  );
  if (canvas.value) preview = await Preview.create(canvas.value, scene.value, loadImage);
  noGpu.value = !preview;
  tickTimer = window.setInterval(() => tick.value++, 250);
  await open(await invoke<{ id?: string } | null>("window_args").catch(() => null));
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  window.clearInterval(tickTimer);
  unlisten.forEach((u) => u());
  preview?.destroy();
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-elevated text-label">
    <!-- Barre de titre -->
    <header class="flex h-10 shrink-0 items-center gap-2 border-b border-separator pr-1 pl-3" data-tauri-drag-region>
      <input
        v-model="scene.name"
        class="type-footnote min-w-0 max-w-[260px] flex-1 rounded-[6px] bg-transparent px-1.5 py-0.5 font-medium outline-none hover:bg-fill-1 focus:bg-fill-1"
        :placeholder="t('wall.untitled')"
        maxlength="80"
      />
      <span class="type-caption flex shrink-0 items-center gap-1 text-label-4" data-tauri-drag-region>
        <template v-if="saving">…</template>
        <template v-else-if="!dirty && !isNew"><PhCheck :size="10" weight="bold" />{{ t("notes.saved") }}</template>
      </span>
      <div class="flex-1 self-stretch" data-tauri-drag-region />
      <Button size="sm" :loading="saving" :disabled="!dirty && !isNew" @click="save">{{ t("common.save") }}</Button>
      <Button size="sm" variant="default" :loading="applying" @click="saveAndApply"><PhMonitor />{{ t("wall.apply") }}</Button>
      <div class="mx-1 h-4 w-px bg-separator" />
      <IconButton :icon="PhMinus" :label="t('notes.minimize')" @click="win.minimize()" />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="close" />
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- Outliner -->
      <aside class="w-[230px] shrink-0 border-r border-separator">
        <Outliner :scene="scene" :selected="selected" @select="selected = $event" @add="addLayer" @duplicate="duplicate" @remove="remove" />
      </aside>

      <!-- Aperçu + poignées -->
      <main class="flex min-w-0 flex-1 flex-col gap-2 p-3">
        <div class="flex min-h-0 flex-1 items-center justify-center">
          <div ref="stage" class="relative aspect-video max-h-full w-full max-w-full overflow-hidden rounded-[12px] bg-black" @pointerdown="pickAt">
            <canvas ref="canvas" class="absolute inset-0 h-full w-full" />
            <div v-if="noGpu" class="type-footnote absolute inset-0 flex items-center justify-center p-6 text-center text-white/70">{{ t("wall.noGpu") }}</div>
            <div
              v-if="gizmo"
              class="absolute cursor-move border border-white/90 shadow-[0_0_0_1px_rgba(0,0,0,0.5)]"
              :style="{ left: `${gizmo.x}px`, top: `${gizmo.y}px`, width: `${gizmo.w}px`, height: `${gizmo.h}px`, transform: `rotate(${gizmo.rot}deg)` }"
              @pointerdown="startDrag($event, 'move')"
              @pointermove="onDrag"
              @pointerup="endDrag"
              @pointercancel="endDrag"
            >
              <span
                v-for="h in [{ c: '-top-1.5 -left-1.5', s: -1, cur: 'nwse' }, { c: '-top-1.5 -right-1.5', s: 1, cur: 'nesw' }, { c: '-bottom-1.5 -left-1.5', s: -1, cur: 'nesw' }, { c: '-bottom-1.5 -right-1.5', s: 1, cur: 'nwse' }]"
                :key="h.c"
                class="absolute size-3 rounded-[3px] border border-black/40 bg-white"
                :class="h.c"
                :style="{ cursor: `${h.cur}-resize` }"
                @pointerdown="startDrag($event, 'resize', h.s)"
                @pointermove="onDrag"
                @pointerup="endDrag"
                @pointercancel="endDrag"
              />
            </div>
          </div>
        </div>
        <p class="type-caption text-label-3">{{ t("wall.previewHint") }}</p>
        <div v-if="error" class="type-footnote flex items-center gap-2 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">
          <PhWarning :size="14" weight="bold" />{{ error }}
        </div>
      </main>

      <!-- Propriétés de la sélection -->
      <aside class="w-[320px] shrink-0 overflow-y-auto border-l border-separator p-3">
        <div v-if="layer" class="mb-3 flex items-center gap-2">
          <input v-model="layer.name" class="type-body min-w-0 flex-1 rounded-[6px] bg-fill-1 px-2 py-1 font-medium outline-none" maxlength="80" />
        </div>
        <LayerPanel v-if="layer" :layer="layer" :importing="importing" @pick-image="changeImage" />
        <ScenePanel v-else :scene="scene" />
      </aside>
    </div>
  </div>
</template>
