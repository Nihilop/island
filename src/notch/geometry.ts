// Géométrie du notch par état (DIP). w = largeur du corps hors oreilles, r = rayon des
// coins bas, ear = rayon des « oreilles » concaves qui raccordent la forme au bord d'écran.

export interface Shape {
  w: number;
  h: number;
  r: number;
  ear: number;
}

export const WINDOW = { w: 760, h: 560 } as const;
/** Zone utile maximale du contenu ouvert. */
export const MAX_OPEN = { w: 740, h: 540 } as const;

export const SHAPES = {
  closed: { w: 188, h: 30, r: 10, ear: 6 },
  peek: { w: 204, h: 34, r: 12, ear: 7 },
  live: { w: 284, h: 30, r: 10, ear: 6 },
  gaming: { w: 436, h: 30, r: 10, ear: 6 },
  /** Bannière de notification complète (notch fermé). */
  notif: { w: 400, h: 72, r: 22, ear: 10 },
  line: { w: 150, h: 5, r: 2.5, ear: 3 },
  linePeek: { w: 166, h: 7, r: 3.5, ear: 3.5 },
} satisfies Record<string, Shape>;

export const OPEN_RADIUS = { r: 30, ear: 14 } as const;

export function openShape(size: { w: number; h: number }): Shape {
  return {
    w: Math.min(size.w, MAX_OPEN.w),
    h: Math.min(size.h, MAX_OPEN.h),
    ...OPEN_RADIUS,
  };
}

/** Marges de la zone de hit autour de la forme (loi de Fitts ; discrète en « ligne »). */
export const HIT_MARGIN = {
  normal: { side: 16, bottom: 12 },
  line: { side: 4, bottom: 4 },
} as const;

/** Arrondi au nombre PAIR de pixels physiques le plus proche : les deux bords d'une
 *  forme centrée tombent alors sur des pixels entiers (pas de micro-tremblement). */
export function snapEven(v: number, dpr = window.devicePixelRatio || 1): number {
  return (Math.round((v * dpr) / 2) * 2) / dpr;
}
