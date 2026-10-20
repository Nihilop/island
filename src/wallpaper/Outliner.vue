<script setup lang="ts">
// Outliner de l'éditeur : « Scène » (effets globaux) puis les calques, du HAUT vers le bas
// (comme Photoshop ; la scène les stocke du bas vers le haut). Glisser-déposer pour
// réordonner, œil, cadenas, double-clic pour renommer, dupliquer, supprimer.
import { computed, nextTick, ref } from "vue";
import { PhCopy, PhDrop, PhEye, PhEyeSlash, PhFilmStrip, PhGradient, PhImage, PhLock, PhLockOpen, PhPlus, PhSlidersHorizontal, PhSparkle, PhTrash } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { MAX_LAYERS, type Layer, type LayerKind, type Scene } from "./scene";
import { t } from "../notch/i18n";

const props = defineProps<{ scene: Scene; selected: string }>();
const emit = defineEmits<{ select: [id: string]; add: [kind: LayerKind]; duplicate: [id: string]; remove: [id: string] }>();

/** Haut de la pile en premier. */
const rows = computed(() => [...props.scene.layers].reverse());
const full = computed(() => props.scene.layers.length >= MAX_LAYERS);
const adding = ref(false);

const ICONS = { image: PhImage, video: PhFilmStrip, fill: PhGradient, particles: PhSparkle } as const;
function icon(l: Layer) {
  if (l.kind === "fill" && l.fill.mode === "solid") return PhDrop;
  return ICONS[l.kind];
}

// --- Renommer ------------------------------------------------------------------------------

const renaming = ref<string | null>(null);
const draft = ref("");
async function rename(l: Layer) {
  renaming.value = l.id;
  draft.value = l.name;
  await nextTick();
  document.querySelector<HTMLInputElement>("[data-rename]")?.select();
}
function commit(l: Layer) {
  const v = draft.value.trim();
  if (v) l.name = v.slice(0, 80);
  renaming.value = null;
}

// --- Réordonner (glisser-déposer) ----------------------------------------------------------

const dragging = ref<string | null>(null);
const over = ref<{ id: string; after: boolean } | null>(null);

function onDragOver(e: DragEvent, l: Layer) {
  if (!dragging.value || dragging.value === l.id) return;
  e.preventDefault();
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  over.value = { id: l.id, after: e.clientY > r.top + r.height / 2 };
}

function onDrop() {
  const from = dragging.value;
  const target = over.value;
  dragging.value = null;
  over.value = null;
  if (!from || !target) return;
  const list = props.scene.layers;
  const moving = list.find((x) => x.id === from);
  if (!moving) return;
  list.splice(list.indexOf(moving), 1);
  // Liste affichée inversée : « après » à l'écran = en dessous = index plus petit.
  const i = list.findIndex((x) => x.id === target.id);
  list.splice(target.after ? i : i + 1, 0, moving);
}

function add(kind: LayerKind) {
  adding.value = false;
  emit("add", kind);
}
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex items-center justify-between px-3 pt-3 pb-1.5">
      <h3 class="type-caption font-medium uppercase text-label-3">{{ t("wall.layers") }}</h3>
      <div class="relative">
        <IconButton :icon="PhPlus" :label="full ? t('wall.layersFull') : t('wall.addLayer')" :disabled="full" @click="adding = !adding" />
        <div v-if="adding" class="absolute top-full right-0 z-20 mt-1 w-[170px] rounded-[10px] border border-separator bg-elevated p-1 shadow-xl">
          <button v-for="k in (['image', 'video', 'fill', 'particles'] as const)" :key="k" type="button" class="type-footnote flex w-full items-center gap-2 rounded-[6px] px-2 py-1.5 text-left hover:bg-fill-2" @click="add(k)">
            <component :is="ICONS[k]" :size="14" class="text-label-3" />{{ t(`wall.kind.${k}`) }}
          </button>
        </div>
      </div>
    </div>

    <div class="min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-2">
      <!-- Scène (effets globaux) -->
      <button
        type="button"
        class="type-footnote flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left"
        :class="selected === 'scene' ? 'bg-sys-link/15 text-label' : 'text-label-2 hover:bg-fill-1'"
        @click="emit('select', 'scene')"
      >
        <PhSlidersHorizontal :size="14" class="shrink-0 text-label-3" />
        <span class="font-medium">{{ t("wall.sceneItem") }}</span>
      </button>
      <div class="mx-2 my-1 h-px bg-separator" />

      <div
        v-for="l in rows"
        :key="l.id"
        draggable="true"
        class="group relative flex items-center gap-1.5 rounded-[8px] py-1 pr-1 pl-2"
        :class="[selected === l.id ? 'bg-sys-link/15' : 'hover:bg-fill-1', dragging === l.id ? 'opacity-40' : '', !l.visible ? 'opacity-60' : '']"
        @click="emit('select', l.id)"
        @dragstart="dragging = l.id"
        @dragend="dragging = null; over = null"
        @dragover="onDragOver($event, l)"
        @drop.prevent="onDrop"
      >
        <span v-if="over?.id === l.id" class="pointer-events-none absolute inset-x-1 h-0.5 rounded bg-sys-link" :class="over.after ? '-bottom-px' : '-top-px'" />
        <component :is="icon(l)" :size="14" class="shrink-0 text-label-3" />
        <input
          v-if="renaming === l.id"
          v-model="draft"
          data-rename
          class="type-footnote min-w-0 flex-1 rounded-[4px] bg-fill-2 px-1 outline-none"
          maxlength="80"
          @click.stop
          @keydown.enter="commit(l)"
          @keydown.escape="renaming = null"
          @blur="commit(l)"
        />
        <span v-else class="type-footnote min-w-0 flex-1 truncate" @dblclick.stop="rename(l)">{{ l.name || t(`wall.kind.${l.kind}`) }}</span>
        <IconButton :icon="l.visible ? PhEye : PhEyeSlash" :label="l.visible ? t('wall.hide') : t('wall.show')" @click.stop="l.visible = !l.visible" />
        <IconButton :icon="l.locked ? PhLock : PhLockOpen" :label="l.locked ? t('wall.unlock') : t('wall.lock')" :class="l.locked ? '' : 'opacity-0 group-hover:opacity-100'" @click.stop="l.locked = !l.locked" />
        <div class="hidden gap-0.5 group-hover:flex">
          <IconButton :icon="PhCopy" :label="t('wall.duplicate')" :disabled="full" @click.stop="emit('duplicate', l.id)" />
          <IconButton :icon="PhTrash" :label="t('wall.deleteLayer')" @click.stop="emit('remove', l.id)" />
        </div>
      </div>
      <p v-if="!rows.length" class="type-caption px-2 py-3 text-label-3">{{ t("wall.noLayer") }}</p>
    </div>
  </div>
</template>
