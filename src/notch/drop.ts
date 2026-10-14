// Dépôt rapide : glisser des fichiers au-dessus du notch l'ouvre (s'il était fermé) sur
// le dernier outil fichier utilisé (Étagère par défaut), le tient ouvert pendant le
// drag, et lui remet les chemins au dépôt. Drag annulé → se referme s'il n'avait été
// ouvert que pour ça.
//
// Le drag-drop natif de Tauri est désactivé (curseur « interdit » selon la version de
// WebView2). On utilise le drop HTML5 de la page, puis `postMessageWithAdditionalObjects`
// remet les objets `File` à Rust, qui en lit les vrais chemins et émet `drop://files`.
// Rust agrandit aussi la zone de hit pendant un vrai drag (overlay.rs).

import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { activeTool, close, hold, open, selectTool, state } from "./state";

/** Un drag de fichiers survole le notch. */
export const dragOver = ref(false);

type Handler = (paths: string[]) => void;
const handlers = new Map<string, Handler>();

/** Un outil fichier s'inscrit pour recevoir les dépôts quand il est actif. */
export function onFilesDropped(tool: string, h: Handler) {
  handlers.set(tool, h);
}

/** Dernier outil fichier utilisé (cible du dépôt rapide). */
let lastFileTool = "shelf";

declare global {
  interface Window {
    chrome?: { webview?: { postMessageWithAdditionalObjects?: (msg: string, objs: ArrayLike<unknown>) => void } };
  }
}

const hasFiles = (e: DragEvent) => !!e.dataTransfer && Array.from(e.dataTransfer.types).includes("Files");

export function initDrop() {
  let release: (() => void) | null = null;
  let openedForDrop = false;
  // Compteur entrées/sorties : dragenter/dragleave se déclenchent aussi sur les enfants.
  let depth = 0;
  let target = "shelf";

  const end = () => {
    depth = 0;
    dragOver.value = false;
    release?.();
    release = null;
  };

  window.addEventListener("dragenter", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    if (depth++ > 0) return;
    dragOver.value = true;
    release ??= hold();
    if (handlers.has(activeTool.value)) lastFileTool = activeTool.value;
    openedForDrop = state.value !== "open";
    if (openedForDrop) open({ tool: lastFileTool });
    else if (!handlers.has(activeTool.value)) selectTool(lastFileTool);
  });

  window.addEventListener("dragover", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault(); // autorise le dépôt
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  });

  window.addEventListener("dragleave", (e) => {
    if (!hasFiles(e) || --depth > 0) return;
    end();
    if (openedForDrop) close();
    openedForDrop = false;
  });

  window.addEventListener("drop", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    target = activeTool.value;
    const files = e.dataTransfer?.files;
    if (files?.length) window.chrome?.webview?.postMessageWithAdditionalObjects?.("island-drop", files);
    openedForDrop = false;
    end();
  });

  // Chemins réels renvoyés par Rust.
  listen<string[]>("drop://files", (e) => {
    const h = handlers.get(target) ?? handlers.get(activeTool.value);
    if (h && e.payload.length) {
      lastFileTool = handlers.has(target) ? target : activeTool.value;
      h(e.payload);
    }
  });
}
