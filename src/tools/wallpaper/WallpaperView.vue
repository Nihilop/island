<script setup lang="ts">
// Outil « Fonds animés » (560 × 250) : scènes (préréglages + les tiennes), appliquer d'un
// clic, modifier / créer dans l'éditeur. Masquer l'outil du dock n'arrête pas le fond.
import { onMounted, reactive, ref } from "vue";
import { PhPause, PhPencilSimple, PhPlus, PhStop, PhTrash, PhWarning } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import { Spinner } from "@/sdk/ui/spinner";
import { apply, deleteScene, gradientCss, imageUrl, openEditor, wallpaper, watchWallpaper } from "../../wallpaper/api";
import type { SceneEntry } from "../../wallpaper/scene";
import { t } from "../../notch/i18n";

const thumbs = reactive<Record<string, string>>({});
const busy = ref<string | null>(null);
const error = ref<string | null>(null);

async function thumb(e: SceneEntry) {
  const img = e.scene.background.image;
  if (e.scene.background.kind === "image" && img && !thumbs[e.id + img]) thumbs[e.id + img] = await imageUrl(e.id, img).catch(() => "");
}

function style(e: SceneEntry) {
  const img = e.scene.background.image;
  if (e.scene.background.kind === "image" && img) {
    void thumb(e);
    const url = thumbs[e.id + img];
    return url ? { backgroundImage: `url(${url})`, backgroundSize: "cover", backgroundPosition: "center" } : { background: "var(--color-fill-2)" };
  }
  return { background: gradientCss(e.scene) };
}

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

const pick = (e: SceneEntry) => run(e.id, () => apply(wallpaper.state.active === e.id ? null : e.id));
const stop = () => run("stop", () => apply(null));
const edit = (e?: SceneEntry) => void openEditor(t("wall.editor"), e?.id);
async function remove(e: SceneEntry) {
  if (!window.confirm(t("wall.deleteConfirm", { name: e.scene.name }))) return;
  await run(e.id, () => deleteScene(e.id));
}

onMounted(watchWallpaper);
</script>

<template>
  <div class="flex h-full flex-col px-4">
    <Teleport defer to="#notch-header-actions">
      <span v-if="wallpaper.state.active && wallpaper.state.paused" class="type-caption mr-1 flex items-center gap-1 text-label-3"><PhPause :size="11" weight="fill" />{{ t("wall.paused") }}</span>
      <IconButton v-if="wallpaper.state.active" :icon="PhStop" :label="t('wall.stop')" :disabled="busy === 'stop'" @click="stop" />
      <IconButton :icon="PhPlus" :label="t('wall.new')" @click="edit()" />
    </Teleport>

    <div v-if="error || wallpaper.state.error" class="type-caption mb-2 flex items-center gap-1.5 rounded-[8px] bg-sys-red/10 px-2 py-1 text-sys-red">
      <PhWarning :size="12" weight="bold" />{{ t("wall.error", { e: error ?? wallpaper.state.error ?? "" }) }}
    </div>

    <div class="-mx-1 flex min-h-0 flex-1 gap-2.5 overflow-x-auto px-1 pb-3">
      <div v-for="e in wallpaper.scenes" :key="e.id" class="group relative w-[150px] shrink-0">
        <button
          type="button"
          class="relative block aspect-video w-full overflow-hidden rounded-[10px] ring-2 transition-[box-shadow,transform] active:scale-[0.98]"
          :class="wallpaper.state.active === e.id ? 'ring-sys-link' : 'ring-transparent hover:ring-fill-3'"
          :style="style(e)"
          :title="wallpaper.state.active === e.id ? t('wall.stop') : t('wall.apply')"
          @click="pick(e)"
        >
          <span v-if="busy === e.id" class="absolute inset-0 flex items-center justify-center bg-black/30"><Spinner class="text-white" /></span>
          <span v-else-if="wallpaper.state.active === e.id" class="type-caption absolute top-1.5 left-1.5 rounded-full bg-sys-link px-1.5 font-medium text-white">{{ t("wall.active") }}</span>
        </button>
        <div class="mt-1 flex items-center gap-1">
          <span class="type-footnote min-w-0 flex-1 truncate">{{ e.scene.name }}</span>
          <span v-if="e.preset" class="type-caption shrink-0 text-label-4">{{ t("wall.preset") }}</span>
        </div>
        <div class="absolute top-1 right-1 flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
          <IconButton :icon="PhPencilSimple" :label="e.preset ? t('wall.editCopy') : t('wall.edit')" class="bg-black/50! text-white!" @click.stop="edit(e)" />
          <IconButton v-if="!e.preset" :icon="PhTrash" :label="t('wall.delete')" class="bg-black/50! text-white!" @click.stop="remove(e)" />
        </div>
      </div>
    </div>
  </div>
</template>
