<script setup lang="ts">
// Header (36 px) : titre de l'outil à gauche, centre libre (là où serait la caméra),
// à droite le slot d'actions de l'outil (#notch-header-actions), puis Mode focus et
// Épingler. L'espace vide sert de poignée de drag (géré par Notch.vue).
import { computed } from "vue";
import { PhCrosshair, PhPushPin } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { activeTool, pinned, setFocusMode, togglePin } from "./state";
import { tools } from "../tools/registry";
import { t } from "./i18n";

const title = computed(() => tools.value.find((x) => x.id === activeTool.value)?.title() ?? "");
</script>

<template>
  <header class="flex h-9 shrink-0 items-center justify-between pr-3 pl-5" data-drag-handle>
    <Transition
      enter-active-class="transition-[opacity,filter] duration-160 ease-out"
      enter-from-class="opacity-0 blur-[4px]"
      leave-active-class="transition-[opacity,filter] duration-160 ease-out"
      leave-to-class="opacity-0 blur-[4px]"
      mode="out-in"
    >
      <div :key="title" class="type-title font-semibold whitespace-nowrap">{{ title }}</div>
    </Transition>
    <div class="flex items-center gap-1">
      <div id="notch-header-actions" class="flex items-center gap-1"></div>
      <IconButton :icon="PhCrosshair" :label="t('notch.focusMode')" @click="setFocusMode(true)" />
      <IconButton :icon="PhPushPin" :active="pinned" :label="t('notch.pin')" @click="togglePin" />
    </div>
  </header>
</template>
