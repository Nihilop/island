<script setup lang="ts">
// Bouton avec indicateur de chargement local ; désactivé pendant l'action (pas de double clic).
defineProps<{ loading?: boolean; disabled?: boolean; variant?: "primary" | "danger" }>();
defineEmits<{ click: [] }>();
</script>

<template>
  <button type="button" class="btn" :class="variant" :disabled="loading || disabled" :aria-busy="loading || undefined" @click="$emit('click')">
    <span v-if="loading" class="spinner" />
    <slot />
  </button>
</template>
