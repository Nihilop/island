<script setup lang="ts">
// Icônes (tracés Lucide, 24 × 24, trait).
defineProps<{ name: string; size?: number }>();
</script>

<script lang="ts">
const PATHS = {
  plus: "M12 5v14M5 12h14",
  refresh: "M21 12a9 9 0 0 1-15.3 6.4L3 16M3 12a9 9 0 0 1 15.3-6.4L21 8M21 3v5h-5M3 21v-5h5",
  play: "m7 4 13 8-13 8Z",
  stop: "M6 6h12v12H6Z",
  restart: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5",
  code: "m16 18 6-6-6-6M8 6l-6 6 6 6",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z",
  terminal: "m4 17 6-6-6-6M12 19h8",
  box: "M21 8 12 3 3 8v8l9 5 9-5ZM3 8l9 5 9-5M12 13v8",
  game: "M6 11h4M8 9v4M15 12h.01M18 10h.01M17.3 5H6.7a4 4 0 0 0-4 3.6L2 15a3 3 0 0 0 5.2 2.2L9 15h6l1.8 2.2A3 3 0 0 0 22 15l-.7-6.4A4 4 0 0 0 17.3 5Z",
  file: "M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9ZM14 3v6h6",
  trash: "M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14",
  search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16ZM21 21l-4.3-4.3",
  up: "m18 15-6-6-6 6",
  down: "m6 9 6 6 6-6",
  x: "M18 6 6 18M6 6l12 12",
} as const;
export type IconName = keyof typeof PATHS;
</script>

<template>
  <svg :width="size ?? 14" :height="size ?? 14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path :d="PATHS[name as IconName]" />
  </svg>
</template>
