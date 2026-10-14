<script setup lang="ts">
// Outil « Presse-papiers » (600 × 236) : rangée horizontale de cartes (la plus récente à
// gauche, molette verticale = défilement horizontal). Clic : texte/lien/fichiers →
// recopie (« ✓ Copié ») ; image → aperçu (le notch grandit vers le bas).
import { useDraft } from "../../stores/drafts";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { PhCheck, PhCopy, PhFiles, PhImage, PhLink, PhTextT, PhTrash, PhX } from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import IconButton from "../../ui/IconButton.vue";
import Empty from "../../ui/Empty.vue";
import FileIcon from "../../ui/FileIcon.vue";
import { Button } from "@/sdk/ui/button";
import { clearAll, copy, filter, preview, remove, visible, entries, type ClipEntry } from "./store";
import { requestSize } from "../registry";
import { presentation } from "../../notch/presentation";

watch(presentation, (on) => {
  if (on) closePreview();
});
import { t } from "../../notch/i18n";

const ICONS = { text: PhTextT, link: PhLink, image: PhImage, files: PhFiles } as const;
const BASE = { w: 600, h: 236 };

const row = ref<HTMLElement>();
const copiedId = ref<number | null>(null);
const failedId = ref<number | null>(null);
const selected = useDraft<ClipEntry | null>("clip.selected", () => null);
const large = useDraft<string | null>("clip.large", () => null);
let copiedTimer: number | undefined;

// Âge relatif, rafraîchi toutes les 30 s.
const now = ref(Date.now());
let ageTimer: number | undefined;
onMounted(() => (ageTimer = window.setInterval(() => (now.value = Date.now()), 30_000)));
onBeforeUnmount(() => {
  window.clearInterval(ageTimer);
  requestSize("clipboard", null);
});

function age(at: number): string {
  const m = Math.floor((now.value - at) / 60_000);
  if (m < 1) return t("clip.now");
  if (m < 60) return t("clip.min", { n: m });
  const h = Math.floor(m / 60);
  return h < 24 ? t("clip.hour", { n: h }) : t("clip.day", { n: Math.floor(h / 24) });
}

function link(text: string) {
  try {
    const u = new URL(text.startsWith("www.") ? `https://${text}` : text);
    return { host: u.host.replace(/^www\./, ""), path: decodeURIComponent(u.pathname + u.search).replace(/^\/$/, "") };
  } catch {
    return { host: text, path: "" };
  }
}
const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;

async function onCard(e: ClipEntry) {
  if (e.kind === "image") return presentation.value ? recopy(e) : togglePreview(e);
  await recopy(e);
}

/** Recopie (carte, bouton Copier, aperçu) avec retour visuel sur la carte. */
async function recopy(e: ClipEntry) {
  const err = await copy(e.id);
  window.clearTimeout(copiedTimer);
  copiedId.value = err ? null : e.id;
  failedId.value = err ? e.id : null;
  copiedTimer = window.setTimeout(() => (copiedId.value = failedId.value = null), err ? 2000 : 1200);
  row.value?.scrollTo({ left: 0, behavior: "smooth" });
}

async function togglePreview(e: ClipEntry) {
  if (selected.value?.id === e.id) return closePreview();
  selected.value = e;
  large.value = await preview(e.id).catch(() => e.thumb);
}
function closePreview() {
  selected.value = null;
  large.value = null;
}

// Taille de l'aperçu : aussi grand que le ratio le permet (≤ largeur − 32, ≤ 290 px).
const previewSize = computed(() => {
  const s = selected.value;
  if (!s?.width || !s.height) return null;
  const maxW = BASE.w - 32;
  const k = Math.min(maxW / s.width, 290 / s.height, 1);
  return { w: Math.round(s.width * k), h: Math.round(s.height * k) };
});
// immediate : un aperçu resté ouvert (brouillon) regrandit le notch à la réouverture.
watch(previewSize, (p) => requestSize("clipboard", p ? { w: BASE.w, h: BASE.h + p.h + 46 } : null), { immediate: true });
watch(entries, (list) => {
  if (selected.value && !list.some((e) => e.id === selected.value!.id)) closePreview();
});

function wheel(e: WheelEvent) {
  if (!row.value || Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;
  e.preventDefault();
  row.value.scrollLeft += e.deltaY;
}
</script>

<template>
  <div class="flex h-full flex-col">
    <Teleport defer to="#notch-header-actions">
      <Segmented
        v-model="filter"
        size="sm"
        :options="[
          { value: 'all', label: t('clip.all') },
          { value: 'text', label: t('clip.text') },
          { value: 'link', label: t('clip.links') },
          { value: 'image', label: t('clip.images') },
          { value: 'files', label: t('clip.files') },
        ]"
      />
      <IconButton :icon="PhTrash" :label="t('clip.clear')" :disabled="!entries.length" @click="clearAll" />
    </Teleport>

    <Empty v-if="!entries.length" :icon="PhCopy" :title="t('clip.empty')" :text="t('clip.emptyText')" />
    <Empty v-else-if="!visible.length" :title="t('clip.emptyFilter')" />
    <div
      v-else
      ref="row"
      class="flex h-[146px] gap-2 overflow-x-auto px-4 [mask-image:linear-gradient(to_right,transparent,#000_12px,#000_calc(100%-24px),transparent)] [scrollbar-width:none]"
      @wheel="wheel"
    >
      <TransitionGroup
        move-class="transition-[transform,opacity,scale] duration-240 ease-out"
        enter-active-class="transition-[transform,opacity,scale] duration-240 ease-out"
        enter-from-class="opacity-0 scale-[0.94]"
        leave-active-class="absolute transition-[transform,opacity,scale] duration-240 ease-out"
        leave-to-class="opacity-0 scale-[0.94]"
      >
        <div
          v-for="e in visible"
          :key="e.id"
          class="group/card relative h-[146px] flex-[0_0_128px] cursor-pointer overflow-hidden rounded-[14px] bg-fill-1 p-2.5 text-left"
          :class="[
            e.kind === 'image' &&
              'after:pointer-events-none after:absolute after:inset-0 after:bg-[linear-gradient(to_bottom,color-mix(in_srgb,var(--scrim)_55%,transparent),transparent_35%,transparent_65%,color-mix(in_srgb,var(--scrim)_60%,transparent))]',
            selected?.id === e.id && 'shadow-[inset_0_0_0_2px_var(--label)]',
          ]"
          role="button"
          tabindex="0"
          @click="onCard(e)"
        >
          <div v-if="presentation" class="mt-3.5 flex flex-col gap-1.5"><span v-for="n in 3" :key="n" class="h-2 rounded-full bg-fill-2 last:w-[55%]" /></div>
          <img v-else-if="e.kind === 'image' && e.thumb" :src="e.thumb" alt="" class="absolute inset-0 size-full object-cover" />
          <div class="relative z-[1] flex h-5 items-center gap-1.5 text-label-2">
            <component :is="ICONS[e.kind]" :size="12" weight="bold" />
            <span class="type-caption ml-auto text-label-3 transition-opacity duration-120 ease-out group-hover/card:opacity-0">{{ age(e.at) }}</span>
            <div class="absolute right-0 flex gap-1 opacity-0 transition-opacity duration-120 ease-out group-hover/card:opacity-100">
              <Button size="icon-xs" class="bg-fill-3 text-label backdrop-blur-[8px]" :title="t('clip.copy')" @click.stop="recopy(e)">
                <PhCopy :size="11" weight="bold" class="size-[11px]" />
              </Button>
              <Button size="icon-xs" class="bg-fill-3 text-label backdrop-blur-[8px]" :title="t('clip.remove')" @click.stop="remove(e.id)">
                <PhX :size="11" weight="bold" class="size-[11px]" />
              </Button>
            </div>
          </div>
          <template v-if="presentation" />
          <div v-else-if="e.kind === 'text'" class="type-footnote relative mt-1.5 line-clamp-3 [word-break:break-word]">{{ e.preview }}</div>
          <div v-else-if="e.kind === 'link'" class="relative mt-1.5 [word-break:break-word]">
            <div class="type-footnote truncate font-semibold text-sys-link">{{ link(e.preview ?? "").host }}</div>
            <div class="type-caption line-clamp-2 text-label-3">{{ link(e.preview ?? "").path }}</div>
          </div>
          <div v-else-if="e.kind === 'image'" class="type-caption tabular absolute bottom-2 left-2.5 z-[1] text-label-2">{{ e.width }} × {{ e.height }}</div>
          <div v-else class="relative mt-1.5 flex flex-col gap-1.5 [word-break:break-word]">
            <img v-if="e.thumb" :src="e.thumb" alt="" class="h-14 w-full rounded-sm object-cover" />
            <FileIcon v-else :name="baseName(e.files[0] ?? '')" class="text-label-2" />
            <div class="type-footnote line-clamp-2">{{ baseName(e.files[0] ?? "") }}</div>
            <div v-if="e.files.length > 1" class="type-caption text-label-3">{{ t("clip.more", { n: e.files.length - 1 }) }}</div>
          </div>
          <Transition
            enter-active-class="transition-opacity duration-160 ease-out"
            enter-from-class="opacity-0"
            leave-active-class="transition-opacity duration-160 ease-out"
            leave-to-class="opacity-0"
          >
            <div v-if="copiedId === e.id" class="type-footnote absolute inset-0 z-[2] flex items-center justify-center gap-1.5 bg-scrim/55 backdrop-blur-[4px]"><PhCheck :size="14" weight="bold" />{{ t("clip.copied") }}</div>
            <div v-else-if="failedId === e.id" class="type-footnote absolute inset-0 z-[2] flex items-center justify-center gap-1.5 bg-scrim/55 text-sys-red backdrop-blur-[4px]">{{ t("clip.copyFailed") }}</div>
          </Transition>
        </div>
      </TransitionGroup>
    </div>

    <div v-if="selected && previewSize" class="flex flex-col items-center px-4 pt-3">
      <img v-if="large" :src="large" alt="" class="rounded-md object-contain" :style="{ width: `${previewSize.w}px`, height: `${previewSize.h}px` }" />
      <div class="flex h-[34px] w-full items-center justify-between">
        <span class="type-caption tabular text-label-3">{{ selected.width }} × {{ selected.height }}</span>
        <div class="flex gap-1">
          <IconButton :icon="PhCopy" :label="t('clip.copy')" @click="recopy(selected)" />
          <IconButton :icon="PhX" :label="t('common.close')" @click="closePreview" />
        </div>
      </div>
    </div>
  </div>
</template>
