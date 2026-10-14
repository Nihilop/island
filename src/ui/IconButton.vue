<script setup lang="ts">
// Bouton rond du header (28 px, icône 14 px) : Button shadcn `ghost` + `icon`. Actif =
// fond fill-3 + label ; chargement = indicateur tournant.
import type { Component } from "vue";
import { Button } from "@/sdk/ui/button";

withDefaults(
  defineProps<{ icon: Component; active?: boolean; disabled?: boolean; loading?: boolean; size?: number; label?: string }>(),
  { size: 14 },
);
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    :data-active="active || undefined"
    :disabled="disabled"
    :loading="loading"
    :aria-label="label"
    :title="label"
  >
    <component :is="icon" :size="size" :weight="active ? 'fill' : 'bold'" />
  </Button>
</template>
