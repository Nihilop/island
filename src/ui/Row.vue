<script setup lang="ts">
// Ligne de réglage : libellé + aide à gauche, contrôle à droite (min 36 px).
defineProps<{ label: string; help?: string }>();
</script>

<template>
  <div class="flex min-h-9 items-center justify-between gap-4 px-3 py-1.5 [&+&]:border-t [&+&]:border-separator">
    <div class="min-w-0">
      <div class="type-body">{{ label }}</div>
      <div v-if="help" class="type-caption mt-0.5 text-label-3">{{ help }}</div>
    </div>
    <div class="flex shrink-0 items-center gap-2"><slot /></div>
  </div>
</template>
