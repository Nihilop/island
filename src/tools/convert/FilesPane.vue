<script setup lang="ts">
// Convertisseur › Fichiers : dépôt (dépôt rapide ou Ctrl+V), une seule catégorie à la fois,
// format cible (présélection du premier qu'aucun fichier n'a déjà), qualité pour les
// formats avec perte, taille max pour les images. Sortie à côté des originaux.
import { useDraft } from "../../stores/drafts";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PhCheckCircle, PhCircleNotch, PhFiles, PhWarningCircle, PhX } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import Segmented from "../../ui/Segmented.vue";
import TrackBar from "../../ui/TrackBar.vue";
import FileIcon from "../../ui/FileIcon.vue";
import { dragOver, onFilesDropped } from "../../notch/drop";
import { formatSize, type FileInfo } from "../shelf/store";
import { t } from "../../notch/i18n";

type Cat = "image" | "audio" | "video";
const TARGETS: Record<Cat, string[]> = {
  image: ["png", "jpg", "webp", "ico", "bmp"],
  audio: ["mp3", "m4a", "wav", "flac"],
  video: ["mp4", "webm", "gif", "mp3"],
};
const LOSSY = ["jpg", "mp3", "m4a", "mp4", "webm"];

const files = useDraft<FileInfo[]>("convert.files", () => []);
const status = useDraft<Record<string, { s: string; error?: string; output?: string }>>("convert.status", () => ({}));
const target = useDraft("convert.target", () => "");
const quality = useDraft("convert.quality", () => 85);
/** Côté long max (px), 0 = taille d'origine. */
const maxSize = useDraft("convert.maxSize", () => 0);
const busy = ref(false);
const result = useDraft<{ converted: string[]; failed: number; firstError: string | null } | null>("convert.result", () => null);
const ffmpeg = ref(true);
const pasteMiss = ref(false);

const cat = computed<Cat | null>(() => {
  const cats = new Set(files.value.map((f) => f.category));
  if (cats.size !== 1) return null;
  const c = [...cats][0];
  return c === "image" || c === "audio" || c === "video" ? c : null;
});
const already = (f: FileInfo) => f.ext === target.value || (f.ext === "jpeg" && target.value === "jpg");
const todo = computed(() => files.value.filter((f) => !already(f)).length);
const needsFfmpeg = computed(() => cat.value === "audio" || cat.value === "video");

async function add(paths: string[]) {
  result.value = null;
  status.value = {};
  files.value = await invoke<FileInfo[]>("files_probe", { paths }).catch(() => []);
  const c = cat.value;
  if (c) target.value = TARGETS[c].find((x) => !files.value.some((f) => f.ext === x)) ?? TARGETS[c][0];
}

async function paste() {
  const paths = await invoke<string[]>("files_from_clipboard").catch(() => [] as string[]);
  if (paths.length) await add(paths);
  else {
    pasteMiss.value = true;
    window.setTimeout(() => (pasteMiss.value = false), 1600);
  }
}

async function convert() {
  busy.value = true;
  result.value = await invoke<typeof result.value>("convert_files", {
    paths: files.value.map((f) => f.path),
    target: target.value,
    quality: quality.value,
    maxSize: cat.value === "image" && maxSize.value ? maxSize.value : null,
  }).catch(() => null);
  busy.value = false;
}

function reset() {
  files.value = [];
  status.value = {};
  result.value = null;
}

let unlisten: UnlistenFn | undefined;
function onKey(e: KeyboardEvent) {
  const inField = (e.target as HTMLElement | null)?.closest("input, textarea");
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v" && !inField) {
    e.preventDefault();
    void paste();
  }
}
onMounted(async () => {
  onFilesDropped("convert", (p) => void add(p));
  ffmpeg.value = await invoke<boolean>("convert_ffmpeg_available").catch(() => false);
  unlisten = await listen<{ path: string; status: string; error: string | null; output: string | null }>("convert://progress", (e) => {
    status.value[e.payload.path] = { s: e.payload.status, error: e.payload.error ?? undefined, output: e.payload.output ?? undefined };
  });
  window.addEventListener("keydown", onKey);
});
onBeforeUnmount(() => {
  unlisten?.();
  window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div class="flex h-full flex-col gap-2">
    <div
      v-if="!files.length"
      class="flex flex-1 flex-col items-center justify-center gap-1 rounded-[14px] border-[1.5px] border-dashed transition-[transform,background-color] duration-200 ease-(--ease-out)"
      :class="dragOver ? 'scale-[1.015] border-label-2 bg-fill-2' : 'border-fill-3 bg-fill-1'"
    >
      <PhFiles :size="22" weight="duotone" class="text-label-3" />
      <div class="type-body text-label-2">{{ dragOver ? t("shelf.release") : t("convert.drop") }}</div>
      <button type="button" class="type-caption text-label-3 hover:underline" @click="paste">
        {{ pasteMiss ? t("shelf.nothingToPaste") : t("shelf.paste") }}
      </button>
    </div>

    <template v-else>
      <div class="relative min-h-0 flex-1 overflow-y-auto rounded-md bg-fill-1 [scrollbar-width:thin]">
        <div v-for="f in files" :key="f.path" class="flex h-8 items-center gap-2 px-2.5 not-first:border-t not-first:border-separator" :class="{ 'opacity-45': already(f) }">
          <FileIcon :name="f.name" :size="15" class="text-label-2" />
          <span class="type-footnote flex-1 truncate">{{ f.name }}</span>
          <span v-if="already(f)" class="type-caption text-label-3">{{ t("convert.already", { f: target.toUpperCase() }) }}</span>
          <span class="type-caption text-label-3">{{ formatSize(f.size) }}</span>
          <PhCircleNotch v-if="status[f.path]?.s === 'running'" :size="13" weight="bold" class="animate-[spin_0.9s_linear_infinite]" />
          <PhCheckCircle v-else-if="status[f.path]?.s === 'done'" :size="13" weight="fill" class="text-sys-green" />
          <PhWarningCircle v-else-if="status[f.path]?.s === 'error'" :size="13" weight="fill" class="text-sys-red" :title="status[f.path]?.error" />
        </div>
        <div v-if="dragOver" class="type-body absolute inset-0 grid place-items-center bg-scrim/70">{{ t("convert.replace") }}</div>
      </div>

      <div v-if="!cat" class="type-caption text-sys-orange">{{ t("convert.mixed") }}</div>
      <div v-else-if="needsFfmpeg && !ffmpeg" class="type-caption text-sys-orange">{{ t("convert.noFfmpeg") }}</div>

      <div v-if="result" class="flex items-center gap-2">
        <span class="type-caption flex-1">
          {{ t("convert.done", { n: result.converted.length }) }}
          <span v-if="result.failed" class="text-sys-red"> · {{ t("convert.failed", { n: result.failed }) }} – {{ result.firstError }}</span>
        </span>
        <Button v-if="result.converted[0]" @click="invoke('reveal_path', { path: result.converted[0] })">{{ t("convert.show") }}</Button>
        <Button variant="default" @click="reset">{{ t("convert.new") }}</Button>
      </div>
      <div v-else-if="cat" class="flex flex-wrap items-center gap-2">
        <Segmented v-model="target" size="sm" :options="TARGETS[cat].map((x) => ({ value: x, label: x.toUpperCase() }))" />
        <Segmented
          v-if="cat === 'image'"
          v-model="maxSize"
          size="sm"
          :options="[
            { value: 0, label: t('convert.original') },
            { value: 2048, label: '2048' },
            { value: 1024, label: '1024' },
            { value: 512, label: '512' },
          ]"
        />
        <div v-if="LOSSY.includes(target)" class="flex items-center gap-2">
          <span class="type-caption text-label-3">{{ t("convert.quality") }}</span>
          <TrackBar class="w-[90px]" :value="(quality - 30) / 70" follow @drag="quality = Math.round((30 + $event * 70) / 5) * 5" @commit="quality = Math.round((30 + $event * 70) / 5) * 5" />
          <span class="type-caption w-6 tabular-nums text-label-2">{{ quality }}</span>
        </div>
        <div class="ml-auto flex gap-2">
          <Button size="icon" class="text-label-2 hover:bg-fill-2 active:scale-100" :title="t('shelf.clear')" @click="reset">
            <PhX :size="12" weight="bold" class="size-3" />
          </Button>
          <Button variant="default" :disabled="!todo || (needsFfmpeg && !ffmpeg)" :loading="busy" @click="convert">
            {{ busy ? t("convert.converting") : todo > 1 ? t("convert.convertN", { n: todo }) : t("convert.convert") }}
          </Button>
        </div>
      </div>
    </template>
  </div>
</template>
