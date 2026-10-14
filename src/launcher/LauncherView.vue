<script setup lang="ts">
// Launcher (640 px de large, hauteur selon les résultats) : barre de recherche en haut,
// résultats dessous, pied avec les raccourcis. Tout au clavier :
//   ↑ ↓ choisir · Entrée ouvrir · Ctrl+Maj+Entrée en administrateur · Alt+Entrée révéler
//   Ctrl+C copier le chemin · Ctrl+Entrée chercher sur le web · Ctrl+K actions · Échap fermer
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  PhAppWindow,
  PhArrowSquareOut,
  PhCalculator,
  PhCheck,
  PhClipboardText,
  PhCopy,
  PhFolderOpen,
  PhGearSix,
  PhGlobe,
  PhMagnifyingGlass,
  PhPuzzlePiece,
  PhShieldCheck,
  PhSpinner,
} from "@phosphor-icons/vue";
import FileIcon from "../ui/FileIcon.vue";
import Kbd from "../ui/Kbd.vue";
import { requestSize } from "../tools/registry";
import { takeKeyboard } from "../notch/state";
import { t } from "../notch/i18n";
import { capturing, command, current, icons, launcher, loadIcons, query, rows, run, stopCapture, type Row } from "./store";

const W = 640;
const INPUT_H = 56;
const ROW_H = 44;
const FOOT_H = 30;
const MAX_ROWS = 8;

const input = ref<HTMLInputElement>();
const list = ref<HTMLElement>();

// Hauteur : barre + résultats (8 max visibles) + pied.
const height = computed(() => {
  const n = Math.min(rows.value.length, MAX_ROWS);
  return INPUT_H + (n ? n * ROW_H + 8 : 0) + FOOT_H;
});
watch(height, (h) => requestSize("launcher", { w: W, h }), { immediate: true });
onBeforeUnmount(() => requestSize("launcher", null));

// Ouvert par la touche Windows : la frappe est captée par le hook, le champ réel n'a pas
// le focus (cf. store). Sinon (raccourci, clic) : clavier normal.
const grabKeyboard = () => !capturing.value && takeKeyboard();

async function focusInput() {
  grabKeyboard();
  await nextTick();
  input.value?.focus();
}

onMounted(() => {
  if (!capturing.value) void focusInput();
});

/** Clic dans le launcher pendant la capture : on rend le clavier normal. */
function onPointerDown() {
  if (!capturing.value) return;
  stopCapture();
  void focusInput();
}

// Icônes des lignes visibles + défilement vers la sélection.
watch(
  rows,
  (r) => void loadIcons(r.filter((x) => x.type === "hit" && x.hit.kind !== "setting").slice(0, 16).map((x) => (x as Extract<Row, { type: "hit" }>).hit.id)),
  { immediate: true },
);
watch(
  () => launcher.selected,
  async (i) => {
    await nextTick();
    list.value?.children[i]?.scrollIntoView({ block: "nearest" });
  },
);

/** Actions possibles sur la ligne choisie (panneau Ctrl+K). */
const actions = computed(() => {
  const r = current.value;
  if (!r || r.type !== "hit") return [];
  const h = r.hit;
  const a: { id: "open" | "admin" | "reveal" | "copy"; label: string; icon: typeof PhCheck; keys: string[] }[] = [
    { id: "open", label: t("launcher.action.open"), icon: PhArrowSquareOut, keys: ["↵"] },
  ];
  if (h.kind === "app" || h.kind === "file") a.push({ id: "admin", label: t("launcher.action.admin"), icon: PhShieldCheck, keys: ["Ctrl", "Maj", "↵"] });
  if (h.path) {
    a.push({ id: "reveal", label: t("launcher.action.reveal"), icon: PhFolderOpen, keys: ["Alt", "↵"] });
    a.push({ id: "copy", label: t("launcher.action.copy"), icon: PhCopy, keys: ["Ctrl", "C"] });
  }
  return a;
});

function onKey(e: KeyboardEvent) {
  const noSelection = input.value?.selectionStart === input.value?.selectionEnd;
  const handled = command({ key: e.key, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey }, noSelection);
  if (!handled) return;
  e.preventDefault();
  // Échap avec le panneau d'actions ouvert : on ferme le panneau, pas le notch.
  if (e.key === "Escape") e.stopPropagation();
}

const KIND_ICON = { app: PhAppWindow, setting: PhGearSix } as const;

function subtitle(r: Row): string {
  switch (r.type) {
    case "hit":
      return r.hit.subtitle || t(`launcher.kind.${r.hit.kind}`);
    case "calc":
      return t("launcher.calc");
    case "url":
      return t("launcher.url");
    case "web":
      return t("launcher.web");
    case "clip":
      return t("launcher.clip");
    case "ext":
      return r.item.subtitle || r.sourceName;
  }
}

function title(r: Row): string {
  switch (r.type) {
    case "hit":
      return r.hit.title;
    case "calc":
      return `= ${r.value}`;
    case "url":
      return r.url.replace(/^https?:\/\//, "");
    case "web":
      return t("launcher.webQuery", { q: r.query });
    case "clip":
      return r.text.replace(/\s+/g, " ").trim();
    case "ext":
      return r.item.title;
  }
}

const placeholder = computed(() => t("launcher.placeholder"));
</script>

<template>
  <div class="flex flex-col" data-no-drag @pointerdown="onPointerDown">
    <!-- Barre de recherche -->
    <div class="flex shrink-0 items-center gap-3 px-5" :style="{ height: `${INPUT_H}px` }">
      <PhMagnifyingGlass :size="18" weight="bold" class="shrink-0 text-label-3" />
      <!-- Frappe captée : miroir de la recherche avec un curseur clignotant. -->
      <div v-if="capturing" class="flex min-w-0 flex-1 items-center overflow-hidden text-[17px] whitespace-pre">
        <span v-if="query" class="truncate text-label">{{ query }}</span>
        <span class="ml-px h-5 w-0.5 shrink-0 rounded-full bg-sys-link animate-blink" />
        <span v-if="!query" class="truncate text-label-4">{{ placeholder }}</span>
      </div>
      <input
        v-else
        ref="input"
        v-model="query"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :placeholder="placeholder"
        class="min-w-0 flex-1 bg-transparent text-[17px] text-label outline-none placeholder:text-label-4"
        @keydown="onKey"
        @focus="grabKeyboard"
      />
      <PhSpinner v-if="launcher.indexing" :size="14" weight="bold" class="shrink-0 animate-spin text-label-3" />
    </div>

    <!-- Résultats -->
    <div v-if="rows.length" class="relative min-h-0 flex-1 border-t border-separator">
      <div
        ref="list"
        class="overflow-y-auto px-2 py-1 [scrollbar-width:none]"
        :style="{ maxHeight: `${MAX_ROWS * ROW_H + 8}px` }"
      >
        <button
          v-for="(r, i) in rows"
          :key="r.key"
          type="button"
          class="flex w-full items-center gap-3 rounded-[10px] px-3 text-left transition-colors duration-100"
          :class="i === launcher.selected ? 'bg-fill-2' : 'hover:bg-fill-1'"
          :style="{ height: `${ROW_H}px` }"
          @mousemove="launcher.selected = i"
          @click="run(r)"
        >
          <span class="grid size-7 shrink-0 place-items-center">
            <template v-if="r.type === 'hit'">
              <img v-if="icons.get(r.hit.id)" :src="icons.get(r.hit.id)!" alt="" class="size-7 object-contain" />
              <component :is="KIND_ICON[r.hit.kind as 'app' | 'setting']" v-else-if="r.hit.kind === 'app' || r.hit.kind === 'setting'" :size="20" weight="duotone" class="text-label-2" />
              <FileIcon v-else :name="r.hit.kind === 'folder' ? '' : r.hit.title" :size="22" class="text-label-2" />
            </template>
            <PhCalculator v-else-if="r.type === 'calc'" :size="20" weight="duotone" class="text-sys-orange" />
            <PhGlobe v-else-if="r.type === 'url'" :size="20" weight="duotone" class="text-sys-link" />
            <PhMagnifyingGlass v-else-if="r.type === 'web'" :size="20" weight="duotone" class="text-label-2" />
            <template v-else-if="r.type === 'ext'">
              <span
                v-if="r.item.icon"
                class="size-5 bg-label-2 [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain]"
                :style="{ maskImage: `url('${r.item.icon}')` }"
              />
              <PhPuzzlePiece v-else :size="20" weight="duotone" class="text-label-2" />
            </template>
            <PhClipboardText v-else :size="20" weight="duotone" class="text-label-2" />
          </span>
          <span class="min-w-0 flex-1">
            <span class="type-body block truncate" :class="r.type === 'calc' ? 'font-semibold tabular-nums' : ''">{{ title(r) }}</span>
            <span class="type-caption block truncate text-label-3">{{ subtitle(r) }}</span>
          </span>
          <PhCheck v-if="launcher.copied === r.key" :size="14" weight="bold" class="shrink-0 text-sys-green" />
          <Kbd v-else-if="i === launcher.selected" :keys="['↵']" class="shrink-0" />
        </button>
      </div>

      <!-- Actions (Ctrl+K) -->
      <Transition
        enter-active-class="transition-[opacity,translate] duration-160 ease-out"
        enter-from-class="opacity-0 translate-y-1"
        leave-active-class="transition-[opacity,translate] duration-120 ease-out"
        leave-to-class="opacity-0 translate-y-1"
      >
        <div
          v-if="launcher.actions && actions.length"
          class="absolute right-3 bottom-2 z-10 flex w-[260px] flex-col rounded-[12px] bg-elevated p-1 shadow-[0_8px_24px_rgb(0_0_0/0.45)]"
        >
          <button
            v-for="a in actions"
            :key="a.id"
            type="button"
            class="type-footnote flex h-8 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-label-2 hover:bg-fill-2 hover:text-label"
            @click="run(current, a.id)"
          >
            <component :is="a.icon" :size="14" weight="bold" />
            <span class="flex-1">{{ a.label }}</span>
            <Kbd :keys="a.keys" />
          </button>
        </div>
      </Transition>
    </div>

    <!-- Pied -->
    <div class="type-caption flex shrink-0 items-center gap-3 border-t border-separator px-5 text-label-3" :style="{ height: `${FOOT_H}px` }">
      <span v-if="!query.trim()" class="truncate">{{ t("launcher.hint") }}</span>
      <template v-else>
        <span class="flex items-center gap-1"><Kbd :keys="['↵']" />{{ t("launcher.action.open") }}</span>
        <span v-if="actions.length" class="flex items-center gap-1"><Kbd :keys="['Ctrl', 'K']" />{{ t("launcher.actions") }}</span>
        <span class="flex items-center gap-1"><Kbd :keys="['Ctrl', '↵']" />{{ t("launcher.web") }}</span>
      </template>
    </div>
  </div>
</template>
