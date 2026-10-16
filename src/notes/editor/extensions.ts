// Jeu d'extensions Tiptap de l'éditeur de notes (paquets open source MIT uniquement).
// Volontairement sans Vue ni alias `@/` : le même jeu sert à l'éditeur et aux vérifications
// de conversion Markdown hors navigateur.
import { Extension, type AnyExtension, type NodeViewRenderer } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import StarterKit from "@tiptap/starter-kit";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import CodeBlockLowlight from "@tiptap/extension-code-block-lowlight";
import Link from "@tiptap/extension-link";
import { Placeholder } from "@tiptap/extensions";
import { Markdown } from "@tiptap/markdown";
import { lowlight } from "./lowlight.ts";

export interface EditorExtensionsOptions {
  /** Texte fantôme de la ligne vide (fonction : relu à chaque rendu, suit la langue). */
  placeholder?: (node: { type: { name: string }; attrs: Record<string, unknown> }) => string;
  /** Ouverture d'un lien (Ctrl/Cmd+clic, ou clic simple en lecture seule). */
  openLink?: (href: string) => void;
  /** Vue des blocs de code (sélecteur de langage) ; absente hors navigateur. */
  codeBlockView?: NodeViewRenderer;
  /** Extensions en plus (menu « / »…). */
  extra?: AnyExtension[];
}

/** Ctrl/Cmd+clic sur un lien : l'ouvre dans le navigateur au lieu de placer le curseur. */
const LinkOpener = Extension.create<{ open?: (href: string) => void }>({
  name: "linkOpener",
  addOptions: () => ({ open: undefined }),
  addProseMirrorPlugins() {
    const open = this.options.open;
    const editor = this.editor;
    return [
      new Plugin({
        key: new PluginKey("linkOpener"),
        props: {
          handleClick(_view, _pos, event) {
            if (!open || event.button !== 0) return false;
            if (editor.isEditable && !event.ctrlKey && !event.metaKey) return false;
            const a = (event.target as HTMLElement | null)?.closest?.("a[href]");
            const href = a?.getAttribute("href");
            if (!href) return false;
            event.preventDefault();
            open(href);
            return true;
          },
        },
      }),
    ];
  },
});

/** Lien dont le texte est l'URL elle-même (auto-détection, collage) : `<https://…>` plutôt que
 *  `[https://…](https://…)`, plus lisible dans le fichier et relu à l'identique. */
const MarkdownLink = Link.extend({
  renderMarkdown: (node, h, ctx) => {
    const href = String(node.attrs?.href ?? "");
    const title = String(node.attrs?.title ?? "");
    const text = h.renderChildren(node);
    // Le gestionnaire Markdown rend les marques via un texte factice : le vrai texte du lien
    // arrive dans `meta.markText`. Forme courte réservée aux URL sans caractère à échapper.
    const raw = (ctx?.meta as { markText?: string } | undefined)?.markText;
    if (!title && raw === href && /^(https?:\/\/|mailto:)[A-Za-z0-9\-.~:/?#@!$&'()+,;=%]+$/i.test(href)) return `<${text}>`;
    return title ? `[${text}](${href} "${title}")` : `[${text}](${href})`;
  },
});

export function createExtensions(options: EditorExtensionsOptions = {}): AnyExtension[] {
  return [
    StarterKit.configure({
      // Remplacé par la version colorée (lowlight).
      codeBlock: false,
      // Pas d'équivalent Markdown : Ctrl+U produirait un balisage non standard.
      underline: false,
      heading: { levels: [1, 2, 3] },
      dropcursor: { color: "var(--sys-link)", width: 2 },
      // Remplacé par MarkdownLink (sérialisation des liens nus).
      link: false,
    }),
    MarkdownLink.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      defaultProtocol: "https",
      HTMLAttributes: { rel: "noopener noreferrer nofollow", target: null },
    }),
    TaskList,
    TaskItem.configure({ nested: true }),
    (options.codeBlockView ? CodeBlockLowlight.extend({ addNodeView: () => options.codeBlockView! }) : CodeBlockLowlight).configure({ lowlight, defaultLanguage: null, enableTabIndentation: true, HTMLAttributes: { spellcheck: "false" } }),
    Placeholder.configure({
      placeholder: ({ node }) => options.placeholder?.(node) ?? "",
      showOnlyCurrent: true,
      includeChildren: false,
    }),
    Markdown.configure({ indentation: { style: "space", size: 2 } }),
    LinkOpener.configure({ open: options.openLink }),
    ...(options.extra ?? []),
  ];
}
