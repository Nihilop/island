<script setup lang="ts">
// État vide sur l'Empty shadcn : icône duotone 22 px, titre, texte d'aide, action optionnelle.
import type { Component } from "vue";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/sdk/ui/empty";

defineProps<{ icon?: Component; title: string; text?: string }>();
</script>

<template>
  <Empty>
    <EmptyHeader>
      <EmptyMedia v-if="icon"><component :is="icon" weight="duotone" /></EmptyMedia>
      <EmptyTitle>{{ title }}</EmptyTitle>
      <EmptyDescription v-if="text">{{ text }}</EmptyDescription>
    </EmptyHeader>
    <EmptyContent v-if="$slots.default"><slot /></EmptyContent>
  </Empty>
</template>
