<script setup lang="ts">
// Outil « Fonds animés » : en haut, les écrans à l'échelle (repliable, ouvert par défaut) —
// « Tous les écrans » ou un écran choisi ; dessous, la grille des scènes (5 × 3 par page,
// recherche au-delà d'une page). Un clic applique la scène à la cible, un second la retire.
// Masquer l'outil du dock n'arrête pas le fond.
import { computed, onMounted, reactive, ref, watch } from "vue";
import { PhCaretDown, PhCaretLeft, PhCaretRight, PhDownloadSimple, PhMonitor, PhPause, PhPencilSimple, PhPlus, PhShareNetwork, PhStop, PhTrash, PhWarning } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import TextField from "../../ui/TextField.vue";
import { Spinner } from "@/sdk/ui/spinner";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/sdk/ui/collapsible";
import { apply, deleteScene, displays as listDisplays, exportScene, fillCss, imageUrl, openEditor, pickAndImport, sceneOn, shareError, thumbLayer, thumbnailUrl, wallpaper, watchWallpaper, type Display } from "../../wallpaper/api";
import type { SceneEntry } from "../../wallpaper/scene";
import { t } from "../../notch/i18n";

const PAGE = 15;

const thumbs = reactive<Record<string, string>>({});
const busy = ref<string | null>(null);
const error = ref<string | null>(null);

// --- Écrans ---------------------------------------------------------------------------------

const screens = ref<Display[]>([]);
/** Écran ciblé (null = tous les écrans). */
const target = ref<string | null>(null);

const COLLAPSE_KEY = "island.wallpaper.screensOpen";
const open = ref(true);
try {
  open.value = localStorage.getItem(COLLAPSE_KEY) !== "0";
} catch {
  /* stockage indisponible : ouvert */
}
watch(open, (v) => {
  try {
    localStorage.setItem(COLLAPSE_KEY, v ? "1" : "0");
  } catch {
    /* rien */
  }
});

/** Écrans placés à l'échelle dans une boîte de 84 px de haut. */
const map = computed(() => {
  const list = screens.value;
  if (!list.length) return { w: 0, items: [] as { d: Display; n: number; style: Record<string, string> }[] };
  const minX = Math.min(...list.map((d) => d.x));
  const minY = Math.min(...list.map((d) => d.y));
  const maxX = Math.max(...list.map((d) => d.x + d.width));
  const maxY = Math.max(...list.map((d) => d.y + d.height));
  const k = Math.min(84 / (maxY - minY), 420 / (maxX - minX));
  return {
    w: (maxX - minX) * k,
    items: list.map((d, i) => ({
      d,
      n: i + 1,
      style: { left: `${(d.x - minX) * k}px`, top: `${(d.y - minY) * k}px`, width: `${d.width * k - 4}px`, height: `${d.height * k - 4}px` },
    })),
  };
});

const byId = computed(() => new Map(wallpaper.scenes.map((e) => [e.id, e])));
const nameOf = (id: string | null) => (id ? (byId.value.get(id)?.scene.name ?? "?") : t("wall.windows"));

/** Résumé quand le bloc est replié : « Aurore » ou « Aurore / Pluie ». */
const summary = computed(() => {
  const names = screens.value.map((d) => nameOf(sceneOn(d.id)));
  return [...new Set(names)].length <= 1 ? (names[0] ?? nameOf(wallpaper.state.all)) : names.join(" / ");
});

/** Scène de la cible courante (celle qu'on surligne dans la grille). */
const current = computed(() => (target.value ? sceneOn(target.value) : wallpaper.state.all));
const anyActive = computed(() => !!wallpaper.state.all || Object.values(wallpaper.state.screens).some(Boolean));

// --- Grille ---------------------------------------------------------------------------------

const query = ref("");
const page = ref(0);
/** Les tiennes d'abord (plus récentes en premier), les préréglages ensuite. */
const ordered = computed(() => [...wallpaper.scenes.filter((e) => !e.preset), ...wallpaper.scenes.filter((e) => e.preset)]);
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return q ? ordered.value.filter((e) => e.scene.name.toLowerCase().includes(q)) : ordered.value;
});
const pages = computed(() => Math.max(1, Math.ceil(filtered.value.length / PAGE)));
const visible = computed(() => filtered.value.slice(page.value * PAGE, page.value * PAGE + PAGE));
watch([query, pages], () => (page.value = Math.min(page.value, pages.value - 1)));
watch(query, () => (page.value = 0));

async function thumb(id: string, img: string) {
  if (!thumbs[id + img]) thumbs[id + img] = await imageUrl(id, img).catch(() => "");
}

const cover = (url: string | undefined) =>
  url ? { backgroundImage: `url(${url})`, backgroundSize: "cover", backgroundPosition: "center" } : { background: "var(--color-fill-2)" };

/** Vignette : le rendu enregistré par l'éditeur ; à défaut, le calque de fond de la scène. */
function style(e: SceneEntry | undefined) {
  if (e?.thumbnail) {
    const k = `snap:${e.id}:${e.updated}`;
    if (!(k in thumbs)) {
      thumbs[k] = "";
      void thumbnailUrl(e.id, e.updated).then((u) => (thumbs[k] = u)).catch(() => {});
    }
    return cover(thumbs[k]);
  }
  const l = e && thumbLayer(e.scene);
  if (!e || !l) return { background: "var(--color-fill-2)" };
  if (l.kind === "image" && l.image) {
    void thumb(e.id, l.image);
    const url = thumbs[e.id + l.image];
    return url ? { backgroundImage: `url(${url})`, backgroundSize: "cover", backgroundPosition: "center" } : { background: "var(--color-fill-2)" };
  }
  return { background: fillCss(l) };
}

// --- Actions --------------------------------------------------------------------------------

async function run(key: string, fn: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = key;
  error.value = null;
  try {
    await fn();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = null;
  }
}

const pick = (e: SceneEntry) => run(e.id, () => apply(current.value === e.id ? null : e.id, target.value));
const stop = () => run("stop", () => apply(null));
const edit = (e?: SceneEntry) => void openEditor(t("wall.editor"), e?.id);
/** Partager : enregistre la scène dans un `.islandscene`. */
async function share(e: SceneEntry) {
  if (busy.value) return;
  busy.value = e.id;
  error.value = null;
  try {
    await exportScene(e.id, e.scene.name);
  } catch (err) {
    error.value = shareError(err);
  } finally {
    busy.value = null;
  }
}

async function importFiles() {
  if (busy.value) return;
  busy.value = "import";
  error.value = null;
  try {
    const done = await pickAndImport();
    // La nouvelle scène est en tête de liste (les tiennes d'abord, récentes d'abord).
    if (done.length) page.value = 0;
  } catch (err) {
    error.value = shareError(err);
  } finally {
    busy.value = null;
  }
}

async function remove(e: SceneEntry) {
  if (!window.confirm(t("wall.deleteConfirm", { name: e.scene.name }))) return;
  await run(e.id, () => deleteScene(e.id));
}

onMounted(async () => {
  watchWallpaper();
  screens.value = await listDisplays().catch(() => []);
});
</script>

<template>
  <div class="flex h-full flex-col overflow-y-auto px-4 pt-1 pb-3">
    <Teleport defer to="#notch-header-actions">
      <span v-if="anyActive && wallpaper.state.paused" class="type-caption mr-1 flex items-center gap-1 text-label-3"><PhPause :size="11" weight="fill" />{{ t("wall.paused") }}</span>
      <IconButton v-if="anyActive" :icon="PhStop" :label="t('wall.stop')" :disabled="busy === 'stop'" @click="stop" />
      <IconButton :icon="PhDownloadSimple" :label="t('wall.share.import')" :disabled="busy === 'import'" @click="importFiles" />
      <IconButton :icon="PhPlus" :label="t('wall.new')" @click="edit()" />
    </Teleport>

    <!-- Écrans -->
    <Collapsible v-model:open="open" class="mb-3 rounded-[12px] bg-fill-1">
      <CollapsibleTrigger class="flex w-full items-center gap-2 px-3 py-2 text-left">
        <PhMonitor :size="14" class="shrink-0 text-label-3" />
        <span class="type-footnote font-medium">{{ t("wall.screens") }}</span>
        <span v-if="!open" class="type-caption min-w-0 flex-1 truncate text-label-3">{{ summary }}</span>
        <span v-else class="flex-1" />
        <PhCaretDown :size="12" class="shrink-0 text-label-3 transition-transform" :class="open ? 'rotate-180' : ''" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div class="flex items-center gap-3 px-3 pb-3">
          <button
            type="button"
            class="type-caption shrink-0 rounded-full px-2.5 py-1 font-medium transition-colors"
            :class="target === null ? 'bg-sys-link text-white' : 'bg-fill-2 text-label-2 hover:bg-fill-3'"
            @click="target = null"
          >
            {{ t("wall.allScreens") }}
          </button>
          <div class="relative h-[84px] shrink-0" :style="{ width: `${map.w}px` }">
            <button
              v-for="s in map.items"
              :key="s.d.id"
              type="button"
              class="absolute overflow-hidden rounded-[6px] ring-2 transition-[box-shadow]"
              :class="target === s.d.id ? 'ring-sys-link' : target === null ? 'ring-sys-link/40' : 'ring-transparent hover:ring-fill-3'"
              :style="{ ...s.style, ...style(byId.get(sceneOn(s.d.id) ?? '')) }"
              :title="`${t('wall.screenN', { n: s.n })} · ${nameOf(sceneOn(s.d.id))}`"
              @click="target = target === s.d.id ? null : s.d.id"
            >
              <span class="type-caption absolute bottom-0.5 left-1 rounded bg-black/55 px-1 font-semibold text-white">{{ s.n }}</span>
            </button>
          </div>
          <p class="type-caption min-w-0 flex-1 text-label-3">
            {{ target === null ? t("wall.targetAll") : t("wall.targetOne", { n: map.items.find((s) => s.d.id === target)?.n ?? "?" }) }}
          </p>
        </div>
      </CollapsibleContent>
    </Collapsible>

    <div v-if="error || wallpaper.state.error" class="type-caption mb-2 flex items-center gap-1.5 rounded-[8px] bg-sys-red/10 px-2 py-1 text-sys-red">
      <PhWarning :size="12" weight="bold" />{{ t("wall.error", { e: error ?? wallpaper.state.error ?? "" }) }}
    </div>

    <div v-if="ordered.length > PAGE" class="mb-2.5">
      <TextField v-model="query" :placeholder="t('wall.search')" />
    </div>

    <!-- Grille (padding : le contour de sélection et les boutons ne sont jamais rognés) -->
    <div class="grid grid-cols-5 gap-x-2.5 gap-y-2 p-0.5">
      <div v-for="e in visible" :key="e.id" class="group relative min-w-0">
        <button
          type="button"
          class="relative block aspect-video w-full overflow-hidden rounded-[8px] ring-2 transition-[box-shadow,transform] active:scale-[0.98]"
          :class="current === e.id ? 'ring-sys-link' : 'ring-transparent hover:ring-fill-3'"
          :style="style(e)"
          :title="current === e.id ? t('wall.remove') : t('wall.apply')"
          @click="pick(e)"
        >
          <span v-if="busy === e.id" class="absolute inset-0 flex items-center justify-center bg-black/30"><Spinner class="text-white" /></span>
        </button>
        <div class="type-caption mt-1 truncate" :class="current === e.id ? 'font-medium text-label' : 'text-label-2'">{{ e.scene.name }}</div>
        <div class="absolute top-1 right-1 flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
          <IconButton :icon="PhPencilSimple" :label="e.preset ? t('wall.editCopy') : t('wall.edit')" class="bg-black/50! text-white!" @click.stop="edit(e)" />
          <IconButton :icon="PhShareNetwork" :label="t('wall.share.export')" class="bg-black/50! text-white!" @click.stop="share(e)" />
          <IconButton v-if="!e.preset" :icon="PhTrash" :label="t('wall.delete')" class="bg-black/50! text-white!" @click.stop="remove(e)" />
        </div>
      </div>
    </div>
    <p v-if="!visible.length" class="type-footnote py-6 text-center text-label-3">{{ t("wall.noResult") }}</p>

    <div v-if="pages > 1" class="mt-2 flex items-center justify-end gap-1">
      <IconButton :icon="PhCaretLeft" :label="t('wall.prev')" :disabled="page === 0" @click="page--" />
      <span class="type-caption tabular-nums text-label-3">{{ page + 1 }} / {{ pages }}</span>
      <IconButton :icon="PhCaretRight" :label="t('wall.next')" :disabled="page >= pages - 1" @click="page++" />
    </div>
  </div>
</template>
