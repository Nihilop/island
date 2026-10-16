<script setup lang="ts">
// Sommaire façon Notion, à droite de l'éditeur : un tiret par titre (H1 > H2 > H3, du plus
// long au plus court), le titre visible en blanc ; au survol, la liste des titres ; un clic
// fait défiler en douceur jusqu'au titre. Lu directement dans le document Tiptap.
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { Editor } from "@tiptap/vue-3";
import { t } from "../../notch/i18n";

const props = defineProps<{ editor: Editor; scroller: HTMLElement | null }>();

interface Heading {
  level: number;
  text: string;
  pos: number;
}

const headings = ref<Heading[]>([]);
const active = ref(0);
/** Marge haute à laquelle un titre est considéré « atteint ». */
const TOP = 32;

function collect() {
  const list: Heading[] = [];
  props.editor.state.doc.descendants((node, pos) => {
    if (node.type.name === "heading") list.push({ level: node.attrs.level as number, text: node.textContent.trim(), pos });
    // Les titres ne sont que de premier niveau : inutile de descendre dans les blocs.
    return false;
  });
  headings.value = list;
  track();
}

/** Position d'un titre dans la zone de défilement (null si son nœud DOM n'existe plus). */
function offsetOf(h: Heading): number | null {
  const el = props.editor.view.nodeDOM(h.pos) as HTMLElement | null;
  if (!el || !props.scroller) return null;
  return el.getBoundingClientRect().top - props.scroller.getBoundingClientRect().top;
}

let raf = 0;
function track() {
  cancelAnimationFrame(raf);
  raf = requestAnimationFrame(() => {
    const s = props.scroller;
    if (!s || !headings.value.length) return;
    // En bas de page, le dernier titre est actif même s'il n'atteint pas le haut.
    if (s.scrollTop + s.clientHeight >= s.scrollHeight - 2) return void (active.value = headings.value.length - 1);
    let i = 0;
    headings.value.forEach((h, k) => {
      const top = offsetOf(h);
      if (top !== null && top <= TOP) i = k;
    });
    active.value = i;
  });
}

function go(h: Heading) {
  const s = props.scroller;
  const top = offsetOf(h);
  if (!s || top === null) return;
  s.scrollTo({ top: s.scrollTop + top - TOP + 8, behavior: "smooth" });
}

props.editor.on("update", collect);
props.editor.on("create", collect);
collect();

watch(
  () => props.scroller,
  (s, old) => {
    old?.removeEventListener("scroll", track);
    s?.addEventListener("scroll", track, { passive: true });
    track();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  props.editor.off("update", collect);
  props.editor.off("create", collect);
  props.scroller?.removeEventListener("scroll", track);
  cancelAnimationFrame(raf);
});

const width = (level: number) => (level === 1 ? "w-4" : level === 2 ? "w-3" : "w-2");
const indent = (level: number) => (level === 1 ? "pl-2" : level === 2 ? "pl-5" : "pl-8");
const show = computed(() => headings.value.length > 0);
</script>

<template>
  <div v-if="show" class="group/toc absolute top-4 right-2 z-10" data-no-drag>
    <!-- Tirets -->
    <div class="flex max-h-[60vh] flex-col items-end gap-2 overflow-hidden px-2 py-3 transition-opacity duration-150 group-hover/toc:opacity-0">
      <span
        v-for="(h, i) in headings"
        :key="h.pos"
        class="h-[2px] shrink-0 rounded-full transition-colors duration-150"
        :class="[width(h.level), i === active ? 'bg-label' : 'bg-label-4']"
      />
    </div>
    <!-- Sommaire (au survol) -->
    <nav
      class="pointer-events-none absolute top-0 right-0 max-h-[60vh] w-60 overflow-y-auto rounded-[14px] border border-separator bg-elevated p-1.5 opacity-0 shadow-[0_12px_32px_-8px_rgb(0_0_0/0.45)] transition-opacity duration-150 [scrollbar-width:none] group-hover/toc:pointer-events-auto group-hover/toc:opacity-100"
      :aria-label="t('editor.toc')"
    >
      <button
        v-for="(h, i) in headings"
        :key="h.pos"
        type="button"
        class="type-footnote block w-full truncate rounded-[8px] py-1 pr-2 text-left transition-colors duration-100 hover:bg-fill-1"
        :class="[indent(h.level), i === active ? 'text-sys-link' : h.text ? 'text-label-2' : 'text-label-4']"
        @click="go(h)"
      >
        {{ h.text || t(`editor.block.h${h.level}`) }}
      </button>
    </nav>
  </div>
</template>
