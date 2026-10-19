<script setup lang="ts">
// Éditeur de fonds animés (fenêtre détachée `wallpaper-editor`) : une image ou un dégradé,
// des effets, des interactions souris, aperçu en direct (WebGPU, même shader que le fond
// natif). Arguments d'ouverture : `{ id }` (scène à éditer ; un préréglage s'édite en copie)
// ou rien (nouvelle scène).
import { computed, onBeforeUnmount, onMounted, ref, toRaw, watch } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { PhCheck, PhImage, PhMinus, PhMonitor, PhWarning, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import Segmented from "../ui/Segmented.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import Param from "./Param.vue";
import { Preview } from "./preview";
import { DEFAULTS, newScene, type Scene } from "./scene";
import { apply, getScene, imageBitmap, importImage, newId, saveScene } from "./api";
import { t } from "../notch/i18n";

const win = getCurrentWindow();
const id = ref(newId());
const scene = ref<Scene>(newScene(t("wall.untitled")));
const saved = ref(JSON.stringify(scene.value));
const dirty = computed(() => JSON.stringify(scene.value) !== saved.value);
const error = ref<string | null>(null);
const saving = ref(false);
const applying = ref(false);
const importing = ref(false);
const noGpu = ref(false);

const canvas = ref<HTMLCanvasElement | null>(null);
let preview: Preview | null = null;

// --- Thème (comme les notes) ---------------------------------------------------------------

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}

// --- Chargement -----------------------------------------------------------------------------

async function loadImage() {
  const img = scene.value.background.image;
  if (!preview) return;
  if (scene.value.background.kind !== "image" || !img) return void (await preview.setImage(null));
  const bmp = await imageBitmap(id.value, img).catch(() => null);
  await preview.setImage(bmp);
}

async function open(args: { id?: string } | null) {
  error.value = null;
  try {
    if (args?.id) {
      const s = await getScene(args.id);
      if (args.id.startsWith("preset-")) {
        // Préréglage : on édite une copie.
        id.value = newId();
        s.name = t("wall.copyOf", { name: s.name });
        scene.value = s;
        saved.value = "";
      } else {
        id.value = args.id;
        scene.value = s;
        saved.value = JSON.stringify(s);
      }
    } else {
      id.value = newId();
      scene.value = newScene(t("wall.untitled"));
      saved.value = "";
    }
    preview?.setScene(scene.value);
    await loadImage();
  } catch (e) {
    error.value = String(e);
  }
}

watch(scene, (s) => preview?.setScene(s), { deep: true });

// --- Fond -----------------------------------------------------------------------------------

async function pickImage() {
  const path = await openDialog({ multiple: false, filters: [{ name: t("wall.images"), extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }] });
  if (typeof path !== "string") return;
  importing.value = true;
  try {
    scene.value.background.image = await importImage(id.value, path);
    scene.value.background.kind = "image";
    await loadImage();
  } catch (e) {
    error.value = String(e);
  } finally {
    importing.value = false;
  }
}

const kind = computed({
  get: () => scene.value.background.kind,
  set: (v) => {
    scene.value.background.kind = v;
    void loadImage();
  },
});

// --- Effets : interrupteurs ----------------------------------------------------------------

type Toggle = "water" | "parallax" | "particles" | "color" | "ripple" | "trail" | "grain" | "vignette";
function isOn(k: Toggle): boolean {
  const s = scene.value;
  if (k === "ripple" || k === "trail") return !!s.interactions[k];
  return s.effects[k] !== null;
}
function setOn(k: Toggle, on: boolean) {
  const s = scene.value;
  if (k === "ripple" || k === "trail") s.interactions[k] = on ? (DEFAULTS[k]() as never) : null;
  else if (k === "grain") s.effects.grain = on ? 0.2 : null;
  else if (k === "vignette") s.effects.vignette = on ? 0.4 : null;
  else s.effects[k] = on ? (DEFAULTS[k]() as never) : null;
}

const effectRows: { k: Toggle; label: string }[] = [
  { k: "water", label: "wall.fx.water" },
  { k: "parallax", label: "wall.fx.parallax" },
  { k: "particles", label: "wall.fx.particles" },
  { k: "color", label: "wall.fx.color" },
  { k: "grain", label: "wall.fx.grain" },
  { k: "vignette", label: "wall.fx.vignette" },
];

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
    saved.value = JSON.stringify(scene.value);
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
    if ((dirty.value || !saved.value) && !(await save())) return;
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
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void save();
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
  if (canvas.value) preview = await Preview.create(canvas.value, scene.value);
  noGpu.value = !preview;
  await open(await invoke<{ id?: string } | null>("window_args").catch(() => null));
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
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
        <template v-else-if="!dirty && saved"><PhCheck :size="10" weight="bold" />{{ t("notes.saved") }}</template>
      </span>
      <div class="flex-1 self-stretch" data-tauri-drag-region />
      <IconButton :icon="PhMinus" :label="t('notes.minimize')" @click="win.minimize()" />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="close" />
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- Aperçu -->
      <main class="flex min-w-0 flex-1 flex-col gap-2 p-3">
        <div class="relative min-h-0 flex-1 overflow-hidden rounded-[12px] bg-black">
          <canvas ref="canvas" class="absolute inset-0 h-full w-full" />
          <div v-if="noGpu" class="type-footnote absolute inset-0 flex items-center justify-center p-6 text-center text-white/70">{{ t("wall.noGpu") }}</div>
        </div>
        <p class="type-caption text-label-3">{{ t("wall.previewHint") }}</p>
        <div v-if="error" class="type-footnote flex items-center gap-2 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">
          <PhWarning :size="14" weight="bold" />{{ error }}
        </div>
      </main>

      <!-- Réglages -->
      <aside class="flex w-[330px] shrink-0 flex-col border-l border-separator">
        <div class="min-h-0 flex-1 space-y-4 overflow-y-auto p-3">
          <!-- Fond -->
          <section class="space-y-2">
            <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.background") }}</h3>
            <Segmented
              v-model="kind"
              size="sm"
              :options="[
                { value: 'image', label: t('wall.bg.image') },
                { value: 'gradient', label: t('wall.bg.gradient') },
              ]"
            />
            <template v-if="kind === 'image'">
              <Button class="w-full" :loading="importing" @click="pickImage"><PhImage />{{ scene.background.image ? t("wall.changeImage") : t("wall.pickImage") }}</Button>
              <Segmented
                v-model="scene.background.fit"
                size="sm"
                :options="[
                  { value: 'cover', label: t('wall.fit.cover') },
                  { value: 'contain', label: t('wall.fit.contain') },
                  { value: 'stretch', label: t('wall.fit.stretch') },
                ]"
              />
            </template>
            <div class="flex items-center gap-3 py-1">
              <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ kind === "gradient" ? t("wall.colors") : t("wall.bands") }}</span>
              <input v-model="scene.background.colors[0]" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
              <input v-if="kind === 'gradient'" v-model="scene.background.colors[1]" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
            </div>
            <Param v-if="kind === 'gradient'" v-model="scene.background.angle" :label="t('wall.angle')" :min="0" :max="360" :step="1" unit="°" :digits="0" />
          </section>

          <!-- Effets -->
          <section class="space-y-1">
            <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.effects") }}</h3>
            <div v-for="row in effectRows" :key="row.k" class="rounded-[10px] bg-fill-1 px-3 py-1.5">
              <div class="flex items-center justify-between">
                <span class="type-body">{{ t(row.label) }}</span>
                <Switch :model-value="isOn(row.k)" @update:model-value="setOn(row.k, $event)" />
              </div>
              <div v-if="row.k === 'water' && scene.effects.water" class="pt-1">
                <Param v-model="scene.effects.water.strength" :label="t('wall.p.strength')" :min="0" :max="1" />
                <Param v-model="scene.effects.water.speed" :label="t('wall.p.speed')" :min="0" :max="1" />
                <Param v-model="scene.effects.water.scale" :label="t('wall.p.scale')" :min="0" :max="1" />
              </div>
              <div v-else-if="row.k === 'parallax' && scene.effects.parallax" class="pt-1">
                <Param v-model="scene.effects.parallax.mouse" :label="t('wall.p.mouse')" :min="0" :max="1" />
                <Param v-model="scene.effects.parallax.drift" :label="t('wall.p.drift')" :min="0" :max="1" />
                <Param v-model="scene.effects.parallax.zoom" :label="t('wall.p.zoom')" :min="1" :max="1.3" />
              </div>
              <div v-else-if="row.k === 'particles' && scene.effects.particles" class="space-y-1 pt-1">
                <Segmented
                  v-model="scene.effects.particles.kind"
                  size="sm"
                  :options="[
                    { value: 'snow', label: t('wall.pk.snow') },
                    { value: 'rain', label: t('wall.pk.rain') },
                    { value: 'fireflies', label: t('wall.pk.fireflies') },
                  ]"
                />
                <Param v-model="scene.effects.particles.density" :label="t('wall.p.density')" :min="0" :max="1" />
                <Param v-model="scene.effects.particles.speed" :label="t('wall.p.speed')" :min="0" :max="2" />
                <Param v-model="scene.effects.particles.size" :label="t('wall.p.size')" :min="0.2" :max="4" />
                <Param v-model="scene.effects.particles.opacity" :label="t('wall.p.opacity')" :min="0" :max="1" />
                <div class="flex items-center gap-3 py-1">
                  <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.p.color") }}</span>
                  <input v-model="scene.effects.particles.color" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
                </div>
              </div>
              <div v-else-if="row.k === 'color' && scene.effects.color" class="pt-1">
                <Param v-model="scene.effects.color.hue" :label="t('wall.p.hue')" :min="-180" :max="180" :step="1" unit="°" :digits="0" />
                <Param v-model="scene.effects.color.saturation" :label="t('wall.p.saturation')" :min="0" :max="2" />
                <Param v-model="scene.effects.color.brightness" :label="t('wall.p.brightness')" :min="0" :max="2" />
                <Param v-model="scene.effects.color.contrast" :label="t('wall.p.contrast')" :min="0" :max="2" />
              </div>
              <div v-else-if="row.k === 'grain' && scene.effects.grain !== null" class="pt-1">
                <Param v-model="scene.effects.grain" :label="t('wall.p.amount')" :min="0" :max="1" />
              </div>
              <div v-else-if="row.k === 'vignette' && scene.effects.vignette !== null" class="pt-1">
                <Param v-model="scene.effects.vignette" :label="t('wall.p.amount')" :min="0" :max="1" />
              </div>
            </div>
          </section>

          <!-- Interactions -->
          <section class="space-y-1">
            <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.interactions") }}</h3>
            <p class="type-caption px-1 text-label-3">{{ t("wall.interactionsHelp") }}</p>
            <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
              <div class="flex items-center justify-between">
                <span class="type-body">{{ t("wall.fx.ripple") }}</span>
                <Switch :model-value="isOn('ripple')" @update:model-value="setOn('ripple', $event)" />
              </div>
              <div v-if="scene.interactions.ripple" class="pt-1">
                <Param v-model="scene.interactions.ripple.radius" :label="t('wall.p.radius')" :min="20" :max="1200" :step="10" unit=" px" :digits="0" />
                <Param v-model="scene.interactions.ripple.strength" :label="t('wall.p.strength')" :min="0" :max="2" />
                <Param v-model="scene.interactions.ripple.duration" :label="t('wall.p.duration')" :min="0.2" :max="5" unit=" s" :digits="1" />
              </div>
            </div>
            <div class="rounded-[10px] bg-fill-1 px-3 py-1.5">
              <div class="flex items-center justify-between">
                <span class="type-body">{{ t("wall.fx.trail") }}</span>
                <Switch :model-value="isOn('trail')" @update:model-value="setOn('trail', $event)" />
              </div>
              <div v-if="scene.interactions.trail" class="pt-1">
                <Param v-model="scene.interactions.trail.width" :label="t('wall.p.width')" :min="1" :max="80" :step="1" unit=" px" :digits="0" />
                <Param v-model="scene.interactions.trail.fade" :label="t('wall.p.duration')" :min="0.05" :max="3" unit=" s" :digits="2" />
                <Param v-model="scene.interactions.trail.intensity" :label="t('wall.p.intensity')" :min="0" :max="2" />
                <div class="flex items-center gap-3 py-1">
                  <span class="type-footnote w-[92px] shrink-0 text-label-2">{{ t("wall.p.color") }}</span>
                  <input v-model="scene.interactions.trail.color" type="color" class="h-6 w-10 cursor-pointer rounded-[6px] bg-transparent" />
                </div>
              </div>
            </div>
          </section>
        </div>

        <footer class="flex shrink-0 gap-2 border-t border-separator p-3">
          <Button class="flex-1" :loading="saving" :disabled="!dirty && !!saved" @click="save">{{ t("common.save") }}</Button>
          <Button class="flex-1" variant="default" :loading="applying" @click="saveAndApply"><PhMonitor />{{ t("wall.apply") }}</Button>
        </footer>
      </aside>
    </div>
  </div>
</template>
