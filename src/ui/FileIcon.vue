<script setup lang="ts">
// Icône de fichier selon l'extension (style duotone).
import { computed } from "vue";
import {
  PhFile,
  PhFileArchive,
  PhFileAudio,
  PhFileCode,
  PhFileDoc,
  PhFileImage,
  PhFilePdf,
  PhFileText,
  PhFileVideo,
  PhFileXls,
  PhFolder,
} from "@phosphor-icons/vue";

const props = withDefaults(defineProps<{ name: string; size?: number }>(), { size: 26 });

const icon = computed(() => {
  const n = props.name.toLowerCase();
  const ext = n.includes(".") ? n.split(".").pop()! : "";
  if (!ext) return PhFolder;
  if (ext === "pdf") return PhFilePdf;
  if (["doc", "docx", "odt", "rtf"].includes(ext)) return PhFileDoc;
  if (["xls", "xlsx", "xlsm", "ods", "csv", "tsv"].includes(ext)) return PhFileXls;
  if (["zip", "rar", "7z", "tar", "gz"].includes(ext)) return PhFileArchive;
  if (["mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"].includes(ext)) return PhFileAudio;
  if (["mp4", "mov", "mkv", "avi", "webm", "wmv"].includes(ext)) return PhFileVideo;
  if (["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "svg", "tif", "tiff", "dds"].includes(ext)) return PhFileImage;
  if (["txt", "md", "markdown", "log"].includes(ext)) return PhFileText;
  if (["rs", "ts", "js", "vue", "py", "json", "toml", "yaml", "yml", "c", "cpp", "cs", "html", "css"].includes(ext)) return PhFileCode;
  return PhFile;
});
</script>

<template>
  <component :is="icon" :size="size" weight="duotone" />
</template>
