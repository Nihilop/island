// Style des nœuds ProseMirror de l'éditeur (pas de <style> : variantes arbitraires Tailwind posées
// sur le conteneur). Typo d'écriture longue façon Notion : corps 15 px / 1,6, titres serrés,
// couleurs par tokens (thème clair/sombre géré par tokens.css).

/** Racine éditable et blocs. */
const blocks = [
  // Surface d'écriture
  "[&_.ProseMirror]:min-h-full [&_.ProseMirror]:cursor-text [&_.ProseMirror]:select-text [&_.ProseMirror]:outline-none",
  "[&_.ProseMirror]:text-[15px] [&_.ProseMirror]:leading-[1.6] [&_.ProseMirror]:tracking-[-0.006em] [&_.ProseMirror]:text-label",
  "[&_.ProseMirror]:caret-sys-link",
  "[&_.ProseMirror_*]:selection:bg-sys-link/25",
  "[&_.ProseMirror>:first-child]:mt-0",
  // Paragraphes
  "[&_.ProseMirror_p]:my-[3px] [&_.ProseMirror_p]:min-h-[1.6em]",
  // Titres
  "[&_.ProseMirror_h1]:mt-8 [&_.ProseMirror_h1]:mb-1 [&_.ProseMirror_h1]:text-[28px] [&_.ProseMirror_h1]:leading-[1.25] [&_.ProseMirror_h1]:font-bold [&_.ProseMirror_h1]:tracking-[-0.026em]",
  "[&_.ProseMirror_h2]:mt-6 [&_.ProseMirror_h2]:mb-1 [&_.ProseMirror_h2]:text-[22px] [&_.ProseMirror_h2]:leading-[1.3] [&_.ProseMirror_h2]:font-semibold [&_.ProseMirror_h2]:tracking-[-0.02em]",
  "[&_.ProseMirror_h3]:mt-5 [&_.ProseMirror_h3]:mb-0.5 [&_.ProseMirror_h3]:text-[18px] [&_.ProseMirror_h3]:leading-[1.35] [&_.ProseMirror_h3]:font-semibold [&_.ProseMirror_h3]:tracking-[-0.014em]",
  // Listes
  "[&_.ProseMirror_ul]:my-1 [&_.ProseMirror_ul]:list-disc [&_.ProseMirror_ul]:pl-6",
  "[&_.ProseMirror_ol]:my-1 [&_.ProseMirror_ol]:list-decimal [&_.ProseMirror_ol]:pl-6",
  "[&_.ProseMirror_ul_ul]:list-[circle] [&_.ProseMirror_ul_ul_ul]:list-[square] [&_.ProseMirror_ul_ul]:my-0 [&_.ProseMirror_ol_ol]:my-0",
  "[&_.ProseMirror_li]:pl-1 [&_.ProseMirror_li]:marker:text-label-3 [&_.ProseMirror_li>p]:my-[1px]",
  // Cases à cocher
  "[&_.ProseMirror_ul[data-type=taskList]]:list-none [&_.ProseMirror_ul[data-type=taskList]]:pl-0.5",
  "[&_.ProseMirror_ul[data-type=taskList]_ul[data-type=taskList]]:pl-6",
  "[&_.ProseMirror_li[data-checked]]:flex [&_.ProseMirror_li[data-checked]]:items-start [&_.ProseMirror_li[data-checked]]:gap-2 [&_.ProseMirror_li[data-checked]]:pl-0",
  "[&_.ProseMirror_li[data-checked]>label]:flex [&_.ProseMirror_li[data-checked]>label]:mt-[3px] [&_.ProseMirror_li[data-checked]>label]:h-[1.6em] [&_.ProseMirror_li[data-checked]>label]:shrink-0 [&_.ProseMirror_li[data-checked]>label]:items-center [&_.ProseMirror_li[data-checked]>label]:select-none",
  "[&_.ProseMirror_li[data-checked]>label>input]:size-[15px] [&_.ProseMirror_li[data-checked]>label>input]:cursor-pointer [&_.ProseMirror_li[data-checked]>label>input]:accent-sys-link",
  "[&_.ProseMirror_li[data-checked]>div]:min-w-0 [&_.ProseMirror_li[data-checked]>div]:flex-1",
  "[&_.ProseMirror_li[data-checked=true]>div>p]:text-label-3 [&_.ProseMirror_li[data-checked=true]>div>p]:line-through [&_.ProseMirror_li[data-checked=true]>div>p]:decoration-label-4",
  // Citation
  "[&_.ProseMirror_blockquote]:my-2 [&_.ProseMirror_blockquote]:border-l-[3px] [&_.ProseMirror_blockquote]:border-label-3 [&_.ProseMirror_blockquote]:pl-4 [&_.ProseMirror_blockquote]:text-label-2",
  // Séparateur (zone de clic plus haute que le trait)
  "[&_.ProseMirror_hr]:my-4 [&_.ProseMirror_hr]:h-px [&_.ProseMirror_hr]:border-0 [&_.ProseMirror_hr]:bg-separator [&_.ProseMirror_hr]:bg-clip-content [&_.ProseMirror_hr]:py-2 [&_.ProseMirror_hr]:box-content",
  // Bloc de code
  "[&_.ProseMirror_pre]:my-3 [&_.ProseMirror_pre]:overflow-x-auto [&_.ProseMirror_pre]:rounded-[10px] [&_.ProseMirror_pre]:bg-fill-1 [&_.ProseMirror_pre]:px-4 [&_.ProseMirror_pre]:py-3",
  "[&_.ProseMirror_pre]:font-mono [&_.ProseMirror_pre]:text-[13px] [&_.ProseMirror_pre]:leading-[1.55] [&_.ProseMirror_pre]:tracking-normal [&_.ProseMirror_pre]:whitespace-pre",
  "[&_.ProseMirror_pre_code]:bg-transparent [&_.ProseMirror_pre_code]:p-0 [&_.ProseMirror_pre_code]:text-label",
  // Nœud sélectionné (séparateur, glisser-déposer)
  "[&_.ProseMirror-selectednode]:rounded-[4px] [&_.ProseMirror-selectednode]:outline-2 [&_.ProseMirror-selectednode]:outline-sys-link/50",
].join(" ");

/** Marques en ligne. */
const marks = [
  "[&_.ProseMirror_strong]:font-semibold",
  "[&_.ProseMirror_s]:text-label-2",
  "[&_.ProseMirror_a]:cursor-pointer [&_.ProseMirror_a]:text-sys-link [&_.ProseMirror_a]:underline [&_.ProseMirror_a]:decoration-sys-link/40 [&_.ProseMirror_a]:underline-offset-[3px] [&_.ProseMirror_a]:hover:decoration-sys-link",
  "[&_.ProseMirror_:not(pre)>code]:rounded-[5px] [&_.ProseMirror_:not(pre)>code]:bg-fill-2 [&_.ProseMirror_:not(pre)>code]:px-[5px] [&_.ProseMirror_:not(pre)>code]:py-px",
  "[&_.ProseMirror_:not(pre)>code]:font-mono [&_.ProseMirror_:not(pre)>code]:text-[0.86em] [&_.ProseMirror_:not(pre)>code]:text-sys-red",
].join(" ");

/** Texte fantôme (extension Placeholder : classe `is-empty` + `data-placeholder`). */
const placeholder = [
  "[&_.ProseMirror_.is-empty]:before:pointer-events-none [&_.ProseMirror_.is-empty]:before:float-left [&_.ProseMirror_.is-empty]:before:h-0",
  "[&_.ProseMirror_.is-empty]:before:text-label-4 [&_.ProseMirror_.is-empty]:before:content-[attr(data-placeholder)]",
].join(" ");

/** Coloration syntaxique (classes highlight.js produites par lowlight), sobre en clair comme en sombre. */
const syntax = [
  "[&_.hljs-comment]:text-label-3 [&_.hljs-comment]:italic [&_.hljs-quote]:text-label-3 [&_.hljs-quote]:italic",
  "[&_.hljs-keyword]:text-sys-red [&_.hljs-selector-tag]:text-sys-red [&_.hljs-doctag]:text-sys-red [&_.hljs-deletion]:text-sys-red",
  "[&_.hljs-string]:text-sys-green [&_.hljs-regexp]:text-sys-green [&_.hljs-addition]:text-sys-green [&_.hljs-selector-attr]:text-sys-green",
  "[&_.hljs-number]:text-sys-orange [&_.hljs-literal]:text-sys-orange [&_.hljs-symbol]:text-sys-orange [&_.hljs-bullet]:text-sys-orange",
  "[&_.hljs-built_in]:text-[color-mix(in_srgb,var(--sys-yellow)_70%,var(--label))] [&_.hljs-type]:text-[color-mix(in_srgb,var(--sys-yellow)_70%,var(--label))] [&_.hljs-title.class_]:text-[color-mix(in_srgb,var(--sys-yellow)_70%,var(--label))]",
  "[&_.hljs-title]:text-sys-link [&_.hljs-section]:text-sys-link [&_.hljs-name]:text-sys-link [&_.hljs-attr]:text-sys-link [&_.hljs-attribute]:text-sys-link [&_.hljs-selector-class]:text-sys-link",
  "[&_.hljs-meta]:text-label-2 [&_.hljs-variable]:text-label [&_.hljs-template-variable]:text-label [&_.hljs-params]:text-label",
  "[&_.hljs-emphasis]:italic [&_.hljs-strong]:font-semibold",
].join(" ");

export const PROSE = [blocks, marks, placeholder, syntax].join(" ");
