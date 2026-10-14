// Springs « façon Apple » paramétrés par durée visuelle + rebond (SwiftUI
// `.spring(duration:bounce:)`). Seuils d'arrêt très bas : sans eux, la fin d'un spring
// produit un saut visible d'~0,25 px.

export type SpringName = "open" | "close" | "peek" | "morph" | "snappy";

const SPRINGS: Record<SpringName, [visualDuration: number, bounce: number]> = {
  open: [0.42, 0.24],
  close: [0.32, 0.04],
  peek: [0.26, 0.32],
  morph: [0.46, 0.1], // plus lent que la référence (0,36) : retour utilisateur
  snappy: [0.24, 0.12],
};

const reduced =
  typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

/** Multiplicateur de lenteur (dev) : `localStorage.setItem("island.slow", "4")`. */
function slow(): number {
  try {
    return Number(localStorage.getItem("island.slow")) || 1;
  } catch {
    return 1;
  }
}

export function spring(name: SpringName) {
  const [visualDuration, bounce] = SPRINGS[name];
  if (reduced?.matches) return { type: "tween" as const, duration: 0.2 * slow(), ease: "easeOut" as const };
  return { type: "spring" as const, visualDuration: visualDuration * slow(), bounce, restDelta: 0.01, restSpeed: 0.1 };
}
