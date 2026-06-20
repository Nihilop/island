import { defineExtension } from "@island/sdk";
import View from "./View.vue";
import Config from "./Config.vue";
import { init } from "./store";
import "./tailwind.css";

// Icône de l'entrée dans le launcher (SVG en string, stroke = currentColor).
const ICON =
  "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M13 2 3 14h7l-1 8 10-12h-7z'/></svg>";

export default defineExtension({
  // Deux surfaces : la "view" (île) et la "config" (modal de réglages).
  surfaces: { view: View, config: Config },

  async activate(ctx) {
    // Restaure l'état persistant + recâble l'île en idle.
    await init(ctx);

    ctx.launcher.register({
      label: "__EXT_NAME__",
      icon: ICON,
      onActivate: () => ctx.view.open(View, { width: 360, height: 320, radius: 28 }),
    });
  },

  deactivate() {
    // Nettoyage si besoin (les contributions idle sont retirées automatiquement).
  },
});
