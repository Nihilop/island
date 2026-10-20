<script setup lang="ts">
// Carte d'un élément (grille) : vignette 16:9, nom, auteur, type, action.
import { PhPuzzlePiece, PhImageSquare } from "@phosphor-icons/vue";
import ActionButton from "./ActionButton.vue";
import { mediaUrl, type Card } from "../market/api";
import { t } from "../notch/i18n";

defineProps<{ item: Card; showKind?: boolean }>();
defineEmits<{ open: [] }>();
</script>

<template>
  <div class="group flex min-w-0 cursor-pointer flex-col gap-2" @click="$emit('open')">
    <div
      class="aspect-video w-full overflow-hidden rounded-[12px] bg-fill-2 bg-cover bg-center ring-1 ring-separator transition-transform group-hover:scale-[1.015]"
      :style="item.thumbnail ? { backgroundImage: `url('${mediaUrl(item.thumbnail)}')` } : {}"
    />
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1">
        <div class="type-footnote truncate font-semibold">{{ item.name }}</div>
        <div class="type-caption flex items-center gap-1 truncate text-label-3">
          <template v-if="showKind">
            <component :is="item.kind === 'scene' ? PhImageSquare : PhPuzzlePiece" :size="11" />
            <span>{{ item.kind === "scene" ? t("store.kind.scene") : t("store.kind.extension") }}</span>
            <span>·</span>
          </template>
          <span class="truncate">{{ item.author }}</span>
        </div>
      </div>
      <ActionButton :item="item" />
    </div>
  </div>
</template>
