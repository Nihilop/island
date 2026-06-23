<script setup lang="ts">
// Séparateur du menu contextuel (design system Island).
import { ContextMenuSeparator } from "reka-ui";
defineProps<{ class?: string }>();
</script>

<template>
  <ContextMenuSeparator :class="['-mx-1 my-1 h-px bg-border/60', $props.class]" />
</template>
