<script setup lang="ts">
// Item du menu contextuel (design system Island) — wrapper reka-ui stylé.
import { computed } from "vue";
import {
  ContextMenuItem,
  useForwardPropsEmits,
  type ContextMenuItemProps,
  type ContextMenuItemEmits,
} from "reka-ui";

const props = defineProps<ContextMenuItemProps & { class?: string }>();
const emits = defineEmits<ContextMenuItemEmits>();

const delegated = computed(() => {
  const { class: _c, ...rest } = props;
  return rest;
});
const forwarded = useForwardPropsEmits(delegated, emits);
</script>

<template>
  <ContextMenuItem
    v-bind="forwarded"
    :class="[
      'relative flex cursor-default select-none items-center rounded-md px-2.5 py-1.5 text-[12.5px] outline-none transition-colors',
      'data-[highlighted]:bg-primary data-[highlighted]:text-primary-foreground',
      'data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
      props.class,
    ]"
  >
    <slot />
  </ContextMenuItem>
</template>
