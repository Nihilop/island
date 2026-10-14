// Slot « outil » des extensions v2 : chaque `contributes.tools[]` devient un outil du dock
// (même contrat que les intégrés), identifiant `ext:<extension>:<outil>`. L'ordre et
// l'activation se règlent dans Réglages › Outils, comme pour les autres.

import { defineComponent, h } from "vue";
import { registerTool } from "../tools/registry";
import { settings } from "../notch/settings";
import ExtensionView from "./ExtensionView.vue";
import { extUrl, type Manifest } from "./runtime";

/** Icône d'extension (SVG) teinte comme les icônes du dock : masque CSS + couleur courante. */
function iconComponent(url: string | null) {
  return defineComponent({
    props: { size: { type: Number, default: 16 } },
    setup(props) {
      return () =>
        url
          ? h("span", {
              class: "inline-block bg-current",
              style: {
                width: `${props.size}px`,
                height: `${props.size}px`,
                maskImage: `url("${url}")`,
                maskSize: "contain",
                maskRepeat: "no-repeat",
                maskPosition: "center",
              },
            })
          : h("span", { class: "inline-block rounded-[4px] bg-current opacity-60", style: { width: `${props.size}px`, height: `${props.size}px` } });
    },
  });
}

export function registerExtensionTools(ext: Manifest) {
  for (const tool of ext.contributes.tools) {
    const id = `ext:${ext.id}:${tool.id}`;
    const icon = tool.icon ?? ext.icon;
    registerTool({
      id,
      icon: iconComponent(icon ? extUrl(ext.id, icon) : null),
      title: () => tool.title,
      size: { w: tool.size[0], h: tool.size[1] },
      view: defineComponent(() => () => h(ExtensionView, { ext, tool })),
      defaultEnabled: true,
    });
    // Premier passage : l'outil rejoint le dock (activé, en dernier).
    if (!settings.tools.some((t) => t.id === id)) settings.tools.push({ id, enabled: true });
  }
}
