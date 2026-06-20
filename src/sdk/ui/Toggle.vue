<script setup lang="ts">
const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ "update:modelValue": [boolean] }>();
</script>

<template>
  <div class="i-sw" :class="{ on: props.modelValue }" @click="emit('update:modelValue', !props.modelValue)"></div>
</template>

<style scoped>
.i-sw { width: 42px; height: 24px; border-radius: 13px; background: rgba(255, 255, 255, 0.18); position: relative; cursor: pointer; transition: background 0.2s; flex: none; }
.i-sw.on { background: #1db954; }
.i-sw::after { content: ""; position: absolute; top: 3px; left: 3px; width: 18px; height: 18px; border-radius: 50%; background: #fff; transition: left 0.2s; }
.i-sw.on::after { left: 21px; }
</style>
