<script setup lang="ts">
// Touches d'un raccourci, une pastille Kbd (shadcn) par touche.
import { Kbd, KbdGroup } from "@/sdk/ui/kbd";

defineProps<{ keys: string[] }>();
</script>

<template>
  <KbdGroup>
    <Kbd v-for="k in keys" :key="k">{{ k }}</Kbd>
  </KbdGroup>
</template>
