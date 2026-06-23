<script setup lang="ts">
// Contenu du menu contextuel (design system Island) — wrapper reka-ui stylé.
// Portalisé (échappe à l'overflow de la boîte) ; passer :collision-boundary pour
// le CONTRAINDRE dans une zone interactive (cf. overlay click-through par hit-region).
import { computed } from "vue";
import {
  ContextMenuContent,
  ContextMenuPortal,
  useForwardPropsEmits,
  type ContextMenuContentProps,
  type ContextMenuContentEmits,
} from "reka-ui";

const props = defineProps<ContextMenuContentProps & { class?: string }>();
const emits = defineEmits<ContextMenuContentEmits>();

const delegated = computed(() => {
  const { class: _c, ...rest } = props;
  return rest;
});
const forwarded = useForwardPropsEmits(delegated, emits);
</script>

<template>
  <ContextMenuPortal>
    <ContextMenuContent
      v-bind="forwarded"
      :class="[
        'z-50 min-w-[10rem] overflow-hidden rounded-lg border border-border/60 bg-popover p-1 text-popover-foreground shadow-[0_12px_32px_rgba(0,0,0,0.4)] outline-none',
        'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
        props.class,
      ]"
    >
      <slot />
    </ContextMenuContent>
  </ContextMenuPortal>
</template>
