<script setup lang="ts">
// Éditeur de note « façon Notion » (Tiptap open source) : le v-model est du Markdown.
// Le document est reconstruit depuis le Markdown au chargement et quand la valeur change de
// l'extérieur ; chaque modification renvoie le Markdown (débouncé) sans reboucler.
import { onBeforeUnmount, ref, watch } from "vue";
import { EditorContent, VueNodeViewRenderer, useEditor } from "@tiptap/vue-3";
import { DragHandle } from "@tiptap/extension-drag-handle-vue-3";
import { EditorState } from "@tiptap/pm/state";
import type { Node as PMNode } from "@tiptap/pm/model";
import { offset } from "@floating-ui/dom";
import { PhDotsSixVertical, PhPlus } from "@phosphor-icons/vue";
import { createExtensions } from "./extensions";
import { SlashCommand, createSlashState } from "./slash";
import SlashMenu from "./SlashMenu.vue";
import BubbleBar from "./BubbleBar.vue";
import CodeBlockView from "./CodeBlockView.vue";
import TableOfContents from "./TableOfContents.vue";
import { PROSE } from "./prose";
import { t } from "../../notch/i18n";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    editable?: boolean;
    autofocus?: boolean;
  }>(),
  { placeholder: undefined, editable: true, autofocus: false },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

/** Délai entre la dernière frappe et l'émission du Markdown. */
const DEBOUNCE = 300;

/** Zone de défilement (sommaire : suivi du titre visible, défilement au clic). */
const scroller = ref<HTMLElement | null>(null);

const canUndo = ref(false);
const canRedo = ref(false);

/** Dernier Markdown émis ou chargé : une valeur entrante identique n'est pas rechargée. */
let last = props.modelValue;
let timer: ReturnType<typeof setTimeout> | null = null;

const slash = createSlashState();

/** Liens : seulement les schémas web / mail, via la commande hôte (navigateur par défaut). */
function openLink(href: string) {
  if (!/^(https?:|mailto:)/i.test(href)) return;
  void import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("open_url", { url: href }))
    .catch(() => window.open(href, "_blank", "noopener"));
}

const editor = useEditor({
  content: props.modelValue,
  contentType: "markdown",
  editable: props.editable,
  autofocus: props.autofocus ? "end" : false,
  extensions: createExtensions({
    placeholder: (node) =>
      node.type.name === "heading" ? t(`editor.block.h${node.attrs.level}`) : (props.placeholder ?? t("editor.placeholder")),
    openLink,
    codeBlockView: VueNodeViewRenderer(CodeBlockView),
    extra: [SlashCommand(slash)],
  }),
  editorProps: { attributes: { spellcheck: "true" } },
  onUpdate: () => schedule(),
  onTransaction: () => refreshHistory(),
  onBlur: () => flush(),
  onCreate: () => refreshHistory(),
});

function refreshHistory() {
  const e = editor.value;
  canUndo.value = !!e?.can().undo();
  canRedo.value = !!e?.can().redo();
}

function schedule() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(flush, DEBOUNCE);
}

/** Émet tout de suite le Markdown en attente (perte de focus, démontage). */
function flush() {
  if (!timer) return;
  clearTimeout(timer);
  timer = null;
  const e = editor.value;
  if (!e || e.isDestroyed) return;
  const md = e.getMarkdown();
  if (md === last) return;
  last = md;
  emit("update:modelValue", md);
}

// Valeur changée de l'extérieur (rechargement, autre note) : on remplace le document et on
// repart d'un historique vide, pour qu'Annuler ne ramène pas l'ancien contenu.
watch(
  () => props.modelValue,
  (value) => {
    const e = editor.value;
    if (!e || value === last) return;
    if (timer) clearTimeout(timer);
    timer = null;
    last = value;
    e.commands.setContent(value, { contentType: "markdown", emitUpdate: false });
    const s = e.state;
    e.view.updateState(EditorState.create({ doc: s.doc, plugins: s.plugins, selection: s.selection }));
    refreshHistory();
  },
);

watch(
  () => props.editable,
  (v) => editor.value?.setEditable(v, false),
);

onBeforeUnmount(flush);

// Poignée de bloc : bloc survolé, pour le bouton « + » qui insère une ligne dessous.
let hovered: { node: PMNode; pos: number } | null = null;
function onNodeChange({ node, pos }: { node: PMNode | null; pos: number }) {
  hovered = node ? { node, pos } : null;
}

/** « + » : ligne vide sous le bloc survolé (ou ce bloc s'il est vide) puis menu « / ». */
function addBelow() {
  const e = editor.value;
  if (!e || !hovered) return;
  const { node, pos } = hovered;
  if (node.type.name === "paragraph" && node.content.size === 0) {
    e.chain().focus().setTextSelection(pos + 1).insertContent("/").run();
    return;
  }
  const at = pos + node.nodeSize;
  e.chain().focus().insertContentAt(at, { type: "paragraph" }).setTextSelection(at + 1).insertContent("/").run();
}

const handlePosition = { placement: "left-start" as const, strategy: "absolute" as const, middleware: [offset({ mainAxis: 6, crossAxis: 1 })] };

defineExpose({
  focus: () => editor.value?.commands.focus(),
  undo: () => editor.value?.chain().focus().undo().run(),
  redo: () => editor.value?.chain().focus().redo().run(),
  canUndo,
  canRedo,
});
</script>

<template>
  <div class="relative h-full">
  <div ref="scroller" class="relative h-full overflow-y-auto overscroll-contain" data-no-drag>
    <!-- Marge gauche portée par .ProseMirror lui-même (et non par ce conteneur) : la poignée
         de bloc s'y affiche, et la souris qui va vers elle reste « dans » l'éditeur — sinon le
         plugin reçoit un mouseleave et masque la poignée avant qu'on l'atteigne. -->
    <div class="mx-auto w-full max-w-[720px] pt-8 [&_.ProseMirror]:px-14 [&_.ProseMirror]:pb-24" :class="PROSE">
      <EditorContent :editor="editor" />
    </div>

    <template v-if="editor">
      <DragHandle v-if="editable" :editor="editor" :compute-position-config="handlePosition" :on-node-change="onNodeChange">
        <div class="flex h-6 items-center text-label-4">
          <button
            type="button"
            class="flex size-6 items-center justify-center rounded-[5px] transition-colors duration-100 hover:bg-fill-2 hover:text-label-2"
            :title="t('editor.add')"
            :aria-label="t('editor.add')"
            @click="addBelow"
          >
            <PhPlus :size="14" weight="bold" />
          </button>
          <span
            class="flex h-6 w-[18px] cursor-grab items-center justify-center rounded-[5px] transition-colors duration-100 hover:bg-fill-2 hover:text-label-2 active:cursor-grabbing"
            :title="t('editor.drag')"
          >
            <PhDotsSixVertical :size="14" weight="bold" />
          </span>
        </div>
      </DragHandle>
      <BubbleBar :editor="editor" />
      <SlashMenu :state="slash" />
    </template>
  </div>
  <TableOfContents v-if="editor" :editor="editor" :scroller="scroller" />
  </div>
</template>
