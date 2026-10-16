// Menu « / » : catalogue des blocs et extension Tiptap qui branche @tiptap/suggestion.
// Le rendu (SlashMenu.vue) est piloté par un état réactif partagé, pas par un VueRenderer :
// l'éditeur le pose dans son propre template.
import type { Component } from "vue";
import { reactive, markRaw } from "vue";
import { Extension, type Editor, type Range } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { exitSuggestion } from "@tiptap/suggestion";
import {
  PhCheckSquare,
  PhCodeBlock,
  PhListBullets,
  PhListNumbers,
  PhMinus,
  PhQuotes,
  PhTextHOne,
  PhTextHThree,
  PhTextHTwo,
  PhTextT,
} from "@phosphor-icons/vue";
import { t } from "../../notch/i18n";

export interface SlashItem {
  id: string;
  icon: Component;
  /** Clé i18n du libellé. */
  label: string;
  /** Mots-clés de filtrage en plus du libellé (fr + en). */
  keywords: string[];
  run: (editor: Editor, range: Range) => void;
}

/** Supprime le « /requête » puis applique la transformation du bloc. */
const chain = (editor: Editor, range: Range) => editor.chain().focus().deleteRange(range);

export const SLASH_ITEMS: SlashItem[] = [
  { id: "text", icon: markRaw(PhTextT), label: "editor.block.text", keywords: ["text", "paragraph", "p"], run: (e, r) => chain(e, r).setParagraph().run() },
  { id: "h1", icon: markRaw(PhTextHOne), label: "editor.block.h1", keywords: ["h1", "heading", "title"], run: (e, r) => chain(e, r).setHeading({ level: 1 }).run() },
  { id: "h2", icon: markRaw(PhTextHTwo), label: "editor.block.h2", keywords: ["h2", "heading", "title"], run: (e, r) => chain(e, r).setHeading({ level: 2 }).run() },
  { id: "h3", icon: markRaw(PhTextHThree), label: "editor.block.h3", keywords: ["h3", "heading", "title"], run: (e, r) => chain(e, r).setHeading({ level: 3 }).run() },
  { id: "bullet", icon: markRaw(PhListBullets), label: "editor.block.bullet", keywords: ["ul", "list", "bullet", "puces"], run: (e, r) => chain(e, r).toggleBulletList().run() },
  { id: "ordered", icon: markRaw(PhListNumbers), label: "editor.block.ordered", keywords: ["ol", "list", "numbered", "1."], run: (e, r) => chain(e, r).toggleOrderedList().run() },
  { id: "task", icon: markRaw(PhCheckSquare), label: "editor.block.task", keywords: ["todo", "task", "check", "tâche", "case"], run: (e, r) => chain(e, r).toggleTaskList().run() },
  { id: "quote", icon: markRaw(PhQuotes), label: "editor.block.quote", keywords: ["quote", "blockquote", ">"], run: (e, r) => chain(e, r).toggleBlockquote().run() },
  { id: "code", icon: markRaw(PhCodeBlock), label: "editor.block.code", keywords: ["code", "pre", "```", "snippet"], run: (e, r) => chain(e, r).toggleCodeBlock().run() },
  { id: "divider", icon: markRaw(PhMinus), label: "editor.block.divider", keywords: ["hr", "divider", "line", "---", "ligne"], run: (e, r) => chain(e, r).setHorizontalRule().run() },
];

/** Comparaison insensible à la casse et aux accents. */
const fold = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

export function filterSlashItems(query: string): SlashItem[] {
  const q = fold(query.trim());
  if (!q) return SLASH_ITEMS;
  return SLASH_ITEMS.filter((it) => [t(it.label), ...it.keywords].some((w) => fold(w).includes(q)));
}

/** État du menu ouvert, lu par SlashMenu.vue. */
export interface SlashState {
  open: boolean;
  items: SlashItem[];
  index: number;
  /** Rectangle du curseur (coordonnées écran) pour placer le menu. */
  rect: (() => DOMRect | null) | null;
  select: (item: SlashItem) => void;
}

export function createSlashState(): SlashState {
  return reactive({ open: false, items: [], index: 0, rect: null, select: () => {} }) as SlashState;
}

export const slashKey = new PluginKey("slash");

export function SlashCommand(state: SlashState) {
  return Extension.create({
    name: "slashCommand",
    addProseMirrorPlugins() {
      return [
        Suggestion<SlashItem, SlashItem>({
          editor: this.editor,
          pluginKey: slashKey,
          char: "/",
          // Pas de menu dans un bloc de code ni dans du code en ligne.
          allow: ({ state: s }) => !s.selection.$from.parent.type.spec.code && !s.selection.$from.marks().some((m) => m.type.spec.code),
          items: ({ query }) => filterSlashItems(query),
          command: ({ editor, range, props }) => props.run(editor, range),
          render: () => ({
            onStart: (p) => {
              Object.assign(state, { open: true, items: p.items, index: 0, rect: p.clientRect ?? null, select: (it: SlashItem) => p.command(it) });
            },
            onUpdate: (p) => {
              Object.assign(state, { items: p.items, index: 0, rect: p.clientRect ?? null, select: (it: SlashItem) => p.command(it) });
            },
            onKeyDown: ({ event, view }) => {
              if (event.key === "Escape") {
                exitSuggestion(view, slashKey);
                return true;
              }
              const n = state.items.length;
              if (!n) return false;
              if (event.key === "ArrowDown") state.index = (state.index + 1) % n;
              else if (event.key === "ArrowUp") state.index = (state.index - 1 + n) % n;
              else if (event.key === "Enter" || event.key === "Tab") state.select(state.items[state.index]);
              else return false;
              return true;
            },
            onExit: () => {
              Object.assign(state, { open: false, items: [], index: 0, rect: null });
            },
          }),
        }),
      ];
    },
  });
}
