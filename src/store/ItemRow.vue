<script setup lang="ts">
// Ligne d'un élément (liste) : vignette, nom, résumé, auteur, téléchargements, action.
import ActionButton from "./ActionButton.vue";
import { mediaUrl, type Card } from "../market/api";
import { t } from "../notch/i18n";

defineProps<{ item: Card; showKind?: boolean }>();
defineEmits<{ open: [] }>();
</script>

<template>
  <div class="flex cursor-pointer items-center gap-3 rounded-[10px] px-2 py-2 hover:bg-fill-1" @click="$emit('open')">
    <div
      class="aspect-video w-28 shrink-0 overflow-hidden rounded-[8px] bg-fill-2 bg-cover bg-center ring-1 ring-separator"
      :style="item.thumbnail ? { backgroundImage: `url('${mediaUrl(item.thumbnail)}')` } : {}"
    />
    <div class="min-w-0 flex-1">
      <div class="type-footnote truncate font-semibold">{{ item.name }}</div>
      <div class="type-caption line-clamp-2 text-label-2">{{ item.summary }}</div>
      <div class="type-caption mt-0.5 truncate text-label-3">
        <template v-if="showKind">{{ item.kind === "scene" ? t("store.kind.scene") : t("store.kind.extension") }} · </template>
        {{ item.author }} · {{ t("market.downloads", { n: item.downloads }) }}
      </div>
    </div>
    <ActionButton :item="item" />
  </div>
</template>
