// Contenus « portés » hors du notch (menus de Select, tooltips, popovers reka-ui /
// shadcn-vue) : ils sont téléportés sous <body>, donc hors de la zone de hit du notch —
// la fenêtre resterait click-through dessous et ils ne seraient pas cliquables. On suit
// ces nœuds (enfants de <body> autres que #app) et on publie leurs rectangles comme zones
// de hit supplémentaires, tant qu'ils sont affichés.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

let rects: Rect[] = [];
let raf = 0;
let notify: () => void = () => {};

/** Rectangles des contenus portés actuellement visibles. */
export const portalRects = () => rects;

function measure() {
  raf = 0;
  const app = document.getElementById("app");
  const next: Rect[] = [];
  for (const el of Array.from(document.body.children)) {
    if (el === app || el.tagName === "SCRIPT" || el.tagName === "STYLE") continue;
    // Le wrapper peut être de taille nulle (popper) : on prend aussi ses descendants directs.
    for (const node of [el, ...Array.from(el.children)]) {
      const r = node.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) next.push({ x: Math.floor(r.left), y: Math.floor(r.top), w: Math.ceil(r.width), h: Math.ceil(r.height) });
    }
  }
  const changed = next.length !== rects.length || next.some((r, i) => r.x !== rects[i].x || r.y !== rects[i].y || r.w !== rects[i].w || r.h !== rects[i].h);
  rects = next;
  if (changed) notify();
  // Tant qu'un contenu est affiché (il peut s'animer ou se repositionner), on suit.
  if (next.length) raf = requestAnimationFrame(measure);
}

export function initPortals(onChange: () => void) {
  notify = onChange;
  new MutationObserver(() => {
    if (!raf) raf = requestAnimationFrame(measure);
  }).observe(document.body, { childList: true });
}
