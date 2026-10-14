<script setup lang="ts">
// Outil « Étagère » (560 × 236) : glisser des fichiers ici (ou Ctrl+V) pour les garder
// sous la main ; ouvrir, révéler dans l'Explorateur, recopier.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { PhArrowSquareOut, PhCheck, PhCopy, PhFolderOpen, PhTrash, PhTray, PhX } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import FileIcon from "../../ui/FileIcon.vue";
import { Button } from "@/sdk/ui/button";
import { dragOver } from "../../notch/drop";
import { clear, copyFiles, formatSize, items, open, pasteFromClipboard, remove, reveal } from "./store";
import { t } from "../../notch/i18n";

const copiedAll = ref(false);
const pasteMiss = ref(false);
const error = ref<string | null>(null);

async function act(fn: () => Promise<unknown>) {
  error.value = null;
  try {
    await fn();
  } catch {
    error.value = t("shelf.missing");
    window.setTimeout(() => (error.value = null), 2000);
  }
}

async function copyAll() {
  await act(() => copyFiles(items.value.map((i) => i.path)));
  copiedAll.value = true;
  window.setTimeout(() => (copiedAll.value = false), 1200);
}

async function paste() {
  if (!(await pasteFromClipboard())) {
    pasteMiss.value = true;
    window.setTimeout(() => (pasteMiss.value = false), 1600);
  }
}

function onKey(e: KeyboardEvent) {
  const inField = (e.target as HTMLElement | null)?.closest("input, textarea");
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v" && !inField) {
    e.preventDefault();
    void paste();
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="flex h-full flex-col px-4">
    <Teleport defer to="#notch-header-actions">
      <span v-if="items.length" class="type-caption mr-1 text-label-3">{{ t(items.length === 1 ? "shelf.count1" : "shelf.count", { n: items.length }) }}</span>
      <IconButton v-if="items.length" :icon="copiedAll ? PhCheck : PhCopy" :label="t('shelf.copyAll')" @click="copyAll" />
      <IconButton v-if="items.length" :icon="PhTrash" :label="t('shelf.clear')" @click="clear" />
    </Teleport>

    <div
      v-if="!items.length"
      class="mt-2 mb-1 flex flex-1 flex-col items-center justify-center gap-1 rounded-[14px] border-[1.5px] border-dashed transition-[scale,background-color,border-color] duration-200 ease-out"
      :class="dragOver ? 'scale-[1.015] border-label-2 bg-fill-2' : 'border-fill-3 bg-fill-1'"
    >
      <PhTray :size="22" weight="duotone" class="text-label-3" />
      <div class="type-body text-label-2">{{ dragOver ? t("shelf.release") : t("shelf.drop") }}</div>
      <button type="button" class="type-caption text-label-3 underline-offset-2 hover:underline" @click="paste">
        {{ pasteMiss ? t("shelf.nothingToPaste") : t("shelf.paste") }}
      </button>
    </div>

    <div v-else class="relative flex gap-2 overflow-x-auto [scrollbar-width:none]">
      <TransitionGroup
        move-class="transition-[transform,opacity,scale] duration-240 ease-out"
        enter-active-class="transition-[transform,opacity,scale] duration-240 ease-out"
        enter-from-class="opacity-0 scale-[0.94]"
        leave-active-class="absolute transition-[transform,opacity,scale] duration-240 ease-out"
        leave-to-class="opacity-0 scale-[0.94]"
      >
        <div v-for="f in items" :key="f.path" class="group/card relative flex flex-[0_0_112px] flex-col gap-1">
          <button type="button" class="grid h-[84px] place-items-center overflow-hidden rounded-md bg-fill-1" :title="t('shelf.open')" @click="act(() => open(f.path))">
            <img v-if="f.thumb" :src="f.thumb" alt="" class="size-full object-cover" />
            <FileIcon v-else :name="f.name" :size="30" class="text-label-2" />
          </button>
          <div class="type-caption truncate font-medium" :title="f.name">{{ f.name }}</div>
          <div class="flex h-5 items-center justify-between">
            <span class="type-caption text-label-3">{{ formatSize(f.size) }}</span>
            <div class="flex gap-0.5 opacity-0 transition-opacity duration-120 ease-out group-hover/card:opacity-100">
              <Button size="icon-xs" class="text-label-2" :title="t('shelf.reveal')" @click="act(() => reveal(f.path))"><PhFolderOpen :size="11" weight="bold" class="size-[11px]" /></Button>
              <Button size="icon-xs" class="text-label-2" :title="t('shelf.copy')" @click="act(() => copyFiles([f.path]))"><PhCopy :size="11" weight="bold" class="size-[11px]" /></Button>
              <Button size="icon-xs" class="text-label-2" :title="t('shelf.open')" @click="act(() => open(f.path))"><PhArrowSquareOut :size="11" weight="bold" class="size-[11px]" /></Button>
            </div>
          </div>
          <Button
            size="icon-xs"
            class="absolute top-1 right-1 size-[18px] bg-fill-3 text-label opacity-0 backdrop-blur-[8px] transition-opacity duration-120 group-hover/card:opacity-100"
            :title="t('clip.remove')"
            @click="remove(f.path)"
          >
            <PhX :size="10" weight="bold" class="size-2.5" />
          </Button>
        </div>
      </TransitionGroup>
      <div
        v-if="dragOver"
        class="type-body absolute inset-0 grid place-items-center rounded-[14px] border-[1.5px] border-dashed border-label-2 bg-scrim/70"
      >{{ t("shelf.release") }}</div>
    </div>
    <div v-if="error" class="type-caption px-4 text-sys-red">{{ error }}</div>
  </div>
</template>
