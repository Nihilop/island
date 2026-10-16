<script setup lang="ts">
// Réglages (660 × 460) : barre de catégories à gauche (136 px, pastille glissante),
// contenu de la catégorie à droite, défilant, avec un fondu de 28 px en bas.
import { computed, nextTick, ref, watch, type Component } from "vue";
import { motion } from "motion-v";
import { PhClockCounterClockwise, PhGearSix, PhLightning, PhMagnifyingGlass, PhMonitor, PhPuzzlePiece, PhPlugs, PhSparkle, PhSquaresFour, PhTimer } from "@phosphor-icons/vue";
import { spring } from "../../design/motion";
import { t } from "../../notch/i18n";
import { page, pendingSection, type SettingsPage } from "./nav";
import GeneralPage from "./GeneralPage.vue";
import DisplayPage from "./DisplayPage.vue";
import ToolsPage from "./ToolsPage.vue";
import TimerPage from "./TimerPage.vue";
import ConnectionsPage from "./ConnectionsPage.vue";
import AiPage from "./AiPage.vue";
import BackupPage from "./BackupPage.vue";
import AutomationsPage from "./AutomationsPage.vue";
import LauncherSettings from "../../launcher/LauncherSettings.vue";
import ExtensionsPage from "../../extensions/ExtensionsPage.vue";

const PAGES: { id: SettingsPage; icon: Component; view: Component }[] = [
  { id: "general", icon: PhGearSix, view: GeneralPage },
  { id: "display", icon: PhMonitor, view: DisplayPage },
  { id: "tools", icon: PhSquaresFour, view: ToolsPage },
  { id: "launcher", icon: PhMagnifyingGlass, view: LauncherSettings },
  { id: "extensions", icon: PhPuzzlePiece, view: ExtensionsPage },
  { id: "timer", icon: PhTimer, view: TimerPage },
  { id: "ai", icon: PhSparkle, view: AiPage },
  { id: "connections", icon: PhPlugs, view: ConnectionsPage },
  { id: "automations", icon: PhLightning, view: AutomationsPage },
  { id: "backup", icon: PhClockCounterClockwise, view: BackupPage },
];

const index = computed(() => PAGES.findIndex((p) => p.id === page.value));
const current = computed(() => PAGES[index.value] ?? PAGES[0]);
const scroller = ref<HTMLElement>();

// Lien profond : la page s'affiche puis la section défile en vue (après 120 ms).
watch(
  [pendingSection, page],
  async ([section]) => {
    if (!section) return;
    await nextTick();
    window.setTimeout(() => {
      scroller.value?.querySelector(`[data-section="${section}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
      pendingSection.value = null;
    }, 120);
  },
  { immediate: true },
);
</script>

<template>
  <div class="flex h-full gap-3 px-3">
    <nav class="relative flex w-[136px] shrink-0 flex-col gap-0.5">
      <motion.div
        class="absolute inset-x-0 top-0 h-8 rounded-sm bg-fill-2"
        :initial="false"
        :animate="{ y: index * 34 }"
        :transition="spring('snappy')"
      />
      <button
        v-for="p in PAGES"
        :key="p.id"
        type="button"
        class="type-footnote relative flex h-8 items-center gap-2 rounded-sm px-2.5 font-medium [transition:color_160ms_var(--ease-out),scale_140ms_var(--ease-out)] active:scale-[0.985]"
        :class="p.id === page ? 'text-label' : 'text-label-2'"
        @click="page = p.id"
      >
        <component :is="p.icon" :size="15" :weight="p.id === page ? 'fill' : 'bold'" />
        {{ t(`settings.page.${p.id}`) }}
      </button>
    </nav>
    <div
      ref="scroller"
      class="min-w-0 flex-1 overflow-y-auto pr-1 pb-7 [mask-image:linear-gradient(to_bottom,#000_calc(100%_-_28px),transparent)] [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]"
    >
      <Transition
        enter-active-class="transition-[opacity,filter] duration-160 ease-out"
        enter-from-class="opacity-0 blur-[4px]"
        leave-active-class="transition-[opacity,filter] duration-160 ease-out"
        leave-to-class="opacity-0 blur-[4px]"
        mode="out-in"
      >
        <component :is="current.view" :key="current.id" />
      </Transition>
    </div>
  </div>
</template>
