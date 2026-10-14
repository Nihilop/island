// Thème du notch (réglage `theme`) : sombre, clair ou « système » (suit Windows via
// `prefers-color-scheme`). Appliqué sur <html> : classe `light` (tokens clairs, cf.
// src/design/tokens.css et src/style.css) + `dark` (variante `dark:` de shadcn et
// `theme.current()` du SDK des extensions) + `color-scheme` (contrôles natifs, scrollbars).
// `data-theme-owner="notch"` signale au legacy appStore de ne plus toucher au thème.

import { watch } from "vue";
import { settings } from "./settings";

const media = window.matchMedia("(prefers-color-scheme: dark)");

function apply() {
  const light = settings.theme === "light" || (settings.theme === "system" && !media.matches);
  const root = document.documentElement;
  root.dataset.themeOwner = "notch";
  root.classList.toggle("light", light);
  root.classList.toggle("dark", !light);
  root.style.colorScheme = light ? "light" : "dark";
}

/** À appeler une fois les réglages chargés. */
export function initTheme() {
  watch(() => settings.theme, apply, { immediate: true });
  media.addEventListener("change", () => {
    if (settings.theme === "system") apply();
  });
}
