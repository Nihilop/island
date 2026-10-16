<script setup lang="ts">
// Menu « / » : liste des blocs, filtrée par la saisie, pilotée au clavier depuis slash.ts
// (↑ ↓ Entrée Échap) et cliquable. Placée sous le curseur par floating-ui.
import { nextTick, ref, watch } from "vue";
import { autoUpdate, computePosition, flip, offset, shift } from "@floating-ui/dom";
import type { SlashState } from "./slash";
import { t } from "../../notch/i18n";

const props = defineProps<{ state: SlashState }>();

const menu = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);
const pos = ref({ x: 0, y: 0 });
let stop: (() => void) | null = null;

/** Élément virtuel ancré au rectangle du curseur, relu à chaque recalcul. */
const anchor = {
  getBoundingClientRect: () => props.state.rect?.() ?? new DOMRect(),
};

async function place() {
  if (!menu.value) return;
  const { x, y } = await computePosition(anchor, menu.value, {
    strategy: "fixed",
    placement: "bottom-start",
    middleware: [offset(6), flip({ padding: 8 }), shift({ padding: 8 })],
  });
  pos.value = { x, y };
}

watch(
  () => props.state.open,
  async (open) => {
    stop?.();
    stop = null;
    if (!open) return;
    await nextTick();
    if (menu.value) stop = autoUpdate(anchor, menu.value, place);
  },
);

watch(() => [props.state.rect, props.state.items.length], () => void place());

// L'élément actif reste visible quand on navigue au clavier.
watch(
  () => props.state.index,
  async (i) => {
    await nextTick();
    (list.value?.children[i] as HTMLElement | undefined)?.scrollIntoView({ block: "nearest" });
  },
);
</script>

<template>
  <Transition
    enter-active-class="transition-[opacity,translate] duration-150 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition-opacity duration-100 ease-out"
    leave-to-class="opacity-0"
  >
    <div
      v-if="state.open"
      ref="menu"
      data-no-drag
      class="fixed top-0 left-0 z-50 flex w-[232px] flex-col rounded-[10px] bg-elevated p-1 text-label shadow-[0_8px_24px_rgb(0_0_0/0.28),0_0_0_0.5px_var(--separator)]"
      :style="{ transform: `translate(${Math.round(pos.x)}px, ${Math.round(pos.y)}px)` }"
      @mousedown.prevent
    >
      <div v-if="!state.items.length" class="type-footnote flex h-[30px] items-center px-2.5 text-label-3">{{ t("editor.slash.empty") }}</div>
      <div v-else ref="list" class="flex max-h-[312px] flex-col overflow-y-auto">
        <button
          v-for="(item, i) in state.items"
          :key="item.id"
          type="button"
          class="type-footnote flex h-[30px] shrink-0 items-center gap-2.5 rounded-[7px] px-2 text-left transition-colors duration-100"
          :class="i === state.index ? 'bg-fill-2 text-label' : 'text-label-2'"
          @mousemove="state.index = i"
          @click="state.select(item)"
        >
          <span class="flex size-5 items-center justify-center rounded-[5px] bg-fill-1 text-label-2">
            <component :is="item.icon" :size="13" weight="bold" />
          </span>
          <span class="flex-1 truncate">{{ t(item.label) }}</span>
        </button>
      </div>
    </div>
  </Transition>
</template>
