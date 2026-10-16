// Registre des outils du dock. Les outils intégrés et (plus tard) ceux des extensions
// suivent le MÊME contrat : c'est ce contrat que le SDK exposera à la refonte des
// extensions (cf. docs/internal/v2-migration.md §4).

import { shallowRef, type Component } from "vue";

export interface Tool {
  id: string;
  /** Icône du dock (composant, 16 px). */
  icon: Component;
  /** Titre affiché dans le header (déjà traduit ou clé i18n résolue par l'appelant). */
  title: () => string;
  /** Taille ouverte par défaut, header et dock compris (DIP). */
  size: { w: number; h: number };
  view: Component;
  /** Boutons injectés à droite du header. */
  headerActions?: Component;
  defaultEnabled: boolean;
  /** false = hors dock : s'ouvre seulement par son raccourci (ex. le launcher). */
  dock?: boolean;
  /** true = la vue occupe tout le notch ouvert (ni header ni dock). */
  bare?: boolean;
  /** Fond décoratif derrière tout le notch ouvert (en-tête et dock compris), ex. le ciel de
   *  la Météo. Posé à la taille finale de l'outil ; démonté quand l'outil se ferme. */
  backdrop?: Component;
}

export const tools = shallowRef<Tool[]>([]);

/** Taille temporaire demandée par un outil (aperçu, mixer…), bornée à 740 × 540 ;
 *  valable tant que cet outil est actif. `null` = taille par défaut. */
export const sizeRequest = shallowRef<{ tool: string; size: { w: number; h: number } } | null>(null);

export function requestSize(tool: string, size: { w: number; h: number } | null) {
  void import("../notch/state").then((m) => m.extendGrace());
  sizeRequest.value = size ? { tool, size } : sizeRequest.value?.tool === tool ? null : sizeRequest.value;
}

export function registerTool(tool: Tool) {
  tools.value = [...tools.value.filter((t) => t.id !== tool.id), tool];
}

export function unregisterTool(id: string) {
  tools.value = tools.value.filter((t) => t.id !== id);
}
