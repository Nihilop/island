<script setup lang="ts">
defineProps<{ modelValue: number; min?: number; max?: number }>();
const emit = defineEmits<{ "update:modelValue": [number] }>();
</script>

<template>
  <input
    class="i-range"
    type="range"
    :min="min ?? 0"
    :max="max ?? 100"
    :value="modelValue"
    @input="emit('update:modelValue', +($event.target as HTMLInputElement).value)"
  />
</template>

<style scoped>
.i-range { width: 100%; accent-color: #1db954; }
</style>
