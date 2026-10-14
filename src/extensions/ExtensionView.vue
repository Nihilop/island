<script setup lang="ts">
// Vue d'outil d'une extension : iframe sandboxée (cf. runtime.ts) posée à la place de la
// vue d'un outil intégré, reliée à l'hôte à chaque chargement.
import { onBeforeUnmount } from "vue";
import { connect, extUrl, sandboxFrame, type Manifest, type ToolDecl } from "./runtime";

const props = defineProps<{ ext: Manifest; tool: ToolDecl }>();

let disconnect: (() => void) | undefined;

function mount(el: Element | null) {
  if (!el || el.firstChild) return;
  const frame = sandboxFrame(extUrl(props.ext.id, props.tool.view));
  frame.className = "block size-full border-0 bg-transparent";
  frame.addEventListener("load", () => {
    disconnect?.();
    disconnect = connect(props.ext, frame);
  });
  el.appendChild(frame);
}

onBeforeUnmount(() => disconnect?.());
</script>

<template>
  <div :ref="(el) => mount(el as Element | null)" class="size-full" data-no-drag />
</template>
