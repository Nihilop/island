<script setup lang="ts">
// Launcher = la grille d'actions/extensions + la recherche extensible (providers).
// L'état vit dans composables/launcher.ts (Island le lit pour se dimensionner) ;
// ce composant en est la présentation + le clavier + le focus.
import { ref, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  query, launcherCells, hasProviders, runSearch, resetLauncherQuery, type LauncherCell,
} from "../composables/launcher";

defineProps<{ dnd: boolean }>();
const emit = defineEmits<{ close: []; toggleDnd: []; openNotifs: [] }>();

const searchEl = ref<HTMLInputElement>();
let queryTimer: number | undefined;

// Débounce : interroge les providers quand l'utilisateur tape.
watch(query, (q) => {
  if (queryTimer) clearTimeout(queryTimer);
  queryTimer = window.setTimeout(() => void runSearch(q.trim()), 120);
});

function onAction(c: LauncherCell) {
  if (c.kind === "settings") invoke("open_settings").catch(() => {});
  else if (c.kind === "dnd") emit("toggleDnd");
  else if (c.kind === "notifs") emit("openNotifs");
  else if (c.kind === "entry" || c.kind === "result") { c.onActivate?.(); resetLauncherQuery(); } // action de l'extension
}
// Entrée = 1er résultat ; Échap = vide le champ puis referme.
function onEnter() { const first = launcherCells.value[0]; if (first) onAction(first); }
function onEsc() { if (query.value) resetLauncherQuery(); else emit("close"); }

onMounted(() => {
  // L'overlay est focus:false → focus explicite pour que l'input reçoive la frappe.
  if (hasProviders.value) {
    invoke("overlay_focus").catch(() => {});
    requestAnimationFrame(() => searchEl.value?.focus());
  }
});
onUnmounted(() => { if (queryTimer) clearTimeout(queryTimer); resetLauncherQuery(); });
</script>

<template>
  <div class="flex h-full w-full flex-col">
    <input v-if="hasProviders" ref="searchEl" v-model="query" type="text"
           class="mx-3 mt-4 mb-0.5 rounded-full border border-border bg-foreground/[0.04] px-3 py-2 text-[13px] text-foreground outline-none transition focus:border-primary placeholder:text-muted-foreground"
           spellcheck="false" placeholder="Rechercher…"
           @keydown.enter.prevent="onEnter" @keydown.esc.prevent="onEsc" @click.stop />
    <div class="grid grid-cols-3 content-start gap-2 p-4 w-full flex-1 min-h-0 overflow-y-auto overflow-x-hidden [scrollbar-width:thin]">
      <button v-for="c in launcherCells" :key="c.id"
              class="flex flex-col items-center gap-1.5 py-2.5 px-1 rounded-xl cursor-pointer"
              :class="c.kind === 'dnd' && dnd ? 'bg-primary text-primary-foreground' : 'hover:bg-card/50'"
              @click.stop="onAction(c)">
        <span class="flex w-5.5 h-5.5 [&_svg]:w-full [&_svg]:h-full" v-html="c.icon"></span>
        <span class="text-[11px] text-muted-foreground">{{ c.label }}</span>
      </button>
    </div>
  </div>
</template>
