<script setup lang="ts">
// Barre flottante sur sélection : marques (gras, italique, barré, code, lien) et type de bloc
// (texte / titres). Le bouton lien bascule la barre sur un petit champ URL.
import { nextTick, ref } from "vue";
import type { Editor } from "@tiptap/vue-3";
import { BubbleMenu } from "@tiptap/vue-3/menus";
import { NodeSelection } from "@tiptap/pm/state";
import {
  PhCode,
  PhLink,
  PhLinkBreak,
  PhTextB,
  PhTextHOne,
  PhTextHThree,
  PhTextHTwo,
  PhTextItalic,
  PhTextStrikethrough,
  PhTextT,
} from "@phosphor-icons/vue";
import { Input } from "@/sdk/ui/input";
import { t } from "../../notch/i18n";

const props = defineProps<{ editor: Editor }>();

const linking = ref(false);
const href = ref("");
const field = ref<{ $el: HTMLInputElement } | null>(null);

type BubbleShow = InstanceType<typeof BubbleMenu>["$props"]["shouldShow"];

/** Visible sur une sélection de texte hors bloc de code, tant que l'éditeur ou la barre a le focus. */
const shouldShow: BubbleShow = ({ editor, view, state, element }) => {
  if (!editor.isEditable) return false;
  const focused = view.hasFocus() || element.contains(document.activeElement);
  if (!focused) return false;
  if (linking.value) return true;
  const { selection } = state;
  if (selection.empty || selection instanceof NodeSelection) return false;
  if (selection.$from.parent.type.spec.code) return false;
  return state.doc.textBetween(selection.from, selection.to).trim().length > 0;
};

const btn = "flex size-7 items-center justify-center rounded-[7px] transition-colors duration-100 hover:bg-fill-2 hover:text-label";
const tone = (on: boolean) => (on ? "bg-fill-2 text-label" : "text-label-2");

const run = (fn: (c: ReturnType<Editor["chain"]>) => ReturnType<Editor["chain"]>) => fn(props.editor.chain().focus()).run();

const marks = [
  { id: "bold", icon: PhTextB, label: "editor.bold", toggle: () => run((c) => c.toggleBold()) },
  { id: "italic", icon: PhTextItalic, label: "editor.italic", toggle: () => run((c) => c.toggleItalic()) },
  { id: "strike", icon: PhTextStrikethrough, label: "editor.strike", toggle: () => run((c) => c.toggleStrike()) },
  { id: "code", icon: PhCode, label: "editor.code", toggle: () => run((c) => c.toggleCode()) },
] as const;

const blocks = [
  { id: "text", icon: PhTextT, label: "editor.block.text", active: () => props.editor.isActive("paragraph"), set: () => run((c) => c.setParagraph()) },
  { id: "h1", icon: PhTextHOne, label: "editor.block.h1", active: () => props.editor.isActive("heading", { level: 1 }), set: () => run((c) => c.toggleHeading({ level: 1 })) },
  { id: "h2", icon: PhTextHTwo, label: "editor.block.h2", active: () => props.editor.isActive("heading", { level: 2 }), set: () => run((c) => c.toggleHeading({ level: 2 })) },
  { id: "h3", icon: PhTextHThree, label: "editor.block.h3", active: () => props.editor.isActive("heading", { level: 3 }), set: () => run((c) => c.toggleHeading({ level: 3 })) },
] as const;

async function openLink() {
  href.value = (props.editor.getAttributes("link").href as string | undefined) ?? "";
  linking.value = true;
  await nextTick();
  field.value?.$el.focus();
  field.value?.$el.select();
}

/** Ajoute le schéma manquant (« exemple.com » → « https://exemple.com »). */
function normalize(url: string) {
  const u = url.trim();
  if (!u || /^[a-z][a-z0-9+.-]*:/i.test(u) || u.startsWith("#") || u.startsWith("/")) return u;
  return `https://${u}`;
}

function applyLink() {
  const url = normalize(href.value);
  const c = props.editor.chain().focus().extendMarkRange("link");
  (url ? c.setLink({ href: url }) : c.unsetLink()).run();
  linking.value = false;
}

function removeLink() {
  props.editor.chain().focus().extendMarkRange("link").unsetLink().run();
  linking.value = false;
}

function cancelLink() {
  linking.value = false;
  props.editor.commands.focus();
}
</script>

<template>
  <BubbleMenu
    :editor="editor"
    :should-show="shouldShow"
    :options="{ placement: 'top', offset: 8, flip: true, shift: { padding: 8 }, onHide: () => (linking = false) }"
    :update-delay="80"
    class="z-40"
  >
    <div
      data-no-drag
      class="flex h-9 items-center gap-0.5 rounded-[10px] bg-elevated px-1 text-label shadow-[0_8px_24px_rgb(0_0_0/0.28),0_0_0_0.5px_var(--separator)]"
      @mousedown="(e: MouseEvent) => (e.target as HTMLElement).closest('input') || e.preventDefault()"
    >
      <template v-if="linking">
        <Input
          ref="field"
          v-model="href"
          :placeholder="t('editor.link.placeholder')"
          class="h-7 w-[220px] bg-transparent px-1.5"
          @keydown.enter.prevent="applyLink"
          @keydown.esc.prevent.stop="cancelLink"
        />
        <button type="button" :class="[btn, 'text-label-2']" :title="t('editor.link.remove')" :aria-label="t('editor.link.remove')" @click="removeLink">
          <PhLinkBreak :size="15" weight="bold" />
        </button>
      </template>
      <template v-else>
        <button
          v-for="b in blocks"
          :key="b.id"
          type="button"
          :class="[btn, tone(b.active())]"
          :title="t(b.label)"
          :aria-label="t(b.label)"
          @click="b.set"
        >
          <component :is="b.icon" :size="15" weight="bold" />
        </button>
        <span class="mx-1 h-4 w-px bg-separator" />
        <button
          v-for="m in marks"
          :key="m.id"
          type="button"
          :class="[btn, tone(editor.isActive(m.id))]"
          :title="t(m.label)"
          :aria-label="t(m.label)"
          @click="m.toggle"
        >
          <component :is="m.icon" :size="15" weight="bold" />
        </button>
        <button type="button" :class="[btn, tone(editor.isActive('link'))]" :title="t('editor.link')" :aria-label="t('editor.link')" @click="openLink">
          <PhLink :size="15" weight="bold" />
        </button>
      </template>
    </div>
  </BubbleMenu>
</template>
