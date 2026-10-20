<script setup lang="ts">
const props = defineProps<{ page: number; total: number; size: number }>();
const emit = defineEmits<{ go: [page: number] }>();
const pages = () => Math.max(1, Math.ceil(props.total / props.size));
</script>

<template>
  <div v-if="pages() > 1" class="row" style="justify-content: flex-end; margin-top: 12px">
    <button class="btn" :disabled="page === 0" @click="emit('go', page - 1)">‹</button>
    <span class="muted small">{{ page + 1 }} / {{ pages() }} · {{ total }}</span>
    <button class="btn" :disabled="page + 1 >= pages()" @click="emit('go', page + 1)">›</button>
  </div>
</template>
