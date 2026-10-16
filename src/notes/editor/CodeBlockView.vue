<script setup lang="ts">
// Vue d'un bloc de code : sélecteur de langage (coloration lowlight + « ```lang » dans le
// Markdown) en haut à droite, visible au survol ou quand le curseur est dans le bloc.
import { computed } from "vue";
import { NodeViewContent, NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/sdk/ui/select";
import { LANGUAGES } from "./lowlight";
import { t } from "../../notch/i18n";

const props = defineProps(nodeViewProps);

/** Valeur du Select ("" = texte brut ; reka-ui refuse la chaîne vide comme valeur d'item). */
const PLAIN = "__plain__";
const language = computed({
  get: () => {
    return (props.node.attrs.language as string | null) || PLAIN;
  },
  set: (v: string) => props.updateAttributes({ language: v === PLAIN ? null : v }),
});
const options = computed(() => {
  const l = (props.node.attrs.language as string | null) ?? "";
  // Langage inconnu lu depuis le Markdown (```kotlin…) : conservé et proposé tel quel.
  const extra = l && !LANGUAGES.some((x) => x.id === l) ? [{ id: l, label: l }] : [];
  return [{ id: PLAIN, label: t("editor.code.plain") }, ...LANGUAGES, ...extra];
});
</script>

<template>
  <NodeViewWrapper class="group/code relative">
    <pre><NodeViewContent as="code" /></pre>
    <div
      v-if="editor.isEditable"
      contenteditable="false"
      class="absolute top-1.5 right-1.5 opacity-0 transition-opacity duration-100 group-hover/code:opacity-100 has-data-[state=open]:opacity-100"
      :class="{ 'opacity-100': selected }"
    >
      <Select v-model="language">
        <SelectTrigger size="sm" class="h-6 bg-fill-2 px-2 font-sans text-[11px] text-label-2" :aria-label="t('editor.code.language')">
          <SelectValue />
        </SelectTrigger>
        <SelectContent position="popper" :side-offset="4" class="max-h-64">
          <SelectItem v-for="o in options" :key="o.id" :value="o.id">{{ o.label }}</SelectItem>
        </SelectContent>
      </Select>
    </div>
  </NodeViewWrapper>
</template>
