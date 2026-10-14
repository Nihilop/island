// Accelerators Tauri ("Control+Alt+Space") ↔ affichage et capture clavier.

import { t } from "./i18n";

const LABELS: Record<string, string> = {
  Control: "Ctrl",
  CommandOrControl: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Super: "Win",
};

/** Touches à afficher (une par élément) : ["Ctrl", "Alt", "Espace"]. */
export function accelKeys(accel: string): string[] {
  return accel
    .split("+")
    .filter(Boolean)
    .map((k) => LABELS[k] ?? (k === "Space" ? t("tray.space") : k.replace(/^Key/, "").replace(/^Digit/, "")));
}

export const formatAccel = (accel: string) => accelKeys(accel).join("+");

/** Convertit un keydown en accelerator, ou null tant qu'il n'y a pas de touche finale
 *  (modificateur seul) ou de modificateur (une touche seule ne fait pas un raccourci). */
export function accelFromEvent(e: KeyboardEvent): string | null {
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return null;
  const mods = [e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(
    Boolean,
  ) as string[];
  if (!mods.length) return null;
  const code = e.code;
  const key = /^Key[A-Z]$/.test(code)
    ? code.slice(3)
    : /^Digit\d$/.test(code)
      ? code.slice(5)
      : /^F\d{1,2}$/.test(code) || code === "Space"
        ? code
        : null;
  return key ? [...mods, key].join("+") : null;
}
