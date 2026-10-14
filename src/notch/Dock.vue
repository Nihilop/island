<script setup lang="ts">
// Dock (46 px) : un bouton 32 × 32 par outil visible, une pastille fill-2 qui glisse
// d'un bouton actif à l'autre, un séparateur, puis l'engrenage des Réglages.
// Débordement : si tous les outils ne tiennent pas dans la largeur de l'outil ouvert
// (beaucoup d'outils, extensions installées…), les derniers passent dans un menu « ••• ».
// L'ordre des Réglages › Outils décide qui reste visible.
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { motion } from "motion-v";
import { PhDotsThreeOutline, PhGear } from "@phosphor-icons/vue";
import { Popover, PopoverContent, PopoverTrigger } from "@/sdk/ui/popover";
import { activeTool, dockTools, hold, selectTool, toggleSettings } from "./state";
import { spring } from "../design/motion";
import { updateState } from "../composables/updater";
import { unseen } from "../composables/changelog";
import { openSettings } from "../tools/settings/nav";

const props = defineProps<{ width: number }>();

/** Point sur l'engrenage : mise à jour disponible ou nouveautés non vues. */
const badge = computed(() => updateState.status === "available" || unseen.value.length > 0);

function onGear() {
  if (badge.value && activeTool.value !== "settings") openSettings("updates");
  else toggleSettings();
}

const BTN = 32;
const GAP = 2;
/** Padding latéral (2 × 16) + séparateur (1 + 2 × 6) + engrenage. */
const FIXED = 32 + 13 + BTN;

/** Nombre de boutons qui tiennent dans la largeur disponible. */
const slots = computed(() => Math.max(1, Math.floor((props.width - FIXED + GAP) / (BTN + GAP))));
const overflowing = computed(() => dockTools.value.length > slots.value);
/** Un emplacement est réservé au bouton « ••• » quand ça déborde. */
const visible = computed(() => (overflowing.value ? dockTools.value.slice(0, slots.value - 1) : dockTools.value));
const hidden = computed(() => (overflowing.value ? dockTools.value.slice(slots.value - 1) : []));
const activeHidden = computed(() => hidden.value.some((t) => t.id === activeTool.value));

const index = computed(() => {
  const i = visible.value.findIndex((t) => t.id === activeTool.value);
  if (i >= 0) return i;
  return activeHidden.value ? visible.value.length : -1;
});
const pillX = computed(() => Math.max(0, index.value) * (BTN + GAP));

// Le menu ouvert tient le notch ouvert (le curseur passe dessus, hors de la forme).
const menuOpen = ref(false);
let release: (() => void) | null = null;
watch(menuOpen, (open) => {
  if (open) release = hold();
  else {
    release?.();
    release = null;
  }
});

// Dock démonté menu ouvert (fermeture du notch) : on rend le hold.
onBeforeUnmount(() => release?.());

function pick(id: string) {
  menuOpen.value = false;
  selectTool(id);
}

const btn =
  "relative grid size-8 place-items-center rounded-[10px] [transition:color_160ms_var(--ease-out),scale_140ms_var(--ease-out)] active:scale-[0.94]";
</script>

<template>
  <nav class="flex h-[46px] shrink-0 items-center justify-center px-4 pt-1">
    <div class="relative flex gap-0.5">
      <motion.div
        v-if="index >= 0"
        class="absolute top-0 left-0 size-8 rounded-[10px] bg-fill-2"
        :initial="false"
        :animate="{ x: pillX }"
        :transition="spring('snappy')"
      />
      <button
        v-for="tool in visible"
        :key="tool.id"
        type="button"
        :class="[btn, tool.id === activeTool ? 'text-label' : 'text-label-3 hover:text-label-2']"
        :title="tool.title()"
        @click="selectTool(tool.id)"
      >
        <component :is="tool.icon" :size="16" :weight="tool.id === activeTool ? 'fill' : 'bold'" />
      </button>

      <Popover v-if="overflowing" v-model:open="menuOpen">
        <PopoverTrigger as-child>
          <button type="button" :class="[btn, activeHidden || menuOpen ? 'text-label' : 'text-label-3 hover:text-label-2']">
            <PhDotsThreeOutline :size="16" :weight="activeHidden ? 'fill' : 'bold'" />
          </button>
        </PopoverTrigger>
        <PopoverContent side="top" :side-offset="8" class="flex min-w-44 flex-col">
          <button
            v-for="tool in hidden"
            :key="tool.id"
            type="button"
            class="flex h-8 items-center gap-2.5 rounded-[6px] px-2 text-left transition-colors duration-150 hover:bg-fill-2"
            :class="tool.id === activeTool ? 'text-label' : 'text-label-2'"
            @click="pick(tool.id)"
          >
            <component :is="tool.icon" :size="15" :weight="tool.id === activeTool ? 'fill' : 'bold'" />
            <span class="type-footnote">{{ tool.title() }}</span>
          </button>
        </PopoverContent>
      </Popover>
    </div>
    <span class="mx-1.5 h-4 w-px bg-separator" />
    <button type="button" :class="[btn, activeTool === 'settings' ? 'text-label' : 'text-label-3 hover:text-label-2']" @click="onGear">
      <PhGear :size="16" :weight="activeTool === 'settings' ? 'fill' : 'bold'" />
      <span v-if="badge" class="absolute top-1 right-1 size-[7px] rounded-full bg-sys-link shadow-[0_0_0_2px_var(--notch)]" />
    </button>
  </nav>
</template>
