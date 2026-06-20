import { defineExtension } from "@island/sdk";
import View from "./View.vue";
import "./tailwind.css";

// Icône de l'entrée dans le launcher (SVG en string, stroke = currentColor).
const ICON =
  "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><rect x='3' y='3' width='18' height='18' rx='4'/><path d='M9 9h6v6H9z'/></svg>";

export default defineExtension({
  // Surface "view" : montée DANS l'île par l'hôte.
  surfaces: { view: View },

  activate(ctx) {
    ctx.launcher.register({
      label: "__EXT_NAME__",
      icon: ICON,
      onActivate: () => ctx.view.open(View, { width: 320, height: 200, radius: 26 }),
    });
  },
});
